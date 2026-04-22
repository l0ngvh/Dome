use crate::core::{
    Hub, WindowId,
    hub::RestrictedAction,
    node::{DisplayMode, WindowRestrictions, WorkspaceId},
};

impl Hub {
    #[tracing::instrument(skip(self))]
    pub(crate) fn set_fullscreen(&mut self, window_id: WindowId, restrictions: WindowRestrictions) {
        let window = self.access.windows.get(window_id);
        let ws = window.workspace;

        match window.mode {
            DisplayMode::Tiling => {
                self.strategy.detach_window(&mut self.access, window_id);
            }
            DisplayMode::Float => {
                let _dim = self.detach_float_from_workspace(window_id);
            }
            DisplayMode::Fullscreen => {
                tracing::debug!(
                    ?window_id,
                    ?restrictions,
                    "Updating restrictions on already-fullscreen window"
                );
                self.access.windows.get_mut(window_id).restrictions = restrictions;
                return;
            }
        }

        let window = self.access.windows.get_mut(window_id);
        window.mode = DisplayMode::Fullscreen;
        window.restrictions = restrictions;
        self.attach_fullscreen_to_workspace(ws, window_id);
        self.access.workspaces.get_mut(ws).is_float_focused = false;
        tracing::info!(%window_id, "Fullscreen set");
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn unset_fullscreen(&mut self, window_id: WindowId) {
        let window = self.access.windows.get(window_id);
        if window.mode != DisplayMode::Fullscreen {
            return;
        }

        let ws = window.workspace;
        self.access.windows.get_mut(window_id).restrictions = WindowRestrictions::None;
        self.detach_fullscreen_from_workspace(window_id);

        self.access.windows.get_mut(window_id).mode = DisplayMode::Tiling;
        self.strategy.attach_window(&mut self.access, window_id, ws);

        tracing::info!(%window_id, "Fullscreen unset");
    }

    pub(super) fn attach_fullscreen_to_workspace(&mut self, ws: WorkspaceId, id: WindowId) {
        let window = self.access.windows.get_mut(id);
        window.workspace = ws;
        self.access
            .workspaces
            .get_mut(ws)
            .fullscreen_windows
            .push(id);
    }

    pub(super) fn detach_fullscreen_from_workspace(&mut self, id: WindowId) {
        let ws_id = self.access.windows.get(id).workspace;
        let workspace = self.access.workspaces.get_mut(ws_id);

        workspace.fullscreen_windows.retain(|&w| w != id);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_fullscreen(&mut self) {
        if self.is_restricted(RestrictedAction::DisplayModeChange) {
            return;
        }
        let current_ws = self.current_workspace();
        let Some(window_id) = self.focused_window(current_ws) else {
            return;
        };

        match self.access.windows.get(window_id).mode {
            DisplayMode::Fullscreen => self.unset_fullscreen(window_id),
            DisplayMode::Tiling | DisplayMode::Float => {
                self.set_fullscreen(window_id, WindowRestrictions::None)
            }
        }
    }
}
