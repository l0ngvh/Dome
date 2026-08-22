use crate::action::MonitorTarget;

use super::allocator::{Node, NodeId};
use super::hub::{Hub, RestrictedAction};
use super::node::{MonitorId, PixelRect, Pixels, WorkspaceId};
use super::workspace::{Attachment, Workspace};

#[derive(Debug, Clone)]
pub(super) struct Monitor {
    /// Raw name the platform reports, never suffixed.
    pub(super) device_name: String,
    /// Stable name Dome derives for this monitor. No platform handle is stable
    /// across platforms, so Dome uses `device_name` when it is unique among
    /// active monitors. On a collision it appends `#N` by screen position, left
    /// to right. The same arrangement always yields the same names.
    pub(super) unique_name: String,
    /// `CGDirectDisplayID`. Windows has no stable counterpart, so `None` there.
    pub(super) cg_display_id: Option<u32>,
    /// Win32 szDevice (`\\.\DISPLAY1`). `None` on macOS. Restamped every
    /// reconcile because Windows can move it to another display.
    pub(super) gdi_device: Option<String>,
    pub(super) work_area: PixelRect,
    /// Multiplier applied to config-denominated lengths before layout math on
    /// this monitor.
    ///
    /// - macOS: always `1.0`. AppKit, AX, and Core Graphics all express window
    ///   geometry in logical points, which is also the config unit.
    /// - Windows: the monitor's DPI scale (e.g. `1.5` at 150%). PMv2 reports
    ///   rects in physical pixels, but config values are logical, so core
    ///   multiplies them into the frame unit.
    pub(super) scale: f32,
    pub(super) active_workspace: WorkspaceId,
}

impl Node for Monitor {
    type Id = MonitorId;
}

/// What a platform reconcile reports for one monitor.
pub(crate) struct ReportedMonitor {
    pub(crate) device_name: String,
    pub(crate) work_area: PixelRect,
    pub(crate) scale: f32,
    pub(crate) cg_display_id: Option<u32>,
    pub(crate) gdi_device: Option<String>,
}

impl Hub {
    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_monitor(&mut self, target: &MonitorTarget) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let Some(target_id) = self.find_monitor_by_target(target) else {
            return;
        };
        if target_id == self.access.focused_monitor {
            return;
        }
        tracing::debug!("Focusing monitor");
        self.access.focused_monitor = target_id;
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn move_focused_to_monitor(&mut self, target: &MonitorTarget) {
        if self.is_restricted(RestrictedAction::MonitorMove) {
            return;
        }
        let Some(target_id) = self.find_monitor_by_target(target) else {
            return;
        };
        if target_id == self.access.focused_monitor {
            return;
        }

        let target_ws = self.access.monitors.get(target_id).active_workspace;
        tracing::debug!("Moving to monitor");
        let current_ws = self.current_workspace();
        if let Some(window_id) = self.focused_window(current_ws) {
            self.move_child_to_workspace_with_id(window_id, target_ws);
        } else {
            self.move_focused_across_workspaces(current_ws, target_ws);
        }
    }

