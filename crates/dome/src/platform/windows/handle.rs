use dome_core::{Coordinate, Offset, Physical, Point};
use std::mem::size_of;

use crate::platform::windows::dispatcher::DispatcherMarker;

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows::Win32::Foundation::{LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND, DWMWCP_ROUNDSMALL,
    DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, GetWindowRgnBox, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect,
    MonitorFromWindow, RGN_ERROR,
};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_ELEVATION,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenElevation, TokenIntegrityLevel,
};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Threading::{
    OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, DPI_AWARENESS_CONTEXT,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, DPI_AWARENESS_CONTEXT_UNAWARE, GetDpiForMonitor,
    GetDpiForWindow, GetWindowDpiAwarenessContext, MDT_EFFECTIVE_DPI, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, EnumWindows, GA_ROOT, GA_ROOTOWNER, GW_OWNER, GWL_EXSTYLE, GWL_STYLE,
    GetAncestor, GetClassNameW, GetWindow, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId,
    HWND_BOTTOM, IsIconic, IsWindowArranged, IsWindowVisible, IsZoomed, MINMAXINFO, PostMessageW,
    SET_WINDOW_POS_FLAGS, SMTO_ABORTIFHUNG, SW_MINIMIZE, SW_RESTORE, SWP_ASYNCWINDOWPOS,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageTimeoutW, SetWindowPos,
    ShowWindowAsync, WM_CLOSE, WM_GETMINMAXINFO, WM_GETTEXT, WM_GETTEXTLENGTH, WS_CAPTION,
    WS_THICKFRAME,
};
use windows::core::{BOOL, PCWSTR, w};

use crate::platform::windows::external::{HwndId, NativeWindowOrder, ShowCmd};
use crate::platform::windows::foreground::force_set_foreground;
use dome_core::{FractionalLength, LimitObservation, LimitUpdate, Logical, PixelRect};

// Unlike macOS, we are allowed to move windows completely offscreen on Windows
pub(super) const OFFSCREEN_POS: Coordinate = Coordinate::new(-32000);

/// The radius Microsoft's Windows 11 geometry guidance gives a top-level window.
pub(super) const ROUND_RADIUS: FractionalLength<Logical> = FractionalLength::new(8.0);
/// Microsoft gives no pixel value for `DWMWCP_ROUNDSMALL`. This value was
/// measured on Windows 11 build 26200.
pub(super) const SMALL_RADIUS: FractionalLength<Logical> = FractionalLength::new(4.0);

const MSG_TIMEOUT_MS: u32 = 100;

/// A buggy WndProc can return garbage from WM_GETTEXTLENGTH.
const MAX_WINDOW_TITLE_U16: usize = 32 * 1024;

const MAX_VERSION_INFO_BYTES: usize = 1 << 20;

const MAX_FILE_DESCRIPTION_U16: usize = 1024;

pub(super) struct ExternalHwnd(HWND);

unsafe impl Send for ExternalHwnd {}

unsafe impl Sync for ExternalHwnd {}

impl ExternalHwnd {
    pub(crate) fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }
}

pub(super) struct OverlayHwnd(HWND);

impl OverlayHwnd {
    pub(crate) fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }

    pub(crate) fn set_z_order(&self, z: NativeWindowOrder) {
        write_z_order(self.0, z);
    }

    pub(crate) fn focus(&self) {
        force_set_foreground(self.0);
    }
}

/// Because Dome is Per-Monitor v2 DPI-aware (see `resources/windows/dome.manifest`),
/// GetWindowRect returns physical pixels regardless of the target HWND's own DPI
/// awareness. Windows virtualizes the return based on the CALLER's awareness, not the
/// target's, and this holds for windows owned by other processes, which is the case
/// Dome depends on.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-physicaltologicalpointforpermonitordpi
/// https://stackoverflow.com/a/37829235
///
/// WM_GETMINMAXINFO is NOT virtualized the same way. See `target_scale_to_physical`.
pub(crate) fn get_pixel_rect(hwnd: HWND) -> PixelRect {
    let mut rect = RECT::default();
    if let Err(e) = unsafe { GetWindowRect(hwnd, &mut rect) } {
        tracing::trace!(?hwnd, "GetWindowRect failed: {e}");
        // Dome rejects observations with zero extent.
        return rect_to_pixel_rect(rect);
    }
    rect_to_pixel_rect(rect)
}

