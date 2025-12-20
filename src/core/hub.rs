use super::allocator::Allocator;
use super::node::{
    Child, Container, ContainerId, Dimension, Direction, Parent, Window, WindowId, Workspace,
    WorkspaceId,
};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct Hub {
    screen: Dimension,
    current: WorkspaceId,
    border_size: f32,

    workspaces: Allocator<Workspace>,
    windows: Allocator<Window>,
    containers: Allocator<Container>,
}

impl Hub {
    pub(crate) fn new(screen: Dimension, border_size: f32) -> Self {
        let mut workspace_allocator: Allocator<Workspace> = Allocator::new();
        let window_allocator: Allocator<Window> = Allocator::new();
        let container_allocator: Allocator<Container> = Allocator::new();
        let default_workspace_name = 0;
        let initial_workspace =
            workspace_allocator.allocate(Workspace::new(screen, default_workspace_name));

        Self {
            current: initial_workspace,
            workspaces: workspace_allocator,
            screen,
            border_size,
            windows: window_allocator,
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

        self.current = workspace_id
    }

    pub(crate) fn current_workspace(&self) -> WorkspaceId {
        self.current
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

    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_window(&mut self) -> WindowId {
        let current_workspace = self.workspaces.get_mut(self.current);
        let focused_node = current_workspace.focused.or(current_workspace.root);

        let window_id = match focused_node {
            Some(focused_node) => {
                match focused_node {
                    // Push to existing container
                    Child::Container(container_id) => {
                        let container = self.containers.get(container_id);
                        let parent = container.parent;
                        let dimension = container.dimension;
                        let new_window_direction = container.new_window_direction;
                        let direction = container.direction;
                        if new_window_direction != direction {
                            match parent {
                                Parent::Workspace(workspace_id) => {
                                    debug_assert_eq!(workspace_id, self.current);
                                    tracing::debug!(
                                        "Creating new parent container with direction {:?}",
                                        new_window_direction
                                    );
                                    let new_container_id = self.containers.allocate(
                                        Container::new(parent, dimension, new_window_direction),
                                    );
                                    current_workspace.root =
                                        Some(Child::Container(new_container_id));
                                    self.containers.get_mut(container_id).parent =
                                        Parent::Container(new_container_id);
                                    self.containers
                                        .get_mut(new_container_id)
                                        .children
                                        .push(focused_node);
                                    let window_id = self.windows.allocate(Window::new(
                                        Parent::Container(new_container_id),
                                        new_window_direction,
                                    ));
                                    self.containers
                                        .get_mut(new_container_id)
                                        .push_window(window_id);
                                    window_id
                                }
                                // must match parent's direction, as child container's
                                // direction must differ from parent
                                Parent::Container(parent_container) => {
                                    debug_assert_eq!(
                                        self.containers.get(parent_container).direction,
                                        new_window_direction
                                    );
                                    let window_id = self.windows.allocate(Window::new(
                                        Parent::Container(parent_container),
                                        new_window_direction,
                                    ));
                                    self.containers
                                        .get_mut(parent_container)
                                        .insert_window_after(window_id, focused_node);
                                    window_id
                                }
                            }
                        } else {
                            let window_id = self
                                .windows
                                .allocate(Window::new(Parent::Container(container_id), direction));
                            self.containers.get_mut(container_id).push_window(window_id);
                            window_id
                        }
                    }
                    // Push to window's parent container. Create the parent container if necessary
                    Child::Window(focused_window_id) => {
                        let focused_window = self.windows.get_mut(focused_window_id);
                        tracing::debug!("Inserting new window next to {focused_window:?}");
                        let container_id = match focused_window.parent {
                            Parent::Container(container_id) => {
                                let container = self.containers.get(container_id);
                                let dimension = container.dimension;
                                let direction = container.direction;
                                if focused_window.new_window_direction != direction {
                                    tracing::debug!(
                                        "Creating new parent container with direction {:?}",
                                        focused_window.new_window_direction
                                    );
                                    let new_container_id =
                                        self.containers.allocate(Container::new(
                                            Parent::Container(container_id),
                                            dimension,
                                            focused_window.new_window_direction,
                                        ));
                                    // Inline insert_parent
                                    self.containers.get_mut(container_id).replace_child(
                                        Child::Window(focused_window_id),
                                        Child::Container(new_container_id),
                                    );
                                    self.windows.get_mut(focused_window_id).parent =
                                        Parent::Container(new_container_id);
                                    self.containers
                                        .get_mut(new_container_id)
                                        .children
                                        .push(Child::Window(focused_window_id));
                                    new_container_id
                                } else {
                                    container_id
                                }
                            }
                            Parent::Workspace(workspace_id) => {
                                debug_assert_eq!(workspace_id, self.current);
                                let workspace = self.workspaces.get_mut(workspace_id);
                                let screen = workspace.screen;
                                let container_id = self.containers.allocate(Container::new(
                                    Parent::Workspace(workspace_id),
                                    screen,
                                    focused_window.new_window_direction,
                                ));
                                focused_window.parent = Parent::Container(container_id);
                                self.containers
                                    .get_mut(container_id)
                                    .push_window(focused_window_id);
                                workspace.root = Some(Child::Container(container_id));
                                container_id
                            }
                        };
                        let window_id = self.windows.allocate(Window::new(
                            Parent::Container(container_id),
                            self.containers.get(container_id).direction,
                        ));
                        self.containers
                            .get_mut(container_id)
                            .insert_window_after(window_id, Child::Window(focused_window_id));
                        window_id
                    }
                }
            }
            None => {
                tracing::debug!("Inserting window in empty workspace");
                let window_id = self.windows.allocate(Window::new(
                    Parent::Workspace(self.current),
                    Direction::default(),
                ));
                // TODO: set window size to workspace's size
                current_workspace.root = Some(Child::Window(window_id));
                let screen = current_workspace.screen;
                self.windows.get_mut(window_id).dimension = screen;
                window_id
            }
        };

        self.workspaces.get_mut(self.current).focused = Some(Child::Window(window_id));
        self.balance_workspace(self.current);
        window_id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn delete_window(&mut self, id: WindowId) -> WorkspaceId {
        let parent = self.windows.get(id).parent;
        match parent {
            Parent::Container(parent_id) => {
                let workspace_id = self.get_containing_workspace(parent_id);
                let workspace = self.workspaces.get_mut(workspace_id);

                // Focus preceded/following sibling if deleting focused window
                if workspace.focused.is_some_and(|f| f == Child::Window(id)) {
                    let children = &self.containers.get(parent_id).children;
                    let pos = children
                        .iter()
                        .position(|c| *c == Child::Window(id))
                        .unwrap();
                    let sibling = if pos > 0 {
                        children[pos - 1]
                    } else {
                        // Safe as container must have 2 or more children
                        children[pos + 1]
                    };
                    let new_focus = match sibling {
                        Child::Window(w) => w,
                        Child::Container(c) => {
                            if pos > 0 {
                                last_window(&self.containers, c)
                            } else {
                                first_window(&self.containers, c)
                            }
                        }
                    };
                    workspace.focused = Some(Child::Window(new_focus));
                }

                let parent = self.containers.get_mut(parent_id);
                parent.remove_window(id);
                // Valid containers must have at least 2 children
                if parent.children.len() == 1 {
                    let grandparent = parent.parent;
                    let parent_last_child = parent.children.pop().unwrap();
                    match parent_last_child {
                        Child::Window(w) => self.windows.get_mut(w).parent = grandparent,
                        Child::Container(c) => self.containers.get_mut(c).parent = grandparent,
                    }
                    if workspace
                        .focused
                        .is_some_and(|f| f == Child::Container(parent_id))
                    {
                        workspace.focused = Some(parent_last_child);
                    }
                    self.containers.delete(parent_id);
                    match grandparent {
                        Parent::Container(grandparent) => {
                            self.containers
                                .get_mut(grandparent)
                                .replace_child(Child::Container(parent_id), parent_last_child);
                        }
                        Parent::Workspace(w) => {
                            debug_assert_eq!(w, workspace_id);
                            workspace.root = Some(parent_last_child);
                        }
                    }
                }
                self.balance_workspace(workspace_id);
                self.windows.delete(id);
                workspace_id
            }
            Parent::Workspace(workspace_id) => {
                self.workspaces.get_mut(workspace_id).root = None;
                self.workspaces.get_mut(workspace_id).focused = None;
                self.windows.delete(id);

                if workspace_id != self.current {
                    self.workspaces.delete(workspace_id);
                }
                workspace_id
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_new_window_direction(&mut self) {
        let current_workspace = self.workspaces.get_mut(self.current);
        if let Some(focused) = current_workspace.focused {
            match focused {
                Child::Container(container_id) => {
                    let container = self.containers.get_mut(container_id);
                    container.new_window_direction = match container.new_window_direction {
                        Direction::Horizontal => Direction::Vertical,
                        Direction::Vertical => Direction::Horizontal,
                    };
                    tracing::info!(
                        "Toggling new window inserting direction for {container_id} to {}",
                        container.new_window_direction
                    );
                }
                Child::Window(window_id) => {
                    let window = self.windows.get_mut(window_id);
                    window.new_window_direction = match window.new_window_direction {
                        Direction::Horizontal => Direction::Vertical,
                        Direction::Vertical => Direction::Horizontal,
                    };
                    tracing::info!(
                        "Toggling new window inserting direction for {window_id} to {}",
                        window.new_window_direction
                    );
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_parent(&mut self) {
        let current_workspace = self.workspaces.get_mut(self.current);
        if let Some(focused) = current_workspace.focused {
            let parent = match focused {
                Child::Window(window_id) => self.windows.get(window_id).parent,
                Child::Container(container_id) => self.containers.get(container_id).parent,
            };

            match parent {
                Parent::Container(container_id) => {
                    current_workspace.focused = Some(Child::Container(container_id));
                }
                Parent::Workspace(_) => {
                    tracing::info!("Cannot focus parent workspace, ignoring");
                }
            }
        }
    }

    pub(crate) fn focus_left(&mut self) {
        let Some(focused) = self.workspaces.get(self.current).focused else {
            return;
        };
        if let Some(id) = self.find_prev(focused, Direction::Horizontal) {
            self.workspaces.get_mut(self.current).focused = Some(Child::Window(id));
        }
    }

    pub(crate) fn focus_right(&mut self) {
        let Some(focused) = self.workspaces.get(self.current).focused else {
            return;
        };
        if let Some(id) = self.find_next(focused, Direction::Horizontal) {
            self.workspaces.get_mut(self.current).focused = Some(Child::Window(id));
        }
    }

    pub(crate) fn focus_up(&mut self) {
        let Some(focused) = self.workspaces.get(self.current).focused else {
            return;
        };
        if let Some(id) = self.find_prev(focused, Direction::Vertical) {
            self.workspaces.get_mut(self.current).focused = Some(Child::Window(id));
        }
    }

    pub(crate) fn focus_down(&mut self) {
        let Some(focused) = self.workspaces.get(self.current).focused else {
            return;
        };
        if let Some(id) = self.find_next(focused, Direction::Vertical) {
            self.workspaces.get_mut(self.current).focused = Some(Child::Window(id));
        }
    }

    #[tracing::instrument(skip(self))]
    fn find_prev(&self, child: Child, direction: Direction) -> Option<WindowId> {
        let mut container_id = self.get_parent_container(child)?;
        let mut current_child = child;
        let mut iterations = 0;
        loop {
            iterations += 1;
            if iterations > 1000 {
                tracing::error!("find_prev exceeded max iterations");
                return None;
            }
            if self.containers.get(container_id).direction != direction {
                current_child = Child::Container(container_id);
                container_id = self.get_parent_container(current_child)?;
                continue;
            }
            let container = self.containers.get(container_id);
            let pos = container
                .children
                .iter()
                .position(|c| *c == current_child)?;
            if pos > 0 {
                return Some(match container.children[pos - 1] {
                    Child::Window(id) => id,
                    Child::Container(id) => last_window(&self.containers, id),
                });
            }
            current_child = Child::Container(container_id);
            container_id = self.get_parent_container(current_child)?;
        }
    }

    #[tracing::instrument(skip(self))]
    fn find_next(&self, child: Child, direction: Direction) -> Option<WindowId> {
        let mut container_id = self.get_parent_container(child)?;
        let mut current_child = child;
        let mut iterations = 0;
        loop {
            iterations += 1;
            if iterations > 1000 {
                tracing::error!("find_next exceeded max iterations");
                return None;
            }
            if self.containers.get(container_id).direction != direction {
                current_child = Child::Container(container_id);
                container_id = self.get_parent_container(current_child)?;
                continue;
            }
            let container = self.containers.get(container_id);
            let pos = container
                .children
                .iter()
                .position(|c| *c == current_child)?;
            if pos + 1 < container.children.len() {
                return Some(match container.children[pos + 1] {
                    Child::Window(id) => id,
                    Child::Container(id) => first_window(&self.containers, id),
                });
            }
            current_child = Child::Container(container_id);
            container_id = self.get_parent_container(current_child)?;
        }
    }

    fn get_parent_container(&self, child: Child) -> Option<ContainerId> {
        match child {
            Child::Window(id) => match self.windows.get(id).parent {
                Parent::Container(c) => Some(c),
                Parent::Workspace(_) => None,
            },
            Child::Container(id) => match self.containers.get(id).parent {
                Parent::Container(c) => Some(c),
                Parent::Workspace(_) => None,
            },
        }
    }

    #[tracing::instrument(skip(self))]
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
                let mut cache = HashMap::new();
                let ((free_h, free_v), _) =
                    self.query_container_structure(container_id, &mut cache);
                self.distribute_available_space(
                    Child::Container(container_id),
                    screen.x,
                    screen.y,
                    screen.width,
                    screen.height,
                    &cache,
                );
            }
        }
    }

    #[tracing::instrument(skip(self, cache))]
    fn distribute_available_space(
        &mut self,
        child: Child,
        x: f32,
        y: f32,
        available_width: f32,
        available_height: f32,
        cache: &HashMap<ContainerId, ((usize, usize), (f32, f32))>,
    ) {
        match child {
            Child::Window(window_id) => {
                let window = self.windows.get_mut(window_id);
                window.dimension.x = x + self.border_size;
                window.dimension.y = y + self.border_size;
                window.dimension.width = available_width - 2.0 * self.border_size;
                window.dimension.height = available_height - 2.0 * self.border_size;
            }
            Child::Container(container_id) => {
                let ((free_h, free_v), _) = cache[&container_id];
                tracing::debug!("{container_id}, {free_h}, {free_v}");
                let container = self.containers.get(container_id);
                let mut actual_width = 0.0;
                let mut actual_height: f32 = 0.0;

                match container.direction {
                    Direction::Horizontal => {
                        let column_width = if free_h > 0 {
                            available_width / free_h as f32
                        } else {
                            0.0
                        };
                        let mut current_x = x;
                        for child_id in container.children.clone() {
                            let child_width = match child_id {
                                Child::Window(_) => column_width,
                                Child::Container(c) => {
                                    let ((child_free_h, _), _) = cache[&c];
                                    column_width * child_free_h as f32
                                }
                            };
                            self.distribute_available_space(
                                child_id,
                                current_x,
                                y,
                                child_width,
                                available_height,
                                cache,
                            );
                            let child_actual_width = match child_id {
                                Child::Window(w) => {
                                    let d = self.windows.get(w).dimension;
                                    actual_height =
                                        actual_height.max(d.height + 2.0 * self.border_size);
                                    d.width + 2.0 * self.border_size
                                }
                                Child::Container(c) => {
                                    let d = self.containers.get(c).dimension;
                                    actual_height = actual_height.max(d.height);
                                    d.width
                                }
                            };
                            current_x += child_actual_width;
                        }
                        actual_width = current_x - x;
                    }
                    Direction::Vertical => {
                        let row_height = if free_v > 0 {
                            available_height / free_v as f32
                        } else {
                            0.0
                        };
                        let mut current_y = y;
                        for child_id in container.children.clone() {
                            let child_height = match child_id {
                                Child::Window(_) => row_height,
                                Child::Container(c) => {
                                    let ((_, child_free_v), _) = cache[&c];
                                    row_height * child_free_v as f32
                                }
                            };
                            self.distribute_available_space(
                                child_id,
                                x,
                                current_y,
                                available_width,
                                child_height,
                                cache,
                            );
                            match child_id {
                                Child::Window(w) => {
                                    let d = self.windows.get(w).dimension;
                                    current_y += d.height + 2.0 * self.border_size;
                                    actual_width =
                                        actual_width.max(d.width + 2.0 * self.border_size);
                                }
                                Child::Container(c) => {
                                    let d = self.containers.get(c).dimension;
                                    current_y += d.height;
                                    actual_width = actual_width.max(d.width);
                                }
                            };
                        }
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

    #[tracing::instrument(skip(self))]
    fn get_containing_workspace(&self, container_id: ContainerId) -> WorkspaceId {
        let mut parent = self.containers.get(container_id).parent;
        let mut counter = 0;
        loop {
            counter += 1;
            if counter > 1000 {
                panic!("Cycle detected in parent hierarchy");
            }
            match parent {
                Parent::Workspace(workspace_id) => return workspace_id,
                Parent::Container(container_id) => {
                    parent = self.containers.get(container_id).parent;
                }
            }
        }
    }

    fn query_container_structure(
        &self,
        container_id: ContainerId,
        cache: &mut HashMap<ContainerId, ((usize, usize), (f32, f32))>,
    ) -> ((usize, usize), (f32, f32)) {
        if let Some(&cached) = cache.get(&container_id) {
            return cached;
        }

        let container = self.containers.get(container_id);
        let mut free_horizontal = 0;
        let mut free_vertical = 0;
        let mut fixed_height = 0.0;
        let mut fixed_width = 0.0;

        for &child in &container.children {
            match child {
                Child::Window(_) => {
                    match container.direction {
                        Direction::Horizontal => {
                            free_horizontal += 1;
                            free_vertical = free_vertical.max(1)
                        }
                        Direction::Vertical => {
                            free_vertical += 1;
                            free_horizontal = free_horizontal.max(1)
                        }
                    }
                    // TODO: calculate fixed size. + border size as well
                }
                Child::Container(child_container_id) => {
                    let ((child_free_ho, child_free_v), (child_fixed_w, child_fixed_h)) =
                        self.query_container_structure(child_container_id, cache);

                    match container.direction {
                        Direction::Horizontal => {
                            free_horizontal += child_free_ho;
                            fixed_width += child_fixed_w;
                            if child_fixed_h > fixed_height {
                                free_vertical = child_free_v;
                                fixed_height = child_fixed_h
                            } else if child_fixed_h == fixed_height {
                                free_vertical = free_vertical.max(child_free_v)
                            }
                        }
                        Direction::Vertical => {
                            free_vertical += child_free_v;
                            fixed_height += child_fixed_h;
                            if child_fixed_w > fixed_width {
                                free_horizontal = free_horizontal.max(child_free_ho);
                                fixed_width = child_fixed_w
                            } else if child_fixed_w == fixed_width {
                                free_horizontal = free_horizontal.max(child_free_ho)
                            }
                        }
                    }
                }
            }
        }

        let result = (
            (free_horizontal, free_vertical),
            (fixed_height, fixed_width),
        );
        cache.insert(container_id, result);
        result
    }
}

// Valid containers must have at least 2 children, so unwrap is safe
fn first_window(containers: &Allocator<Container>, container_id: ContainerId) -> WindowId {
    match containers.get(container_id).children.first().unwrap() {
        Child::Window(id) => *id,
        Child::Container(id) => first_window(containers, *id),
    }
}

fn last_window(containers: &Allocator<Container>, container_id: ContainerId) -> WindowId {
    // Valid containers must have at least 2 children, so unwrap is safe
    match containers.get(container_id).children.last().unwrap() {
        Child::Window(id) => *id,
        Child::Container(id) => last_window(containers, *id),
    }
}
