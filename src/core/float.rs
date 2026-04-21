use crate::core::{
    Hub, WindowId,
    node::{Dimension, DisplayMode, Parent, WorkspaceId},
};

impl Hub {
    /// Move the given float to the end of float_windows (making it topmost)
    /// and mark float as focused.
    pub(super) fn focus_float(&mut self, ws: WorkspaceId, window_id: WindowId) {
        let workspace = self.workspaces.get_mut(ws);
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

    pub(super) fn attach_float_to_workspace(&mut self, workspace_id: WorkspaceId, id: WindowId) {
        let window = self.windows.get_mut(id);
        window.parent = Parent::Workspace(workspace_id);
        window.workspace = workspace_id;
        let dim = self.windows.get(id).dimension;
        let workspace = self.workspaces.get_mut(workspace_id);
        workspace.float_windows.push((id, dim));
        self.focus_float(workspace_id, id);
    }

    pub(super) fn attach_split_as_float(
        &mut self,
        workspace_id: WorkspaceId,
        id: WindowId,
        dim: Dimension,
    ) {
        let window = self.windows.get_mut(id);
        window.mode = DisplayMode::Float;
        window.parent = Parent::Workspace(workspace_id);
        window.workspace = workspace_id;
        let workspace = self.workspaces.get_mut(workspace_id);
        workspace.float_windows.push((id, dim));
        self.focus_float(workspace_id, id);
    }

    pub(super) fn detach_float_from_workspace(&mut self, id: WindowId) {
        let ws_id = self.windows.get(id).workspace;
        let workspace = self.workspaces.get_mut(ws_id);

        let was_focused = workspace.is_float_focused
            && workspace.float_windows.last().map(|&(fid, _)| fid) == Some(id);
        workspace.float_windows.retain(|&(f, _)| f != id);

        if !was_focused {
            return;
        }

        // Topmost focused float was removed. If more floats remain,
        // is_float_focused stays true and focused() picks the new topmost.
        if workspace.float_windows.is_empty() {
            workspace.is_float_focused = false;
        }
    }
}