/// Converts a Win32 `RECT` (left, top, right, bottom edges) into (x, y, width, height).
pub(super) fn rect_to_pixel_rect(rect: RECT) -> PixelRect {
    PixelRect::new(
        rect.left,
        rect.top,
        u32::try_from((i64::from(rect.right) - i64::from(rect.left)).max(0))
            .expect("width out of range"),
        u32::try_from((i64::from(rect.bottom) - i64::from(rect.top)).max(0))
            .expect("height out of range"),
    )
}

pub(super) fn enum_windows<F>(mut callback: F) -> windows::core::Result<()>
where
    F: FnMut(HWND),
{
    unsafe extern "system" fn enum_proc<F: FnMut(HWND)>(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let callback = unsafe { &mut *(lparam.0 as *mut F) };
        callback(hwnd);
        BOOL(1)
    }

    unsafe {
        EnumWindows(
            Some(enum_proc::<F>),
            LPARAM(&mut callback as *mut _ as isize),
        )
    }
}

/// Returns the invisible border widths (left, top, right, bottom) as raw i32 in physical pixels.
pub(super) fn get_invisible_border(hwnd: HWND) -> (i32, i32, i32, i32) {
    let mut window_rect = RECT::default();
    let mut frame_rect = RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return (0, 0, 0, 0);
        }
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut frame_rect as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_err()
        {
            return (0, 0, 0, 0);
        }
    }
    (
        frame_rect.left - window_rect.left,
        frame_rect.top - window_rect.top,
        window_rect.right - frame_rect.right,
        window_rect.bottom - frame_rect.bottom,
    )
}

impl ExternalHwnd {
    pub(super) fn pid(&self) -> u32 {
        let mut pid = 0u32;
        // Non-blocking thread/process-map lookup, safe on external HWNDs.
        // Returns 0 on a zombie HWND (window already destroyed). 0 is never a
        // valid Windows pid, so callers can use it as an unambiguous sentinel.
        unsafe { GetWindowThreadProcessId(self.0, Some(&mut pid)) };
        if pid == 0 {
            tracing::warn!(id = %HwndId::from(self.0), "GetWindowThreadProcessId returned 0");
        }
        pid
    }

    /// Compensates for invisible borders (the gap between `GetWindowRect` and
    /// `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)`) and moves any thread-owned
    /// child windows by the same delta.
    pub(super) fn set_position(&self, z: NativeWindowOrder, rect: PixelRect) {
        let hwnd = self.0;
        let old = get_pixel_rect(hwnd);
        let (bl, bt, br, bb) = get_invisible_border(hwnd);
        let near = Point {
            x: rect.x() - Offset::new(i64::from(bl)),
            y: rect.y() - Offset::new(i64::from(bt)),
        };
        let far = Point {
            x: rect.right() + Offset::new(i64::from(br)),
            y: rect.bottom() + Offset::new(i64::from(bb)),
        };

        let insert_after: Option<HWND> = z.into();
        let mut flags = SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
        if insert_after.is_none() {
            flags |= SWP_NOZORDER;
        }

        place_window(hwnd, insert_after, near, far, flags);

        let delta_x = near.x() - old.x();
        let delta_y = near.y() - old.y();
        if delta_x != Offset::ZERO || delta_y != Offset::ZERO {
            for_each_owned(hwnd, |child| {
                let mut child_rect = RECT::default();
                if unsafe { GetWindowRect(child, &mut child_rect).is_ok() }
                    && let Err(e) = unsafe {
                        SetWindowPos(
                            child,
                            None,
                            (Coordinate::new(child_rect.left) + delta_x).value(),
                            (Coordinate::new(child_rect.top) + delta_y).value(),
                            0,
                            0,
                            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
                        )
                    }
                {
                    tracing::trace!(
                        ?child,
                        ?delta_x,
                        ?delta_y,
                        "SetWindowPos (child propagate) failed: {e}"
                    );
                }
            });
        }
    }

    pub(super) fn set_z_order(&self, z: NativeWindowOrder) {
        write_z_order(self.0, z);
    }

    pub(super) fn move_offscreen(&self) {
        move_window_offscreen(self.0);
    }

