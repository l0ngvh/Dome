use crate::core::{Hub, hub::RestrictedAction, node::WorkspaceId};

impl Hub {
    #[tracing::instrument(skip(self))]
    pub(super) fn focus_workspace_with_id(&mut self, workspace_id: WorkspaceId) {
        tracing::debug!("Focusing workspace");
        let current_ws = self.current_workspace();
        if workspace_id == current_ws {
            return;
        }
        let monitor_id = self.access.workspaces.get(workspace_id).monitor;
        self.access.focused_monitor = monitor_id;
        self.access.monitors.get_mut(monitor_id).active_workspace = workspace_id;
        self.prune_workspace(current_ws);
    }

    /// Deletes workspace if empty, not active on its monitor, and not pinned
    /// by a config-named override.
    #[tracing::instrument(skip(self))]
    pub(super) fn prune_workspace(&mut self, ws_id: WorkspaceId) {
        if self.strategies.is_pinned(ws_id) {
            return;
        }
        let ws = self.access.workspaces.get(ws_id);
        let has_tiling = self
            .strategies
            .for_workspace(ws_id)
            .has_tiling_windows(&self.access, ws_id);
        if has_tiling || !ws.float_windows.is_empty() || !ws.fullscreen_windows.is_empty() {
            return;
        }
        if self.access.monitors.get(ws.monitor).active_workspace != ws_id {
            self.strategies
                .for_workspace_mut(ws_id)
                .prune_workspace(ws_id);
            self.strategies.unregister(ws_id);
            self.access.workspaces.delete(ws_id);
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_workspace(&mut self, name: &str) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let ws_id = self.get_or_create_workspace(name);
        self.focus_workspace_with_id(ws_id);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn move_focused_to_workspace(&mut self, target_workspace: &str) {
        if self.is_restricted(RestrictedAction::WorkspaceMove) {
            return;
        }
        let current_ws = self.current_workspace();
        if let Some(window_id) = self.focused_window(current_ws) {
            let target_ws = self.get_or_create_workspace(target_workspace);
            self.move_child_to_workspace_with_id(window_id, target_ws);
        } else {
            let has_tiling = self
                .strategies
                .for_workspace(current_ws)
                .has_tiling_windows(&self.access, current_ws);
            if has_tiling {
                let target_ws = self.get_or_create_workspace(target_workspace);
                if current_ws == target_ws {
                    return;
                }
                tracing::debug!(?current_ws, ?target_ws, "Moving container to workspace");
                self.move_focused_across_workspaces(current_ws, target_ws);
            }
        }
    }
}
