use std::collections::{HashMap, HashSet};

use crate::platform::windows::display::MonitorInfo;
use dome_core::{Hub, MonitorId, Physical, PixelRect, ReportedMonitor};

impl From<&MonitorInfo> for ReportedMonitor {
    fn from(info: &MonitorInfo) -> Self {
        ReportedMonitor {
            device_name: info.name.clone(),
            work_area: info.work_area,
            scale: info.scale,
            cg_display_id: None,
            gdi_device: Some(info.gdi_device.clone()),
        }
    }
}

pub(super) struct MonitorRegistry {
    monitors: HashMap<MonitorId, MonitorInfo>,
}

impl MonitorRegistry {
    pub(super) fn new() -> Self {
        Self {
            monitors: HashMap::new(),
        }
    }

    pub(super) fn monitor(&self, id: MonitorId) -> &MonitorInfo {
        &self.monitors[&id]
    }

    pub(super) fn insert(&mut self, id: MonitorId, info: &MonitorInfo) {
        self.monitors.insert(id, info.clone());
    }

    pub(super) fn id_for_handle(&self, handle: isize) -> Option<MonitorId> {
        self.monitors
            .iter()
            .find(|(_, m)| m.handle == handle)
            .map(|(&id, _)| id)
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

    pub(super) fn reconcile(&mut self, hub: &mut Hub, monitors: &[MonitorInfo]) {
        let current_handles: HashSet<isize> = monitors.iter().map(|s| s.handle).collect();

        // Ahead of the add loop, so the display the primary vacates is picked up
        // as a new monitor below and the carried primary is not seen as departed.
        if let Some(new_primary) = monitors.iter().find(|s| s.is_primary) {
            let primary_id = hub.primary_monitor();
            let carried = self.monitors.get(&primary_id).map(|m| m.handle);
            if carried != Some(new_primary.handle) {
                let occupant = self.replace_primary(primary_id, new_primary);
                hub.update_monitor(primary_id, new_primary.into(), occupant);
            }
        }

        for monitor in monitors {
            let already_tracked = self.monitors.values().any(|m| m.handle == monitor.handle);
            if !already_tracked {
                let id = hub.add_monitor(monitor.into());
                self.insert(id, monitor);
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
            .iter()
            .filter(|(_, m)| !current_handles.contains(&m.handle))
            .map(|(&id, _)| id)
            .collect();

        for monitor_id in &to_remove {
            self.monitors.remove(monitor_id);
        }
        if !to_remove.is_empty() {
            for monitor_id in &to_remove {
                hub.remove_monitor(*monitor_id);
            }
            let primary = hub.primary_monitor();
            tracing::info!(?to_remove, primary = %primary, "Monitors removed");
        }

        for monitor in monitors {
            let Some((id, old_work_area, old_scale)) = self.update_monitor(monitor) else {
                continue;
            };
            if old_work_area != monitor.work_area || old_scale != monitor.scale {
                tracing::info!(
                    name = %monitor.name,
                    ?old_work_area,
                    new_work_area = ?monitor.work_area,
                    ?old_scale,
                    new_scale = ?monitor.scale,
                    "Monitor work area changed"
                );
            }
            hub.update_monitor(id, monitor.into(), None);
        }
    }

    /// Re-keys the primary entry onto the incoming primary display's handle.
    /// Returns the monitor displaced from that handle, if the registry already
    /// tracked one there.
    fn replace_primary(
        &mut self,
        primary_id: MonitorId,
        new_primary: &MonitorInfo,
    ) -> Option<MonitorId> {
        let occupant = self.id_for_handle(new_primary.handle);
        if let Some(displaced) = occupant {
            self.monitors.remove(&displaced);
        }
        // Keyed by MonitorId with the handle in the value, so the carry is an
        // in-place move onto the new panel.
        if let Some(entry) = self.monitors.get_mut(&primary_id) {
            *entry = new_primary.clone();
        }
        tracing::info!(
            name = %new_primary.name,
            handle = ?new_primary.handle,
            ?occupant,
            "Primary display changed"
        );
        occupant
    }

    /// Mirrors `monitor` into the tracked entry and returns its id with the
    /// previous work area and scale. Windows can move a szDevice or rename a
    /// display with no geometry change, so the mirror is unconditional.
    fn update_monitor(&mut self, monitor: &MonitorInfo) -> Option<(MonitorId, PixelRect, f64)> {
        let (&id, entry) = self
            .monitors
            .iter_mut()
            .find(|(_, m)| m.handle == monitor.handle)?;
        let previous = (id, entry.work_area, entry.scale);
        *entry = monitor.clone();
        Some(previous)
    }
}
