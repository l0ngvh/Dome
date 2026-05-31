use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, MONITORINFOF_PRIMARY};
use windows::core::BOOL;

use crate::platform::windows::dome::QueryDisplay;
use crate::platform::windows::external::HwndId;
use crate::platform::windows::handle;

use super::MonitorInfo;

pub(super) struct Win32Display;

impl QueryDisplay for Win32Display {
    fn get_all_monitors(&self) -> anyhow::Result<Vec<MonitorInfo>> {
        get_all_monitors()
    }

    fn get_exclusive_fullscreen_hwnd(&self) -> Option<HwndId> {
        if is_d3d_exclusive_fullscreen_active() {
            Some(HwndId::from(unsafe { GetForegroundWindow() }))
        } else {
            None
        }
    }
}

fn get_all_monitors() -> anyhow::Result<Vec<MonitorInfo>> {
    let mut monitors = Vec::new();

    unsafe extern "system" fn enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let monitors = unsafe { &mut *(lparam.0 as *mut Vec<MonitorInfo>) };
        let mut info = MONITORINFOEXW {
            monitorInfo: windows::Win32::Graphics::Gdi::MONITORINFO {
                cbSize: size_of::<MONITORINFOEXW>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        if unsafe { GetMonitorInfoW(hmonitor, &mut info.monitorInfo) }.as_bool() {
            let rc = info.monitorInfo.rcWork;
            let name = String::from_utf16_lossy(
                &info
                    .szDevice
                    .iter()
                    .take_while(|&&c| c != 0)
                    .copied()
                    .collect::<Vec<_>>(),
            );

            let scale = scale_for_monitor(hmonitor);

            monitors.push(MonitorInfo {
                handle: hmonitor.0 as isize,
                name,
                dimension: handle::rect_to_dimension(rc),
                is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                scale,
            });
        }
        BOOL(1)
    }

    let success = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        )
    };
    anyhow::ensure!(success.as_bool(), "EnumDisplayMonitors failed");
    Ok(monitors)
}

fn is_d3d_exclusive_fullscreen_active() -> bool {
    unsafe { SHQueryUserNotificationState() }
        .is_ok_and(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN)
}

/// Windows baseline DPI (100% scaling). All scale factors are relative to this.
pub(super) const BASE_DPI: f32 = 96.0;

/// Returns the scale factor for a monitor, e.g. 1.5 for 144 DPI (150%).
/// Falls back to 1.0 with a warning on API failure.
pub(super) fn scale_for_monitor(hmonitor: HMONITOR) -> f32 {
    let mut dpi_x: u32 = 0;
    let mut dpi_y: u32 = 0;
    if let Err(e) = unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }
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