    pub(super) fn show_cmd(&self, cmd: ShowCmd) -> anyhow::Result<()> {
        let sw = match cmd {
            ShowCmd::Restore => SW_RESTORE,
            ShowCmd::Minimize => SW_MINIMIZE,
        };
        // ShowWindowAsync reports issuance, unlike ShowWindow's previous visibility.
        anyhow::ensure!(
            unsafe { ShowWindowAsync(self.0, sw) }.as_bool(),
            "ShowWindowAsync failed for {:?}",
            self.0
        );
        Ok(())
    }

    pub(super) fn close(&self) -> anyhow::Result<()> {
        unsafe { PostMessageW(Some(self.0), WM_CLOSE, WPARAM(0), LPARAM(0)) }?;
        Ok(())
    }

    pub(super) fn set_foreground_window(&self) {
        force_set_foreground(self.0);
    }

    pub(super) fn is_maximized(&self) -> bool {
        unsafe { IsZoomed(self.0) }.as_bool()
    }

    pub(super) fn corner_radius(&self) -> FractionalLength<Logical> {
        let hwnd = self.0;
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
            return FractionalLength::ZERO;
        }
        // Microsoft lists these as states that are never rounded, even when the app
        // asks for rounding. Microsoft also lists a layered window with per-pixel
        // alpha, but `WS_EX_LAYERED` also marks a window that uses one alpha over
        // the whole window, so a layered window can be round or square. This check
        // skips the style, so the preference and the frame styles decide whether a
        // layered window is round. A wrong guess then puts a round border around a
        // square window, which looks better than a square border around a round
        // one.
        if self.is_maximized()
            || unsafe { IsWindowArranged(hwnd) }.as_bool()
            || unsafe { GetWindowRgnBox(hwnd, &mut RECT::default()) } != RGN_ERROR
        {
            return FractionalLength::ZERO;
        }
        match preference {
            DWMWCP_DONOTROUND => return FractionalLength::ZERO,
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
            FractionalLength::ZERO
        }
    }
}

impl ExternalHwnd {
    pub(super) fn read_native_state(
        &self,
        _marker: &DispatcherMarker,
    ) -> super::window_server::NativeWindowState {
        let hwnd = self.0;
        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        let mut information = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let monitor_work_area = (!monitor.0.is_null()
            && unsafe { GetMonitorInfoW(monitor, &mut information) }.as_bool())
        .then(|| rect_to_pixel_rect(information.rcWork));
        super::window_server::NativeWindowState {
            visible: unsafe { IsWindowVisible(hwnd) }.as_bool(),
            minimized: self.is_minimized(),
            cloaked: is_cloaked(hwnd),
            is_root: unsafe { GetAncestor(hwnd, GA_ROOT) } == hwnd,
            has_owner: unsafe { GetWindow(hwnd, GW_OWNER) }.is_ok_and(|owner| !owner.is_invalid()),
            style: unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32,
            extended_style: unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32,
            elevated: is_process_elevated(self.pid()),
            window_rect: get_pixel_rect(hwnd),
            monitor_work_area,
        }
    }

    pub(super) fn is_minimized(&self) -> bool {
        unsafe { IsIconic(self.0) }.as_bool()
    }

