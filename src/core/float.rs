use crate::core::{
    Hub, WindowId,
    hub::RestrictedAction,
    node::{Child, DisplayMode, MonitorId, PixelRect, WindowHost, WorkspaceId},
};

impl Hub {
    /// Move the given float to the end of float_windows (making it topmost)
    /// and mark float as focused.
    pub(super) fn focus_float(&mut self, ws: WorkspaceId, window_id: WindowId) {
        let workspace = self.access.workspaces.get_mut(ws);
        if let Some(pos) = workspace
            .float_windows
            .iter()
            .position(|&id| id == window_id)
        {
            workspace.float_windows.remove(pos);
            workspace.float_windows.push(window_id);
        }
        workspace.is_float_focused = true;
        let monitor_id = workspace.monitor;
        self.access.monitors.get_mut(monitor_id).is_sticky_focused = false;
    }

    pub(super) fn attach_float_to_workspace(
        &mut self,
        workspace_id: WorkspaceId,
        id: WindowId,
        border_box: PixelRect,
        entry_index: Option<usize>,
    ) {
        let window = self.access.windows.get_mut(id);
        window.mode = DisplayMode::Float { border_box };
        window.set_host(WindowHost::Workspace(workspace_id));
        let workspace = self.access.workspaces.get_mut(workspace_id);
        workspace.float_windows.push(id);
        if let Some(idx) = entry_index {
            workspace.float_entries[idx].window = Some(id);
        }
        self.focus_float(workspace_id, id);
    }

    pub(super) fn detach_float_from_workspace(&mut self, id: WindowId) -> PixelRect {
        let window = self.access.windows.get(id);
        let DisplayMode::Float { border_box, .. } = window.mode else {
            panic!("detach_float_from_workspace: {id} is not Float");
        };
        let ws_id = match window.host {
            WindowHost::Minimized { .. } => panic!("detaching float window must have a workspace"),
            WindowHost::Monitor(_) => panic!("detach_float_from_workspace: {id} is sticky"),
            WindowHost::Workspace(ws) => ws,
        };
        let workspace = self.access.workspaces.get_mut(ws_id);

        let was_focused =
            workspace.is_float_focused && workspace.float_windows.last().copied() == Some(id);

        let pos = workspace
            .float_windows
            .iter()
            .position(|&fid| fid == id)
            .expect("detach_float_from_workspace: window not in float_windows");
        workspace.float_windows.remove(pos);
        for entry in workspace.float_entries.iter_mut() {
            if entry.window == Some(id) {
                entry.window = None;
            }
        }

        if was_focused && workspace.float_windows.is_empty() {
            workspace.is_float_focused = false;
        }

        border_box
    }

    /// Places `border_box` on `target` at the same size. A centre inside another
    /// monitor's work area keeps its relative position in the work area. A centre on no
    /// monitor keeps the rect when the rect overlaps `target`, and centres it otherwise.
    pub(super) fn border_box_on_monitor(
        &self,
        border_box: PixelRect,
        target: MonitorId,
    ) -> PixelRect {
        // Doubled coordinates, so an odd extent does not lose half a unit to division.
        let doubled_centre = |rect: PixelRect| {
            (
                2 * i64::from(rect.x().value()) + i64::from(rect.width().value()),
                2 * i64::from(rect.y().value()) + i64::from(rect.height().value()),
            )
        };
        let (cx2, cy2) = doubled_centre(border_box);
        let holds_centre = |area: PixelRect| {
            let left = 2 * i64::from(area.x().value());
            let top = 2 * i64::from(area.y().value());
            let right = 2 * i64::from(area.right().value());
            let bottom = 2 * i64::from(area.bottom().value());
            (left..right).contains(&cx2) && (top..bottom).contains(&cy2)
        };

        let target_area = self.access.monitors.get(target).work_area;
        if holds_centre(target_area) {
            return border_box;
        }

        let width = i64::from(border_box.width().value());
        let height = i64::from(border_box.height().value());
        let source_area = self
            .access
            .monitors
            .sorted_ids()
            .into_iter()
            .filter(|&id| id != target)
            .map(|id| self.access.monitors.get(id).work_area)
            .find(|&area| holds_centre(area));

        let (new_cx2, new_cy2) = match source_area {
            Some(source) => {
                let scale = |offset2: i64, target_extent: i32, source_extent: i32| {
                    (offset2 * i64::from(target_extent)).div_euclid(i64::from(source_extent))
                };
                (
                    2 * i64::from(target_area.x().value())
                        + scale(
                            cx2 - 2 * i64::from(source.x().value()),
                            target_area.width().value(),
                            source.width().value(),
                        ),
                    2 * i64::from(target_area.y().value())
                        + scale(
                            cy2 - 2 * i64::from(source.y().value()),
                            target_area.height().value(),
                            source.height().value(),
                        ),
                )
            }
            None if border_box.clip(target_area).is_some() => return border_box,
            None => doubled_centre(target_area),
        };

        let to_i32 = |v: i64| i32::try_from(v).expect("mapped rect fits the pixel grid");
        PixelRect::new(
            to_i32((new_cx2 - width).div_euclid(2)),
            to_i32((new_cy2 - height).div_euclid(2)),
            border_box.width().value(),
            border_box.height().value(),
        )
    }

