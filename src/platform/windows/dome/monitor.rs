use std::collections::{HashMap, HashSet};

use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, MONITORINFOF_PRIMARY};
use windows::core::BOOL;

use crate::core::{Dimension, Hub, MonitorId, Physical, PixelRect};
use crate::platform::windows::external::HwndId;
use crate::platform::windows::handle;

#[derive(Clone)]
pub(in crate::platform::windows) struct MonitorInfo {
    pub(in crate::platform::windows) handle: isize,
    /// EDID friendly name (e.g. "DELL U2720Q") when one is available, else the
    /// GDI device string (`\\.\DISPLAY1`) as a last-resort human-usable label.
    pub(in crate::platform::windows) name: String,
    /// GDI device string (`\\.\DISPLAY1`). Join key for the EDID friendly name
    /// lookup after enumeration, and published on `dome query monitors`. Not a
    /// display label.
    pub(in crate::platform::windows) gdi_device: String,
    pub(in crate::platform::windows) work_area: PixelRect,
    /// Stays fractional because its only consumer is `reserve_for_bar`, whose
    /// f32 edge math is shared with macOS.
    pub(in crate::platform::windows) bounds: Dimension,
    pub(in crate::platform::windows) is_primary: bool,
    /// Always > 0.
    pub(in crate::platform::windows) scale: f32,
}

pub(in crate::platform::windows) trait QueryDisplay {
    fn get_all_monitors(&self) -> anyhow::Result<Vec<MonitorInfo>>;
    /// Returns the hwnd of the foreground window if D3D exclusive fullscreen is active.
    fn get_exclusive_fullscreen_hwnd(&self) -> Option<HwndId>;
}

pub(in crate::platform::windows) struct Win32Display;

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

pub(super) struct Monitor {
    id: MonitorId,
    handle: isize,
    name: String,
    work_area: PixelRect,
    scale: f32,
}

impl Monitor {
    #[expect(
        dead_code,
        reason = "read by the monitor-name selector filter for --monitor targeting"
    )]
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn work_area(&self) -> PixelRect {
        self.work_area
    }

    pub(super) fn scale(&self) -> f32 {
        self.scale
    }
}

pub(super) struct MonitorChange {
    pub(super) added: Vec<MonitorId>,
    pub(super) removed: Vec<MonitorId>,
}

pub(super) struct MonitorRegistry {
    monitors: HashMap<MonitorId, Monitor>,
    primary: Option<MonitorId>,
}

impl MonitorRegistry {
    pub(super) fn new() -> Self {
        Self {
            monitors: HashMap::new(),
            primary: None,
        }
    }

    pub(super) fn monitor(&self, id: MonitorId) -> &Monitor {
        &self.monitors[&id]
    }

    pub(super) fn insert(
        &mut self,
        handle: isize,
        id: MonitorId,
        name: String,
        work_area: PixelRect,
        scale: f32,
    ) {
        self.monitors.insert(
            id,
            Monitor {
                id,
                handle,
                name,
                work_area,
                scale,
            },
        );
    }

    pub(super) fn id_for_handle(&self, handle: isize) -> Option<MonitorId> {
        self.monitors
            .values()
            .find(|m| m.handle == handle)
            .map(|m| m.id)
    }

    pub(super) fn is_borderless_fullscreen_at(
        &self,
        rect: PixelRect<Physical>,
        handle: isize,
    ) -> bool {
        self.monitors
            .values()
            .find(|m| m.handle == handle)
            .map(|m| {
                let mon = m.work_area;
                rect.x() <= mon.x()
                    && rect.y() <= mon.y()
                    && rect.right() >= mon.right()
                    && rect.bottom() >= mon.bottom()
            })
            .unwrap_or(false)
    }

