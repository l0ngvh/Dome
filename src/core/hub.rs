use super::allocator::Allocator;
use super::node::{
    Child, Container, ContainerId, Dimension, Direction, FloatWindow, FloatWindowId, Focus, Parent,
    SpawnMode, Window, WindowId, Workspace, WorkspaceId,
};

#[derive(Debug)]
pub(crate) struct Hub {
    screen: Dimension,
    current: WorkspaceId,
    border_size: f32,
    tab_bar_height: f32,

    workspaces: Allocator<Workspace>,
    windows: Allocator<Window>,
    float_windows: Allocator<FloatWindow>,
    containers: Allocator<Container>,
}

impl Hub {
    pub(crate) fn new(screen: Dimension, border_size: f32, tab_bar_height: f32) -> Self {
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
            border_size,
            tab_bar_height,
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
        self.toggle_container_direction(root_id);
        self.balance_workspace(self.current);
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
        container.is_tabbed = !container.is_tabbed;
        let children = container.children.clone();
        tracing::debug!(%container_id, from = ?direction, "Toggled container layout");
        if let Some(mut direction) = container.direction() {
            // When toggling from tabbed to split, ensure direction differs from parent and
            // children
            if let Parent::Container(parent_cid) = container.parent {
                let parent_container = self.containers.get(parent_cid);
                if parent_container.has_direction(direction) {
                    direction = self.containers.get_mut(container_id).toggle_direction();
                }
            }
            for c in children {
                if let Child::Container(child_cid) = c
                    && self.containers.get(child_cid).has_direction(direction)
                {
                    self.toggle_container_direction(child_cid);
                }
            }
        } else {
            // Toggled from split to tabbed
            tracing::info!(
                "Focused: {} {} {}",
                container.focused,
                focused,
                container_id
            );
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
        self.balance_workspace(self.current);
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

    pub(crate) fn is_focusing(&self, child: Child) -> bool {
        self.workspaces.get(self.current).focused == Some(Focus::Tiling(child))
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
                self.balance_workspace(self.current);
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
                    self.balance_workspace(self.current);
                    return;
                }
                Parent::Workspace(workspace_id) => {
                    tracing::debug!(?child, %workspace_id, "Moving child to new root container");
                    self.detach_child_from_container(direct_parent_id, child);
                    let root = self.workspaces.get(workspace_id).root.unwrap();
                    let screen = self.workspaces.get(workspace_id).screen;

                    let children = if forward {
                        vec![root, child]
                    } else {
                        vec![child, root]
                    };
                    let new_root_id = self.create_container_with_children(
                        children,
                        root,
                        Parent::Workspace(workspace_id),
                        workspace_id,
                        screen,
                        SpawnMode::from_direction(direction),
                    );
                    self.workspaces.get_mut(workspace_id).root =
                        Some(Child::Container(new_root_id));

                    self.focus_child(child);
                    self.balance_workspace(workspace_id);
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
            self.balance_workspace(workspace_id);
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
        self.balance_workspace(workspace_id);
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
        let dimension = parent_container.dimension;
        if parent_container.can_accomodate(spawn_mode) {
            let anchor_index = self.containers.get(container_id).position_of(anchor);
            self.attach_child_to_container(child, container_id, Some(anchor_index + 1));
        } else {
            let workspace_id = parent_container.workspace;
            let new_container_id = self.create_container_with_children(
                vec![anchor, child],
                anchor,
                Parent::Container(container_id),
                workspace_id,
                dimension,
                spawn_mode,
            );
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
        let screen = ws.screen;
        let new_container_id = self.create_container_with_children(
            vec![anchor, child],
            child,
            Parent::Workspace(workspace_id),
            workspace_id,
            screen,
            spawn_mode,
        );
        self.workspaces.get_mut(workspace_id).root = Some(Child::Container(new_container_id));
    }

    fn toggle_container_direction(&mut self, container_id: ContainerId) {
        let mut stack = vec![container_id];
        for _ in super::bounded_loop() {
            let Some(id) = stack.pop() else {
                return;
            };
            self.containers.get_mut(id).toggle_direction();
            for &child in &self.containers.get(id).children {
                if let Child::Container(child_id) = child
                    && !self.containers.get(child_id).is_tabbed
                {
                    stack.push(child_id);
                }
            }
        }
    }

    /// Create a container with children. Use this instead of Container constructors directly
    /// to ensure direction invariant (child containers toggle if same direction as parent).
    /// If spawn_mode is Tab, creates a tabbed container.
    /// Does not change workspace focus - pass the previously focused child as `focused`.
    fn create_container_with_children(
        &mut self,
        children: Vec<Child>,
        focused: Child,
        parent: Parent,
        workspace_id: WorkspaceId,
        dimension: Dimension,
        spawn_mode: SpawnMode,
    ) -> ContainerId {
        if let Some(direction) = spawn_mode.as_direction() {
            let container_id = self.containers.allocate(Container::split(
                parent,
                workspace_id,
                children.clone(),
                focused,
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
                        let container = self.containers.get_mut(cid);
                        container.parent = Parent::Container(container_id);
                        if container.has_direction(direction) {
                            self.toggle_container_direction(cid);
                        }
                    }
                }
            }
            container_id
        } else {
            let container_id = self.containers.allocate(Container::tabbed(
                parent,
                workspace_id,
                children.clone(),
                focused,
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
        }
    }

    /// Attach child to existing container. Ensures direction invariant (child containers
    /// toggle if same direction as parent). Does not change focus.
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
                let parent_direction = parent.direction();
                if parent_direction.is_some_and(|d| {
                    self.containers
                        .get(cid)
                        .direction()
                        .is_some_and(|c_d| d == c_d)
                }) {
                    self.toggle_container_direction(cid);
                }
                self.containers.get_mut(cid).parent = Parent::Container(container_id);
            }
        }
    }

    fn balance_workspace(&mut self, workspace_id: WorkspaceId) {
        let workspace = self.workspaces.get(workspace_id);
        let Some(root) = workspace.root else {
            return;
        };
        let screen = workspace.screen;
        match root {
            Child::Window(window_id) => {
                let window = self.windows.get_mut(window_id);
                window.dimension.x = screen.x + self.border_size;
                window.dimension.y = screen.y + self.border_size;
                window.dimension.width = screen.width - 2.0 * self.border_size;
                window.dimension.height = screen.height - 2.0 * self.border_size;
            }
            Child::Container(container_id) => {
                self.update_container_structure(container_id);
                self.distribute_available_space(
                    Child::Container(container_id),
                    screen.x,
                    screen.y,
                    screen.width,
                    screen.height,
                );
            }
        }
    }

    fn distribute_available_space(
        &mut self,
        root: Child,
        root_x: f32,
        root_y: f32,
        root_width: f32,
        root_height: f32,
    ) {
        let mut stack = vec![(root, root_x, root_y, root_width, root_height)];

        for _ in super::bounded_loop() {
            let Some((child, x, y, available_width, available_height)) = stack.pop() else {
                return;
            };

            match child {
                Child::Window(window_id) => {
                    let window = self.windows.get_mut(window_id);
                    window.dimension.x = x + self.border_size;
                    window.dimension.y = y + self.border_size;
                    window.dimension.width = available_width - 2.0 * self.border_size;
                    window.dimension.height = available_height - 2.0 * self.border_size;
                }
                Child::Container(container_id) => {
                    let container = self.containers.get(container_id);
                    let children = container.children.clone();
                    let direction = container.direction();
                    let free_horizontal = container.freely_sized_horizontal;
                    let free_vertical = container.freely_sized_vertical;

                    let mut actual_width = 0.0;
                    let mut actual_height: f32 = 0.0;

                    match direction {
                        Some(Direction::Horizontal) => {
                            let column_width = if free_horizontal > 0 {
                                available_width / free_horizontal as f32
                            } else {
                                0.0
                            };
                            let mut current_x = x;
                            for child_id in children {
                                let child_width = match child_id {
                                    Child::Window(_) => column_width,
                                    Child::Container(c) => {
                                        let child_free_horizontal =
                                            self.containers.get(c).freely_sized_horizontal;
                                        column_width * child_free_horizontal as f32
                                    }
                                };
                                stack.push((child_id, current_x, y, child_width, available_height));
                                current_x += child_width;
                            }
                            actual_width = current_x - x;
                            actual_height = available_height;
                        }
                        Some(Direction::Vertical) => {
                            let row_height = if free_vertical > 0 {
                                available_height / free_vertical as f32
                            } else {
                                0.0
                            };
                            let mut current_y = y;
                            for child_id in children {
                                let child_height = match child_id {
                                    Child::Window(_) => row_height,
                                    Child::Container(c) => {
                                        let child_free_vertical =
                                            self.containers.get(c).freely_sized_vertical;
                                        row_height * child_free_vertical as f32
                                    }
                                };
                                stack.push((child_id, x, current_y, available_width, child_height));
                                current_y += child_height;
                            }
                            actual_width = available_width;
                            actual_height = current_y - y;
                        }
                        None => {
                            let content_y = y + self.tab_bar_height;
                            let content_height = available_height - self.tab_bar_height;
                            for child in children {
                                stack.push((child, x, content_y, available_width, content_height));
                            }
                            actual_height = available_height;
                            actual_width = available_width;
                        }
                    }

                    let container = self.containers.get_mut(container_id);
                    container.dimension = Dimension {
                        x,
                        y,
                        width: actual_width,
                        height: actual_height,
                    };
                }
            }
        }
    }

    fn update_container_structure(&mut self, root_id: ContainerId) {
        let mut stack = vec![root_id];
        let mut post_order = Vec::new();

        // Build post-order traversal (children before parents)
        for _ in super::bounded_loop() {
            let Some(container_id) = stack.pop() else {
                break;
            };
            post_order.push(container_id);
            for &child in &self.containers.get(container_id).children {
                if let Child::Container(child_id) = child {
                    stack.push(child_id);
                }
            }
        }

        // Process in reverse (children first)
        for container_id in post_order.into_iter().rev() {
            let container = self.containers.get(container_id);
            let children = container.children.clone();
            let direction = container.direction();

            let mut free_horizontal = 0;
            let mut free_vertical = 0;
            let mut fixed_width = 0.0;
            let mut fixed_height = 0.0;

            match direction {
                Some(Direction::Horizontal) => {
                    for child in children {
                        match child {
                            Child::Window(_) => {
                                free_horizontal += 1;
                                free_vertical = free_vertical.max(1);
                            }
                            Child::Container(child_id) => {
                                let child_container = self.containers.get(child_id);
                                free_horizontal += child_container.freely_sized_horizontal;
                                fixed_width += child_container.fixed_width;
                                if child_container.fixed_height > fixed_height {
                                    free_vertical = child_container.freely_sized_vertical;
                                    fixed_height = child_container.fixed_height;
                                } else if child_container.fixed_height == fixed_height {
                                    free_vertical =
                                        free_vertical.max(child_container.freely_sized_vertical);
                                }
                            }
                        }
                    }
                }
                Some(Direction::Vertical) => {
                    for child in children {
                        match child {
                            Child::Window(_) => {
                                free_vertical += 1;
                                free_horizontal = free_horizontal.max(1);
                            }
                            Child::Container(child_id) => {
                                let child_container = self.containers.get(child_id);
                                free_vertical += child_container.freely_sized_vertical;
                                fixed_height += child_container.fixed_height;
                                if child_container.fixed_width > fixed_width {
                                    free_horizontal = free_horizontal
                                        .max(child_container.freely_sized_horizontal);
                                    fixed_width = child_container.fixed_width;
                                } else if child_container.fixed_width == fixed_width {
                                    free_horizontal = free_horizontal
                                        .max(child_container.freely_sized_horizontal);
                                }
                            }
                        }
                    }
                }
                None => {
                    free_horizontal = 1;
                    free_vertical = 1;
                    fixed_width = 0.0;
                    fixed_height = 0.0;
                    for child in children {
                        if let Child::Container(child_id) = child {
                            let child_container = self.containers.get(child_id);
                            free_horizontal =
                                free_horizontal.max(child_container.freely_sized_horizontal);
                            free_vertical =
                                free_vertical.max(child_container.freely_sized_vertical);
                        }
                    }
                }
            }

            let container = self.containers.get_mut(container_id);
            container.freely_sized_horizontal = free_horizontal;
            container.freely_sized_vertical = free_vertical;
            container.fixed_width = fixed_width;
            container.fixed_height = fixed_height;
        }

        let root = self.containers.get(root_id);
        tracing::debug!(
            %root_id,
            free_horizontal = root.freely_sized_horizontal,
            free_vertical = root.freely_sized_vertical,
            "Updated container structure"
        );
    }

    fn detach_child_from_its_parent(&mut self, child: Child) {
        let parent = self.get_parent(child);
        match parent {
            Parent::Container(parent_id) => {
                let workspace_id = self.containers.get(parent_id).workspace;

                self.detach_child_from_container(parent_id, child);
                self.balance_workspace(workspace_id);
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

        // When promoting a container to grandparent, ensure direction invariant is maintained
        if let (Child::Container(child_cid), Parent::Container(gp_cid)) = (last_child, grandparent)
            && let (Some(child_dir), Some(gp_dir)) = (
                self.containers.get(child_cid).direction(),
                self.containers.get(gp_cid).direction(),
            )
            && child_dir == gp_dir
        {
            self.toggle_container_direction(child_cid);
        }

        // If this container was being focused, changing focus to last_child regardless of whether
        // it's a container makes sense. Don't need to focus just window here
        self.replace_focus(Child::Container(container_id), last_child);
        self.containers.delete(container_id);
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
        self.balance_workspace(self.current);
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
        // If direct parent is tabbed, start from the tabbed container itself
        if let Parent::Container(cid) = self.get_parent(child)
            && self.containers.get(cid).is_tabbed
        {
            current = Child::Container(cid);
        }

        for _ in super::bounded_loop() {
            let Parent::Container(container_id) = self.get_parent(current) else {
                return;
            };
            if self
                .containers
                .get(container_id)
                .direction()
                .is_some_and(|d| d != direction)
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