    pub(crate) fn add_monitor(&mut self, reported: ReportedMonitor) -> MonitorId {
        let monitor_id = self.access.monitors.allocate(Monitor {
            device_name: reported.device_name.clone(),
            unique_name: reported.device_name,
            cg_display_id: reported.cg_display_id,
            gdi_device: reported.gdi_device,
            work_area: reported.work_area,
            scale: reported.scale,
            // Placeholder. A workspace needs this monitor's id, so the real
            // active one is chosen at the end.
            active_workspace: WorkspaceId::new(0),
        });
        self.recompute_monitor_names();

        // Read `unique_name` after the recompute above, never before. A stale
        // pre-recompute suffix would miss a parked workspace's frozen origin.
        let origin_name = self.access.monitors.get(monitor_id).unique_name.clone();
        let mut returning: Vec<WorkspaceId> = self
            .access
            .workspaces
            .sorted_ids()
            .into_iter()
            .filter(|&ws_id| {
                self.access.workspaces.get(ws_id).origin() == Some(origin_name.as_str())
            })
            .collect();
        // Ordered by name so the choice below is deterministic, not allocator order.
        returning.sort_by_key(|ws_id| self.access.workspaces.get(*ws_id).name.clone());

        for &ws_id in &returning {
            let ws = self.access.workspaces.get_mut(ws_id);
            // While parked, `monitor` still points at the rental host. The
            // re-home writes below overwrite it, so capture the old host now.
            let previous_host = ws.monitor;
            ws.attachment = Attachment::Attached;
            ws.monitor = monitor_id;
            self.strategies
                .for_workspace_mut(ws_id)
                .compute_placement(&self.access, ws_id);
            // If the old host's active pointer named this workspace, it now
            // dangles. Fall it back to a workspace the host still owns.
            if self.access.monitors.get(previous_host).active_workspace == ws_id {
                let own = self
                    .access
                    .workspaces
                    .sorted_ids()
                    .into_iter()
                    .find(|&ws_id| {
                        let ws = self.access.workspaces.get(ws_id);
                        ws.monitor == previous_host && ws.is_attached()
                    });
                if let Some(own_id) = own {
                    self.access.monitors.get_mut(previous_host).active_workspace = own_id;
                }
            }
        }

        // A monitor that brought workspaces back needs no default. A default
        // minted regardless would leave two workspaces named "0" after a replug,
        // and only one is reachable by name.
        //
        // Prefer a returning workspace that holds windows. The monitor's active
        // pointer died with it, so an empty active workspace would read as lost
        // windows.
        let returning_active = returning
            .iter()
            .find(|&&ws_id| {
                let ws = self.access.workspaces.get(ws_id);
                self.count_workspace_windows(ws_id, ws) > 0
            })
            .or(returning.first())
            .copied();

        let active = match returning_active {
            Some(ws_id) => ws_id,
            None => {
                let workspace_name = "0".to_string();
                let ws_id = self
                    .access
                    .workspaces
                    .allocate(Workspace::new(workspace_name.clone(), monitor_id));
                self.strategies.register(&mut self.access, ws_id);
                ws_id
            }
        };
        self.access.monitors.get_mut(monitor_id).active_workspace = active;
        monitor_id
    }

    /// Re-derives every active monitor's `unique_name`.
    ///
    /// A `#N` rank depends on the whole active set. Call this on every add,
    /// remove, or move.
    fn recompute_monitor_names(&mut self) {
        let all: Vec<(MonitorId, String, PixelRect)> = self
            .access
            .monitors
            .sorted_ids()
            .into_iter()
            .map(|id| {
                let m = self.access.monitors.get(id);
                (id, m.device_name.clone(), m.work_area)
            })
            .collect();
        for (id, device_name, _) in &all {
            let mut colliders: Vec<&(MonitorId, String, PixelRect)> =
                all.iter().filter(|(_, dn, _)| dn == device_name).collect();
            let unique = if colliders.len() == 1 {
                device_name.clone()
            } else {
                colliders.sort_by_key(|(_, _, r)| (r.x(), r.y()));
                let rank = colliders
                    .iter()
                    .position(|(cid, _, _)| cid == id)
                    .expect("self in colliders");
                format!("{device_name} #{}", rank + 1)
            };
            self.access.monitors.get_mut(*id).unique_name = unique;
        }
    }

    pub(crate) fn remove_monitor(&mut self, monitor_id: MonitorId) {
        let primary = self.access.primary_monitor;

        assert!(
            monitor_id != primary,
            "removed monitor must not be the rental host primary"
        );

        // If focus sits on the departing monitor, it moves to the primary, a
        // guaranteed survivor. The primary's active workspace becomes current,
        // because current tracks focus.
        if self.access.focused_monitor == monitor_id {
            self.access.focused_monitor = primary;
        }

        // Snapshot this monitor's `unique_name` before the delete. An Attached
        // workspace here has no origin yet, so it should freeze this monitor's
        // name.
        let this_origin = self.access.monitors.get(monitor_id).unique_name.clone();
        let ws_on_this: Vec<WorkspaceId> = self
            .access
            .workspaces
            .sorted_ids()
            .into_iter()
            .filter(|&ws_id| self.access.workspaces.get(ws_id).monitor == monitor_id)
            .collect();

        // Every workspace here is Attached. A parked one rents to the primary,
        // which is never removed, so the frozen origin is always this monitor's
        // own name.
        for ws_id in ws_on_this {
            let ws = self.access.workspaces.get_mut(ws_id);
            // Rent to the primary. A monitor stays detached for long essentially
            // only on a laptop undock, so the primary is the display in front of
            // the user.
            ws.monitor = primary;
            ws.attachment = Attachment::Parked {
                origin: this_origin.clone(),
            };
            self.strategies
                .for_workspace_mut(ws_id)
                .compute_placement(&self.access, ws_id);
        }

        // Safe to delete now, because no workspace's `monitor` field points at it.
        self.access.monitors.delete(monitor_id);

        // Restamp `unique_name` across the survivors. After the snapshot, so the
        // frozen origin reflects the pre-removal set. After the delete, so the
        // recompute sees only survivors.
        self.recompute_monitor_names();
    }