    pub(super) fn reconcile(&mut self, hub: &mut Hub, monitors: &[MonitorInfo]) -> MonitorChange {
        let mut added = Vec::new();
        let mut removed = Vec::new();

        let current_handles: HashSet<isize> = monitors.iter().map(|s| s.handle).collect();

        for monitor in monitors {
            let already_tracked = self.monitors.values().any(|m| m.handle == monitor.handle);
            if !already_tracked {
                let id = hub.add_monitor(monitor.name.clone(), monitor.work_area, monitor.scale);
                self.insert(
                    monitor.handle,
                    id,
                    monitor.name.clone(),
                    monitor.work_area,
                    monitor.scale,
                );
                added.push(id);
                tracing::info!(
                    name = %monitor.name,
                    handle = ?monitor.handle,
                    work_area = ?monitor.work_area,
                    "Monitor added"
                );
            }
        }

        let to_remove: Vec<MonitorId> = self
            .monitors
            .values()
            .filter(|m| !current_handles.contains(&m.handle))
            .map(|m| m.id)
            .collect();

        let fallback = monitors
            .iter()
            .find(|s| s.is_primary)
            .and_then(|s| self.id_for_handle(s.handle));
        self.primary = fallback;

        let primary = self
            .primary
            .expect("a primary monitor must exist after reconcile");
        for monitor_id in &to_remove {
            self.monitors.remove(monitor_id);
        }
        if !to_remove.is_empty() {
            for monitor_id in &to_remove {
                hub.remove_monitor(*monitor_id, primary);
            }
            removed.extend(&to_remove);
            tracing::info!(?to_remove, primary = %primary, "Monitors removed");
        }

        for monitor in monitors {
            let Some(id) = self.id_for_handle(monitor.handle) else {
                continue;
            };
            // Ahead of the change check, because Windows can move a szDevice to
            // another display without the work area or scale moving with it.
            hub.set_monitor_gdi_device(id, monitor.gdi_device.clone());
            if let Some(ms) = self.monitors.get(&id)
                && (ms.work_area != monitor.work_area || ms.scale != monitor.scale)
            {
                let old_work_area = Some(ms.work_area);
                let old_scale = Some(ms.scale);
                tracing::info!(
                    name = %monitor.name,
                    ?old_work_area,
                    new_work_area = ?monitor.work_area,
                    ?old_scale,
                    new_scale = ?monitor.scale,
                    "Monitor work area changed"
                );
                let ms = self.monitors.get_mut(&id).expect("just checked");
                ms.work_area = monitor.work_area;
                ms.scale = monitor.scale;
                hub.update_monitor(id, monitor.work_area, monitor.scale);
            }
        }

        MonitorChange { added, removed }
    }

    /// Returns `true` when the scale changed and was applied.
    pub(super) fn apply_dpi_change(&mut self, handle: isize, dpi: u32, hub: &mut Hub) -> bool {
        let Some(id) = self.id_for_handle(handle) else {
            tracing::warn!(handle, dpi, "DPI change for unknown monitor handle");
            return false;
        };
        let scale = dpi as f32 / BASE_DPI;
        if self.monitors.get(&id).is_some_and(|ms| ms.scale == scale) {
            return false;
        }
        let previous = self.monitors.get_mut(&id).map(|ms| {
            let prev = ms.scale;
            ms.scale = scale;
            prev
        });
        let dim = self.monitors[&id].work_area;
        hub.update_monitor(id, dim, scale);
        tracing::info!(%id, dpi, scale, ?previous, "Monitor scale updated via DPI change");
        true
    }
}

/// Windows baseline DPI (100% scaling).
const BASE_DPI: f32 = 96.0;