    pub(super) fn get_window_title(&self, _marker: &DispatcherMarker) -> Option<String> {
        let hwnd = self.0;
        let mut len = 0usize;
        let ret = unsafe {
            SendMessageTimeoutW(
                hwnd,
                WM_GETTEXTLENGTH,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                MSG_TIMEOUT_MS,
                Some(&mut len),
            )
        };
        if ret == LRESULT(0) {
            return None;
        }
        if len == 0 || len > MAX_WINDOW_TITLE_U16 {
            return None;
        }
        let mut buf = vec![0u16; len + 1];
        let mut copied = 0usize;
        let ret = unsafe {
            SendMessageTimeoutW(
                hwnd,
                WM_GETTEXT,
                WPARAM(buf.len()),
                LPARAM(buf.as_mut_ptr() as isize),
                SMTO_ABORTIFHUNG,
                MSG_TIMEOUT_MS,
                Some(&mut copied),
            )
        };
        if ret == LRESULT(0) {
            return None;
        }
        // Clamp a buggy WndProc that reports copied > buf.len().
        let end = copied.min(buf.len());
        if end == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..end]))
    }

    pub(super) fn get_process_name(&self, _marker: &DispatcherMarker) -> anyhow::Result<String> {
        let hwnd = self.0;
        let path_wide = crate::platform::windows::process::get_exe_path(hwnd)
            .ok_or_else(|| anyhow::anyhow!("could not query process image name"))?;
        // Strip the trailing null before converting to a Rust string
        let path = String::from_utf16_lossy(&path_wide[..path_wide.len().saturating_sub(1)]);
        path.rsplit('\\')
            .next()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("no filename in path"))
    }

    pub(super) fn get_process_path(&self, _marker: &DispatcherMarker) -> anyhow::Result<String> {
        let hwnd = self.0;
        let path_wide = crate::platform::windows::process::get_exe_path(hwnd)
            .ok_or_else(|| anyhow::anyhow!("could not query process image name"))?;
        let end = path_wide.len().saturating_sub(1);
        Ok(String::from_utf16_lossy(&path_wide[..end]))
    }

    /// A failed or timed-out read reports `Unchanged`, leaving core's stored limits alone.
    pub(super) fn get_size_constraints(&self, _marker: &DispatcherMarker) -> LimitObservation {
        let hwnd = self.0;
        let mut info = MINMAXINFO::default();
        let sent = unsafe {
            SendMessageTimeoutW(
                hwnd,
                WM_GETMINMAXINFO,
                WPARAM(0),
                LPARAM(&mut info as *mut _ as isize),
                SMTO_ABORTIFHUNG,
                MSG_TIMEOUT_MS,
                None,
            )
        };
        if sent.0 == 0 {
            tracing::trace!(?hwnd, "WM_GETMINMAXINFO failed or timed out");
            return LimitObservation::default();
        }
        let scale = target_scale_to_physical(hwnd);
        let (left, top, right, bottom) = get_invisible_border(hwnd);
        let horizontal = left + right;
        let vertical = top + bottom;
        LimitObservation {
            min_width: scaled_track_limit(info.ptMinTrackSize.x, horizontal, scale),
            min_height: scaled_track_limit(info.ptMinTrackSize.y, vertical, scale),
            max_width: scaled_track_limit(info.ptMaxTrackSize.x, horizontal, scale),
            max_height: scaled_track_limit(info.ptMaxTrackSize.y, vertical, scale),
        }
    }

    /// Returns the DWM extended frame bounds in physical pixels. Falls back to
    /// `GetWindowRect` if the DWM attribute is unavailable.
    pub(super) fn get_visible_rect(&self) -> PixelRect {
        let hwnd = self.0;
        let mut frame_rect = RECT::default();
        let result = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut frame_rect as *mut _ as *mut _,
                std::mem::size_of::<RECT>() as u32,
            )
        };
        if result.is_ok() {
            rect_to_pixel_rect(frame_rect)
        } else {
            get_pixel_rect(hwnd)
        }
    }

    // Returns None for UWP shells, elevated processes we can't open, apps with no
    // version info, or empty FileDescription. Callers fall back to the executable name.
    pub(super) fn get_app_display_name(&self, _marker: &DispatcherMarker) -> Option<String> {
        let hwnd = self.0;
        let path = crate::platform::windows::process::get_exe_path(hwnd)?;
        let path_ptr = PCWSTR(path.as_ptr());

        let size = unsafe { GetFileVersionInfoSizeW(path_ptr, None) };
        if size == 0 || size as usize > MAX_VERSION_INFO_BYTES {
            return None;
        }

        let mut buf = vec![0u8; size as usize];
        unsafe { GetFileVersionInfoW(path_ptr, None, size, buf.as_mut_ptr().cast()) }.ok()?;

        let buf_ptr = buf.as_ptr().cast();
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;

        let ok = unsafe {
            VerQueryValueW(
                buf_ptr,
                w!("\\VarFileInfo\\Translation"),
                &mut ptr,
                &mut len,
            )
        };
        if !ok.as_bool() || len < 4 || ptr.is_null() {
            return None;
        }
        let lang = unsafe { *(ptr as *const u16) };
        let codepage = unsafe { *((ptr as *const u16).add(1)) };

        // key_wide must live until after VerQueryValueW returns.
        let key_wide: Vec<u16> =
            format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

        let mut desc_ptr = std::ptr::null_mut();
        let mut desc_len = 0u32;
        let ok = unsafe {
            VerQueryValueW(
                buf_ptr,
                PCWSTR(key_wide.as_ptr()),
                &mut desc_ptr,
                &mut desc_len,
            )
        };
        if !ok.as_bool() || desc_len == 0 || desc_ptr.is_null() {
            return None;
        }
        let slice_len = clamp_desc_len(
            desc_ptr as usize,
            buf.as_ptr() as usize,
            buf.len(),
            desc_len,
        );
        if slice_len == 0 {
            return None;
        }
        let desc_slice = unsafe { std::slice::from_raw_parts(desc_ptr as *const u16, slice_len) };
        let result = String::from_utf16_lossy(desc_slice).trim().to_string();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    // `MonitorFromWindow` is non-blocking and safe to call on external HWNDs.
    pub(super) fn get_monitor(&self) -> isize {
        unsafe { MonitorFromWindow(self.0, MONITOR_DEFAULTTONEAREST) }.0 as isize
    }

    pub(super) fn get_class_name(&self) -> Option<String> {
        let mut buf = [0u16; 256];
        let len = unsafe { GetClassNameW(self.0, &mut buf) };
        if len <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }

    pub(super) fn get_aumid(&self, _marker: &DispatcherMarker) -> Option<String> {
        use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
        use windows::Win32::System::Com::CoTaskMemFree;
        use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
        use windows::Win32::UI::Shell::PropertiesSystem::{
            IPropertyStore, SHGetPropertyStoreForWindow,
        };
        unsafe {
            let store: IPropertyStore = SHGetPropertyStoreForWindow(self.0).ok()?;
            let pv = store.GetValue(&PKEY_AppUserModel_ID).ok()?;
            let pwstr = PropVariantToStringAlloc(&pv).ok()?;
            let result = pwstr.to_string().ok();
            CoTaskMemFree(Some(pwstr.as_ptr() as *const _));
            result
        }
    }
}

