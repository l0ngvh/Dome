use crate::core::{
    Child, Dimension, Hub, WindowId,
    node::{DisplayMode, Parent, WorkspaceId},
};

impl Hub {
    pub(super) fn attach_float_to_workspace(&mut self, workspace_id: WorkspaceId, id: WindowId) {
        let window = self.windows.get_mut(id);
        window.parent = Parent::Workspace(workspace_id);
        window.workspace = workspace_id;
        let workspace = self.workspaces.get_mut(workspace_id);
        workspace.float_windows.push(id);
        self.set_workspace_focus(Child::Window(id));
    }

    pub(super) fn attach_split_as_float(&mut self, workspace_id: WorkspaceId, id: WindowId) {
        let window = self.windows.get_mut(id);
        let dim = window.dimension;

        window.mode = DisplayMode::Float;
        window.parent = Parent::Workspace(workspace_id);
        window.workspace = workspace_id;
        let workspace = self.workspaces.get_mut(workspace_id);

        let screen = self.monitors.get(workspace.monitor).dimension;
        window.dimension = Dimension {
            width: dim.width,
            height: dim.height,
            x: screen.x + (screen.width - dim.width) / 2.0,
            y: screen.y + (screen.height - dim.height) / 2.0,
        };
        workspace.float_windows.push(id);
        self.set_workspace_focus(Child::Window(id));
    }

    pub(super) fn detach_float_from_workspace(&mut self, id: WindowId) {
        let window = self.windows.get(id);
        let ws_id = window.workspace;

        let workspace = self.workspaces.get_mut(ws_id);
        workspace.float_windows.retain(|&f| f != id);

        let new_focus = workspace
            .fullscreen_windows
            .last()
            .or(workspace.float_windows.last())
            .map(|&f| Child::Window(f))
            .or_else(|| match workspace.root {
                Some(root) => Some(match root {
                    Child::Window(_) => root,
                    Child::Container(c) => self.containers.get(c).focused,
                }),
                None => None,
            });

        if workspace.focused == Some(Child::Window(id)) {
            workspace.focused = new_focus;
            tracing::debug!(
                %id, %ws_id, ?new_focus, "Detached focused float, focus changed"
            );
        } else {
            tracing::debug!(%id, %ws_id, "Detached unfocused float");
        }
    }
}
