use crate::action::MonitorTarget;
use crate::core::Hub;
use crate::core::node::{
    Child, ContainerId, Direction, DisplayMode, MonitorId, Parent, WindowRestrictions,
};

enum RestrictedAction {
    /// Operations that navigate or rearrange within the current tiling paradigm.
    /// Blocked by: BlockAll.
    TilingNavigation,
    /// Operations that change the window's display mode (float, fullscreen).
    /// Blocked by: BlockAll, ProtectFullscreen.
    DisplayModeChange,
    /// Move the window to a different workspace (same or different monitor).
    /// Blocked by: BlockAll only. ProtectFullscreen does NOT block this — on macOS
    /// and Windows, fullscreen windows (native, borderless) can freely move across workspaces.
    WorkspaceMove,
    /// Move the window to a different monitor's active workspace.
    /// Blocked by: BlockAll, ProtectFullscreen. Fullscreen windows are bound to their
    /// monitor — moving them cross-monitor would break the fullscreen association.
    MonitorMove,
}

impl Hub {
    fn is_restricted(&self, action: RestrictedAction) -> bool {
        let ws = self.workspaces.get(self.current_workspace());
        let Some(Child::Window(id)) = ws.focused else {
            return false;
        };
        let restrictions = self.windows.get(id).restrictions;
        match action {
            RestrictedAction::TilingNavigation => restrictions == WindowRestrictions::BlockAll,
            RestrictedAction::DisplayModeChange => restrictions != WindowRestrictions::None,
            RestrictedAction::WorkspaceMove => restrictions == WindowRestrictions::BlockAll,
            RestrictedAction::MonitorMove => restrictions != WindowRestrictions::None,
        }
    }
}

