use super::*;
use crate::platform::windows::dome::MonitorInfo;
use crate::platform::windows::window_server::{OwnedWindowRead, PlacementRead};

pub(in crate::platform::windows) fn read_root_owner(window: HwndId) -> Option<HwndId> {
    native_query();
    let window: HWND = window.into();
    let owner = native::root_owner(window);
    if owner.0.is_null() || owner == window {
        return None;
    }
    let thread = native::thread(window);
    (thread != 0 && thread == native::thread(owner)).then(|| owner.into())
}

pub(in crate::platform::windows) fn read_visible_frame(window: HwndId) -> Option<PixelRect> {
    native_query();
    let window = window.into();
    native::extended_frame(window)
        .or_else(|| native::outer(window))
        .map(rect_to_pixel_rect)
}

pub(in crate::platform::windows) fn read_placement(
    window: HwndId,
    mut may_read_child: impl FnMut(HwndId) -> bool,
) -> Option<PlacementRead> {
    native_query();
    let window: HWND = window.into();
    if !native::valid(window) {
        return None;
    }
    let outer = native::outer(window);
    let borders = get_invisible_border(window);
    let mut owned_windows = Vec::new();
    let mut deferred_children = Vec::new();
    if !for_each_owned(window, |child| {
        let may_read = may_read_child(child.into());
        if !may_read {
            deferred_children.push(child.into());
        }
        let offset = if may_read
            && let Some(outer) = outer
            && let Some(rectangle) = native::outer(child)
        {
            Some((
                Pixels::new(rectangle.left - outer.left),
                Pixels::new(rectangle.top - outer.top),
            ))
        } else {
            None
        };
        owned_windows.push(OwnedWindowRead {
            window: child.into(),
            offset,
        });
    }) {
        return None;
    }
    Some(PlacementRead {
        invisible_border: (
            Pixels::new(borders.0),
            Pixels::new(borders.1),
            Pixels::new(borders.2),
            Pixels::new(borders.3),
        ),
        unaware: target_has_context(window, DPI_AWARENESS_CONTEXT_UNAWARE),
        owned_windows,
        deferred_children,
    })
}

pub(in crate::platform::windows) fn read_foreground(
    notified: Option<HwndId>,
) -> Option<(HwndId, bool, Option<HwndId>)> {
    native_query();
    let foreground = native::foreground();
    if foreground.0.is_null() {
        return None;
    }
    let key = HwndId::from(foreground);
    if notified.is_some_and(|notified| notified != key) {
        return None;
    }
    let internal = ExternalHwnd::new(foreground).process_id() == native::current_process();
    let exclusive = native::exclusive_fullscreen().then_some(key);
    Some((key, internal, exclusive))
}

pub(in crate::platform::windows) fn read_corner_radius(window: HwndId) -> Length<Logical> {
    native_query();
    let hwnd = window.into();
    #[cfg(test)]
    if native::fixtures::read(hwnd, "corner_radius").is_some() {
        return Length::ZERO;
    }
    let mut preference = DWM_WINDOW_CORNER_PREFERENCE::default();
    let read = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            std::ptr::from_mut(&mut preference).cast(),
            size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        )
    };
    // The attribute exists from Windows 11 build 22000. Earlier builds reject it and draw
    // square corners.
    if read.is_err() {
        return Length::ZERO;
    }
    // Microsoft lists these as states that are never rounded, even when the app
    // asks for rounding. Microsoft also lists a layered window with per-pixel
    // alpha, but `WS_EX_LAYERED` also marks a window that uses one alpha over
    // the whole window, so a layered window can be round or square. This check
    // skips the style, so the preference and the frame styles decide whether a
    // layered window is round. A wrong guess then puts a round border around a
    // square window, which looks better than a square border around a round
    // one.
    if ExternalHwnd::new(hwnd).is_maximized()
        || unsafe { IsWindowArranged(hwnd) }.as_bool()
        || unsafe { GetWindowRgnBox(hwnd, &mut RECT::default()) } != RGN_ERROR
    {
        return Length::ZERO;
    }
    match preference {
        DWMWCP_DONOTROUND => return Length::ZERO,
        DWMWCP_ROUNDSMALL => return SMALL_RADIUS,
        DWMWCP_ROUND => return ROUND_RADIUS,
        _ => {}
    }
    // Microsoft's guide says DWM rounds a window that sets both WS_CAPTION and
    // WS_THICKFRAME. Measured windows that set only one of the two were rounded
    // as well.
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    if style & WS_CAPTION.0 == WS_CAPTION.0 || style & WS_THICKFRAME.0 != 0 {
        ROUND_RADIUS
    } else {
        Length::ZERO
    }
}

