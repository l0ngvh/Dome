use crate::core::Dimension;

/// Windows baseline DPI (100% scaling). All scale factors are relative to this.
pub(super) const BASE_DPI: f32 = 96.0;

/// Convert a logical pixel value to physical pixels at the given scale.
pub(super) fn logical_to_physical(v: f32, scale: f32) -> i32 {
    debug_assert!(scale > 0.0, "scale must be positive, got {scale}");
    (v * scale).round() as i32
}

/// Convert a physical-pixel frame to (x, y, width, height) with unsigned
/// width/height, for overlay HWND sizing and `Renderer::resize`.
/// Under the agnostic-core design, monitor dimensions are already physical
/// on Windows, so this is a cast-only conversion.
pub(super) fn surface_size_from_physical(dim: Dimension) -> (i32, i32, u32, u32) {
    (
        dim.x.round() as i32,
        dim.y.round() as i32,
        dim.width.round() as u32,
        dim.height.round() as u32,
    )
}

/// Convert physical-pixel WM_GETMINMAXINFO constraints to physical f32 values.
/// Subtracts invisible borders (left+right, top+bottom) before casting,
/// with a floor of 0 so a pathologically large border never produces negative values.
/// Pure helper: no Win32 calls, runs on every target.
pub(super) fn constraints_to_physical(
    min_track: (i32, i32),
    max_track: (i32, i32),
    border: (i32, i32, i32, i32),
) -> (f32, f32, f32, f32) {
    let h_border = border.0 + border.2; // left + right
    let v_border = border.1 + border.3; // top + bottom
    (
        (min_track.0 - h_border).max(0) as f32,
        (min_track.1 - v_border).max(0) as f32,
        (max_track.0 - h_border).max(0) as f32,
        (max_track.1 - v_border).max(0) as f32,
    )
}

const PICKER_WIDTH_LOGICAL: f32 = 400.0;
const PICKER_HEIGHT_LOGICAL: f32 = 300.0;

/// Logical icon edge length matching `ICON_SIZE` in `src/picker.rs`.
/// Used by [`icon_px_for_scale`] to derive the physical capture size.
pub(in crate::platform::windows) const ICON_PX_LOGICAL: i32 = 24;

/// Compute centred physical-pixel rect for the picker window.
/// Scales the logical 400x300 picker size to physical at entry, then clamps
/// and centres within the (already physical) monitor rect.
/// The `.max(1)` floor on dimensions prevents wgpu surface validation failure
/// at degenerate scales.
pub(in crate::platform::windows) fn picker_physical_rect(
    scale: f32,
    monitor_physical: Dimension,
) -> (i32, i32, u32, u32) {
    let picker_w = PICKER_WIDTH_LOGICAL * scale;
    let picker_h = PICKER_HEIGHT_LOGICAL * scale;
    let w = picker_w.min(monitor_physical.width);
    let h = picker_h.min(monitor_physical.height);
    let x = monitor_physical.x + (monitor_physical.width - w) / 2.0;
    let y = monitor_physical.y + (monitor_physical.height - h) / 2.0;
    (
        x.round() as i32,
        y.round() as i32,
        w.round().max(1.0) as u32,
        h.round().max(1.0) as u32,
    )
}

/// Physical icon capture size for a given monitor scale.
/// Scales [`ICON_PX_LOGICAL`] by the monitor scale factor, with a 16-physical-pixel
/// floor because `DrawIconEx` rasterising a shared HICON below ~16px loses
/// recognisable shape.
pub(in crate::platform::windows) fn icon_px_for_scale(scale: f32) -> i32 {
    ((ICON_PX_LOGICAL as f32 * scale).round() as i32).max(16)
}

// --- Windows-only scale lookups ---
// GetDpiForMonitor requires Win10 1607+.
// We rely on PMv2 awareness which itself requires Win10 1703+.
#[cfg(target_os = "windows")]
mod win32 {
    use super::BASE_DPI;
    use windows::Win32::Graphics::Gdi::HMONITOR;
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    /// Returns the scale factor for a monitor, e.g. 1.5 for 144 DPI (150%).
    /// Falls back to 1.0 with a warning on API failure.
    pub(in crate::platform::windows) fn scale_for_monitor(hmonitor: HMONITOR) -> f32 {
        let mut dpi_x: u32 = 0;
        let mut dpi_y: u32 = 0;
        if let Err(e) =
            unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }
        {
            tracing::warn!(%e, ?hmonitor, "GetDpiForMonitor failed, falling back to 1.0");
            return 1.0;
        }
        if dpi_x == 0 {
            tracing::warn!(
                ?hmonitor,
                "GetDpiForMonitor returned 0, falling back to 1.0"
            );
            return 1.0;
        }
        dpi_x as f32 / BASE_DPI
    }
}