    /// Write back the observed screen-absolute content box for a floating window,
    /// storing it as a border box.
    /// Called by platform shells after a user drag/resize settles.
    /// Clients must make sure that the content box and the monitor_id are consistent. If the
    /// `monitor_id` is not what the operating system agreed with, the window will be assigned to a
    /// wrong workspace and toggling workspace on this monitor will hide/show this window, causing
    /// confusion. It's not the end of the world though.
    #[tracing::instrument(skip(self))]
    pub(crate) fn update_float_rect(
        &mut self,
        window_id: WindowId,
        content_box: PixelRect,
        monitor_id: MonitorId,
    ) {
        let border = self.access.border(monitor_id);
        let border_box = content_box.outset_by(border);
        let old_host = {
            let window = self.access.windows.get_mut(window_id);
            assert!(
                window.is_float(),
                "update_float_rect: {window_id} is not Float"
            );
            let host = window.host;
            window.mode = DisplayMode::Float { border_box };
            host
        };
        let old_ws = match old_host {
            WindowHost::Minimized { .. } => panic!("non-minimized float window has a workspace"),
            WindowHost::Workspace(ws) => ws,
            WindowHost::Monitor(old_monitor) => {
                tracing::debug!(%old_monitor, %monitor_id, ?border_box, "Sticky rect updated");
                if monitor_id != old_monitor {
                    let stored_dim = self.detach_sticky_from_monitor(window_id);
                    self.attach_sticky_to_monitor(monitor_id, window_id, stored_dim);
                }
                return;
            }
        };

        let old_monitor = self.access.workspaces.get(old_ws).monitor;
        tracing::debug!(%old_monitor, %monitor_id, ?border_box, "Float rect updated");
        if monitor_id != old_monitor {
            let target_ws = self.access.monitors.get(monitor_id).active_workspace;
            if target_ws != old_ws {
                let stored_dim = self.detach_float_from_workspace(window_id);
                self.attach_float_to_workspace(target_ws, window_id, stored_dim, None);
            }
        }
    }

    /// Toggle the focused window between tiling and floating mode.
    /// Does nothing if no window is focused or a container is focused.
    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_float(&mut self) {
        if self.is_restricted(RestrictedAction::DisplayModeChange) {
            return;
        }
        let current_ws = self.current_workspace();
        let Some(window_id) = self.focused_window(current_ws) else {
            return;
        };

        let window = self.access.windows.get(window_id);
        let (host, mode) = (window.host, window.mode);
        let tiling_ws = match host {
            WindowHost::Monitor(monitor_id) => {
                self.detach_sticky_from_monitor(window_id);
                self.access.monitors.get(monitor_id).active_workspace
            }
            WindowHost::Minimized { .. } | WindowHost::Workspace(_) => match mode {
                DisplayMode::Fullscreen { .. } => return,
                DisplayMode::Float { .. } => {
                    self.detach_float_from_workspace(window_id);
                    current_ws
                }
                DisplayMode::Tiling => {
                    let border_box = self
                        .strategies
                        .for_workspace_mut(current_ws)
                        .detach_window(&mut self.access, window_id);
                    self.attach_float_to_workspace(current_ws, window_id, border_box, None);
                    tracing::debug!(%window_id, "Window is now floating");
                    return;
                }
            },
        };

        self.access.windows.get_mut(window_id).mode = DisplayMode::Tiling;
        self.strategies.for_workspace_mut(tiling_ws).reattach_child(
            &mut self.access,
            Child::Window(window_id),
            tiling_ws,
        );
        self.set_workspace_focus(window_id);
        tracing::debug!(%window_id, "Window is now tiling");
    }
}