impl Hub {
    pub(crate) fn focus_workspace(&mut self, name: &str) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let ws_id = self.get_or_create_workspace(name);
        self.focus_workspace_with_id(ws_id);
    }

    pub(crate) fn focus_monitor(&mut self, target: &MonitorTarget) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let Some(target_id) = self.find_monitor_by_target(target) else {
            return;
        };
        if target_id == self.focused_monitor {
            return;
        }
        tracing::debug!(?target, "Focusing monitor");
        self.focused_monitor = target_id;
    }

    pub(crate) fn focus_left(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.focus_in_direction(Direction::Horizontal, false);
    }

    pub(crate) fn focus_right(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.focus_in_direction(Direction::Horizontal, true);
    }

    pub(crate) fn focus_up(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.focus_in_direction(Direction::Vertical, false);
    }

    pub(crate) fn focus_down(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.focus_in_direction(Direction::Vertical, true);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_parent(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.focus_split_parent()
    }

    pub(crate) fn focus_next_tab(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.focus_tab(true);
    }

    pub(crate) fn focus_prev_tab(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.focus_tab(false);
    }

    pub(crate) fn focus_tab_index(&mut self, container_id: ContainerId, index: usize) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let container = self.containers.get_mut(container_id);
        let Some(new_child) = container.set_active_tab_by_index(index) else {
            return;
        };
        let focus_target = match new_child {
            Child::Window(_) => new_child,
            Child::Container(id) => self.containers.get(id).focused,
        };
        self.set_workspace_focus(focus_target);
    }

    pub(crate) fn move_left(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.move_in_direction(Direction::Horizontal, false);
    }

    pub(crate) fn move_right(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.move_in_direction(Direction::Horizontal, true);
    }

    pub(crate) fn move_up(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.move_in_direction(Direction::Vertical, false);
    }

    pub(crate) fn move_down(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        self.move_in_direction(Direction::Vertical, true);
    }

    pub(crate) fn move_focused_to_workspace(&mut self, target_workspace: &str) {
        if self.is_restricted(RestrictedAction::WorkspaceMove) {
            return;
        }
        let current_ws = self.current_workspace();
        let Some(focused) = self.workspaces.get(current_ws).focused else {
            return;
        };
        let target_ws = self.get_or_create_workspace(target_workspace);
        self.move_child_to_workspace_with_id(focused, target_ws);
    }

    pub(crate) fn move_focused_to_monitor(&mut self, target: &MonitorTarget) {
        if self.is_restricted(RestrictedAction::MonitorMove) {
            return;
        }
        let Some(target_id) = self.find_monitor_by_target(target) else {
            return;
        };
        if target_id == self.focused_monitor {
            return;
        }

        let target_ws = self.monitors.get(target_id).active_workspace;
        tracing::debug!(?target, "Moving to monitor");
        let current_ws = self.current_workspace();
        let Some(focused) = self.workspaces.get(current_ws).focused else {
            return;
        };
        self.move_child_to_workspace_with_id(focused, target_ws);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_spawn_mode(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let current_ws = self.current_workspace();
        let Some(focused) = self.workspaces.get(current_ws).focused else {
            return;
        };

        let (current_mode, is_float) = match focused {
            Child::Container(id) => (self.containers.get(id).spawn_mode(), false),
            Child::Window(id) => {
                let w = self.windows.get(id);
                (w.spawn_mode(), w.is_float())
            }
        };
        if is_float {
            return;
        }
        let new_mode = current_mode.toggle();

        match focused {
            Child::Container(id) => self.containers.get_mut(id).switch_spawn_mode(new_mode),
            Child::Window(id) => self.windows.get_mut(id).switch_spawn_mode(new_mode),
        }
        tracing::debug!(?focused, ?new_mode, "Toggled spawn mode");
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_direction(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let current_ws = self.current_workspace();
        self.toggle_split_direction(current_ws);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_container_layout(&mut self) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let current_ws = self.current_workspace();
        let Some(focused) = self.workspaces.get(current_ws).focused else {
            return;
        };
        let container_id = match focused {
            Child::Container(id) => id,
            Child::Window(id) => {
                if self.windows.get(id).is_float() {
                    return;
                }
                match self.get_parent(Child::Window(id)) {
                    Parent::Container(cid) => cid,
                    Parent::Workspace(_) => return,
                }
            }
        };
        self.toggle_layout_for_container_with_id(container_id);
    }

    /// Toggle the focused window between tiling and floating mode.
    /// Does nothing if no window is focused or a container is focused.
    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_float(&mut self) {
        if self.is_restricted(RestrictedAction::DisplayModeChange) {
            return;
        }
        let current_ws = self.current_workspace();
        let Some(Child::Window(window_id)) = self.workspaces.get(current_ws).focused else {
            return;
        };

        match self.windows.get(window_id).mode {
            DisplayMode::Fullscreen => (),
            DisplayMode::Float => {
                self.detach_float_from_workspace(window_id);
                self.reattach_float_window_as_split(window_id);

                tracing::debug!(%window_id, "Window is now tiling");
            }
            DisplayMode::Tiling => {
                let dim = self.detach_split_child_from_workspace(Child::Window(window_id));
                self.windows.get_mut(window_id).dimension = dim;
                self.attach_split_as_float(current_ws, window_id, dim);
                tracing::debug!(%window_id, "Window is now floating");
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_fullscreen(&mut self) {
        if self.is_restricted(RestrictedAction::DisplayModeChange) {
            return;
        }
        let current_ws = self.current_workspace();
        let Some(Child::Window(window_id)) = self.workspaces.get(current_ws).focused else {
            return;
        };

        match self.windows.get(window_id).mode {
            DisplayMode::Fullscreen => self.unset_fullscreen(window_id),
            DisplayMode::Tiling | DisplayMode::Float => {
                self.set_fullscreen(window_id, WindowRestrictions::None)
            }
        }
    }

    fn find_monitor_by_target(&self, target: &MonitorTarget) -> Option<MonitorId> {
        match target {
            MonitorTarget::Name(name) => self
                .monitors
                .all_active()
                .iter()
                .find(|(_, m)| m.name == *name)
                .map(|(id, _)| *id),
            direction => {
                let current = self.monitors.get(self.focused_monitor);
                let cx = current.dimension.x + current.dimension.width / 2.0;
                let cy = current.dimension.y + current.dimension.height / 2.0;

                self.monitors
                    .all_active()
                    .iter()
                    .filter(|(id, _)| *id != self.focused_monitor)
                    .filter_map(|(id, m)| {
                        let mx = m.dimension.x + m.dimension.width / 2.0;
                        let my = m.dimension.y + m.dimension.height / 2.0;
                        let dx = mx - cx;
                        let dy = my - cy;

                        let valid = match direction {
                            MonitorTarget::Left => dx < 0.0,
                            MonitorTarget::Right => dx > 0.0,
                            MonitorTarget::Up => dy < 0.0,
                            MonitorTarget::Down => dy > 0.0,
                            MonitorTarget::Name(_) => false,
                        };
                        valid.then_some((*id, dx * dx + dy * dy))
                    })
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .map(|(id, _)| id)
            }
        }
    }
}
