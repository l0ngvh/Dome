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
        self.workspaces.get_mut(ws).focused = Some(Child::Window(window_id));
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

        if let Some(&top) = self.workspaces.get(ws).fullscreen_windows.last() {
            self.workspaces.get_mut(ws).focused = Some(Child::Window(top));
        }
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
        workspace.fullscreen_windows.retain(|&w| w != id);

        if workspace.focused != Some(Child::Window(id)) {
            return;
        }

        workspace.focused = workspace
            .fullscreen_windows
            .last()
            .copied()
            .or(workspace.float_windows.last().map(|&(id, _)| id))
            .map(Child::Window)
            .or_else(|| match workspace.root {
                Some(root) => Some(match root {
                    Child::Window(_) => root,
                    Child::Container(c) => self.containers.get(c).focused,
                }),
                None => None,
            });
    }
}
