use crate::core::{Hub, hub::RestrictedAction, node::WorkspaceId};

impl Hub {
    pub(super) fn focus_workspace_with_id(&mut self, workspace_id: WorkspaceId) {
        tracing::debug!("Focusing workspace {workspace_id}");
        let current_ws = self.current_workspace();
        if workspace_id == current_ws {
            return;
        }
        let monitor_id = self.access.workspaces.get(workspace_id).monitor;
        self.access.focused_monitor = monitor_id;
        self.access.monitors.get_mut(monitor_id).active_workspace = workspace_id;
        self.prune_workspace(current_ws);
    }

    /// Deletes workspace if empty and not active on its monitor
    pub(super) fn prune_workspace(&mut self, ws_id: WorkspaceId) {
        let ws = self.access.workspaces.get(ws_id);
        if self.strategy.has_tiling_windows(&self.access, ws_id)
            || !ws.float_windows.is_empty()
            || !ws.fullscreen_windows.is_empty()
        {
            return;
        }
        if self.access.monitors.get(ws.monitor).active_workspace != ws_id {
            self.strategy.prune_workspace(ws_id);
            self.access.workspaces.delete(ws_id);
        }
    }

    pub(crate) fn focus_workspace(&mut self, name: &str) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let ws_id = self.get_or_create_workspace(name);
        self.focus_workspace_with_id(ws_id);
    }

    pub(crate) fn move_focused_to_workspace(&mut self, target_workspace: &str) {
        if self.is_restricted(RestrictedAction::WorkspaceMove) {
            return;
        }
        let current_ws = self.current_workspace();
        if let Some(window_id) = self.focused_window(current_ws) {
            let target_ws = self.get_or_create_workspace(target_workspace);
            self.move_child_to_workspace_with_id(window_id, target_ws);
        } else if self.strategy.has_tiling_windows(&self.access, current_ws) {
            // Container is highlighted (focused_tiling is Child::Container).
            // Bypass focused_window() which returns None for containers,
            // and call the strategy directly to move the whole container.
            let target_ws = self.get_or_create_workspace(target_workspace);
            if current_ws == target_ws {
                return;
            }
            tracing::debug!(?current_ws, ?target_ws, "Moving container to workspace");
            self.strategy
                .move_focused_to_workspace(&mut self.access, current_ws, target_ws);
        }
    }
}
