use crate::core::{
    Child, Hub, WindowId,
    node::{DisplayMode, Parent, WorkspaceId},
};

impl Hub {
    pub(crate) fn set_fullscreen(&mut self, window_id: WindowId) {
        let window = self.windows.get(window_id);
        let ws = window.workspace;

        match window.mode {
            DisplayMode::Tiling => {
                self.detach_split_child_from_workspace(Child::Window(window_id));
            }
            DisplayMode::Float => {
                self.detach_float_from_workspace(window_id);
            }
            DisplayMode::Fullscreen => return,
        }

        self.windows.get_mut(window_id).mode = DisplayMode::Fullscreen;
        self.attach_fullscreen_to_workspace(ws, window_id);
        let ws_mut = self.workspaces.get_mut(ws);
        ws_mut.focused = Some(Child::Window(window_id));
        ws_mut.viewport_offset = (0.0, 0.0);
    }

    pub(crate) fn unset_fullscreen(&mut self, window_id: WindowId) {
        let window = self.windows.get(window_id);
        if window.mode != DisplayMode::Fullscreen {
            return;
        }

        let ws = window.workspace;
        self.detach_fullscreen_from_workspace(window_id);

        self.windows.get_mut(window_id).mode = DisplayMode::Tiling;
        self.attach_split_child_to_workspace(Child::Window(window_id), ws);

        if let Some(&top) = self.workspaces.get(ws).fullscreen_windows.last() {
            self.workspaces.get_mut(ws).focused = Some(Child::Window(top));
        }
    }

    pub(crate) fn toggle_fullscreen(&mut self) {
        let current_ws = self.current_workspace();
        let Some(Child::Window(window_id)) = self.workspaces.get(current_ws).focused else {
            return;
        };

        match self.windows.get(window_id).mode {
            DisplayMode::Fullscreen => self.unset_fullscreen(window_id),
            DisplayMode::Tiling | DisplayMode::Float => self.set_fullscreen(window_id),
        }
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
            .or(workspace.float_windows.last())
            .map(|&w| Child::Window(w))
            .or_else(|| match workspace.root {
                Some(root) => Some(match root {
                    Child::Window(_) => root,
                    Child::Container(c) => self.containers.get(c).focused,
                }),
                None => None,
            });
    }
}
