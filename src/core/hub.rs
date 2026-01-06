use super::allocator::Allocator;
use super::node::{
    Child, Container, ContainerId, Dimension, Direction, FloatWindow, FloatWindowId, Focus, Parent,
    SpawnMode, Window, WindowId, Workspace, WorkspaceId,
};

#[derive(Debug)]
pub(crate) struct Hub {
    screen: Dimension,
    current: WorkspaceId,
    tab_bar_height: f32,
    auto_tile: bool,

    workspaces: Allocator<Workspace>,
    windows: Allocator<Window>,
    float_windows: Allocator<FloatWindow>,
    containers: Allocator<Container>,
}

impl Hub {
    pub(crate) fn new(screen: Dimension, tab_bar_height: f32, auto_tile: bool) -> Self {
        let mut workspace_allocator: Allocator<Workspace> = Allocator::new();
        let window_allocator: Allocator<Window> = Allocator::new();
        let float_window_allocator: Allocator<FloatWindow> = Allocator::new();
        let container_allocator: Allocator<Container> = Allocator::new();
        let default_workspace_name = 0;
        let initial_workspace =
            workspace_allocator.allocate(Workspace::new(screen, default_workspace_name));

        Self {
            current: initial_workspace,
            workspaces: workspace_allocator,
            screen,
            tab_bar_height,
            auto_tile,
            windows: window_allocator,
            float_windows: float_window_allocator,
            containers: container_allocator,
        }
    }

    // TODO: Close empty workspaces on switching out
    pub(crate) fn focus_workspace(&mut self, name: usize) {
        let workspace_id = match self.workspaces.find(|w| w.name == name) {
            Some(workspace_id) => {
                if workspace_id == self.current {
                    return;
                }
                workspace_id
            }
            None => self.workspaces.allocate(Workspace::new(self.screen, name)),
        };

        tracing::debug!(name, %workspace_id, "Focusing workspace");
        self.current = workspace_id
    }

    pub(crate) fn current_workspace(&self) -> WorkspaceId {
        self.current
    }

    pub(crate) fn set_focus(&mut self, window_id: WindowId) {
        let workspace_id = self.windows.get(window_id).workspace;
        tracing::debug!(%window_id, %workspace_id, "Setting focus to window");
        self.current = workspace_id;
        self.focus_child(Child::Window(window_id));
    }

    pub(crate) fn set_float_focus(&mut self, float_id: FloatWindowId) {
        let workspace_id = self.float_windows.get(float_id).workspace;
        tracing::debug!(%float_id, %workspace_id, "Setting focus to float");
        self.current = workspace_id;
        self.workspaces.get_mut(workspace_id).focused = Some(Focus::Float(float_id));
    }

    pub(crate) fn screen(&self) -> Dimension {
        self.screen
    }