    /// Apply the latest reported description to an existing monitor.
    ///
    /// `displaced` is set when another monitor is replaced by this one, for
    /// example when display mirroring turns on.
    pub(crate) fn update_monitor(
        &mut self,
        monitor_id: MonitorId,
        reported: ReportedMonitor,
        displaced: Option<MonitorId>,
    ) {
        // Remove displaced before the rename below, while it still owns its name.
        // Its workspaces park and freeze that name so they return on replug. If
        // the rename ran first, both monitors would share the name, each would
        // take a numbered suffix, and the parked name would never match.
        if let Some(displaced) = displaced {
            self.remove_monitor(displaced);
        }

        let monitor = self.access.monitors.get_mut(monitor_id);
        let geometry_changed =
            monitor.work_area != reported.work_area || monitor.scale != reported.scale;
        if !geometry_changed
            && monitor.device_name == reported.device_name
            && monitor.cg_display_id == reported.cg_display_id
            && monitor.gdi_device == reported.gdi_device
        {
            return;
        }
        monitor.device_name = reported.device_name;
        monitor.work_area = reported.work_area;
        monitor.scale = reported.scale;
        monitor.cg_display_id = reported.cg_display_id;
        monitor.gdi_device = reported.gdi_device;

        if geometry_changed {
            // Collect IDs first, so the strategy call can take `&mut self.access`
            // without a live borrow of `self.access.workspaces`.
            let ws_ids: Vec<WorkspaceId> = self
                .access
                .workspaces
                .sorted_ids()
                .into_iter()
                .filter(|&id| self.access.workspaces.get(id).monitor == monitor_id)
                .collect();
            for ws_id in ws_ids {
                self.strategies
                    .for_workspace_mut(ws_id)
                    .compute_placement(&self.access, ws_id);
            }
        }

        self.recompute_monitor_names();
    }

    pub(super) fn monitor_id_by_disambiguated_name(&self, name: &str) -> Option<MonitorId> {
        self.access
            .monitors
            .sorted_ids()
            .into_iter()
            .find(|&id| self.access.monitors.get(id).unique_name == *name)
    }

    fn find_monitor_by_target(&self, target: &MonitorTarget) -> Option<MonitorId> {
        match target {
            MonitorTarget::Name(name) => self
                .access
                .monitors
                .sorted_ids()
                .into_iter()
                .find(|&id| self.access.monitors.get(id).unique_name == *name),
            direction => {
                let current = self
                    .access
                    .monitors
                    .get(self.access.focused_monitor)
                    .work_area;
                // Doubled centres, so an odd extent does not lose half a unit to integer
                // division. Only differences of centres are used, so the factor cancels.
                let cx2 = 2 * current.x() + current.width();
                let cy2 = 2 * current.y() + current.height();

                self.access
                    .monitors
                    .sorted_ids()
                    .into_iter()
                    .filter(|&id| id != self.access.focused_monitor)
                    .filter_map(|id| {
                        let m = self.access.monitors.get(id).work_area;
                        let dx = 2 * m.x() + m.width() - cx2;
                        let dy = 2 * m.y() + m.height() - cy2;

                        let valid = match direction {
                            MonitorTarget::Left => dx < Pixels::ZERO,
                            MonitorTarget::Right => dx > Pixels::ZERO,
                            MonitorTarget::Up => dy < Pixels::ZERO,
                            MonitorTarget::Down => dy > Pixels::ZERO,
                            MonitorTarget::Name(_) => false,
                        };
                        let dx = i64::from(dx.value());
                        let dy = i64::from(dy.value());
                        valid.then_some((id, dx * dx + dy * dy))
                    })
                    .min_by_key(|(_, dist_sq)| *dist_sq)
                    .map(|(id, _)| id)
            }
        }
    }
}
