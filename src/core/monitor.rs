use super::allocator::{Node, NodeId};
use super::hub::{Hub, RestrictedAction};
use super::node::{Dimension, Length, Logical, MonitorId, PixelRect, Pixels, WorkspaceId};
use super::workspace::Attachment;
use crate::config::lua::deserializer::{FromLuaValue, LoadContext, as_table};

#[derive(Debug, Clone)]
pub(super) struct Monitor {
    /// Raw name the platform reports, never suffixed.
    pub(super) device_name: String,
    /// Stable name Dome derives for this monitor. No platform handle is stable
    /// across platforms, so Dome uses `device_name` when it is unique among
    /// active monitors. On a collision it appends `#N` by `system_work_area`
    /// position, left to right. The same arrangement always yields the same
    /// names.
    pub(super) unique_name: String,
    /// `CGDirectDisplayID`. Windows has no stable counterpart, so `None` there.
    pub(super) cg_display_id: Option<u32>,
    /// Win32 szDevice (`\\.\DISPLAY1`). `None` on macOS. Restamped every
    /// reconcile because Windows can move it to another display.
    pub(super) gdi_device: Option<String>,
    /// `system_work_area` minus `reserved_area`.
    pub(super) work_area: PixelRect,
    /// The work area the operating system reports, before the reserved area.
    pub(super) system_work_area: PixelRect,
    /// The inset set the config's `reserved_area` function last returned for
    /// this monitor.
    pub(super) reserved_area: ReservedArea,
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

/// Logical pixels to keep clear on each edge of one monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReservedArea {
    pub(crate) top: Pixels<Logical>,
    pub(crate) bottom: Pixels<Logical>,
    pub(crate) left: Pixels<Logical>,
    pub(crate) right: Pixels<Logical>,
}

impl ReservedArea {
    pub(crate) const ZERO: Self = Self {
        top: Pixels::ZERO,
        bottom: Pixels::ZERO,
        left: Pixels::ZERO,
        right: Pixels::ZERO,
    };

    pub(crate) fn subtract_from(self, work_area: PixelRect, scale: f32) -> PixelRect {
        let inset = |px: Pixels<Logical>| Length::from_pixels(px).to_unit(scale);
        let area = work_area.to_dimension();
        let far_x = area.x + area.width;
        let far_y = area.y + area.height;
        // A near edge stops at the far edge, so a collapsed area stays inside the work area.
        let left = (area.x + inset(self.left)).min(far_x);
        let top = (area.y + inset(self.top)).min(far_y);
        let right = far_x - inset(self.right);
        let bottom = far_y - inset(self.bottom);
        PixelRect::from_dimension_inward(Dimension::new(left, top, right - left, bottom - top))
    }
}

impl FromLuaValue for ReservedArea {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a table of insets")?;
        Ok(ReservedArea {
            top: cx.field_or_else(table, "top", || Pixels::ZERO),
            bottom: cx.field_or_else(table, "bottom", || Pixels::ZERO),
            left: cx.field_or_else(table, "left", || Pixels::ZERO),
            right: cx.field_or_else(table, "right", || Pixels::ZERO),
        })
    }
}

/// What a platform reconcile reports for one monitor.
pub(crate) struct ReportedMonitor {
    pub(crate) device_name: String,
    pub(crate) work_area: PixelRect,
    pub(crate) scale: f32,
    pub(crate) cg_display_id: Option<u32>,
    pub(crate) gdi_device: Option<String>,
}

/// Selects a monitor by direction from the focused one, or by its stable unique
/// name. The core counterpart to the wire `MonitorTarget`.
#[derive(Debug, Clone)]
pub(crate) enum MonitorSelector {
    Up,
    Down,
    Left,
    Right,
    Name(String),
}

impl From<&crate::action::MonitorTarget> for MonitorSelector {
    fn from(target: &crate::action::MonitorTarget) -> Self {
        use crate::action::MonitorTarget;

        match target {
            MonitorTarget::Up => Self::Up,
            MonitorTarget::Down => Self::Down,
            MonitorTarget::Left => Self::Left,
            MonitorTarget::Right => Self::Right,
            MonitorTarget::Name(name) => Self::Name(name.clone()),
        }
    }
}