/// Deliberately omits `SWP_NOZORDER` so the z-drop to HWND_BOTTOM takes effect. This ensures
/// offscreen windows cannot occlude visible windows and the reposition does not
/// steal foreground activation.
fn move_window_offscreen(hwnd: HWND) {
    if let Err(e) = unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_BOTTOM),
            OFFSCREEN_POS.value(),
            OFFSCREEN_POS.value(),
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
        )
    } {
        tracing::trace!(?hwnd, "move_window_offscreen SetWindowPos failed: {e}");
    }
}

// SAFETY: GetWindowDpiAwarenessContext accepts HWNDs from other processes.
fn target_has_context(hwnd: HWND, ctx: DPI_AWARENESS_CONTEXT) -> bool {
    let target = unsafe { GetWindowDpiAwarenessContext(hwnd) };
    unsafe { AreDpiAwarenessContextsEqual(target, ctx) }.as_bool()
}

/// Scale from the units WM_GETMINMAXINFO answers in to Dome's physical pixels.
///
/// MINMAXINFO fields are filled by the target HWND's wndproc under the
/// awareness context the HWND was created with. A PMv2 target matches Dome's
/// caller context and reports physical pixels already. System-aware targets
/// report in system-DPI units, scaled via GetDpiForWindow. Unaware targets
/// report in their virtualized space, scaled via the monitor they occupy.
fn target_scale_to_physical(hwnd: HWND) -> f64 {
    if target_has_context(hwnd, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
        return 1.0;
    }
    if !target_has_context(hwnd, DPI_AWARENESS_CONTEXT_UNAWARE) {
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        return if dpi == 0 {
            // this means an invalid hwnd was passed in
            1.0
        } else {
            dpi as f64 / 96.0
        };
    }
    // An unaware wndproc fills MINMAXINFO in its virtualized space, which
    // Windows re-materializes at the landing monitor's scale. GetDpiForWindow
    // reports a constant 96 there regardless of location, so scale by the
    // monitor the window currently sits on.
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return 1.0;
    }
    monitor_effective_dpi(&rect).map_or(1.0, |dpi| dpi as f64 / 96.0)
}

/// Converts one MINMAXINFO track size into core's limit vocabulary.
/// Win32 reports a zero track size when the app set no limit on that axis. A positive
/// track size that does not exceed the invisible frame has no content-box equivalent, so
/// it is reported as no limit rather than as a zero-sized one.
fn scaled_track_limit(track: i32, border: i32, scale: f64) -> LimitUpdate {
    let track = (track as f64 * scale) as i32;
    if track <= 0 || track <= border {
        return LimitUpdate::Cleared;
    }
    LimitUpdate::Set(FractionalLength::new((track - border) as f64))
}

fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            std::ptr::from_mut(&mut cloaked).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    result.is_ok() && cloaked != 0
}

fn is_process_elevated(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return false;
    };
    let mut token = windows::Win32::Foundation::HANDLE::default();
    let token_ok = unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.is_ok();
    let elevated = if token_ok {
        is_token_elevated_or_high_integrity(token)
    } else {
        false
    };
    unsafe {
        let _ = CloseHandle(process);
        if token_ok {
            let _ = CloseHandle(token);
        }
    }
    elevated
}

fn is_token_elevated_or_high_integrity(token: windows::Win32::Foundation::HANDLE) -> bool {
    let mut elevation = TOKEN_ELEVATION::default();
    let mut ret_len = 0u32;
    if unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        )
    }
    .is_ok()
        && elevation.TokenIsElevated != 0
    {
        return true;
    }
    let mut needed = 0u32;
    let _ = unsafe { GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut needed) };
    if needed == 0 {
        return false;
    }
    let mut buf = vec![0u8; needed as usize];
    if unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buf.as_mut_ptr() as *mut _),
            needed,
            &mut needed,
        )
    }
    .is_err()
    {
        return false;
    }
    let label = unsafe { &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL) };
    let sid = label.Label.Sid;
    if sid.is_invalid() {
        return false;
    }
    let count = unsafe { GetSidSubAuthorityCount(sid) };
    if count.is_null() {
        return false;
    }
    let sub_count = unsafe { *count };
    if sub_count == 0 {
        return false;
    }
    let rid_ptr = unsafe { GetSidSubAuthority(sid, u32::from(sub_count - 1)) };
    if rid_ptr.is_null() {
        return false;
    }
    let rid = unsafe { *rid_ptr };
    rid > 0x2000
}

fn write_z_order(hwnd: HWND, z: NativeWindowOrder) {
    let insert_after: Option<HWND> = z.into();
    let mut flags = SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS | SWP_NOMOVE | SWP_NOSIZE;
    if insert_after.is_none() {
        flags |= SWP_NOZORDER;
    }
    if let Err(e) = unsafe { SetWindowPos(hwnd, insert_after, 0, 0, 0, 0, flags) } {
        tracing::trace!(?hwnd, "SetWindowPos (z-order only) failed: {e}");
    }
}

fn for_each_owned<F: FnMut(HWND)>(hwnd: HWND, callback: F) {
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, None) };
    if thread_id == 0 {
        return;
    }

    unsafe extern "system" fn enum_proc<F: FnMut(HWND)>(child: HWND, lparam: LPARAM) -> BOOL {
        let (owner, callback) = unsafe { &mut *(lparam.0 as *mut (HWND, F)) };
        let root_owner = unsafe { GetAncestor(child, GA_ROOTOWNER) };
        if root_owner == *owner && child != *owner {
            callback(child);
        }
        BOOL(1)
    }

    let mut data = (hwnd, callback);
    // BOOL is FALSE when the callback returns FALSE or no windows are found,
    // neither of which is an error condition.
    unsafe {
        EnumThreadWindows(
            thread_id,
            Some(enum_proc::<F>),
            LPARAM(&mut data as *mut _ as isize),
        )
        .ok()
        .ok();
    }
}

/// Clamps `desc_len` so the returned slice cannot read past `buf`. `desc_len`
/// from VerQueryValueW counts the trailing null u16, hence the `-1`.
fn clamp_desc_len(desc_ptr: usize, buf_ptr: usize, buf_len: usize, desc_len: u32) -> usize {
    let offset_bytes = desc_ptr.saturating_sub(buf_ptr);
    let remaining_u16 = buf_len.saturating_sub(offset_bytes) / 2;
    (desc_len as usize)
        .saturating_sub(1)
        .min(remaining_u16)
        .min(MAX_FILE_DESCRIPTION_U16)
}

/// Effective DPI of the nearest monitor to `rect`, or None when either query
/// fails.
fn monitor_effective_dpi(rect: &RECT) -> Option<u32> {
    let hmonitor = unsafe { MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST) };
    if hmonitor.0.is_null() {
        return None;
    }
    let mut dpi = 0u32;
    if unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi) }.is_err() {
        return None;
    }
    (dpi > 0).then_some(dpi)
}

