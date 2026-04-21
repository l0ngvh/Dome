use crate::core::{
    Child, Hub, WindowId,
    node::{DisplayMode, Parent, WindowRestrictions, WorkspaceId},
};

impl Hub {
    #[tracing::instrument(skip(self))]
    pub(crate) fn set_fullscreen(&mut self, window_id: WindowId, restrictions: WindowRestrictions) {
        let window = self.windows.get(window_id);
        let ws = window.workspace;

        match window.mode {
            DisplayMode::Tiling => {
                self.detach_split_child_from_workspace(Child::Window(window_id));
            }
            DisplayMode::Float => {
                self.detach_float_from_workspace(window_id);
            }
            DisplayMode::Fullscreen => {
                tracing::debug!(
                    ?window_id,
                    ?restrictions,
                    "Updating restrictions on already-fullscreen window"
                );
                self.windows.get_mut(window_id).restrictions = restrictions;
                return;
            }
        }

        let window = self.windows.get_mut(window_id);
        window.mode = DisplayMode::Fullscreen;
        window.restrictions = restrictions;
        self.attach_fullscreen_to_workspace(ws, window_id);
        self.workspaces.get_mut(ws).is_float_focused = false;
        self.workspaces.get_mut(ws).viewport_offset = (0.0, 0.0);
        tracing::info!(%window_id, "Fullscreen set");
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn unset_fullscreen(&mut self, window_id: WindowId) {
        let window = self.windows.get(window_id);
        if window.mode != DisplayMode::Fullscreen {
            return;
        }

        let ws = window.workspace;
        self.windows.get_mut(window_id).restrictions = WindowRestrictions::None;
        self.detach_fullscreen_from_workspace(window_id);

        self.windows.get_mut(window_id).mode = DisplayMode::Tiling;
        self.attach_split_child_to_workspace(Child::Window(window_id), ws);

        tracing::info!(%window_id, "Fullscreen unset");
    }

    pub(super) fn attach_fullscreen_to_workspace(&mut self, ws: WorkspaceId, id: WindowId) {
        let window = self.windows.get_mut(id);
        window.workspace = ws;
        window.parent = Parent::Workspace(ws);
        self.workspaces.get_mut(ws).fullscreen_windows.push(id);
    }

    pub(super) fn detach_fullscreen_from_workspace(&mut self, id: WindowId) {
        let ws_id = self.windows.get(id).workspace;
        let workspace = self.workspaces.get_mut(ws_id);

        let was_topmost = workspace.fullscreen_windows.last() == Some(&id);
        workspace.fullscreen_windows.retain(|&w| w != id);

        if !was_topmost {
            return;
        }

        // Topmost was removed. If more fullscreen windows remain, focused() picks
        // the new topmost implicitly. Otherwise fall back to float or tiling.
        if !workspace.fullscreen_windows.is_empty() {
            return;
        }
        // Rarely do people focus float, so there is no need to fallback to float
        workspace.focused_tiling = match workspace.root {
            Some(Child::Window(_)) => workspace.root,
            Some(Child::Container(c)) => Some(self.containers.get(c).focused),
            None => None,
        };
    }
}
