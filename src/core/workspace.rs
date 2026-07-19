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
        let target_ws = self.get_or_create_workspace(target_workspace);
        if let Some(window_id) = self.focused_window(current_ws) {
            self.move_child_to_workspace_with_id(window_id, target_ws);
        } else {
            self.move_focused_across_workspaces(current_ws, target_ws);
        }
    }
}
