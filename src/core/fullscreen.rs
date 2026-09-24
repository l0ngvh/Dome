use crate::core::{
    Hub, WindowId,
    hub::RestrictedAction,
    node::{Child, DisplayMode, PriorMode, WindowHost, WindowRestrictions, WorkspaceId},
};

impl Hub {
    #[tracing::instrument(skip(self))]
    pub(crate) fn set_fullscreen(&mut self, window_id: WindowId, restrictions: WindowRestrictions) {
        let window = self.access.windows.get(window_id);
        let (ws, prior_mode) = match window.host {
            WindowHost::Minimized { .. } => panic!("non-minimized window has a workspace"),
            WindowHost::Monitor(monitor_id) => {
                let ws = self.access.monitors.get(monitor_id).active_workspace;
                let border_box = self.detach_sticky_from_monitor(window_id);
                (
                    ws,
                    PriorMode::Float {
                        border_box,
                        sticky: true,
                    },
                )
            }
            WindowHost::Workspace(ws) => match window.mode {
                DisplayMode::Tiling => {
                    self.strategies
                        .for_workspace_mut(ws)
                        .detach_window(&mut self.access, window_id);
                    (ws, PriorMode::Tiling)
                }
                DisplayMode::Float { .. } => (
                    ws,
                    PriorMode::Float {
                        border_box: self.detach_float_from_workspace(window_id),
                        sticky: false,
                    },
                ),
                DisplayMode::Fullscreen { .. } => {
                    tracing::debug!("Updating restrictions on already-fullscreen window");
                    self.access.windows.get_mut(window_id).restrictions = restrictions;
                    return;
                }
            },
        };

        let window = self.access.windows.get_mut(window_id);
        window.restrictions = restrictions;
        self.attach_fullscreen_to_workspace(ws, window_id, None, prior_mode);
        self.access.workspaces.get_mut(ws).is_float_focused = false;
        tracing::info!("Fullscreen set");
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn unset_fullscreen(&mut self, window_id: WindowId) {
        let window = self.access.windows.get(window_id);
        let DisplayMode::Fullscreen { prior_mode } = window.mode else {
            return;
        };

        let ws = match window.host {
            WindowHost::Minimized { .. } => {
                panic!("non-minimized fullscreen window has a workspace")
            }
            WindowHost::Monitor(_) => panic!("a sticky window is never fullscreen"),
            WindowHost::Workspace(ws) => ws,
        };
        self.access.windows.get_mut(window_id).restrictions = WindowRestrictions::None;
        self.detach_fullscreen_from_workspace(window_id);

        match prior_mode {
            PriorMode::Tiling => {
                self.access.windows.get_mut(window_id).mode = DisplayMode::Tiling;
                self.strategies.for_workspace_mut(ws).reattach_child(
                    &mut self.access,
                    Child::Window(window_id),
                    ws,
                );
            }
            PriorMode::Float { border_box, sticky } => {
                let monitor_id = self.access.workspaces.get(ws).monitor;
                let border_box = self.border_box_on_monitor(border_box, monitor_id);
                if sticky {
                    self.attach_sticky_to_monitor(monitor_id, window_id, border_box);
                } else {
                    self.attach_float_to_workspace(ws, window_id, border_box, None);
                }
            }
        }
        self.set_workspace_focus(window_id);

        tracing::info!("Fullscreen unset");
    }

    pub(super) fn attach_fullscreen_to_workspace(
        &mut self,
        ws: WorkspaceId,
        id: WindowId,
        entry_index: Option<usize>,
        prior_mode: PriorMode,
    ) {
        let window = self.access.windows.get_mut(id);
        window.mode = DisplayMode::Fullscreen { prior_mode };
        window.set_host(WindowHost::Workspace(ws));
        let workspace = self.access.workspaces.get_mut(ws);
        workspace.fullscreen_windows.push(id);
        if let Some(idx) = entry_index {
            workspace.fullscreen_entries[idx].window = Some(id);
        }
    }

    pub(super) fn detach_fullscreen_from_workspace(&mut self, id: WindowId) {
        let ws_id = match self.access.windows.get(id).host {
            WindowHost::Minimized { .. } => {
                panic!("detaching fullscreen window must have a workspace")
            }
            WindowHost::Monitor(_) => panic!("a sticky window is never fullscreen"),
            WindowHost::Workspace(ws) => ws,
        };
        let workspace = self.access.workspaces.get_mut(ws_id);

        workspace.fullscreen_windows.retain(|&w| w != id);
        for entry in workspace.fullscreen_entries.iter_mut() {
            if entry.window == Some(id) {
                entry.window = None;
            }
        }
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
            DisplayMode::Fullscreen { .. } => self.unset_fullscreen(window_id),
            DisplayMode::Tiling | DisplayMode::Float { .. } => {
                self.set_fullscreen(window_id, WindowRestrictions::None)
            }
        }
    }
}
