use crate::core::{
    Hub, WindowId,
    hub::RestrictedAction,
    node::{Dimension, DisplayMode, WorkspaceId},
};

impl Hub {
    /// Move the given float to the end of float_windows (making it topmost)
    /// and mark float as focused.
    pub(super) fn focus_float(&mut self, ws: WorkspaceId, window_id: WindowId) {
        let workspace = self.access.workspaces.get_mut(ws);
        if let Some(pos) = workspace
            .float_windows
            .iter()
            .position(|&(id, _)| id == window_id)
        {
            let entry = workspace.float_windows.remove(pos);
            workspace.float_windows.push(entry);
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
        // Setting mode is idempotent for callers where the window is already Float.
        window.mode = DisplayMode::Float;
        window.workspace = workspace_id;
        let workspace = self.access.workspaces.get_mut(workspace_id);
        workspace.float_windows.push((id, dim));
        self.focus_float(workspace_id, id);
    }

    pub(super) fn detach_float_from_workspace(&mut self, id: WindowId) -> Dimension {
        let ws_id = self.access.windows.get(id).workspace;
        let workspace = self.access.workspaces.get_mut(ws_id);

        let was_focused = workspace.is_float_focused
            && workspace.float_windows.last().map(|&(fid, _)| fid) == Some(id);

        let pos = workspace
            .float_windows
            .iter()
            .position(|&(fid, _)| fid == id)
            .expect("detach_float_from_workspace: window not in float_windows");
        let (_id, dim) = workspace.float_windows.remove(pos);

        if was_focused {
            // Topmost focused float was removed. If more floats remain,
            // is_float_focused stays true and focused() picks the new topmost.
            if workspace.float_windows.is_empty() {
                workspace.is_float_focused = false;
            }
        }

        dim
    }

    /// Write back the observed screen-absolute dimension for a floating window.
    /// Called by platform shells after a user drag/resize settles. Preserves
    /// z-order and focus -- only the Dimension in float_windows is updated.
    /// Panics if the window is not Float or is missing from float_windows
    /// (those are invariant violations in the caller).
    pub(crate) fn update_float_dimension(&mut self, window_id: WindowId, dim: Dimension) {
        let window = self.access.windows.get(window_id);
        assert!(
            window.is_float(),
            "update_float_dimension: {window_id} is not Float"
        );
        let ws_id = window.workspace;
        let workspace = self.access.workspaces.get_mut(ws_id);
        let entry = workspace
            .float_windows
            .iter_mut()
            .find(|(id, _)| *id == window_id)
            .expect("update_float_dimension: window not in float_windows");
        entry.1 = dim;
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
            DisplayMode::Float => {
                let _dim = self.detach_float_from_workspace(window_id);
                self.access.windows.get_mut(window_id).mode = DisplayMode::Tiling;
                self.strategy
                    .attach_window(&mut self.access, window_id, current_ws);

                tracing::debug!(%window_id, "Window is now tiling");
            }
            DisplayMode::Tiling => {
                let dim = self.strategy.detach_window(&mut self.access, window_id);
                self.attach_float_to_workspace(current_ws, window_id, dim);
                tracing::debug!(%window_id, "Window is now floating");
            }
            DisplayMode::Minimized => (),
        }
    }
}
