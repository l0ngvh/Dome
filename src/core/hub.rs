use crate::config::SizeConstraint;

use super::allocator::{Allocator, NodeId};
use super::node::{
    Child, Container, ContainerId, Dimension, Direction, FloatWindow, FloatWindowId, Focus,
    Monitor, MonitorId, Parent, SpawnMode, Window, WindowId, Workspace, WorkspaceId,
};

#[derive(Debug)]
pub(crate) struct Hub {
    monitors: Allocator<Monitor>,
    focused_monitor: MonitorId,
    config: HubConfig,

    workspaces: Allocator<Workspace>,
    windows: Allocator<Window>,
    float_windows: Allocator<FloatWindow>,
    containers: Allocator<Container>,
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
            float_windows: Allocator::new(),
            containers: Allocator::new(),
        }
    }

    pub(crate) fn focus_workspace(&mut self, name: &str) {
        let current_ws = self.current_workspace();

        if let Some(ws_id) = self.workspaces.find(|w| w.name == name) {
            if ws_id == current_ws {
                return;
            }
            let monitor_id = self.workspaces.get(ws_id).monitor;
            self.focused_monitor = monitor_id;
            self.monitors.get_mut(monitor_id).active_workspace = ws_id;
            tracing::debug!(name, %ws_id, "Focusing existing workspace");
            return;
        }

        let ws_id = self
            .workspaces
            .allocate(Workspace::new(name.to_string(), self.focused_monitor));
        self.monitors.get_mut(self.focused_monitor).active_workspace = ws_id;
        tracing::debug!(name, %ws_id, "Created and focused new workspace");
    }

    pub(crate) fn current_workspace(&self) -> WorkspaceId {
        self.monitors.get(self.focused_monitor).active_workspace
    }

    pub(crate) fn set_focus(&mut self, window_id: WindowId) {
        let workspace_id = self.windows.get(window_id).workspace;
        let monitor_id = self.workspaces.get(workspace_id).monitor;
        tracing::debug!(%window_id, %workspace_id, "Setting focus to window");
        self.focused_monitor = monitor_id;
        self.monitors.get_mut(monitor_id).active_workspace = workspace_id;
        self.focus_child(Child::Window(window_id));
    }

    pub(crate) fn set_float_focus(&mut self, float_id: FloatWindowId) {
        let workspace_id = self.float_windows.get(float_id).workspace;
        let monitor_id = self.workspaces.get(workspace_id).monitor;
        tracing::debug!(%float_id, %workspace_id, "Setting focus to float");
        self.focused_monitor = monitor_id;
        self.monitors.get_mut(monitor_id).active_workspace = workspace_id;
        self.workspaces.get_mut(workspace_id).focused = Some(Focus::Float(float_id));
    }

    pub(crate) fn screen(&self) -> Dimension {
        self.monitors.get(self.focused_monitor).dimension
    }

    fn workspace_screen(&self, workspace_id: WorkspaceId) -> Dimension {
        let monitor_id = self.workspaces.get(workspace_id).monitor;
        self.monitors.get(monitor_id).dimension
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

    pub(crate) fn get_workspace(&self, id: WorkspaceId) -> &Workspace {
        self.workspaces.get(id)
    }

    pub(crate) fn get_container(&self, id: ContainerId) -> &Container {
        self.containers.get(id)
    }

    pub(crate) fn get_window(&self, id: WindowId) -> &Window {
        self.windows.get(id)
    }

    pub(crate) fn get_float(&self, id: FloatWindowId) -> &FloatWindow {
        self.float_windows.get(id)
    }

    /// Insert a new window as tiling to the current workspace.
    /// Update workspace focus to the newly inserted window.
    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_tiling(&mut self) -> WindowId {
        let current_ws = self.current_workspace();
        let window_id = self.windows.allocate(Window::new(
            Parent::Workspace(current_ws),
            current_ws,
            SpawnMode::default(),
        ));
        self.attach_child_to_workspace(Child::Window(window_id), current_ws);
        window_id
    }

    /// Insert a new window as float to the current workspace.
    /// Update workspace focus to the newly inserted window.
    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_float(&mut self, dimension: Dimension) -> FloatWindowId {
        let current_ws = self.current_workspace();
        let float_id = self
            .float_windows
            .allocate(FloatWindow::new(current_ws, dimension));
        tracing::debug!("Inserting float window {float_id} with dimension {dimension:?}");
        self.attach_float_to_workspace(current_ws, float_id);
        float_id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn delete_float(&mut self, id: FloatWindowId) {
        self.detach_float_from_workspace(id);
        self.float_windows.delete(id);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn delete_window(&mut self, id: WindowId) {
        self.detach_child_from_workspace(Child::Window(id));
        self.windows.delete(id);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_spawn_mode(&mut self) {
        let current_ws = self.current_workspace();
        let Some(Focus::Tiling(focused)) = self.workspaces.get(current_ws).focused else {
            return;
        };

        let current_mode = match focused {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => self.windows.get(id).spawn_mode(),
        };
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
        let Some(Focus::Tiling(child)) = self.workspaces.get(current_ws).focused else {
            return;
        };
        let mut root_id = match child {
            Child::Container(id) => id,
            Child::Window(_) => {
                let Parent::Container(id) = self.get_parent(child) else {
                    return;
                };
                id
            }
        };
        for _ in super::bounded_loop() {
            let Parent::Container(parent_id) = self.containers.get(root_id).parent else {
                break;
            };
            if self.containers.get(parent_id).is_tabbed {
                break;
            }
            root_id = parent_id;
        }
        self.containers.get_mut(root_id).toggle_direction();
        self.maintain_direction_invariance(Parent::Container(root_id));
        self.adjust_workspace(current_ws);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_parent(&mut self) {
        let current_ws = self.current_workspace();
        let Some(Focus::Tiling(child)) = self.workspaces.get(current_ws).focused else {
            return;
        };
        let Parent::Container(container_id) = self.get_parent(child) else {
            tracing::debug!("Cannot focus parent of workspace root, ignoring");
            return;
        };
        tracing::debug!(parent = %container_id, %child, "Focusing parent");
        self.focus_child(Child::Container(container_id));
    }

    pub(crate) fn focus_next_tab(&mut self) {
        self.focus_tab(true);
    }

    pub(crate) fn focus_prev_tab(&mut self) {
        self.focus_tab(false);
    }

    pub(crate) fn toggle_container_layout(&mut self) {
        let current_ws = self.current_workspace();
        let Some(Focus::Tiling(focused)) = self.workspaces.get(current_ws).focused else {
            return;
        };
        let container_id = match focused {
            Child::Container(id) => id,
            Child::Window(_) => match self.get_parent(focused) {
                Parent::Container(cid) => cid,
                Parent::Workspace(_) => return,
            },
        };
        let container = self.containers.get_mut(container_id);
        let direction = container.direction();
        let parent = container.parent;
        container.is_tabbed = !container.is_tabbed;
        tracing::debug!(%container_id, from = ?direction, "Toggled container layout");
        if container.is_tabbed() {
            // Toggled from split to tabbed - find the direct child matching container's focus
            let container = self.containers.get(container_id);
            let active_tab = *container
                .children()
                .iter()
                .find(|c| **c == container.focused || matches!(c, Child::Container(cid) if self.containers.get(*cid).focused == container.focused))
                .unwrap();
            self.containers
                .get_mut(container_id)
                .set_active_tab(active_tab);
        } else {
            // Toggled from tabbed to split
            self.maintain_direction_invariance(Parent::Container(container_id));
        }
        self.maintain_direction_invariance(parent);
        self.adjust_workspace(current_ws);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_float(&mut self) -> Option<(WindowId, FloatWindowId)> {
        let current_ws = self.current_workspace();
        let focused = self.workspaces.get(current_ws).focused?;
        match focused {
            Focus::Float(float_id) => {
                self.delete_float(float_id);
                let window_id = self.insert_tiling();
                tracing::debug!(%window_id, "Window is now tiling");
                Some((window_id, float_id))
            }
            Focus::Tiling(Child::Window(window_id)) => {
                let dim = self.windows.get(window_id).dimension;
                self.delete_window(window_id);
                let screen = self.screen();
                let dimension = Dimension {
                    width: dim.width,
                    height: dim.height,
                    x: screen.x + (screen.width - dim.width) / 2.0,
                    y: screen.y + (screen.height - dim.height) / 2.0,
                };
                let float_id = self.insert_float(dimension);
                tracing::debug!(%float_id, "Window is now floating");
                Some((window_id, float_id))
            }
            Focus::Tiling(Child::Container(_)) => None,
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

        let target_workspace_id = match self.workspaces.find(|w| w.name == target_workspace) {
            Some(id) => id,
            None => self.workspaces.allocate(Workspace::new(
                target_workspace.to_string(),
                self.focused_monitor,
            )),
        };
        if current_ws == target_workspace_id {
            return;
        }

        match focused {
            Focus::Tiling(child) => {
                self.detach_child_from_workspace(child);
                self.attach_child_to_workspace(child, target_workspace_id);
            }
            Focus::Float(float_id) => {
                self.detach_float_from_workspace(float_id);
                self.float_windows.get_mut(float_id).workspace = target_workspace_id;
                self.attach_float_to_workspace(target_workspace_id, float_id);
            }
        }

        tracing::debug!(?focused, target_workspace, "Moved to workspace");
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

    fn move_in_direction(&mut self, direction: Direction, forward: bool) {
        let current_ws = self.current_workspace();
        let Some(Focus::Tiling(child)) = self.workspaces.get(current_ws).focused else {
            return;
        };
        let Parent::Container(direct_parent_id) = self.get_parent(child) else {
            return;
        };

        // Handle swap within same container
        let direct_parent = self.containers.get(direct_parent_id);
        if direct_parent.direction().is_some_and(|d| d == direction) {
            let pos = direct_parent.position_of(child);
            let target_pos = if forward {
                pos + 1
            } else {
                pos.saturating_sub(1)
            };
            if target_pos != pos && target_pos < direct_parent.children.len() {
                tracing::debug!(
                    ?child, from = pos, to = target_pos, %direct_parent_id, "Swapping child position"
                );
                self.containers
                    .get_mut(direct_parent_id)
                    .children
                    .swap(pos, target_pos);
                self.adjust_workspace(current_ws);
                return;
            }
            // At edge, fall through to find ancestor
        }

        let mut current_anchor = Child::Container(direct_parent_id);

        for _ in super::bounded_loop() {
            let parent = self.get_parent(current_anchor);
            match parent {
                Parent::Container(container_id) => {
                    let container = self.containers.get(container_id);

                    if container.direction().is_none_or(|d| d != direction) {
                        current_anchor = Child::Container(container_id);
                        continue;
                    }

                    let pos = container
                        .children
                        .iter()
                        .position(|c| *c == current_anchor)
                        .unwrap();
                    let insert_pos = if forward { pos + 1 } else { pos };

                    tracing::debug!(
                        ?child, from = %direct_parent_id, to = %container_id, insert_pos, "Moving child to ancestor"
                    );
                    self.detach_child_from_container(direct_parent_id, child);
                    self.attach_child_to_container(child, container_id, Some(insert_pos));
                    self.adjust_workspace(current_ws);
                    self.focus_child(child);
                    return;
                }
                Parent::Workspace(workspace_id) => {
                    tracing::debug!(?child, %workspace_id, "Moving child to new root container");
                    self.detach_child_from_container(direct_parent_id, child);
                    let root = self.workspaces.get(workspace_id).root.unwrap();

                    let children = if forward {
                        vec![root, child]
                    } else {
                        vec![child, root]
                    };
                    let new_root_id = self.replace_anchor_with_container(
                        children,
                        root,
                        SpawnMode::from_direction(direction),
                    );
                    self.workspaces.get_mut(workspace_id).root =
                        Some(Child::Container(new_root_id));

                    self.adjust_workspace(current_ws);
                    self.focus_child(child);
                    return;
                }
            }
        }
    }

    /// Attach child to workspace at focused position. Child must be detached from previous
    /// parent before calling. Sets focus to child.
    fn attach_child_to_workspace(&mut self, child: Child, workspace_id: WorkspaceId) {
        self.set_workspace(child, workspace_id);
        let ws = self.workspaces.get(workspace_id);
        let insert_anchor = match ws.focused {
            Some(Focus::Tiling(c)) => Some(c),
            _ => ws.root,
        };
        let Some(insert_anchor) = insert_anchor else {
            self.workspaces.get_mut(workspace_id).root = Some(child);
            self.set_parent(child, Parent::Workspace(workspace_id));
            self.workspaces.get_mut(workspace_id).focused = Some(Focus::Tiling(child));
            self.adjust_workspace(workspace_id);
            return;
        };

        let spawn_mode = match insert_anchor {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => self.windows.get(id).spawn_mode(),
        };

        if spawn_mode.is_tab()
            && let Some(tabbed_ancestor) = self.find_tabbed_ancestor(insert_anchor)
        {
            let container = self.containers.get(tabbed_ancestor);
            self.attach_child_to_container(
                child,
                tabbed_ancestor,
                Some(container.active_tab_index() + 1),
            );
        } else if let Child::Container(cid) = insert_anchor
            && self.containers.get(cid).can_accomodate(spawn_mode)
        {
            self.attach_child_to_container(child, cid, None);
        } else {
            match self.get_parent(insert_anchor) {
                Parent::Container(container_id) => {
                    self.try_attach_child_to_container_next_to(child, container_id, insert_anchor);
                }
                Parent::Workspace(workspace_id) => {
                    self.attach_child_next_to_workspace_root(child, workspace_id);
                }
            }
        }

        self.adjust_workspace(workspace_id);
        self.focus_child(child);
    }

    fn set_workspace(&mut self, child: Child, workspace_id: WorkspaceId) {
        let mut stack = vec![child];
        for _ in super::bounded_loop() {
            let Some(current) = stack.pop() else { break };
            match current {
                Child::Window(wid) => {
                    self.windows.get_mut(wid).workspace = workspace_id;
                }
                Child::Container(cid) => {
                    self.containers.get_mut(cid).workspace = workspace_id;
                    stack.extend(self.containers.get(cid).children.iter().copied());
                }
            }
        }
    }

    /// Attach `child` next to `anchor` in container with id `container_id`, or create a new parent
    /// to house both `child` and `anchor` if any Invariances are violated
    fn try_attach_child_to_container_next_to(
        &mut self,
        child: Child,
        container_id: ContainerId,
        anchor: Child,
    ) {
        let spawn_mode = match anchor {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => self.windows.get(id).spawn_mode(),
        };
        let parent_container = self.containers.get(container_id);
        if parent_container.can_accomodate(spawn_mode) {
            let anchor_index = self.containers.get(container_id).position_of(anchor);
            self.attach_child_to_container(child, container_id, Some(anchor_index + 1));
        } else {
            let new_container_id =
                self.replace_anchor_with_container(vec![anchor, child], anchor, spawn_mode);
            self.containers
                .get_mut(container_id)
                .replace_child(anchor, Child::Container(new_container_id));
        }
    }

    fn attach_child_next_to_workspace_root(&mut self, child: Child, workspace_id: WorkspaceId) {
        let ws = self.workspaces.get(workspace_id);
        let anchor = ws.root.unwrap();
        let spawn_mode = match anchor {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => self.windows.get(id).spawn_mode(),
        };
        let new_container_id =
            self.replace_anchor_with_container(vec![anchor, child], anchor, spawn_mode);
        self.workspaces.get_mut(workspace_id).root = Some(Child::Container(new_container_id));
    }

    /// Ensures all child containers have different direction than their parent.
    /// Skips tabbed containers.
    fn maintain_direction_invariance(&mut self, parent: Parent) {
        let container_id = match parent {
            Parent::Container(id) => id,
            Parent::Workspace(ws_id) => match self.workspaces.get(ws_id).root {
                Some(Child::Container(id)) => id,
                _ => return,
            },
        };
        let mut stack = vec![container_id];
        for _ in super::bounded_loop() {
            let Some(id) = stack.pop() else {
                return;
            };
            let Some(direction) = self.containers.get(id).direction() else {
                continue; // Tabbed container, no invariant needed
            };

            for &child in &self.containers.get(id).children.clone() {
                if let Child::Container(child_id) = child {
                    if self.containers.get(child_id).has_direction(direction) {
                        self.containers.get_mut(child_id).toggle_direction();
                    }
                    stack.push(child_id);
                }
            }
        }
    }

    /// Replace anchor with a new container containing children.
    /// Gets parent, workspace, and dimension from anchor.
    fn replace_anchor_with_container(
        &mut self,
        children: Vec<Child>,
        anchor: Child,
        spawn_mode: SpawnMode,
    ) -> ContainerId {
        let (parent, workspace_id, dimension) = match anchor {
            Child::Window(wid) => {
                let w = self.windows.get(wid);
                (w.parent, w.workspace, w.dimension)
            }
            Child::Container(cid) => {
                let c = self.containers.get(cid);
                (c.parent, c.workspace, c.dimension)
            }
        };
        let container_id = if let Some(direction) = spawn_mode.as_direction() {
            let container_id = self.containers.allocate(Container::split(
                parent,
                workspace_id,
                children.clone(),
                anchor,
                dimension,
                direction,
            ));
            for child in children {
                match child {
                    Child::Window(wid) => {
                        let window = self.windows.get_mut(wid);
                        window.set_spawn_mode(spawn_mode);
                        window.parent = Parent::Container(container_id);
                    }
                    Child::Container(cid) => {
                        self.containers.get_mut(cid).parent = Parent::Container(container_id);
                    }
                }
            }
            container_id
        } else {
            let container_id = self.containers.allocate(Container::tabbed(
                parent,
                workspace_id,
                children.clone(),
                anchor,
                dimension,
            ));
            for child in children {
                match child {
                    Child::Window(wid) => {
                        self.windows.get_mut(wid).set_spawn_mode(spawn_mode);
                        self.windows.get_mut(wid).parent = Parent::Container(container_id);
                    }
                    Child::Container(cid) => {
                        self.containers.get_mut(cid).set_spawn_mode(spawn_mode);
                        self.containers.get_mut(cid).parent = Parent::Container(container_id);
                    }
                }
            }
            container_id
        };
        self.maintain_direction_invariance(Parent::Container(container_id));
        container_id
    }

    /// Attach child to existing container. Does not change focus.
    fn attach_child_to_container(
        &mut self,
        child: Child,
        container_id: ContainerId,
        insert_pos: Option<usize>,
    ) {
        let parent = self.containers.get_mut(container_id);
        if let Some(pos) = insert_pos {
            parent.children.insert(pos, child);
        } else {
            parent.children.push(child);
        }
        match child {
            Child::Window(wid) => {
                self.windows
                    .get_mut(wid)
                    .set_spawn_mode(parent.spawn_mode());
                self.windows.get_mut(wid).parent = Parent::Container(container_id);
            }
            Child::Container(cid) => {
                self.containers.get_mut(cid).parent = Parent::Container(container_id);
            }
        }
        self.maintain_direction_invariance(Parent::Container(container_id));
    }

    fn adjust_workspace(&mut self, ws_id: WorkspaceId) {
        let ws = self.workspaces.get(ws_id);
        let Some(root) = ws.root else { return };
        let screen = self.workspace_screen(ws_id);

        let Child::Container(root_id) = root else {
            self.set_root_dimension(root, screen);
            return;
        };

        // Collect containers in pre-order
        let mut stack = vec![root_id];
        let mut order = vec![];
        for _ in super::bounded_loop() {
            let Some(cid) = stack.pop() else { break };
            order.push(cid);
            for child in &self.containers.get(cid).children {
                if let Child::Container(child_cid) = child {
                    stack.push(*child_cid);
                }
            }
        }

        // Update minimum sizes bottom-up, as parent's minimum size depends on children's minimum size
        for &cid in order.iter().rev() {
            self.update_container_min_size(cid);
        }

        self.set_root_dimension(root, screen);
        for cid in order {
            let container = self.containers.get(cid);
            let dim = container.dimension;
            let children = container.children.clone();
            let direction = container.direction();
            for (child, child_dim) in children
                .iter()
                .zip(self.layout_children(&children, dim, direction))
            {
                self.set_child_dimension(*child, child_dim);
            }
        }

        // Focused window can go out of view due to resizing other windows
        self.scroll_into_view(ws_id);
    }

    // Windows can extend beyond screen visible area
    fn set_root_dimension(&mut self, root: Child, screen: Dimension) {
        let (min_w, min_h) = self.get_effective_min_size(root);
        let base_dim = Dimension {
            width: screen.width.max(min_w),
            height: screen.height.max(min_h),
            ..screen
        };

        // Apply max_size centering for single window at root
        let dim = match root {
            Child::Window(id) => {
                let (max_w, max_h) = self.get_effective_max_size(id);
                let w = if max_w > 0.0 && max_w < base_dim.width {
                    max_w
                } else {
                    base_dim.width
                };
                let h = if max_h > 0.0 && max_h < base_dim.height {
                    max_h
                } else {
                    base_dim.height
                };
                Dimension {
                    x: base_dim.x + (base_dim.width - w) / 2.0,
                    y: base_dim.y + (base_dim.height - h) / 2.0,
                    width: w,
                    height: h,
                }
            }
            Child::Container(_) => base_dim,
        };

        self.set_child_dimension(root, dim);
    }

    fn layout_children(
        &self,
        children: &[Child],
        dim: Dimension,
        direction: Option<Direction>,
    ) -> Vec<Dimension> {
        match direction {
            Some(Direction::Horizontal) => {
                let constraints: Vec<_> = children
                    .iter()
                    .map(|&c| {
                        let (min_w, min_h) = self.get_effective_min_size(c);
                        let (max_w, max_h) = match c {
                            Child::Window(id) => self.get_effective_max_size(id),
                            Child::Container(_) => (0.0, 0.0),
                        };
                        (min_w, max_w, min_h, max_h)
                    })
                    .collect();
                let height = dim.height.max(
                    constraints
                        .iter()
                        .map(|(_, _, min_h, _)| *min_h)
                        .fold(0.0, f32::max),
                );
                let width_constraints: Vec<_> = constraints
                    .iter()
                    .map(|(min_w, max_w, _, _)| (*min_w, *max_w))
                    .collect();
                let widths = distribute_space(&width_constraints, dim.width);
                let total_width: f32 = widths.iter().sum();
                let x_start = dim.x + (dim.width - total_width) / 2.0;
                let mut x = x_start;
                children
                    .iter()
                    .zip(widths)
                    .zip(constraints.iter())
                    .map(|((&child, w), (_, _, _, max_h))| {
                        let (actual_height, y_offset) = match child {
                            Child::Window(_) => {
                                if *max_h > 0.0 && *max_h < height {
                                    (*max_h, (height - *max_h) / 2.0)
                                } else {
                                    (height, 0.0)
                                }
                            }
                            Child::Container(_) => (height, 0.0),
                        };
                        let d = Dimension {
                            x,
                            y: dim.y + y_offset,
                            width: w,
                            height: actual_height,
                        };
                        x += w;
                        d
                    })
                    .collect()
            }
            Some(Direction::Vertical) => {
                let constraints: Vec<_> = children
                    .iter()
                    .map(|&c| {
                        let (min_w, min_h) = self.get_effective_min_size(c);
                        let (max_w, max_h) = match c {
                            Child::Window(id) => self.get_effective_max_size(id),
                            Child::Container(_) => (0.0, 0.0),
                        };
                        (min_w, max_w, min_h, max_h)
                    })
                    .collect();
                let width = dim.width.max(
                    constraints
                        .iter()
                        .map(|(min_w, _, _, _)| *min_w)
                        .fold(0.0, f32::max),
                );
                let height_constraints: Vec<_> = constraints
                    .iter()
                    .map(|(_, _, min_h, max_h)| (*min_h, *max_h))
                    .collect();
                let heights = distribute_space(&height_constraints, dim.height);
                let total_height: f32 = heights.iter().sum();
                let y_start = dim.y + (dim.height - total_height) / 2.0;
                let mut y = y_start;
                children
                    .iter()
                    .zip(heights)
                    .zip(constraints.iter())
                    .map(|((&child, h), (_, max_w, _, _))| {
                        let (actual_width, x_offset) = match child {
                            Child::Window(_) => {
                                if *max_w > 0.0 && *max_w < width {
                                    (*max_w, (width - *max_w) / 2.0)
                                } else {
                                    (width, 0.0)
                                }
                            }
                            Child::Container(_) => (width, 0.0),
                        };
                        let d = Dimension {
                            x: dim.x + x_offset,
                            y,
                            width: actual_width,
                            height: h,
                        };
                        y += h;
                        d
                    })
                    .collect()
            }
            None => {
                let content_y = dim.y + self.config.tab_bar_height;
                let content_height = dim.height - self.config.tab_bar_height;
                children
                    .iter()
                    .map(|&child| {
                        let (actual_width, x_offset, actual_height, y_offset) = match child {
                            Child::Window(id) => {
                                let (max_w, max_h) = self.get_effective_max_size(id);
                                let w = if max_w > 0.0 && max_w < dim.width {
                                    max_w
                                } else {
                                    dim.width
                                };
                                let h = if max_h > 0.0 && max_h < content_height {
                                    max_h
                                } else {
                                    content_height
                                };
                                (w, (dim.width - w) / 2.0, h, (content_height - h) / 2.0)
                            }
                            Child::Container(_) => (dim.width, 0.0, content_height, 0.0),
                        };
                        Dimension {
                            x: dim.x + x_offset,
                            y: content_y + y_offset,
                            width: actual_width,
                            height: actual_height,
                        }
                    })
                    .collect()
            }
        }
    }

    fn set_child_dimension(&mut self, child: Child, dim: Dimension) {
        let spawn_mode = if dim.width >= dim.height {
            SpawnMode::horizontal()
        } else {
            SpawnMode::vertical()
        };
        match child {
            Child::Window(wid) => {
                let w = self.windows.get_mut(wid);
                w.dimension = dim;
                if self.config.auto_tile && !w.spawn_mode().is_tab() {
                    w.set_spawn_mode(spawn_mode);
                }
            }
            Child::Container(cid) => {
                let c = self.containers.get_mut(cid);
                c.dimension = dim;
                if self.config.auto_tile && !c.spawn_mode().is_tab() {
                    c.set_spawn_mode(spawn_mode);
                }
            }
        }
    }

    fn detach_child_from_workspace(&mut self, child: Child) {
        let parent = self.get_parent(child);
        match parent {
            Parent::Container(parent_id) => {
                let workspace_id = self.containers.get(parent_id).workspace;
                self.detach_child_from_container(parent_id, child);
                self.adjust_workspace(workspace_id);
            }
            Parent::Workspace(workspace_id) => {
                self.workspaces.get_mut(workspace_id).root = None;

                let ws = self.workspaces.get(workspace_id);
                let has_floats = !ws.float_windows.is_empty();

                // Set focus to a float if available, otherwise None
                let new_focus = ws.float_windows.last().map(|&f| Focus::Float(f));
                self.workspaces.get_mut(workspace_id).focused = new_focus;

                if workspace_id != self.current_workspace() && !has_floats {
                    self.workspaces.delete(workspace_id);
                } else {
                    self.adjust_workspace(workspace_id);
                }
            }
        }
    }

    /// Detach child from container. If container has one child left, promotes that child
    /// to grandparent (toggling direction if needed). Replaces focus from detached child to sibling.
    fn detach_child_from_container(&mut self, container_id: ContainerId, child: Child) {
        tracing::debug!(%child, %container_id, "Detaching child from container");
        // Focus preceded/following sibling if detaching focused window
        let children = &self.containers.get(container_id).children;
        let pos = children.iter().position(|c| *c == child).unwrap();
        let sibling = if pos > 0 {
            children[pos - 1]
        } else {
            children[pos + 1]
        };
        let new_focus = match sibling {
            Child::Window(_) => sibling,
            Child::Container(c) => self.containers.get(c).focused,
        };
        self.replace_focus(child, new_focus);

        self.containers.get_mut(container_id).remove_child(child);
        if self.containers.get(container_id).children.len() != 1 {
            return;
        }
        let grandparent = self.containers.get(container_id).parent;
        let last_child = self
            .containers
            .get_mut(container_id)
            .children
            .pop()
            .unwrap();
        tracing::debug!(%container_id, %last_child, "Container has one child left, cleaning up");
        self.set_parent(last_child, grandparent);
        match grandparent {
            Parent::Container(gp) => self
                .containers
                .get_mut(gp)
                .replace_child(Child::Container(container_id), last_child),
            Parent::Workspace(ws) => self.workspaces.get_mut(ws).root = Some(last_child),
        }

        // If this container was being focused, changing focus to last_child regardless of whether
        // it's a container makes sense. Don't need to focus just window here
        self.replace_focus(Child::Container(container_id), last_child);
        self.containers.delete(container_id);

        // When promoting a container to grandparent, ensure direction invariant is maintained
        self.maintain_direction_invariance(grandparent);
    }

    fn get_parent(&self, child: Child) -> Parent {
        match child {
            Child::Window(id) => self.windows.get(id).parent,
            Child::Container(id) => self.containers.get(id).parent,
        }
    }

    fn set_parent(&mut self, child: Child, parent: Parent) {
        match child {
            Child::Window(id) => self.windows.get_mut(id).parent = parent,
            Child::Container(id) => self.containers.get_mut(id).parent = parent,
        }
    }

    fn focus_tab(&mut self, forward: bool) {
        let current_ws = self.current_workspace();
        let Some(Focus::Tiling(child)) = self.workspaces.get(current_ws).focused else {
            return;
        };
        let Some(container_id) = self.find_tabbed_ancestor(child) else {
            return;
        };
        let container = self.containers.get_mut(container_id);
        let new_child = container.switch_tab(forward).unwrap();
        let focus_target = match new_child {
            Child::Window(_) => new_child,
            Child::Container(id) => self.containers.get(id).focused,
        };
        tracing::debug!(forward, %container_id, ?focus_target, "Focusing tab");
        self.focus_child(focus_target);
    }

    fn find_tabbed_ancestor(&self, child: Child) -> Option<ContainerId> {
        let mut current = child;
        for _ in super::bounded_loop() {
            if let Child::Container(id) = current
                && self.containers.get(id).is_tabbed
            {
                return Some(id);
            }
            match self.get_parent(current) {
                Parent::Container(id) => current = Child::Container(id),
                Parent::Workspace(_) => return None,
            }
        }
        unreachable!()
    }

    fn focus_in_direction(&mut self, direction: Direction, forward: bool) {
        let current_ws = self.current_workspace();
        let Some(Focus::Tiling(child)) = self.workspaces.get(current_ws).focused else {
            return;
        };

        let mut current = child;

        for _ in super::bounded_loop() {
            let Parent::Container(container_id) = self.get_parent(current) else {
                return;
            };
            if self
                .containers
                .get(container_id)
                .direction()
                .is_none_or(|d| d != direction)
            {
                current = Child::Container(container_id);
                continue;
            }
            let container = self.containers.get(container_id);
            let pos = container.position_of(current);
            let has_sibling = if forward {
                pos + 1 < container.children.len()
            } else {
                pos > 0
            };
            if has_sibling {
                let sibling_pos = if forward { pos + 1 } else { pos - 1 };
                let sibling = container.children[sibling_pos];
                let focus_target = match sibling {
                    Child::Window(_) => sibling,
                    Child::Container(id) => self.containers.get(id).focused,
                };
                tracing::debug!(?direction, forward, from = ?child, to = ?focus_target, "Changing focus");
                self.focus_child(focus_target);
                return;
            }
            current = Child::Container(container_id);
        }
    }

    /// Update primary focus, i.e. focus of the whole workspace
    fn focus_child(&mut self, child: Child) {
        let mut current = child;
        for _ in super::bounded_loop() {
            match self.get_parent(current) {
                Parent::Container(cid) => {
                    let container = self.containers.get_mut(cid);
                    if container.is_tabbed {
                        container.set_active_tab(current);
                    }
                    container.focused = child;
                    current = Child::Container(cid);
                }
                Parent::Workspace(ws) => {
                    self.workspaces.get_mut(ws).focused = Some(Focus::Tiling(child));
                    self.scroll_into_view(ws);
                    break;
                }
            }
        }
    }

    /// Replace all references of old_child, but don't take primary focus unless old_child was the
    /// focus. Given that `A container's focus must either match a child's focus or point directly
    /// to a child`, we can find the highest focusing container
    fn replace_focus(&mut self, old_child: Child, new_child: Child) {
        let mut current = old_child;
        let mut highest_focusing_container = None;
        for _ in super::bounded_loop() {
            match self.get_parent(current) {
                Parent::Container(cid) => {
                    if self.containers.get(cid).focused == old_child {
                        highest_focusing_container = Some(cid)
                    } else {
                        break;
                    }
                    current = Child::Container(cid);
                }
                Parent::Workspace(_) => {
                    highest_focusing_container = None;
                    break;
                }
            }
        }

        let mut current = new_child;
        for _ in super::bounded_loop() {
            match self.get_parent(current) {
                Parent::Container(cid) => {
                    let container = self.containers.get_mut(cid);
                    if container.focused == old_child {
                        if container.is_tabbed {
                            container.set_active_tab(current);
                        }
                        container.focused = new_child;
                    }

                    if highest_focusing_container.is_some_and(|c| c == cid) {
                        break;
                    }
                    current = Child::Container(cid);
                }
                Parent::Workspace(ws) => {
                    let workspace = self.workspaces.get_mut(ws);
                    if workspace.focused == Some(Focus::Tiling(old_child)) {
                        workspace.focused = Some(Focus::Tiling(new_child));
                        tracing::debug!(?old_child, ?new_child, "Workspace focus replaced");
                    }
                    break;
                }
            }
        }
    }

    fn attach_float_to_workspace(&mut self, ws: WorkspaceId, id: FloatWindowId) {
        self.workspaces.get_mut(ws).float_windows.push(id);
        self.workspaces.get_mut(ws).focused = Some(Focus::Float(id));
    }

    fn detach_float_from_workspace(&mut self, id: FloatWindowId) {
        let ws = self.float_windows.get(id).workspace;
        let workspace = self.workspaces.get_mut(ws);
        workspace.float_windows.retain(|&f| f != id);
        if workspace.focused == Some(Focus::Float(id)) {
            workspace.focused = workspace
                .float_windows
                .last()
                .map(|&f| Focus::Float(f))
                .or_else(|| match workspace.root {
                    Some(Child::Window(w)) => Some(Focus::window(w)),
                    Some(Child::Container(c)) => {
                        Some(Focus::Tiling(self.containers.get(c).focused))
                    }
                    None => None,
                });
            tracing::debug!(
                %id, %ws, new_focus = ?workspace.focused, "Detached focused float, focus changed"
            );
        } else {
            tracing::debug!(%id, %ws, "Detached unfocused float");
        }

        // Delete workspace if empty and not current
        let workspace = self.workspaces.get(ws);
        if ws != self.current_workspace()
            && workspace.root.is_none()
            && workspace.float_windows.is_empty()
        {
            self.workspaces.delete(ws);
        }
    }

    fn get_effective_min_size(&self, child: Child) -> (f32, f32) {
        let ws_id = match child {
            Child::Window(id) => self.windows.get(id).workspace,
            Child::Container(id) => self.containers.get(id).workspace,
        };
        let screen = self.workspace_screen(ws_id);
        let global_min_w = self.config.min_width.resolve(screen.width);
        let global_min_h = self.config.min_height.resolve(screen.height);

        match child {
            Child::Window(id) => {
                let (w, h) = self.windows.get(id).min_size();
                (w.max(global_min_w), h.max(global_min_h))
            }
            Child::Container(id) => self.containers.get(id).min_size(),
        }
    }

    fn get_effective_max_size(&self, window_id: WindowId) -> (f32, f32) {
        let window = self.windows.get(window_id);
        let screen = self.workspace_screen(window.workspace);
        let global_max_w = self.config.max_width.resolve(screen.width);
        let global_max_h = self.config.max_height.resolve(screen.height);

        let w = if window.max_width > 0.0 {
            window.max_width
        } else {
            global_max_w
        };
        let h = if window.max_height > 0.0 {
            window.max_height
        } else {
            global_max_h
        };
        (w, h)
    }

    fn update_container_min_size(&mut self, container_id: ContainerId) {
        let container = self.containers.get(container_id);
        let children = container.children.clone();
        let direction = container.direction();

        let child_mins: Vec<(f32, f32)> = children
            .iter()
            .map(|&c| self.get_effective_min_size(c))
            .collect();

        let (min_w, min_h) = match direction {
            Some(Direction::Horizontal) => {
                let sum_w: f32 = child_mins.iter().map(|(w, _)| *w).sum();
                let max_h = child_mins.iter().map(|(_, h)| *h).fold(0.0, f32::max);
                (sum_w, max_h)
            }
            Some(Direction::Vertical) => {
                let max_w = child_mins.iter().map(|(w, _)| *w).fold(0.0, f32::max);
                let sum_h: f32 = child_mins.iter().map(|(_, h)| *h).sum();
                (max_w, sum_h)
            }
            None => {
                // Tabbed
                let max_w = child_mins.iter().map(|(w, _)| *w).fold(0.0, f32::max);
                let max_h = child_mins.iter().map(|(_, h)| *h).fold(0.0, f32::max);
                (max_w, max_h + self.config.tab_bar_height)
            }
        };

        let container = self.containers.get_mut(container_id);
        container.min_width = min_w;
        container.min_height = min_h;

        if container.dimension.width < min_w {
            container.dimension.width = min_w;
        }
        if container.dimension.height < min_h {
            container.dimension.height = min_h;
        }
    }

    fn scroll_into_view(&mut self, workspace_id: WorkspaceId) {
        let ws = self.workspaces.get(workspace_id);
        let screen = self.workspace_screen(workspace_id);

        let Some(Focus::Tiling(focused)) = ws.focused else {
            return;
        };

        let focused_dim = match focused {
            Child::Window(id) => self.windows.get(id).dimension,
            Child::Container(id) => self.containers.get(id).dimension,
        };

        let mut offset_x = 0.0;
        let mut offset_y = 0.0;

        if focused_dim.x + focused_dim.width > screen.x + screen.width {
            offset_x = (screen.x + screen.width) - (focused_dim.x + focused_dim.width);
        }
        if focused_dim.x + offset_x < screen.x {
            offset_x = screen.x - focused_dim.x;
        }

        if focused_dim.y + focused_dim.height > screen.y + screen.height {
            offset_y = (screen.y + screen.height) - (focused_dim.y + focused_dim.height);
        }
        if focused_dim.y + offset_y < screen.y {
            offset_y = screen.y - focused_dim.y;
        }

        if offset_x == 0.0 && offset_y == 0.0 {
            return;
        }

        tracing::debug!(offset_x, offset_y, "Scrolling workspace into view");
        self.apply_scroll_offset(workspace_id, offset_x, offset_y);
    }

    fn apply_scroll_offset(&mut self, workspace_id: WorkspaceId, offset_x: f32, offset_y: f32) {
        let ws = self.workspaces.get(workspace_id);
        let Some(root) = ws.root else { return };

        let mut stack = vec![root];
        for _ in super::bounded_loop() {
            let Some(child) = stack.pop() else { break };
            match child {
                Child::Window(id) => {
                    let w = self.windows.get_mut(id);
                    w.dimension.x += offset_x;
                    w.dimension.y += offset_y;
                }
                Child::Container(id) => {
                    let c = self.containers.get_mut(id);
                    c.dimension.x += offset_x;
                    c.dimension.y += offset_y;
                    stack.extend(c.children.iter().copied());
                }
            }
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

fn distribute_space(constraints: &[(f32, f32)], container_size: f32) -> Vec<f32> {
    let constraints: Vec<(f32, f32)> = constraints
        .iter()
        .map(|&(min, max)| {
            let max = if max == 0.0 { f32::INFINITY } else { max };
            let max = if min > max { min } else { max };
            (min, max)
        })
        .collect();

    let sum_mins: f32 = constraints.iter().map(|(min, _)| min).sum();
    if sum_mins >= container_size {
        return constraints.iter().map(|(min, _)| *min).collect();
    }

    let all_finite = constraints.iter().all(|(_, max)| max.is_finite());
    if all_finite {
        let sum_maxes: f32 = constraints.iter().map(|(_, max)| max).sum();
        if sum_maxes <= container_size {
            return constraints.iter().map(|(_, max)| *max).collect();
        }
    }

    let mut low = 0.0;
    let mut high = container_size;
    const EPSILON: f32 = 0.001;

    while high - low > EPSILON {
        let mid = (low + high) / 2.0;
        let total: f32 = constraints
            .iter()
            .map(|(min, max)| mid.clamp(*min, *max))
            .sum();
        if total > container_size {
            high = mid;
        } else {
            low = mid;
        }
    }

    constraints
        .iter()
        .map(|(min, max)| low.clamp(*min, *max))
        .collect()
}