impl Hub {
    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_monitor(&mut self, target: &MonitorSelector) {
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
    pub(crate) fn move_focused_to_monitor(&mut self, target: &MonitorSelector) {
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
        self.move_focused_across_workspaces(current_ws, target_ws);
    }

    pub(crate) fn add_monitor(&mut self, reported: ReportedMonitor) -> MonitorId {
        let monitor_id = self.access.monitors.allocate(Monitor {
            device_name: reported.device_name.clone(),
            unique_name: reported.device_name,
            cg_display_id: reported.cg_display_id,
            gdi_device: reported.gdi_device,
            work_area: reported.work_area,
            system_work_area: reported.work_area,
            reserved_area: ReservedArea::ZERO,
            scale: reported.scale,
            // Placeholder. A workspace needs this monitor's id, so the real
            // active one is chosen at the end.
            active_workspace: WorkspaceId::new(0),
        });
        self.rederive_monitors();

        // Read `unique_name` only after `rederive_monitors`. A suffix from before
        // it would miss a parked workspace's frozen origin.
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

        let host = self.access.monitors.get(monitor_id);
        let (work_area, scale) = (host.work_area, host.scale);
        for &ws_id in &returning {
            let ws = self.access.workspaces.get_mut(ws_id);
            // While parked, `monitor` still points at the rental host. The
            // re-home writes below overwrite it, so capture the old host now.
            let previous_host = ws.monitor;
            ws.attachment = Attachment::Attached;
            ws.monitor = monitor_id;
            self.strategies.for_workspace_mut(ws_id).update_work_area(
                &self.access,
                ws_id,
                work_area,
                scale,
            );
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
            .find(|&&ws_id| self.workspace_has_windows(ws_id))
            .or(returning.first())
            .copied();

        let active = match returning_active {
            Some(ws_id) => ws_id,
            None => self.create_workspace("0".to_string(), monitor_id),
        };
        self.access.monitors.get_mut(monitor_id).active_workspace = active;

        self.create_named_workspaces();

        monitor_id
    }

    /// Re-derives every active monitor's `unique_name`, `reserved_area`, and
    /// `work_area`. Recomputes the workspace placements on each monitor whose
    /// `work_area` moved. Calls the config's `reserved_area` function once per
    /// monitor.
    ///
    /// A `#N` rank depends on the whole active set. Call this on every add,
    /// remove, or update, and after every successful config reload.
    pub(super) fn rederive_monitors(&mut self) {
        let all: Vec<(MonitorId, String, PixelRect)> = self
            .access
            .monitors
            .sorted_ids()
            .into_iter()
            .map(|id| {
                let m = self.access.monitors.get(id);
                (id, m.device_name.clone(), m.system_work_area)
            })
            .collect();
        let names: Vec<(MonitorId, String)> = all
            .iter()
            .map(|(id, device_name, _)| {
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
                (*id, unique)
            })
            .collect();

        let mut moved: Vec<MonitorId> = Vec::new();
        for (id, name) in names {
            let (system_work_area, scale) = {
                let m = self.access.monitors.get(id);
                (m.system_work_area, m.scale)
            };
            let reserved_area = self.runtime.reserved_area(&name);
            let work_area = reserved_area.subtract_from(system_work_area, scale);
            let m = self.access.monitors.get_mut(id);
            if work_area != m.work_area {
                moved.push(id);
            }
            m.unique_name = name;
            m.reserved_area = reserved_area;
            m.work_area = work_area;
        }
        for id in moved {
            self.compute_monitor_placements(id);
        }
    }

    fn compute_monitor_placements(&mut self, monitor_id: MonitorId) {
        let host = self.access.monitors.get(monitor_id);
        let (work_area, scale) = (host.work_area, host.scale);
        let ws_ids: Vec<WorkspaceId> = self
            .access
            .workspaces
            .sorted_ids()
            .into_iter()
            .filter(|&ws_id| self.access.workspaces.get(ws_id).monitor == monitor_id)
            .collect();
        for ws_id in ws_ids {
            self.strategies.for_workspace_mut(ws_id).update_work_area(
                &self.access,
                ws_id,
                work_area,
                scale,
            );
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

        let host = self.access.monitors.get(primary);
        let (work_area, scale) = (host.work_area, host.scale);
        // Every workspace here is Attached. A parked one rents to the primary,
        // which is never removed, so the frozen origin is always this monitor's
        // own name.
        for ws_id in ws_on_this {
            let ws = self.access.workspaces.get_mut(ws_id);
            // A monitor stays detached for long essentially only on a laptop
            // undock, so the primary is the display in front of the user.
            ws.monitor = primary;
            ws.attachment = Attachment::Parked {
                origin: this_origin.clone(),
            };
            self.strategies.for_workspace_mut(ws_id).update_work_area(
                &self.access,
                ws_id,
                work_area,
                scale,
            );
        }

        // Safe to delete now, because no workspace's `monitor` field points at it.
        self.access.monitors.delete(monitor_id);

        // Restamp `unique_name` across the survivors. After the snapshot, so the
        // frozen origin reflects the pre-removal set. After the delete, so the
        // recompute sees only survivors.
        self.rederive_monitors();
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
        let scale_changed = monitor.scale != reported.scale;
        if !scale_changed
            && monitor.system_work_area == reported.work_area
            && monitor.device_name == reported.device_name
            && monitor.cg_display_id == reported.cg_display_id
            && monitor.gdi_device == reported.gdi_device
        {
            return;
        }
        let work_area_before = monitor.work_area;
        monitor.device_name = reported.device_name;
        monitor.system_work_area = reported.work_area;
        monitor.scale = reported.scale;
        monitor.cg_display_id = reported.cg_display_id;
        monitor.gdi_device = reported.gdi_device;

        self.rederive_monitors();

        // A new scale changes every scaled length in the layout, even when the
        // work area stays put.
        if scale_changed && self.access.monitors.get(monitor_id).work_area == work_area_before {
            self.compute_monitor_placements(monitor_id);
        }
    }

    pub(super) fn monitor_id_by_disambiguated_name(&self, name: &str) -> Option<MonitorId> {
        self.access
            .monitors
            .sorted_ids()
            .into_iter()
            .find(|&id| self.access.monitors.get(id).unique_name == *name)
    }

    fn find_monitor_by_target(&self, target: &MonitorSelector) -> Option<MonitorId> {
        match target {
            MonitorSelector::Name(name) => self
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
                            MonitorSelector::Left => dx < Pixels::ZERO,
                            MonitorSelector::Right => dx > Pixels::ZERO,
                            MonitorSelector::Up => dy < Pixels::ZERO,
                            MonitorSelector::Down => dy > Pixels::ZERO,
                            MonitorSelector::Name(_) => false,
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

#[cfg(test)]
mod tests {
    use super::*;

    const WORK_AREA: PixelRect = PixelRect::new(100, 50, 1000, 800);

    fn insets(top: i32, bottom: i32, left: i32, right: i32) -> ReservedArea {
        ReservedArea {
            top: Pixels::new(top),
            bottom: Pixels::new(bottom),
            left: Pixels::new(left),
            right: Pixels::new(right),
        }
    }

    #[test]
    fn zero_leaves_the_work_area_unchanged() {
        assert_eq!(ReservedArea::ZERO.subtract_from(WORK_AREA, 1.0), WORK_AREA);
    }

    #[test]
    fn each_inset_moves_only_its_own_edge() {
        assert_eq!(
            insets(30, 10, 20, 5).subtract_from(WORK_AREA, 1.0),
            PixelRect::new(120, 80, 975, 760)
        );
    }

    #[test]
    fn an_inset_wider_than_the_work_area_collapses_it_inside() {
        assert_eq!(
            insets(900, 0, 1200, 0).subtract_from(WORK_AREA, 1.0),
            PixelRect::new(1100, 850, 0, 0)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_scaled_inset_rounds_inward() {
        assert_eq!(
            insets(25, 25, 0, 0).subtract_from(PixelRect::new(0, 0, 1920, 1080), 1.5),
            PixelRect::new(0, 38, 1920, 1004)
        );
    }
}
