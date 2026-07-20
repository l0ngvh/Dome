use crate::core::{
    Hub, WindowId,
    hub::RestrictedAction,
    node::{Dimension, DisplayMode, MonitorId, WorkspaceId},
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
    }

    pub(super) fn attach_float_to_workspace(
        &mut self,
        workspace_id: WorkspaceId,
        id: WindowId,
        dim: Dimension,
    ) {
        let window = self.access.windows.get_mut(id);
        window.mode = DisplayMode::Float { dim };
        window.set_workspace(Some(workspace_id));
        let workspace = self.access.workspaces.get_mut(workspace_id);
        workspace.float_windows.push(id);
        self.focus_float(workspace_id, id);
    }

    pub(super) fn detach_float_from_workspace(&mut self, id: WindowId) -> Dimension {
        let window = self.access.windows.get(id);
        let DisplayMode::Float { dim } = window.mode else {
            panic!("detach_float_from_workspace: {id} is not Float");
        };
        let ws_id = window
            .workspace()
            .expect("detaching float window must have a workspace");
        let workspace = self.access.workspaces.get_mut(ws_id);

        let was_focused =
            workspace.is_float_focused && workspace.float_windows.last().copied() == Some(id);

        let pos = workspace
            .float_windows
            .iter()
            .position(|&fid| fid == id)
            .expect("detach_float_from_workspace: window not in float_windows");
        workspace.float_windows.remove(pos);

        if was_focused && workspace.float_windows.is_empty() {
            workspace.is_float_focused = false;
        }

        dim
    }

    /// Write back the observed screen-absolute dimension for a floating window.
    /// Called by platform shells after a user drag/resize settles.
    /// Clients must make sure that the dimension and the monitor_id are consistent. If the
    /// `monitor_id` is not what the operating system agreed with, the window will be assigned to a
    /// wrong workspace and toggling workspace on this monitor will hide/show this window, causing
    /// confusion. It's not the end of the world though.
    #[tracing::instrument(skip(self))]
    pub(crate) fn update_float_dimension(
        &mut self,
        window_id: WindowId,
        dim: Dimension,
        monitor_id: MonitorId,
    ) {
        let old_ws = {
            let window = self.access.windows.get_mut(window_id);
            assert!(
                window.is_float(),
                "update_float_dimension: {window_id} is not Float"
            );
            let ws = window
                .workspace()
                .expect("non-minimized float window has a workspace");
            window.mode = DisplayMode::Float { dim };
            ws
        };

        let old_monitor = self.access.workspaces.get(old_ws).monitor;
        tracing::debug!("{old_monitor} {monitor_id} {dim:?}");
        if monitor_id != old_monitor {
            let target_ws = self.access.monitors.get(monitor_id).active_workspace;
            if target_ws != old_ws {
                let stored_dim = self.detach_float_from_workspace(window_id);
                self.attach_float_to_workspace(target_ws, window_id, stored_dim);
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

        match self.access.windows.get(window_id).mode {
            DisplayMode::Fullscreen => (),
            DisplayMode::Float { .. } => {
                let _dim = self.detach_float_from_workspace(window_id);
                self.access.windows.get_mut(window_id).mode = DisplayMode::Tiling;
                self.strategies.for_workspace_mut(current_ws).attach_window(
                    &mut self.access,
                    window_id,
                    current_ws,
                );

                tracing::debug!(%window_id, "Window is now tiling");
            }
            DisplayMode::Tiling => {
                let dim = self
                    .strategies
                    .for_workspace_mut(current_ws)
                    .detach_window(&self.access, window_id);
                self.attach_float_to_workspace(current_ws, window_id, dim);
                tracing::debug!(%window_id, "Window is now floating");
            }
        }
    }
}