#[cfg(target_os = "windows")]
pub(super) use win32::scale_for_monitor;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_to_physical_identity_at_100() {
        assert_eq!(logical_to_physical(100.0, 1.0), 100);
    }

    #[test]
    fn logical_to_physical_125() {
        assert_eq!(logical_to_physical(100.0, 1.25), 125);
    }

    #[test]
    fn logical_to_physical_150() {
        assert_eq!(logical_to_physical(100.0, 1.5), 150);
    }

    #[test]
    fn logical_to_physical_175() {
        assert_eq!(logical_to_physical(100.0, 1.75), 175);
    }

    #[test]
    fn logical_to_physical_200() {
        assert_eq!(logical_to_physical(100.0, 2.0), 200);
    }

    /// f32::round ties round away from zero (not half-up), which matters for
    /// negative coordinates on virtual desktops.
    #[test]
    fn logical_to_physical_rounds_half_away_from_zero() {
        assert_eq!(logical_to_physical(0.5, 1.0), 1);
        assert_eq!(logical_to_physical(1.5, 1.0), 2);
    }

    /// Negative coordinates arise on virtual desktops where a secondary monitor
    /// sits to the left of or above the primary.
    #[test]
    fn logical_to_physical_negative() {
        assert_eq!(logical_to_physical(-100.0, 1.5), -150);
    }

    #[test]
    fn surface_size_from_physical_casts() {
        let dim = Dimension {
            x: 0.0,
            y: 0.0,
            width: 2160.0,
            height: 1350.0,
        };
        assert_eq!(surface_size_from_physical(dim), (0, 0, 2160, 1350));
    }

    #[test]
    fn constraints_to_physical_subtracts_border() {
        // No border: raw physical values cast to f32
        assert_eq!(
            constraints_to_physical((200, 200), (1600, 1200), (0, 0, 0, 0)),
            (200.0, 200.0, 1600.0, 1200.0)
        );
        // With border: subtract (left+right) from width, (top+bottom) from height
        assert_eq!(
            constraints_to_physical((420, 320), (2060, 1160), (10, 10, 10, 10)),
            (400.0, 300.0, 2040.0, 1140.0)
        );
    }

    #[test]
    fn icon_px_for_scale_100() {
        assert_eq!(icon_px_for_scale(1.0), 24);
    }

    #[test]
    fn icon_px_for_scale_125() {
        assert_eq!(icon_px_for_scale(1.25), 30);
    }

    #[test]
    fn icon_px_for_scale_150() {
        assert_eq!(icon_px_for_scale(1.5), 36);
    }

    #[test]
    fn icon_px_for_scale_200() {
        assert_eq!(icon_px_for_scale(2.0), 48);
    }

    #[test]
    fn icon_px_for_scale_below_floor() {
        assert_eq!(icon_px_for_scale(0.5), 16);
    }

    #[test]
    fn picker_physical_rect_100() {
        let monitor = Dimension {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        assert_eq!(picker_physical_rect(1.0, monitor), (760, 390, 400, 300));
    }

    #[test]
    fn picker_physical_rect_150() {
        // Physical monitor: 1920*1.5 = 2880, 1080*1.5 = 1620
        let monitor = Dimension {
            x: 0.0,
            y: 0.0,
            width: 2880.0,
            height: 1620.0,
        };
        assert_eq!(picker_physical_rect(1.5, monitor), (1140, 585, 600, 450));
    }

    #[test]
    fn picker_physical_rect_200() {
        // Physical monitor: 1920*2 = 3840, 1080*2 = 2160
        let monitor = Dimension {
            x: 0.0,
            y: 0.0,
            width: 3840.0,
            height: 2160.0,
        };
        assert_eq!(picker_physical_rect(2.0, monitor), (1520, 780, 800, 600));
    }

    #[test]
    fn picker_physical_rect_centers_offset_origin() {
        // Physical monitor: origin (200,100), size (3840,2160) at 2.0x
        let monitor = Dimension {
            x: 200.0,
            y: 100.0,
            width: 3840.0,
            height: 2160.0,
        };
        assert_eq!(picker_physical_rect(2.0, monitor), (1720, 880, 800, 600));
    }

    #[test]
    fn picker_physical_rect_clamped_to_monitor() {
        // Physical monitor: 200*2=400, 100*2=200 at 2.0x
        let monitor = Dimension {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 200.0,
        };
        assert_eq!(picker_physical_rect(2.0, monitor), (0, 0, 400, 200));
    }

    #[test]
    fn picker_physical_rect_exact_monitor_match() {
        let monitor = Dimension {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 300.0,
        };
        assert_eq!(picker_physical_rect(1.0, monitor), (0, 0, 400, 300));
    }
}
