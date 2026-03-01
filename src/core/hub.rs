use crate::action::MonitorTarget;
use crate::config::SizeConstraint;

use super::allocator::{Allocator, NodeId};
use super::node::{
    Child, Container, ContainerId, Dimension, Direction, DisplayMode, Monitor, MonitorId, Parent,
    SpawnMode, Window, WindowId, Workspace, WorkspaceId,
};

#[derive(Clone, Copy)]
pub(crate) struct WindowPlacement {
    pub(crate) id: WindowId,
    pub(crate) frame: Dimension,
    pub(crate) visible_frame: Dimension,
    pub(crate) is_float: bool,
    pub(crate) is_focused: bool,
    pub(crate) spawn_mode: SpawnMode,
}

#[derive(Clone, Copy)]
pub(crate) struct ContainerPlacement {
    pub(crate) id: ContainerId,
    #[cfg_attr(
        all(target_os = "windows", not(test)),
        expect(dead_code, reason = "used on macOS")
    )]
    pub(crate) frame: Dimension,
    pub(crate) visible_frame: Dimension,
    pub(crate) is_focused: bool,
    pub(crate) spawn_mode: SpawnMode,
    pub(crate) is_tabbed: bool,
    #[cfg_attr(
        all(target_os = "windows", not(test)),
        expect(dead_code, reason = "used on macOS")
    )]
    pub(crate) active_tab_index: usize,
}

pub(crate) struct MonitorPlacements {
    pub(crate) monitor_id: MonitorId,
    pub(crate) layout: MonitorLayout,
}

pub(crate) enum MonitorLayout {
    Normal {
        windows: Vec<WindowPlacement>,
        containers: Vec<ContainerPlacement>,
    },
    Fullscreen(WindowId),
}

#[derive(Debug)]
pub(crate) struct Hub {
    pub(super) monitors: Allocator<Monitor>,
    pub(super) focused_monitor: MonitorId,
    pub(super) config: HubConfig,

    pub(super) workspaces: Allocator<Workspace>,
    pub(super) windows: Allocator<Window>,
    pub(super) containers: Allocator<Container>,
}

impl Hub {
    pub(crate) fn new(primary_screen: Dimension, config: HubConfig) -> Self {
        let mut monitors: Allocator<Monitor> = Allocator::new();
        let mut workspaces: Allocator<Workspace> = Allocator::new();

        let primary_id = monitors.allocate(Monitor {
            name: "primary".to_string(),
            dimension: primary_screen,
            active_workspace: WorkspaceId::new(0),
        });

        let ws_id = workspaces.allocate(Workspace::new("0".to_string(), primary_id));
        monitors.get_mut(primary_id).active_workspace = ws_id;

        Self {
            monitors,
            focused_monitor: primary_id,
            config,
            workspaces,
            windows: Allocator::new(),
            containers: Allocator::new(),
        }
    }

    pub(crate) fn focus_workspace(&mut self, name: &str) {
        let ws_id = self.get_or_create_workspace(name);
        self.focus_workspace_with_id(ws_id);
    }

    pub(crate) fn current_workspace(&self) -> WorkspaceId {
        self.monitors.get(self.focused_monitor).active_workspace
    }

    pub(crate) fn set_focus(&mut self, window_id: WindowId) {
        tracing::debug!(%window_id, "Setting focus to window");
        self.set_workspace_focus(Child::Window(window_id));
        let workspace_id = self.windows.get(window_id).workspace;
        self.focus_workspace_with_id(workspace_id);
    }

    pub(crate) fn focused_monitor(&self) -> MonitorId {
        self.focused_monitor
    }

    pub(crate) fn visible_workspaces(&self) -> Vec<WorkspaceId> {
        self.monitors
            .all_active()
            .into_iter()
            .map(|(_, m)| m.active_workspace)
            .collect()
    }

