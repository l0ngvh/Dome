use crate::action::MonitorTarget;

use super::allocator::{Node, NodeId};
use super::hub::{Hub, RestrictedAction};
use super::node::{MonitorId, PixelRect, Pixels, WorkspaceId};
use super::workspace::{Attachment, Workspace};

/// Core is coordinate-system-agnostic: `work_area` holds whatever rect
/// the platform supplies in its own native frame (logical on macOS,
/// physical on Windows). Core never characterises or converts the
/// unit -- all layout math is unit-agnostic.
#[derive(Debug, Clone)]
pub(super) struct Monitor {
    /// Raw name the platform reports for this monitor, never modified or
    /// suffixed. Two monitors of the same model can report the same name, so
    /// `unique_name` appends a `#N` suffix to tell them apart.
    pub(super) device_name: String,
    /// Stable, human-readable name Dome derives for this monitor. No monitor
    /// identifier is consistent across platforms, so rather than rely on an
    /// opaque platform handle Dome names each monitor by its `device_name`
    /// alone when that is unique among the active monitors. When several active
    /// monitors report the same `device_name`, Dome appends a `#N` suffix
    /// assigned consistently by screen position from left to right, so the same
    /// set of monitors in the same arrangement always yields the same names.
    pub(super) unique_name: String,
    // Stamped by the owning platform, not derived from the monitor set, so
    // `recompute_monitor_names` leaves them alone.
    /// `CGDirectDisplayID`. Windows has no stable counterpart, so `None` there.
    pub(super) cg_display_id: Option<u32>,
    /// Win32 szDevice (`\\.\DISPLAY1`). `None` on macOS. Restamped every
    /// reconcile because Windows can move it to another display.
    pub(super) gdi_device: Option<String>,
    pub(super) work_area: PixelRect,
    /// Multiplier applied to config-denominated lengths before use in
    /// layout math on this monitor. Stored here so `SizeConstraint::resolve`
    /// can convert logical config values without re-reading platform state.
    ///
    /// - macOS: always `1.0`. AppKit, AX, and Core Graphics all express
    ///   window geometry in logical points, which is also the config unit.
    /// - Windows: the monitor's DPI scale (e.g. `1.5` at 150%). PMv2
    ///   reports rects in physical pixels, but config values are logical
    ///   pixels, so they must be multiplied to reach the frame unit.
    pub(super) scale: f32,
    pub(super) active_workspace: WorkspaceId,
}

impl Node for Monitor {
    type Id = MonitorId;
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