    pub(crate) fn sync_config(&mut self, tab_bar_height: f32, auto_tile: bool) {
        self.tab_bar_height = tab_bar_height;
        self.auto_tile = auto_tile;
        for (ws_id, _) in self.workspaces.all_active() {
            self.adjust_children_dimension_of(Parent::Workspace(ws_id));
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
        let window_id = self.windows.allocate(Window::new(
            Parent::Workspace(self.current),
            self.current,
            SpawnMode::default(),
        ));
        self.attach_child_to_workspace(Child::Window(window_id), self.current);
        window_id
    }

    /// Insert a new window as float to the current workspace.
    /// Update workspace focus to the newly inserted window.
    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_float(&mut self, dimension: Dimension) -> FloatWindowId {
        let float_id = self
            .float_windows
            .allocate(FloatWindow::new(self.current, dimension));
        self.attach_float_to_workspace(self.current, float_id);
        float_id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn delete_float(&mut self, id: FloatWindowId) {
        self.detach_float_from_workspace(id);
        self.float_windows.delete(id);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn delete_window(&mut self, id: WindowId) {
        self.detach_child_from_its_parent(Child::Window(id));
        self.windows.delete(id);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_spawn_mode(&mut self) {
        let Some(Focus::Tiling(focused)) = self.workspaces.get(self.current).focused else {
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
        let Some(Focus::Tiling(child)) = self.workspaces.get(self.current).focused else {
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
        self.adjust_children_dimension_of(Parent::Container(root_id));
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_parent(&mut self) {
        let Some(Focus::Tiling(child)) = self.workspaces.get(self.current).focused else {
            return;
        };
        let Parent::Container(container_id) = self.get_parent(child) else {
            tracing::debug!("Cannot focus parent of workspace root, ignoring");
            return;
        };
        self.focus_child(Child::Container(container_id));
    }

    pub(crate) fn focus_next_tab(&mut self) {
        self.focus_tab(true);
    }

    pub(crate) fn focus_prev_tab(&mut self) {
        self.focus_tab(false);
    }

    pub(crate) fn toggle_container_layout(&mut self) {
        let Some(Focus::Tiling(focused)) = self.workspaces.get(self.current).focused else {
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
        if container.direction().is_none() {
            // Toggled from split to tabbed
            let tabbed_container = self.containers.get(container_id);

            let active_tab = if Child::Container(container_id) == focused {
                // `tabbed_container`'s focused at this moment must be a direct child, since the
                // only way to focus a container is by calling focus_parent from a direct child
                tabbed_container.focused
            } else {
                focused
            };
            self.containers
                .get_mut(container_id)
                .set_active_tab(active_tab);
        }
        self.maintain_direction_invariance(parent);
        self.adjust_children_dimension_of(parent);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_float(&mut self) -> Option<(WindowId, FloatWindowId)> {
        let focused = self.workspaces.get(self.current).focused?;
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
                let dimension = Dimension {
                    width: dim.width,
                    height: dim.height,
                    x: self.screen.x + (self.screen.width - dim.width) / 2.0,
                    y: self.screen.y + (self.screen.height - dim.height) / 2.0,
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

    pub(crate) fn move_focused_to_workspace(&mut self, target_workspace: usize) {
        let Some(focused) = self.workspaces.get(self.current).focused else {
            return;
        };

        let current_workspace_id = self.current;
        let target_workspace_id = match self.workspaces.find(|w| w.name == target_workspace) {
            Some(id) => id,
            None => self
                .workspaces
                .allocate(Workspace::new(self.screen, target_workspace)),
        };
        if current_workspace_id == target_workspace_id {
            return;
        }

        match focused {
            Focus::Tiling(child) => {
                self.detach_child_from_its_parent(child);
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

    pub(crate) fn is_focusing(&self, id: WindowId) -> bool {
        self.workspaces.get(self.current).focused == Some(Focus::Tiling(Child::Window(id)))
    }

    fn move_in_direction(&mut self, direction: Direction, forward: bool) {
        let Some(Focus::Tiling(child)) = self.workspaces.get(self.current).focused else {
            return;
        };
        let Parent::Container(direct_parent_id) = self.get_parent(child) else {
            return;
        };

        // Handle swap within same container
        let direct_parent = self.containers.get(direct_parent_id);
        if direct_parent.direction().is_some_and(|d| d == direction) {
            let pos = direct_parent
                .children
                .iter()
                .position(|c| *c == child)
                .unwrap();
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
                self.adjust_children_dimension_of(Parent::Container(direct_parent_id));
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
            self.adjust_children_dimension_of(Parent::Workspace(workspace_id));
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
        self.adjust_children_dimension_of(Parent::Container(container_id));
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
        self.adjust_children_dimension_of(Parent::Container(container_id));
    }

    fn adjust_children_dimension_of(&mut self, parent: Parent) {
        let root_id = match parent {
            Parent::Container(id) => id,
            Parent::Workspace(ws_id) => {
                let ws = self.workspaces.get(ws_id);
                match ws.root {
                    Some(Child::Container(cid)) => {
                        self.containers.get_mut(cid).dimension = ws.screen;
                        cid
                    }
                    Some(Child::Window(wid)) => {
                        self.windows.get_mut(wid).dimension = ws.screen;
                        return;
                    }
                    None => return,
                }
            }
        };
        let mut stack = vec![root_id];
        for _ in super::bounded_loop() {
            let Some(container_id) = stack.pop() else {
                return;
            };
            let container = self.containers.get(container_id);
            let dim = container.dimension;
            let children = container.children.clone();
            let direction = container.direction();
            let child_count = children.len();

            match direction {
                Some(Direction::Horizontal) => {
                    let child_width = dim.width / child_count as f32;
                    for (i, child) in children.into_iter().enumerate() {
                        let child_dim = Dimension {
                            x: dim.x + child_width * i as f32,
                            y: dim.y,
                            width: child_width,
                            height: dim.height,
                        };
                        self.set_child_dimension(child, child_dim, &mut stack);
                    }
                }
                Some(Direction::Vertical) => {
                    let child_height = dim.height / child_count as f32;
                    for (i, child) in children.into_iter().enumerate() {
                        let child_dim = Dimension {
                            x: dim.x,
                            y: dim.y + child_height * i as f32,
                            width: dim.width,
                            height: child_height,
                        };
                        self.set_child_dimension(child, child_dim, &mut stack);
                    }
                }
                None => {
                    let child_dim = Dimension {
                        x: dim.x,
                        y: dim.y + self.tab_bar_height,
                        width: dim.width,
                        height: dim.height - self.tab_bar_height,
                    };
                    for child in children {
                        self.set_child_dimension(child, child_dim, &mut stack);
                    }
                }
            }
        }
    }

    fn set_child_dimension(&mut self, child: Child, dim: Dimension, stack: &mut Vec<ContainerId>) {
        let spawn_mode = if dim.width >= dim.height {
            SpawnMode::horizontal()
        } else {
            SpawnMode::vertical()
        };
        match child {
            Child::Window(wid) => {
                let w = self.windows.get_mut(wid);
                w.dimension = dim;
                if self.auto_tile && !w.spawn_mode().is_tab() {
                    w.set_spawn_mode(spawn_mode);
                }
            }
            Child::Container(cid) => {
                let c = self.containers.get_mut(cid);
                c.dimension = dim;
                if self.auto_tile && !c.spawn_mode().is_tab() {
                    c.set_spawn_mode(spawn_mode);
                }
                stack.push(cid);
            }
        }
    }

    fn detach_child_from_its_parent(&mut self, child: Child) {
        let parent = self.get_parent(child);
        match parent {
            Parent::Container(parent_id) => {
                self.detach_child_from_container(parent_id, child);
            }
            Parent::Workspace(workspace_id) => {
                self.workspaces.get_mut(workspace_id).root = None;

                let ws = self.workspaces.get(workspace_id);
                let has_floats = !ws.float_windows.is_empty();

                // Set focus to a float if available, otherwise None
                let new_focus = ws.float_windows.last().map(|&f| Focus::Float(f));
                self.workspaces.get_mut(workspace_id).focused = new_focus;

                if workspace_id != self.current && !has_floats {
                    self.workspaces.delete(workspace_id);
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
        self.adjust_children_dimension_of(Parent::Container(container_id));
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
        self.adjust_children_dimension_of(grandparent);
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
        let Some(Focus::Tiling(child)) = self.workspaces.get(self.current).focused else {
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
        let Some(Focus::Tiling(child)) = self.workspaces.get(self.current).focused else {
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
            let pos = container
                .children
                .iter()
                .position(|c| *c == current)
                .unwrap();
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
        if ws != self.current && workspace.root.is_none() && workspace.float_windows.is_empty() {
            self.workspaces.delete(ws);
        }
    }
}