    #[cfg_attr(
        all(target_os = "macos", not(test)),
        expect(dead_code, reason = "used on Windows")
    )]
    pub(crate) fn get_monitor(&self, id: MonitorId) -> &Monitor {
        self.monitors.get(id)
    }

    #[cfg(test)]
    pub(super) fn all_monitors(&self) -> Vec<(MonitorId, Monitor)> {
        self.monitors.all_active()
    }

    pub(crate) fn add_monitor(&mut self, name: String, dimension: Dimension) -> MonitorId {
        let monitor_id = self.monitors.allocate(Monitor {
            name: name.clone(),
            dimension,
            active_workspace: WorkspaceId::new(0),
        });
        let ws_id = self.workspaces.allocate(Workspace::new(name, monitor_id));
        self.monitors.get_mut(monitor_id).active_workspace = ws_id;
        monitor_id
    }

    pub(crate) fn remove_monitor(&mut self, monitor_id: MonitorId, fallback_id: MonitorId) {
        assert!(
            fallback_id != monitor_id,
            "fallback must differ from removed monitor"
        );

        let workspaces_to_migrate: Vec<WorkspaceId> = self
            .workspaces
            .all_active()
            .iter()
            .filter(|(_, ws)| ws.monitor == monitor_id)
            .map(|(id, _)| *id)
            .collect();

        for ws_id in workspaces_to_migrate {
            self.workspaces.get_mut(ws_id).monitor = fallback_id;
            self.adjust_workspace(ws_id);
        }

        if self.focused_monitor == monitor_id {
            self.focused_monitor = fallback_id;
        }
        self.monitors.delete(monitor_id);
    }

    pub(crate) fn update_monitor_dimension(&mut self, monitor_id: MonitorId, dimension: Dimension) {
        self.monitors.get_mut(monitor_id).dimension = dimension;
        for (ws_id, ws) in self.workspaces.all_active() {
            if ws.monitor == monitor_id {
                self.adjust_workspace(ws_id);
            }
        }
    }

    pub(crate) fn focus_monitor(&mut self, target: &MonitorTarget) {
        let Some(target_id) = self.find_monitor_by_target(target) else {
            return;
        };
        if target_id == self.focused_monitor {
            return;
        }
        tracing::debug!(?target, "Focusing monitor");
        self.focused_monitor = target_id;
    }

    pub(crate) fn move_focused_to_monitor(&mut self, target: &MonitorTarget) {
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

    pub(crate) fn sync_config(&mut self, config: HubConfig) {
        self.config = config;
        for (ws_id, _) in self.workspaces.all_active() {
            self.adjust_workspace(ws_id);
        }
    }

    #[cfg(test)]
    pub(super) fn all_workspaces(&self) -> Vec<(WorkspaceId, Workspace)> {
        self.workspaces.all_active()
    }

    #[cfg_attr(
        all(target_os = "macos", not(test)),
        expect(dead_code, reason = "used on Windows")
    )]
    pub(crate) fn get_workspace(&self, id: WorkspaceId) -> &Workspace {
        self.workspaces.get(id)
    }

    pub(crate) fn get_container(&self, id: ContainerId) -> &Container {
        self.containers.get(id)
    }

    pub(crate) fn get_window(&self, id: WindowId) -> &Window {
        self.windows.get(id)
    }

    pub(crate) fn get_visible_placements(&self) -> Vec<MonitorPlacements> {
        let current_ws = self.current_workspace();

        self.visible_workspaces()
            .into_iter()
            .map(|ws_id| {
                let ws = self.workspaces.get(ws_id);
                let screen = self.monitors.get(ws.monitor).dimension;
                let (offset_x, offset_y) = ws.viewport_offset;
                let focused = if ws_id == current_ws {
                    ws.focused
                } else {
                    None
                };

                let mut windows = Vec::new();
                let mut containers = Vec::new();

                // Fullscreen: only return topmost, skip tiling/float
                if let Some(&fs_id) = ws.fullscreen_windows.last() {
                    return MonitorPlacements {
                        monitor_id: ws.monitor,
                        layout: MonitorLayout::Fullscreen(fs_id),
                    };
                }

                let mut stack: Vec<Child> = ws.root.into_iter().collect();
                for _ in super::bounded_loop() {
                    let Some(child) = stack.pop() else { break };
                    match child {
                        Child::Window(id) => {
                            let window = self.windows.get(id);
                            let frame = translate(window.dimension, offset_x, offset_y, screen);
                            if let Some(visible_frame) = clip(frame, screen) {
                                windows.push(WindowPlacement {
                                    id,
                                    frame,
                                    visible_frame,
                                    is_float: false,
                                    is_focused: focused == Some(Child::Window(id)),
                                    spawn_mode: window.spawn_mode(),
                                });
                            }
                        }
                        Child::Container(id) => {
                            let container = self.containers.get(id);
                            let frame = translate(container.dimension, offset_x, offset_y, screen);
                            let Some(visible_frame) = clip(frame, screen) else {
                                continue;
                            };
                            containers.push(ContainerPlacement {
                                id,
                                frame,
                                visible_frame,
                                is_focused: focused == Some(Child::Container(id)),
                                spawn_mode: container.spawn_mode(),
                                is_tabbed: container.is_tabbed(),
                                active_tab_index: container.active_tab_index(),
                            });
                            if let Some(active) = container.active_tab() {
                                stack.push(active);
                            } else {
                                for &c in container.children() {
                                    stack.push(c);
                                }
                            }
                        }
                    }
                }

                for &id in &ws.float_windows {
                    let window = self.windows.get(id);
                    let frame = translate(window.dimension, offset_x, offset_y, screen);
                    if let Some(visible_frame) = clip(frame, screen) {
                        windows.push(WindowPlacement {
                            id,
                            frame,
                            visible_frame,
                            is_float: true,
                            is_focused: focused == Some(Child::Window(id)),
                            spawn_mode: window.spawn_mode(),
                        });
                    }
                }

                MonitorPlacements {
                    monitor_id: ws.monitor,
                    layout: MonitorLayout::Normal {
                        windows,
                        containers,
                    },
                }
            })
            .collect()
    }

    /// Insert a new window as tiling to the current workspace.
    /// Update workspace focus to the newly inserted window.
    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_tiling(&mut self) -> WindowId {
        let current_ws = self.current_workspace();
        let window_id = self.windows.allocate(Window::tiling(current_ws));
        self.attach_split_child_to_workspace(Child::Window(window_id), current_ws);
        window_id
    }

    /// Insert a new window as float to the current workspace.
    /// Update workspace focus to the newly inserted window.
    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_float(&mut self, dimension: Dimension) -> WindowId {
        let current_ws = self.current_workspace();
        let window_id = self.windows.allocate(Window::float(current_ws, dimension));
        tracing::debug!("Inserting float window {window_id} with dimension {dimension:?}");
        self.attach_float_to_workspace(current_ws, window_id);
        window_id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_fullscreen(&mut self) -> WindowId {
        let current_ws = self.current_workspace();
        let window_id = self.windows.allocate(Window::fullscreen(current_ws));
        self.attach_fullscreen_to_workspace(current_ws, window_id);
        window_id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn delete_window(&mut self, id: WindowId) {
        let window = self.windows.get(id);
        let ws = window.workspace;
        match window.mode {
            DisplayMode::Float => self.detach_float_from_workspace(id),
            DisplayMode::Fullscreen => self.detach_fullscreen_from_workspace(id),
            DisplayMode::Tiling => self.detach_split_child_from_workspace(Child::Window(id)),
        }
        self.prune_workspace(ws);
        self.windows.delete(id);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_spawn_mode(&mut self) {
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
        let current_ws = self.current_workspace();
        self.toggle_split_direction(current_ws);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_parent(&mut self) {
        self.focus_split_parent()
    }

    pub(crate) fn focus_next_tab(&mut self) {
        self.focus_tab(true);
    }

    pub(crate) fn focus_prev_tab(&mut self) {
        self.focus_tab(false);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_container_layout(&mut self) {
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
        let current_ws = self.current_workspace();
        let Some(Child::Window(window_id)) = self.workspaces.get(current_ws).focused else {
            return;
        };

        match self.windows.get(window_id).mode {
            DisplayMode::Fullscreen => return,
            DisplayMode::Float => {
                self.detach_float_from_workspace(window_id);
                self.reattach_float_window_as_split(window_id);

                tracing::debug!(%window_id, "Window is now tiling");
            }
            DisplayMode::Tiling => {
                self.detach_split_child_from_workspace(Child::Window(window_id));
                self.attach_split_as_float(current_ws, window_id);
                tracing::debug!(%window_id, "Window is now floating");
            }
        }
    }

    pub(crate) fn focus_left(&mut self) {
        self.focus_in_direction(Direction::Horizontal, false);
    }

    pub(crate) fn focus_right(&mut self) {
        self.focus_in_direction(Direction::Horizontal, true);
    }

    pub(crate) fn focus_up(&mut self) {
        self.focus_in_direction(Direction::Vertical, false);
    }

    pub(crate) fn focus_down(&mut self) {
        self.focus_in_direction(Direction::Vertical, true);
    }

    pub(crate) fn move_left(&mut self) {
        self.move_in_direction(Direction::Horizontal, false);
    }

    pub(crate) fn move_right(&mut self) {
        self.move_in_direction(Direction::Horizontal, true);
    }

    pub(crate) fn move_up(&mut self) {
        self.move_in_direction(Direction::Vertical, false);
    }

    pub(crate) fn move_down(&mut self) {
        self.move_in_direction(Direction::Vertical, true);
    }

    pub(crate) fn move_focused_to_workspace(&mut self, target_workspace: &str) {
        let current_ws = self.current_workspace();
        let Some(focused) = self.workspaces.get(current_ws).focused else {
            return;
        };
        let target_ws = self.get_or_create_workspace(target_workspace);
        self.move_child_to_workspace_with_id(focused, target_ws);
    }

    #[tracing::instrument(skip(self))]
    /// Set size constraints for a window.
    ///
    /// - `None`: don't change existing value
    /// - `Some(0.0)`: clear constraint
    /// - `Some(x)`: set constraint to x
    ///
    /// If setting min above existing max, max is raised to match min.
    pub(crate) fn set_window_constraint(
        &mut self,
        window_id: WindowId,
        min_width: Option<f32>,
        min_height: Option<f32>,
        max_width: Option<f32>,
        max_height: Option<f32>,
    ) {
        let window = self.windows.get_mut(window_id);

        let update = |name: &str,
                      min: &mut f32,
                      max: &mut f32,
                      new_min: Option<f32>,
                      new_max: Option<f32>| {
            if let Some(new_min) = new_min {
                *min = new_min;
                if *max > 0.0 && *max < new_min {
                    tracing::debug!(window_id = %window_id, "{name}: existing max {:.2} < new min {:.2}, raising max", *max, new_min);
                    *max = new_min;
                }
            }
            if let Some(new_max) = new_max {
                *max = if new_max > 0.0 { new_max } else { 0.0 };
                if *max > 0.0 && *min > *max {
                    tracing::debug!(window_id = %window_id, "{name}: existing min {:.2} > new max {:.2}, lowering min", *min, *max);
                    *min = *max;
                }
            }
        };

        update(
            "width",
            &mut window.min_width,
            &mut window.max_width,
            min_width,
            max_width,
        );
        update(
            "height",
            &mut window.min_height,
            &mut window.max_height,
            min_height,
            max_height,
        );

        tracing::debug!(%window_id, ?min_width, ?min_height, ?max_width, ?max_height, "Window constraint set");

        let workspace_id = window.workspace;
        self.adjust_workspace(workspace_id);
    }

    /// Forces child as a parameter to prevent target_ws from being empty after the operation
    fn move_child_to_workspace_with_id(&mut self, child: Child, target_ws: WorkspaceId) {
        let current_ws = self.current_workspace();
        if current_ws == target_ws {
            return;
        }

        match child {
            Child::Window(id) if self.windows.get(id).mode == DisplayMode::Fullscreen => {
                self.detach_fullscreen_from_workspace(id);
                self.attach_fullscreen_to_workspace(target_ws, id);
            }
            Child::Window(id) if self.windows.get(id).is_float() => {
                self.detach_float_from_workspace(id);
                self.attach_float_to_workspace(target_ws, id);
            }
            _ => {
                self.detach_split_child_from_workspace(child);
                self.attach_split_child_to_workspace(child, target_ws);
            }
        }

        tracing::debug!(?child, ?target_ws, "Moved to workspace");
    }

    fn get_or_create_workspace(&mut self, name: &str) -> WorkspaceId {
        match self.workspaces.find(|w| w.name == name) {
            Some(id) => id,
            None => self
                .workspaces
                .allocate(Workspace::new(name.to_string(), self.focused_monitor)),
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

    pub(super) fn get_parent(&self, child: Child) -> Parent {
        match child {
            Child::Window(id) => self.windows.get(id).parent,
            Child::Container(id) => self.containers.get(id).parent,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HubConfig {
    pub(super) tab_bar_height: f32,
    pub(super) auto_tile: bool,
    pub(super) min_width: SizeConstraint,
    pub(super) min_height: SizeConstraint,
    pub(super) max_width: SizeConstraint,
    pub(super) max_height: SizeConstraint,
}

impl From<crate::config::Config> for HubConfig {
    fn from(config: crate::config::Config) -> Self {
        Self {
            tab_bar_height: config.tab_bar_height,
            auto_tile: config.automatic_tiling,
            min_width: config.min_width,
            min_height: config.min_height,
            max_width: config.max_width,
            max_height: config.max_height,
        }
    }
}

fn translate(dim: Dimension, offset_x: f32, offset_y: f32, screen: Dimension) -> Dimension {
    Dimension {
        x: dim.x - offset_x + screen.x,
        y: dim.y - offset_y + screen.y,
        width: dim.width,
        height: dim.height,
    }
}

fn clip(dim: Dimension, bounds: Dimension) -> Option<Dimension> {
    let x1 = dim.x.max(bounds.x);
    let y1 = dim.y.max(bounds.y);
    let x2 = (dim.x + dim.width).min(bounds.x + bounds.width);
    let y2 = (dim.y + dim.height).min(bounds.y + bounds.height);
    if x1 >= x2 || y1 >= y2 {
        return None;
    }
    Some(Dimension {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
    })
}
