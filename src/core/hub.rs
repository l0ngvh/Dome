// Invariances:
// 1. All containers must have at least 2 children.
// 2. Parent container and child container must differ in direction, unless one of them are tabbed
// 3. Container's focus must be equal to, be parent of, or don't belong to children's focus nodes' descendant.
// 4. Container's title must be equal to focused child's title
use super::allocator::Allocator;
use super::node::{
    Child, Container, ContainerId, Dimension, Direction, FloatWindow, FloatWindowId, Focus, Parent,
    Window, WindowId, Workspace, WorkspaceId,
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
        self.focus_window(window_id);
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
            Direction::default(),
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
    pub(crate) fn toggle_spawn_direction(&mut self) {
        let Some(Focus::Tiling(child)) = self.workspaces.get(self.current).focused else {
            return;
        };
        match child {
            Child::Container(container_id) => {
                let container = self.containers.get_mut(container_id);
                container.spawn_direction = match container.spawn_direction {
                    Direction::Horizontal => Direction::Vertical,
                    Direction::Vertical => Direction::Horizontal,
                };
                tracing::debug!(
                    %container_id,
                    direction = ?container.spawn_direction,
                    "Toggled spawn direction"
                );
            }
            Child::Window(window_id) => {
                let window = self.windows.get_mut(window_id);
                window.spawn_direction = match window.spawn_direction {
                    Direction::Horizontal => Direction::Vertical,
                    Direction::Vertical => Direction::Horizontal,
                };
                tracing::debug!(
                    %window_id,
                    direction = ?window.spawn_direction,
                    "Toggled spawn direction"
                );
            }
        }
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
        let mut i = 0;
        loop {
            if i >= 10000 {
                panic!("cycle detected");
            }
            let Parent::Container(parent_id) = self.containers.get(root_id).parent else {
                break;
            };
            if self.containers.get(parent_id).is_tabbed {
                break;
            }
            root_id = parent_id;
            i += 1;
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
        self.focus_container(container_id);
    }

    pub(crate) fn focus_next_tab(&mut self) {
        self.focus_tab(true);
    }

    pub(crate) fn focus_prev_tab(&mut self) {
        self.focus_tab(false);
    }

    pub(crate) fn toggle_container_layout(&mut self) {
        let Some(Focus::Tiling(child)) = self.workspaces.get(self.current).focused else {
            return;
        };
        let container_id = match child {
            Child::Container(id) => id,
            Child::Window(_) => match self.get_parent(child) {
                Parent::Container(cid) => cid,
                Parent::Workspace(_) => return,
            },
        };
        let container = self.containers.get_mut(container_id);
        container.is_tabbed = !container.is_tabbed;
        let is_tabbed = container.is_tabbed;
        let parent = container.parent;
        let mut direction = container.direction;
        let children = container.children.clone();
        tracing::debug!(%container_id, is_tabbed, "Toggled container layout");
        if is_tabbed {
            let container = self.containers.get_mut(container_id);
            if let Some(pos) = container.children.iter().position(|c| *c == child) {
                container.active_tab = pos;
            }
        } else {
            // When toggling from tabbed to non-tabbed, ensure direction differs from parent and
            // children
            if let Parent::Container(parent_cid) = parent {
                let parent_container = self.containers.get(parent_cid);
                if !parent_container.is_tabbed && parent_container.direction == direction {
                    self.containers.get_mut(container_id).toggle_direction();
                    direction = self.containers.get(container_id).direction;
                }
            }
            for c in &children {
                if let Child::Container(child_cid) = c {
                    let child_container = self.containers.get(*child_cid);
                    if !child_container.is_tabbed && child_container.direction == direction {
                        self.toggle_container_direction(*child_cid);
                    }
                }
            }
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

        // Handle float window move
        if let Focus::Float(float_id) = focused {
            self.detach_float_from_workspace(float_id);
            self.float_windows.get_mut(float_id).workspace = target_workspace_id;
            self.attach_float_to_workspace(target_workspace_id, float_id);
            tracing::debug!(?focused, target_workspace, "Moved to workspace");
            return;
        }

        let Focus::Tiling(child) = focused else {
            return;
        };

        self.detach_child_from_its_parent(child);
        self.attach_child_to_workspace(child, target_workspace_id);
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

        // Handle swap within same container (skip if parent is tabbed)
        let direct_parent = self.containers.get(direct_parent_id);
        if !direct_parent.is_tabbed && direct_parent.direction == direction {
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
        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > 1000 {
                panic!("move_in_direction exceeded max iterations");
            }

            let parent = self.get_parent(current_anchor);
            match parent {
                Parent::Container(container_id) => {
                    let container = self.containers.get(container_id);

                    if container.direction != direction {
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
                        direction,
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
            self.focus_child(child);
            self.balance_workspace(workspace_id);
            return;
        };
        match insert_anchor {
            Child::Window(anchor_id) => self.insert_next_to_window(child, anchor_id),
            Child::Container(container_id) => {
                self.attach_child_to_matching_container_closest_to(child, container_id)
            }
        }
        self.focus_child(child);
        self.balance_workspace(workspace_id);
    }

    fn set_workspace(&mut self, child: Child, workspace_id: WorkspaceId) {
        let mut stack = vec![child];
        let mut iterations = 0;
        while let Some(current) = stack.pop() {
            iterations += 1;
            if iterations > 10000 {
                panic!("set_workspace exceeded max iterations");
            }
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

    /// Insert child next to anchor window. If spawn_direction matches parent container's
    /// direction, insert as sibling. Otherwise, wrap anchor and child in a new container.
    fn insert_next_to_window(&mut self, child: Child, anchor_window_id: WindowId) {
        let anchor_window = self.windows.get(anchor_window_id);
        let spawn_direction = anchor_window.spawn_direction;
        match anchor_window.parent {
            Parent::Container(container_id) => {
                let container = self.containers.get(container_id);
                let direction = container.direction;
                let dimension = container.dimension;
                let workspace_id = container.workspace;
                let anchor_index = container.window_position(anchor_window_id);
                if spawn_direction != direction {
                    let anchored_child = Child::Window(anchor_window_id);
                    let new_container_id = self.create_container_with_children(
                        vec![anchored_child, child],
                        anchored_child,
                        Parent::Container(container_id),
                        workspace_id,
                        dimension,
                        spawn_direction,
                    );
                    self.containers.get_mut(container_id).replace_child(
                        Child::Window(anchor_window_id),
                        Child::Container(new_container_id),
                    );
                } else {
                    self.attach_child_to_container(child, container_id, Some(anchor_index + 1));
                }
            }
            Parent::Workspace(workspace_id) => {
                let screen = self.workspaces.get(workspace_id).screen;
                let anchor_child = Child::Window(anchor_window_id);
                let parent_id = self.create_container_with_children(
                    vec![anchor_child, child],
                    anchor_child,
                    Parent::Workspace(workspace_id),
                    workspace_id,
                    screen,
                    spawn_direction,
                );
                self.workspaces.get_mut(workspace_id).root = Some(Child::Container(parent_id));
            }
        }
    }

    fn attach_child_to_matching_container_closest_to(
        &mut self,
        child: Child,
        anchor_container_id: ContainerId,
    ) {
        let container = self.containers.get(anchor_container_id);
        let spawn_direction = container.spawn_direction;
        let direction = container.direction;
        let parent = container.parent;
        let dimension = container.dimension;
        if spawn_direction != direction {
            match parent {
                Parent::Container(parent_id) => {
                    // `spawn_direction` must match parent's direction, as parent and child containers must differ in direction
                    let anchor_index = self
                        .containers
                        .get(parent_id)
                        .container_position(anchor_container_id);
                    self.attach_child_to_container(child, parent_id, Some(anchor_index + 1));
                }
                Parent::Workspace(workspace_id) => {
                    let anchor_child = Child::Container(anchor_container_id);
                    let new_container_id = self.create_container_with_children(
                        vec![anchor_child, child],
                        child,
                        Parent::Workspace(workspace_id),
                        workspace_id,
                        dimension,
                        spawn_direction,
                    );
                    self.workspaces.get_mut(workspace_id).root =
                        Some(Child::Container(new_container_id));
                }
            }
        } else {
            self.attach_child_to_container(child, anchor_container_id, None);
        }
    }

    fn toggle_container_direction(&mut self, container_id: ContainerId) {
        let mut stack = vec![container_id];
        for _ in 0..10000 {
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
        panic!("cycle detected");
    }

    /// Create a container with children. Use this instead of Container::new directly
    /// to ensure direction invariant (child containers toggle if same direction as parent).
    /// Does not change workspace focus - pass the previously focused child as `focused`.
    fn create_container_with_children(
        &mut self,
        children: Vec<Child>,
        focused: Child,
        parent: Parent,
        workspace_id: WorkspaceId,
        dimension: Dimension,
        direction: Direction,
    ) -> ContainerId {
        let container_id = self.containers.allocate(Container::new(
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
                    self.windows.get_mut(wid).spawn_direction = direction;
                    self.windows.get_mut(wid).parent = Parent::Container(container_id);
                }
                Child::Container(cid) => {
                    if self.containers.get(cid).direction == direction {
                        self.toggle_container_direction(cid);
                    }
                    self.containers.get_mut(cid).parent = Parent::Container(container_id);
                }
            }
        }
        container_id
    }

    /// Attach child to existing container. Ensures direction invariant (child containers
    /// toggle if same direction as parent). Does not change focus.
    fn attach_child_to_container(
        &mut self,
        child: Child,
        container_id: ContainerId,
        insert_pos: Option<usize>,
    ) {
        let parent_direction = self.containers.get(container_id).direction;
        match child {
            Child::Window(wid) => {
                self.windows.get_mut(wid).spawn_direction = parent_direction;
                self.windows.get_mut(wid).parent = Parent::Container(container_id);
            }
            Child::Container(cid) => {
                if parent_direction == self.containers.get(cid).direction {
                    self.toggle_container_direction(cid);
                }
                self.containers.get_mut(cid).parent = Parent::Container(container_id);
            }
        }
        let parent = self.containers.get_mut(container_id);
        if let Some(pos) = insert_pos {
            parent.children.insert(pos, child);
        } else {
            parent.children.push(child);
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

        for _ in 0..10000 {
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
                    let is_tabbed = container.is_tabbed;
                    let direction = container.direction;
                    let free_horizontal = container.freely_sized_horizontal;
                    let free_vertical = container.freely_sized_vertical;

                    if is_tabbed {
                        let content_y = y + self.tab_bar_height;
                        let content_height = available_height - self.tab_bar_height;
                        for child in children {
                            stack.push((child, x, content_y, available_width, content_height));
                        }
                        self.containers.get_mut(container_id).dimension = Dimension {
                            x,
                            y,
                            width: available_width,
                            height: available_height,
                        };
                        continue;
                    }

                    let mut actual_width = 0.0;
                    let mut actual_height: f32 = 0.0;

                    match direction {
                        Direction::Horizontal => {
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
                        Direction::Vertical => {
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
        panic!("distribute_available_space: too many iterations");
    }

    fn update_container_structure(&mut self, root_id: ContainerId) {
        let mut stack = vec![root_id];
        let mut post_order = Vec::new();

        // Build post-order traversal (children before parents)
        for _ in 0..10000 {
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
        assert!(
            stack.is_empty(),
            "update_container_structure: too many containers"
        );

        // Process in reverse (children first)
        for container_id in post_order.into_iter().rev() {
            let container = self.containers.get(container_id);
            let children = container.children.clone();
            let is_tabbed = container.is_tabbed;
            let direction = container.direction;

            if is_tabbed {
                let mut max_horizontal = 1;
                let mut max_vertical = 1;
                for child in children {
                    if let Child::Container(child_id) = child {
                        let child_container = self.containers.get(child_id);
                        max_horizontal =
                            max_horizontal.max(child_container.freely_sized_horizontal);
                        max_vertical = max_vertical.max(child_container.freely_sized_vertical);
                    }
                }
                let container = self.containers.get_mut(container_id);
                container.freely_sized_horizontal = max_horizontal;
                container.freely_sized_vertical = max_vertical;
                container.fixed_width = 0.0;
                container.fixed_height = 0.0;
                continue;
            }

            let mut free_horizontal = 0;
            let mut free_vertical = 0;
            let mut fixed_width = 0.0;
            let mut fixed_height = 0.0;

            for child in children {
                match child {
                    Child::Window(_) => match direction {
                        Direction::Horizontal => {
                            free_horizontal += 1;
                            free_vertical = free_vertical.max(1);
                        }
                        Direction::Vertical => {
                            free_vertical += 1;
                            free_horizontal = free_horizontal.max(1);
                        }
                    },
                    Child::Container(child_id) => {
                        let child_container = self.containers.get(child_id);
                        let child_free_horizontal = child_container.freely_sized_horizontal;
                        let child_free_vertical = child_container.freely_sized_vertical;
                        let child_fixed_width = child_container.fixed_width;
                        let child_fixed_height = child_container.fixed_height;

                        match direction {
                            Direction::Horizontal => {
                                free_horizontal += child_free_horizontal;
                                fixed_width += child_fixed_width;
                                if child_fixed_height > fixed_height {
                                    free_vertical = child_free_vertical;
                                    fixed_height = child_fixed_height;
                                } else if child_fixed_height == fixed_height {
                                    free_vertical = free_vertical.max(child_free_vertical);
                                }
                            }
                            Direction::Vertical => {
                                free_vertical += child_free_vertical;
                                fixed_height += child_fixed_height;
                                if child_fixed_width > fixed_width {
                                    free_horizontal = free_horizontal.max(child_free_horizontal);
                                    fixed_width = child_fixed_width;
                                } else if child_fixed_width == fixed_width {
                                    free_horizontal = free_horizontal.max(child_free_horizontal);
                                }
                            }
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
        let new_focus = sibling_window(&self.containers, container_id, child);
        self.replace_focus(child, Child::Window(new_focus));

        self.unfocus(child, container_id);
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
        // If this container was being focused, changing focus to last_child regardless of whether
        // it's a container makes sense. Don't need to focus just window here
        self.replace_focus(Child::Container(container_id), last_child);

        // When promoting a container to grandparent, ensure direction invariant is maintained
        if let (Child::Container(child_cid), Parent::Container(gp_cid)) = (last_child, grandparent)
        {
            let child_dir = self.containers.get(child_cid).direction;
            let gp_dir = self.containers.get(gp_cid).direction;
            if child_dir == gp_dir {
                self.toggle_container_direction(child_cid);
            }
        }

        self.set_parent(last_child, grandparent);
        let focused = self.containers.get(container_id).focused;
        self.unfocus(focused, container_id);
        self.containers.delete(container_id);
        match grandparent {
            Parent::Container(gp) => self
                .containers
                .get_mut(gp)
                .replace_child(Child::Container(container_id), last_child),
            Parent::Workspace(ws) => self.workspaces.get_mut(ws).root = Some(last_child),
        }
    }

    fn unfocus(&mut self, child: Child, container_id: ContainerId) {
        match child {
            Child::Window(wid) => {
                self.windows.get_mut(wid).focused_by.remove(&container_id);
            }
            Child::Container(cid) => {
                self.containers
                    .get_mut(cid)
                    .focused_by
                    .remove(&container_id);
            }
        }
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
        let container = self.containers.get(container_id);
        let len = container.children.len();
        let new_tab = if forward {
            (container.active_tab + 1) % len
        } else {
            (container.active_tab + len - 1) % len
        };
        let container = self.containers.get_mut(container_id);
        container.active_tab = new_tab;
        let child = container.children.get(new_tab).copied().unwrap();
        let focus_target = match child {
            Child::Window(_) => child,
            Child::Container(id) => self.containers.get(id).focused,
        };
        tracing::debug!(forward, %container_id, new_tab, ?focus_target, "Focusing tab");
        self.focus_child(focus_target);
        self.balance_workspace(self.current);
    }

    fn find_tabbed_ancestor(&self, child: Child) -> Option<ContainerId> {
        let mut current = child;
        let mut iterations = 0;
        loop {
            iterations += 1;
            if iterations > 1000 {
                panic!("find_tabbed_ancestor exceeded max iterations");
            }
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
    }

    fn focus_in_direction(&mut self, direction: Direction, forward: bool) {
        let Some(Focus::Tiling(child)) = self.workspaces.get(self.current).focused else {
            return;
        };
        let Parent::Container(mut container_id) = self.get_parent(child) else {
            return;
        };
        // If direct parent is tabbed, skip to parent's sibling
        let mut current = if self.containers.get(container_id).is_tabbed {
            Child::Container(container_id)
        } else {
            child
        };
        let mut iterations = 0;
        loop {
            iterations += 1;
            if iterations > 1000 {
                panic!("focus_in_direction exceeded max iterations");
            }
            if self.containers.get(container_id).direction != direction {
                current = Child::Container(container_id);
                let Parent::Container(parent) = self.get_parent(current) else {
                    return;
                };
                container_id = parent;
                continue;
            }
            let container = self.containers.get(container_id);
            let Some(pos) = container.children.iter().position(|c| *c == current) else {
                return;
            };
            let has_sibling = if forward {
                pos + 1 < container.children.len()
            } else {
                pos > 0
            };
            if has_sibling {
                let sibling_pos = if forward { pos + 1 } else { pos - 1 };
                let sibling = container.children[sibling_pos];
                let focus_target = match sibling {
                    Child::Window(id) => Child::Window(id),
                    Child::Container(id) => self.containers.get(id).focused,
                };
                tracing::debug!(?direction, forward, from = ?child, to = ?focus_target, "Changing focus");
                self.focus_child(focus_target);
                return;
            }
            current = Child::Container(container_id);
            let Parent::Container(parent) = self.get_parent(current) else {
                return;
            };
            container_id = parent;
        }
    }

    fn focus_window(&mut self, id: WindowId) {
        self.focus_child(Child::Window(id));
    }

    fn focus_container(&mut self, id: ContainerId) {
        self.focus_child(Child::Container(id));
    }

    /// Update primary focus, i.e. focus of the whole workspace
    fn focus_child(&mut self, child: Child) {
        let mut current = child;
        let mut iterations = 0;
        loop {
            iterations += 1;
            if iterations > 1000 {
                panic!("focus_child exceeded max iterations");
            }
            match self.get_parent(current) {
                Parent::Container(cid) => {
                    let old_focused = self.containers.get(cid).focused;
                    // Remove cid from old focused child's focused_by
                    if old_focused != child {
                        match old_focused {
                            Child::Window(wid) => {
                                self.windows.get_mut(wid).focused_by.remove(&cid);
                            }
                            Child::Container(ccid) => {
                                self.containers.get_mut(ccid).focused_by.remove(&cid);
                            }
                        }
                    }
                    // Add cid to new focused child's focused_by
                    match child {
                        Child::Window(wid) => {
                            self.windows.get_mut(wid).focused_by.insert(cid);
                        }
                        Child::Container(ccid) => {
                            self.containers.get_mut(ccid).focused_by.insert(cid);
                        }
                    }
                    let container = self.containers.get_mut(cid);
                    container.focused = child;
                    // Update active_tab if this is a tabbed container
                    if container.is_tabbed
                        && let Some(pos) = container.children.iter().position(|c| *c == current)
                    {
                        container.active_tab = pos;
                    }
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
    /// focus
    fn replace_focus(&mut self, old_child: Child, new_child: Child) {
        let (focused_by, workspace_id) = match old_child {
            Child::Window(wid) => {
                let window = self.windows.get_mut(wid);
                let focused_by: Vec<_> = window.focused_by.drain().collect();
                (focused_by, window.workspace)
            }
            Child::Container(cid) => {
                let container = self.containers.get_mut(cid);
                let focused_by: Vec<_> = container.focused_by.drain().collect();
                (focused_by, container.workspace)
            }
        };
        for cid in focused_by {
            match new_child {
                Child::Window(wid) => {
                    self.windows.get_mut(wid).focused_by.insert(cid);
                }
                Child::Container(ccid) => {
                    self.containers.get_mut(ccid).focused_by.insert(cid);
                }
            }
            let container = self.containers.get_mut(cid);
            container.focused = new_child;
        }
        let workspace = self.workspaces.get_mut(workspace_id);
        if workspace.focused == Some(Focus::Tiling(old_child)) {
            workspace.focused = Some(Focus::Tiling(new_child));
            tracing::debug!(?old_child, ?new_child, "Workspace focus replaced");
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

fn sibling_window(
    containers: &Allocator<Container>,
    parent_id: ContainerId,
    child: Child,
) -> WindowId {
    let children = &containers.get(parent_id).children;
    let pos = children.iter().position(|c| *c == child).unwrap();
    let sibling = if pos > 0 {
        children[pos - 1]
    } else {
        children[pos + 1]
    };
    match sibling {
        Child::Window(w) => w,
        Child::Container(c) => {
            if pos > 0 {
                let mut current = c;
                for _ in 0..10000 {
                    match containers.get(current).children.last().unwrap() {
                        Child::Window(id) => return *id,
                        Child::Container(id) => current = *id,
                    }
                }
                panic!("sibling_window exceeded max iterations");
            } else {
                let mut current = c;
                for _ in 0..10000 {
                    match containers.get(current).children.first().unwrap() {
                        Child::Window(id) => return *id,
                        Child::Container(id) => current = *id,
                    }
                }
                panic!("sibling_window exceeded max iterations");
            }
        }
    }
}