    pub(crate) fn add_monitor(
        &mut self,
        name: String,
        work_area: PixelRect,
        scale: f32,
    ) -> MonitorId {
        let monitor_id = self.access.monitors.allocate(Monitor {
            device_name: name.clone(),
            unique_name: name.clone(),
            cg_display_id: None,
            gdi_device: None,
            work_area,
            scale,
            // Set at the end, once it is known whether a parked workspace
            // returns or a default has to be minted. Workspace::new needs the
            // monitor id, so neither can be built first.
            active_workspace: WorkspaceId::new(0),
        });
        self.recompute_monitor_names();

        // The name recompute above restamped `unique_name` across the now-larger
        // set, so the returning monitor's stored `unique_name` is fresh here. Any
        // parked workspace whose frozen origin matches it re-homes onto this
        // monitor. The match must read the stored name post-recompute: a
        // pre-recompute read could carry a stale suffix and miss a real origin.
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
        // Ordered by name so the choice below breaks ties deterministically
        // rather than by allocator order.
        returning.sort_by_key(|ws_id| self.access.workspaces.get(*ws_id).name.clone());

        for &ws_id in &returning {
            let ws = self.access.workspaces.get_mut(ws_id);
            // While parked, `monitor` still points at the rental host. The
            // re-home writes below overwrite it, so capture the old host now.
            let previous_host = ws.monitor;
            // Re-home: flip to Attached and repoint `monitor` off the old host
            // onto the returning origin. Two writes because the enum carries no id.
            ws.attachment = Attachment::Attached;
            ws.monitor = monitor_id;
            self.strategies
                .for_workspace_mut(ws_id)
                .compute_placement(&self.access, ws_id);
            // If the old host was showing this workspace, its active pointer now
            // dangles on a workspace that left. Fall it back to one it still owns.
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

        // A monitor that brought workspaces back needs no default. Minting one
        // regardless would leave two workspaces named "0" after every replug,
        // because the previous default parks and returns alongside the new one,
        // and only one of two same-named workspaces is reachable by name.
        //
        // Among the returning ones the first holding windows wins. The monitor's
        // own active pointer died with it, so no stored answer survives to
        // restore, and showing an empty workspace while the windows sit on a
        // sibling reads as the replug having lost them.
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

    /// Restamps every active monitor's `unique_name`.
    ///
    /// A monitor that shares its `device_name` with others gets a `#N` position
    /// rank, so the label depends on the whole active set and must be re-derived
    /// whenever monitors are added, removed, or moved.
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

    pub(crate) fn remove_monitor(&mut self, monitor_id: MonitorId, primary: MonitorId) {
        // The monitor going away cannot also be the rental host the workspaces
        // park onto. This is the sole guard here: a well-formed caller passes a
        // primary that is a present surviving monitor, so this assert plus that
        // contract guarantees a survivor remains to hold the always-live
        // `monitor` field the parked workspaces rent to. The platform
        // recomputes its primary before removing the old primary, so this holds
        // in practice, but assert it so correctness fails fast rather than
        // resting on that unasserted caller contract.
        assert!(
            monitor_id != primary,
            "removed monitor must not be the rental host primary"
        );

        // If the focused monitor is the one going away, focus follows to the
        // primary before any parking. A monitor is detached essentially only
        // when a laptop is undocked, so the primary built-in display is the one
        // still in front of the user. The primary is a guaranteed surviving
        // present monitor, and its active workspace becomes current because
        // current tracks the focused monitor.
        if self.access.focused_monitor == monitor_id {
            self.access.focused_monitor = primary;
        }

        // Snapshot this monitor's current stored `unique_name` before its
        // delete. An Attached workspace on this monitor has no origin in its
        // variant yet, so the origin it should remember is this monitor's own
        // `unique_name`. Reading the stored field here reflects the set present
        // when this specific monitor is removed: the name recompute last ran
        // when the current set was live (the initial state for the first
        // removal, then the tail recompute of the previous call for each later
        // one), and nothing above mutated any monitor's geometry, so the stored
        // field still holds the pre-delete value.
        let this_origin = self.access.monitors.get(monitor_id).unique_name.clone();
        let ws_on_this: Vec<WorkspaceId> = self
            .access
            .workspaces
            .sorted_ids()
            .into_iter()
            .filter(|&ws_id| self.access.workspaces.get(ws_id).monitor == monitor_id)
            .collect();

        // Park every workspace on this monitor, renting it to the primary so
        // `monitor` stays a live present id. The origin each workspace remembers
        // depends on its current variant. A newly parked Attached workspace
        // uses the snapshot above (this monitor's own pre-delete name). An
        // already Parked workspace was rented here from an earlier-removed
        // monitor, so its true origin is recorded in its variant origin;
        // overwriting it would make replug reattach it to the hosting primary
        // rather than its true origin, so the frozen origin is kept.
        for ws_id in ws_on_this {
            let ws = self.access.workspaces.get_mut(ws_id);
            let origin = match &ws.attachment {
                Attachment::Attached => this_origin.clone(),
                Attachment::Parked { origin } => origin.clone(),
            };
            // Rent to the primary: a monitor is detached for a prolonged period
            // essentially only when undocking a laptop, so the primary is the
            // built-in display the user is actually looking at. Parked windows
            // therefore land on the screen in front of the user.
            ws.monitor = primary;
            ws.attachment = Attachment::Parked { origin };
        }

        // Delete this one monitor. Safe now: no workspace's `monitor` field
        // still points at it.
        self.access.monitors.delete(monitor_id);

        // Restamp `unique_name` across the now-smaller surviving set. This must
        // come after the snapshot so the frozen origin reflects the pre-removal
        // set, and after the delete so the recompute sees only survivors.
        // Removing a same-named sibling flips the survivor's suffix, so every
        // survivor is restamped, not just one. A later `remove_monitor` call
        // reads these fresh names.
        self.recompute_monitor_names();
    }

    pub(crate) fn update_monitor(
        &mut self,
        monitor_id: MonitorId,
        work_area: PixelRect,
        scale: f32,
    ) {
        let monitor = self.access.monitors.get_mut(monitor_id);
        monitor.work_area = work_area;
        monitor.scale = scale;
        // Collect IDs first to avoid borrowing self.access.workspaces while
        // passing &mut self.access to the strategy.
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
        self.recompute_monitor_names();
    }

    #[cfg_attr(
        all(not(target_os = "macos"), not(test)),
        expect(dead_code, reason = "stamped only by the macOS display list")
    )]
    pub(crate) fn set_monitor_cg_display_id(
        &mut self,
        monitor_id: MonitorId,
        cg_display_id: Option<u32>,
    ) {
        self.access.monitors.get_mut(monitor_id).cg_display_id = cg_display_id;
    }

    #[cfg_attr(
        all(not(target_os = "windows"), not(test)),
        expect(dead_code, reason = "stamped only by the Windows monitor reconcile")
    )]
    pub(crate) fn set_monitor_gdi_device(&mut self, monitor_id: MonitorId, gdi_device: String) {
        self.access.monitors.get_mut(monitor_id).gdi_device = Some(gdi_device);
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