/// Rounds opposite physical edges independently, preserving signed extents.
fn to_logical_points(
    dpi: u32,
    near: Point<Physical>,
    far: Point<Physical>,
) -> (Point<Logical>, Point<Logical>) {
    let scale = f64::from(dpi) / 96.0;
    (near.to_logical(scale), far.to_logical(scale))
}

/// Places physical edges in the target's DPI context and restores Dome's context
/// before returning.
///
/// When a window declares itself DPI unaware, Windows rewrites every
/// SetWindowPos call on it. It first divides each edge by the scale of the
/// monitor that edge sits on, converting the rect to logical units. It then
/// multiplies the whole rect by the scale of the monitor the rect mostly sits
/// on, the anchor monitor, converting back to physical pixels. An edge that
/// sits on a monitor other than the anchor is therefore net rescaled by the
/// ratio between the two.
///
/// A right edge that sits on a 100% monitor while the rect anchors on a 125%
/// monitor divides by 1.0 and multiplies by 1.25, so a requested 2563 lands at
/// 3204. A left edge that sits on a 125% monitor while the rect anchors on a
/// 100% monitor divides by 1.25 and multiplies by 1.0, so a requested 2565
/// lands at 2052, dragging the whole window about 500px sideways. Converting
/// into the target's context and issuing the converted coordinates skips the
/// rewrite.
fn place_window(
    hwnd: HWND,
    insert_after: Option<HWND>,
    near: Point<Physical>,
    far: Point<Physical>,
    flags: SET_WINDOW_POS_FLAGS,
) {
    if target_has_context(hwnd, DPI_AWARENESS_CONTEXT_UNAWARE) {
        // Resolve the landing monitor while RECT still means physical pixels.
        let rect = RECT {
            left: near.x().value(),
            top: near.y().value(),
            right: far.x().value(),
            bottom: far.y().value(),
        };
        let (logical_near, logical_far) =
            to_logical_points(monitor_effective_dpi(&rect).unwrap_or(96), near, far);
        let previous = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_UNAWARE) };
        if !previous.0.is_null() {
            issue_window_position(hwnd, insert_after, logical_near, logical_far, flags);
            unsafe { SetThreadDpiAwarenessContext(previous) };
            return;
        }
    }
    // A failed context swap leaves the OS translating physical edges.
    issue_window_position(hwnd, insert_after, near, far, flags);
}

fn issue_window_position<U>(
    hwnd: HWND,
    insert_after: Option<HWND>,
    near: Point<U>,
    far: Point<U>,
    flags: SET_WINDOW_POS_FLAGS,
) {
    let width = i32::try_from((far.x() - near.x()).value()).expect("window width out of range");
    let height = i32::try_from((far.y() - near.y()).value()).expect("window height out of range");
    if let Err(e) = unsafe {
        SetWindowPos(
            hwnd,
            insert_after,
            near.x().value(),
            near.y().value(),
            width,
            height,
            flags,
        )
    } {
        tracing::trace!(?hwnd, ?near, ?far, "SetWindowPos failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::{scaled_track_limit, to_logical_points};
    use dome_core::{LimitUpdate, Point};

    #[test]
    fn convert_physical_edges_to_target_dpi_preserves_independent_rounding() {
        assert_eq!(
            to_logical_points(120, Point::new(1277, 43), Point::new(2563, 1443)),
            (Point::new(1022, 34), Point::new(2050, 1154))
        );
        assert_eq!(
            to_logical_points(192, Point::new(5, -5), Point::new(15, 15)),
            (Point::new(3, -3), Point::new(8, 8))
        );
    }

    #[test]
    fn scale_zero_negative_or_subframe_tracks_clears_limits() {
        assert!(matches!(
            scaled_track_limit(0, 18, 1.0),
            LimitUpdate::Cleared
        ));
        assert!(matches!(
            scaled_track_limit(-5, 18, 1.0),
            LimitUpdate::Cleared
        ));
        assert!(matches!(
            scaled_track_limit(17, 18, 1.0),
            LimitUpdate::Cleared
        ));
    }

    #[test]
    fn scale_track_before_stripping_borders_preserves_physical_limit() {
        let LimitUpdate::Set(length) = scaled_track_limit(30, 18, 1.25) else {
            panic!("expected a set limit");
        };
        assert!((length.value() - 19.0).abs() < 1e-4);
    }
}
