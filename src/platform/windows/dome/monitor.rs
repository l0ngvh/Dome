use std::collections::{HashMap, HashSet};

use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, MONITORINFOF_PRIMARY};
use windows::core::BOOL;

use crate::config::OuterGaps;
use crate::core::{Dimension, Hub, Length, MonitorId, Physical, WindowId};
use crate::platform::windows::external::HwndId;
use crate::platform::windows::handle;

#[derive(Clone)]
pub(in crate::platform::windows) struct MonitorInfo {
    pub handle: isize,
    pub name: String,
    pub dimension: Dimension,
    pub is_primary: bool,
    /// DPI scale factor for this monitor (e.g. 1.5 for 150%). Always > 0.
    pub scale: f32,
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

/// Per-monitor state: physical dimension, DPI scale, and the set of windows
/// currently laid out on this monitor (rebuilt each `apply_layout` pass).
pub(super) struct Monitor {
    id: MonitorId,
    handle: isize,
    dimension: Dimension,
    scale: f32,
    displayed: HashSet<WindowId>,
}

impl Monitor {
    pub(super) fn dimension(&self) -> Dimension {
        self.dimension
    }

    pub(super) fn scale(&self) -> f32 {
        self.scale
    }

    pub(super) fn physical_border(&self, border_size: f32) -> Length<Physical> {
        Length::new(border_size * self.scale)
    }

    pub(super) fn displayed(&self) -> &HashSet<WindowId> {
        &self.displayed
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

    pub(super) fn monitors(&self) -> impl Iterator<Item = &Monitor> + '_ {
        self.monitors.values()
    }

    pub(super) fn insert(
        &mut self,
        handle: isize,
        id: MonitorId,
        dimension: Dimension,
        scale: f32,
    ) {
        self.monitors.insert(
            id,
            Monitor {
                id,
                handle,
                dimension,
                scale,
                displayed: HashSet::new(),
            },
        );
    }

    pub(super) fn id_for_handle(&self, handle: isize) -> Option<MonitorId> {
        self.monitors
            .values()
            .find(|m| m.handle == handle)
            .map(|m| m.id)
    }

    pub(super) fn remove_window_from_displayed(&mut self, window_id: WindowId) {
        for m in self.monitors.values_mut() {
            m.displayed.remove(&window_id);
        }
    }

    pub(super) fn clear_all_displayed(&mut self) {
        for m in self.monitors.values_mut() {
            m.displayed.clear();
        }
    }

    pub(super) fn set_displayed_windows(
        &mut self,
        monitor_id: MonitorId,
        displayed: HashSet<WindowId>,
    ) {
        self.monitors
            .get_mut(&monitor_id)
            .expect("monitor present")
            .displayed = displayed;
    }

    pub(super) fn physical_border(&self, id: MonitorId, border_size: f32) -> Length<Physical> {
        self.monitor(id).physical_border(border_size)
    }

    pub(super) fn is_borderless_fullscreen_at(
        &self,
        rect: Dimension<Physical>,
        handle: isize,
    ) -> bool {
        self.monitors
            .values()
            .find(|m| m.handle == handle)
            .map(|m| {
                rect.x <= m.dimension.x
                    && rect.y <= m.dimension.y
                    && rect.x + rect.width >= m.dimension.x + m.dimension.width
                    && rect.y + rect.height >= m.dimension.y + m.dimension.height
            })
            .unwrap_or(false)
    }

    /// Reconciles the registry against a fresh monitor list from the OS.
    /// Adds new monitors, removes stale ones, and updates dimensions/scales
    /// for monitors that changed. Returns the set of added and removed IDs
    /// so the caller can drive overlay creation/destruction.
    pub(super) fn reconcile(
        &mut self,
        hub: &mut Hub,
        monitors: &[MonitorInfo],
        outer_gaps: OuterGaps,
    ) -> MonitorChange {
        let mut added = Vec::new();
        let mut removed = Vec::new();

        let current_handles: HashSet<isize> = monitors.iter().map(|s| s.handle).collect();

        for monitor in monitors {
            let already_tracked = self.monitors.values().any(|m| m.handle == monitor.handle);
            if !already_tracked {
                let id = hub.add_monitor(
                    monitor.name.clone(),
                    outer_gaps.apply_to(monitor.dimension, monitor.scale),
                    monitor.scale,
                );
                self.insert(monitor.handle, id, monitor.dimension, monitor.scale);
                added.push(id);
                tracing::info!(
                    name = %monitor.name,
                    handle = ?monitor.handle,
                    dimension = ?monitor.dimension,
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

        for monitor_id in to_remove {
            if let Some(fallback_id) = fallback
                && fallback_id != monitor_id
            {
                hub.remove_monitor(monitor_id, fallback_id);
                self.monitors.remove(&monitor_id);
                removed.push(monitor_id);
                tracing::info!(%monitor_id, fallback = %fallback_id, "Monitor removed");
            }
        }

        for monitor in monitors {
            if let Some(id) = self.id_for_handle(monitor.handle)
                && let Some(ms) = self.monitors.get(&id)
                && (ms.dimension != monitor.dimension || ms.scale != monitor.scale)
            {
                let old_dim = Some(ms.dimension);
                let old_scale = Some(ms.scale);
                tracing::info!(
                    name = %monitor.name,
                    ?old_dim,
                    new_dim = ?monitor.dimension,
                    ?old_scale,
                    new_scale = ?monitor.scale,
                    "Monitor dimension changed"
                );
                let ms = self.monitors.get_mut(&id).expect("just checked");
                ms.dimension = monitor.dimension;
                ms.scale = monitor.scale;
                hub.update_monitor(
                    id,
                    outer_gaps.apply_to(monitor.dimension, monitor.scale),
                    monitor.scale,
                );
            }
        }

        MonitorChange { added, removed }
    }

    pub(super) fn apply_dpi_change(
        &mut self,
        handle: isize,
        dpi: u32,
        hub: &mut Hub,
        outer_gaps: OuterGaps,
    ) {
        let Some(id) = self.id_for_handle(handle) else {
            tracing::warn!(handle, dpi, "DPI change for unknown monitor handle");
            return;
        };
        let scale = dpi as f32 / BASE_DPI;
        if self.monitors.get(&id).is_some_and(|ms| ms.scale == scale) {
            return;
        }
        let previous = self.monitors.get_mut(&id).map(|ms| {
            let prev = ms.scale;
            ms.scale = scale;
            prev
        });
        let dim = self.monitors[&id].dimension;
        hub.update_monitor(id, outer_gaps.apply_to(dim, scale), scale);
        tracing::info!(%id, dpi, scale, ?previous, "Monitor scale updated via DPI change");
    }

    pub(super) fn update_hub_dimensions(&self, hub: &mut Hub, outer_gaps: OuterGaps) {
        for monitor in self.monitors.values() {
            hub.update_monitor(
                monitor.id,
                outer_gaps.apply_to(monitor.dimension, monitor.scale),
                monitor.scale,
            );
        }
    }
}

/// Windows baseline DPI (100% scaling). All scale factors are relative to this.
const BASE_DPI: f32 = 96.0;

/// Returns the scale factor for a monitor, e.g. 1.5 for 144 DPI (150%).
/// Falls back to 1.0 with a warning on API failure.
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