fn destination_dots_per_inch(frame: PixelRect, monitors: &[MonitorInfo]) -> u32 {
    let edges = |rectangle: PixelRect| {
        let left = i64::from(rectangle.x().value());
        let top = i64::from(rectangle.y().value());
        (
            left,
            top,
            left + i64::from(rectangle.width().value()),
            top + i64::from(rectangle.height().value()),
        )
    };
    let (left, top, right, bottom) = edges(frame);
    let mut best = None;
    for monitor in monitors {
        let (monitor_left, monitor_top, monitor_right, monitor_bottom) = edges(monitor.full_rect);
        let overlap = (right.min(monitor_right) - left.max(monitor_left)).max(0) as i128
            * (bottom.min(monitor_bottom) - top.max(monitor_top)).max(0) as i128;
        let horizontal = (monitor_left - right).max(left - monitor_right).max(0) as i128;
        let vertical = (monitor_top - bottom).max(top - monitor_bottom).max(0) as i128;
        let distance = horizontal * horizontal + vertical * vertical;
        if best.is_none_or(|(area, nearest, _)| {
            overlap > area || (overlap == area && overlap == 0 && distance < nearest)
        }) {
            best = Some((overlap, distance, monitor.dots_per_inch));
        }
    }
    best.map(|(_, _, dots)| dots)
        .filter(|dots| *dots > 0)
        .unwrap_or(96)
}

struct FrameCoordinates {
    physical: (i32, i32, i32, i32),
    native: (i32, i32, i32, i32),
}

fn prepare_frame(
    frame: PixelRect,
    read: &PlacementRead,
    monitors: &[MonitorInfo],
) -> FrameCoordinates {
    let (left, top, right, bottom) = read.invisible_border;
    let physical = (
        frame.x().value() - left.value(),
        frame.y().value() - top.value(),
        frame.width().value() + left.value() + right.value(),
        frame.height().value() + top.value() + bottom.value(),
    );
    let outer = PixelRect::new(physical.0, physical.1, physical.2, physical.3);
    let dots = destination_dots_per_inch(outer, monitors);
    let native = if read.unaware {
        to_logical_rect(dots, physical.0, physical.1, physical.2, physical.3)
    } else {
        physical
    };
    FrameCoordinates { physical, native }
}

pub(in crate::platform::windows) fn submit_frame(
    window: HwndId,
    frame: PixelRect,
    read: &PlacementRead,
    monitors: &[MonitorInfo],
) {
    let FrameCoordinates { physical, native } = prepare_frame(frame, read, monitors);
    let previous = read
        .unaware
        .then(|| submission::context(DPI_AWARENESS_CONTEXT_UNAWARE))
        .filter(|previous| !previous.0.is_null());
    let coordinates = if previous.is_some() { native } else { physical };
    let _ = submission::position(
        window.into(),
        None,
        coordinates,
        SWP_ASYNCWINDOWPOS | SWP_NOZORDER | SWP_NOACTIVATE,
    );
    if let Some(previous) = previous {
        submission::context(previous);
    }
    for child in &read.owned_windows {
        if let Some((x, y)) = child.offset {
            let _ = submission::position(
                child.window.into(),
                None,
                (physical.0 + x.value(), physical.1 + y.value(), 0, 0),
                SWP_ASYNCWINDOWPOS | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE,
            );
        }
    }
}

pub(in crate::platform::windows) fn submit_z_order(window: HwndId, order: ZOrder) {
    write_z_order(window.into(), order);
}
pub(in crate::platform::windows) fn submit_show(window: HwndId, restore: bool) {
    let _ = submission::show(
        window.into(),
        if restore { SW_RESTORE } else { SW_MINIMIZE },
    );
}
pub(in crate::platform::windows) fn submit_close(window: HwndId) {
    let _ = submission::close(window.into());
}
pub(in crate::platform::windows) fn submit_foreground(window: HwndId) {
    submission::foreground(window.into());
}
pub(in crate::platform::windows) fn submit_recovery(window: HwndId, maximized: bool) {
    if maximized {
        let _ = submission::show(window.into(), SW_RESTORE);
    }
    let _ = submission::position(
        window.into(),
        None,
        (100, 100, 0, 0),
        SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE,
    );
    if maximized {
        let _ = submission::show(window.into(), SW_MAXIMIZE);
    }
}