fn scale_for_monitor(hmonitor: HMONITOR) -> f32 {
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

fn get_all_monitors() -> anyhow::Result<Vec<MonitorInfo>> {
    let mut monitors: Vec<MonitorInfo> = Vec::new();

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
            let rc_monitor = info.monitorInfo.rcMonitor;
            let gdi_device = utf16_to_string(&info.szDevice);

            let scale = scale_for_monitor(hmonitor);

            // name is resolved once, after enumeration, from the friendly-name
            // map keyed on gdi_device. It is left empty here so it never
            // transiently holds the GDI device string.
            monitors.push(MonitorInfo {
                handle: hmonitor.0 as isize,
                name: String::new(),
                gdi_device,
                work_area: handle::rect_to_pixel_rect(rc),
                bounds: handle::rect_to_dimension(rc_monitor),
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

    // Resolve each monitor's name exactly once, now that enumeration is done.
    // The DisplayConfig API is path-keyed rather than per-HMONITOR, so the
    // friendly-name map can only be built after enumeration and correlated
    // back via the GDI device string. Use the EDID friendly name when present,
    // else fall back to the GDI device string (`\\.\DISPLAY1`). The fallback is
    // required: an empty monitorFriendlyDeviceName is a documented case for
    // headless/forced targets, virtual/RDP displays, and pass-through panels
    // with no readable EDID, so name must always carry something human-usable.
    let friendly = friendly_names_by_gdi_device();
    for monitor in &mut monitors {
        monitor.name = friendly
            .get(&monitor.gdi_device)
            .cloned()
            .unwrap_or_else(|| monitor.gdi_device.clone());
    }

    Ok(monitors)
}

/// Maps each active monitor's GDI device name (`\\.\DISPLAY1`) to its EDID
/// friendly name (e.g. "DELL U2720Q") via one QueryDisplayConfig pass. Monitors
/// with an empty friendly name or a failed path lookup are omitted, so callers
/// fall back to the GDI device name.
fn friendly_names_by_gdi_device() -> HashMap<String, String> {
    let mut map = HashMap::new();

    // Display config can change between GetDisplayConfigBufferSizes and
    // QueryDisplayConfig, which then returns ERROR_INSUFFICIENT_BUFFER. Retry a
    // bounded number of times rather than risk an unbounded loop.
    // See https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-querydisplayconfig
    const MAX_RETRIES: u32 = 5;
    for _ in 0..MAX_RETRIES {
        let mut path_count: u32 = 0;
        let mut mode_count: u32 = 0;
        let sizes = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        };
        if sizes != ERROR_SUCCESS {
            tracing::warn!(
                error = ?sizes,
                "GetDisplayConfigBufferSizes failed, falling back to GDI device names"
            );
            return map;
        }

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        let query = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                None,
            )
        };
        if query == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if query != ERROR_SUCCESS {
            tracing::warn!(
                error = ?query,
                "QueryDisplayConfig failed, falling back to GDI device names"
            );
            return map;
        }

        paths.truncate(path_count as usize);
        for path in &paths {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                },
                ..Default::default()
            };
            if unsafe { DisplayConfigGetDeviceInfo(&mut source.header) } != ERROR_SUCCESS.0 as i32 {
                continue;
            }
            let gdi_device = utf16_to_string(&source.viewGdiDeviceName);
            if gdi_device.is_empty() {
                continue;
            }

            let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                    size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                    adapterId: path.targetInfo.adapterId,
                    id: path.targetInfo.id,
                },
                ..Default::default()
            };
            if unsafe { DisplayConfigGetDeviceInfo(&mut target.header) } != ERROR_SUCCESS.0 as i32 {
                continue;
            }
            let friendly = utf16_to_string(&target.monitorFriendlyDeviceName);
            if !friendly.is_empty() {
                map.insert(gdi_device, friendly);
            }
        }
        return map;
    }

    map
}

fn utf16_to_string(units: &[u16]) -> String {
    let end = units.iter().take_while(|&&c| c != 0).count();
    String::from_utf16_lossy(&units[..end])
}

fn is_d3d_exclusive_fullscreen_active() -> bool {
    unsafe { SHQueryUserNotificationState() }
        .is_ok_and(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_to_string_stops_at_nul() {
        let units: Vec<u16> = "AB\0CD".encode_utf16().collect();
        assert_eq!(utf16_to_string(&units), "AB");
    }

    #[test]
    fn utf16_to_string_empty_input() {
        assert_eq!(utf16_to_string(&[]), "");
    }

    #[test]
    fn utf16_to_string_no_nul_decodes_whole_slice() {
        let units: Vec<u16> = "ABCD".encode_utf16().collect();
        assert_eq!(utf16_to_string(&units), "ABCD");
    }
}
