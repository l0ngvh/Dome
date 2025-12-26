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
    tab_bar_height: f32,

    workspaces: Allocator<Workspace>,
    windows: Allocator<Window>,
    containers: Allocator<Container>,
}

impl Hub {
    pub(crate) fn new(screen: Dimension, border_size: f32, tab_bar_height: f32) -> Self {
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
            tab_bar_height,
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

    pub(crate) fn set_focus(&mut self, window_id: WindowId) {
        let workspace_id = match self.windows.get(window_id).parent {
            Parent::Container(c) => self.get_containing_workspace(c),
            Parent::Workspace(w) => w,
        };
        if workspace_id == self.current {
            self.workspaces.get_mut(self.current).focused = Some(Child::Window(window_id));
        }
    }

    pub(crate) fn window_at(&self, x: f32, y: f32) -> Option<WindowId> {
        let workspace = self.workspaces.get(self.current);
        workspace
            .root()
            .and_then(|root| self.window_at_in_child(root, x, y))
    }

    fn window_at_in_child(&self, child: Child, x: f32, y: f32) -> Option<WindowId> {
        match child {
            Child::Window(id) => {
                let dim = self.windows.get(id).dimension;
                // Include border in hit area
                if x >= dim.x && x <= dim.x + dim.width && y >= dim.y && y <= dim.y + dim.height {
                    Some(id)
                } else {
                    None
                }
            }
            Child::Container(id) => {
                for child in self.containers.get(id).children() {
                    if let Some(window_id) = self.window_at_in_child(*child, x, y) {
                        return Some(window_id);
                    }
                }
                None
            }
        }
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
    pub(crate) fn insert_tiling(&mut self, title: String) -> WindowId {
        let (parent, insert_after) = self.get_insert_target(self.current);
        let window_id = match parent {
            Parent::Container(container_id) => {
                let direction = self.containers.get(container_id).direction;
                let window_id = self.windows.allocate(Window::new(parent, direction, title));
                if let Some(after) = insert_after {
                    self.containers
                        .get_mut(container_id)
                        .insert_window_after(window_id, after);
                } else {
                    self.containers.get_mut(container_id).push_window(window_id);
                }
                window_id
            }
            Parent::Workspace(workspace_id) => {
                let window_id =
                    self.windows
                        .allocate(Window::new(parent, Direction::default(), title));
                let screen = self.workspaces.get(workspace_id).screen;
                self.windows.get_mut(window_id).dimension = screen;
                self.workspaces.get_mut(workspace_id).root = Some(Child::Window(window_id));
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
                    let new_focus = sibling_window(&self.containers, parent_id, Child::Window(id));
                    workspace.focused = Some(Child::Window(new_focus));
                }

                self.remove_child_and_cleanup(parent_id, Child::Window(id), workspace_id);
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

    fn remove_child_and_cleanup(
        &mut self,
        container_id: ContainerId,
        child: Child,
        workspace_id: WorkspaceId,
    ) {
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
        self.set_parent(last_child, grandparent);
        if self.workspaces.get(workspace_id).focused == Some(Child::Container(container_id)) {
            self.workspaces.get_mut(workspace_id).focused = Some(last_child);
        }
        self.containers.delete(container_id);
        match grandparent {
            Parent::Container(gp) => self
                .containers
                .get_mut(gp)
                .replace_child(Child::Container(container_id), last_child),
            Parent::Workspace(ws) => self.workspaces.get_mut(ws).root = Some(last_child),
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
        if let Some(focused) = self.workspaces.get(self.current).focused {
            let parent = self.get_parent(focused);

            match parent {
                Parent::Container(container_id) => {
                    self.workspaces.get_mut(self.current).focused =
                        Some(Child::Container(container_id));
                }
                Parent::Workspace(_) => {
                    tracing::info!("Cannot focus parent workspace, ignoring");
                }
            }
        }
    }

    pub(crate) fn focus_next_tab(&mut self) {
        self.focus_tab(true);
    }

    pub(crate) fn focus_prev_tab(&mut self) {
        self.focus_tab(false);
    }

    fn focus_tab(&mut self, forward: bool) {
        let Some(focused) = self.workspaces.get(self.current).focused else {
            return;
        };
        let Some(container_id) = self.find_tabbed_ancestor(focused) else {
            return;
        };
        let container = self.containers.get(container_id);
        let len = container.children.len();
        if len == 0 {
            return;
        }
        let new_tab = if forward {
            (container.active_tab + 1) % len
        } else {
            (container.active_tab + len - 1) % len
        };
        self.containers.get_mut(container_id).active_tab = new_tab;
        if let Some(&child) = self.containers.get(container_id).children.get(new_tab) {
            let window_id = match child {
                Child::Window(id) => id,
                Child::Container(id) => first_window(&self.containers, id),
            };
            self.workspaces.get_mut(self.current).focused = Some(Child::Window(window_id));
        }
        self.balance_workspace(self.current);
    }

    fn find_tabbed_ancestor(&self, child: Child) -> Option<ContainerId> {
        let mut current = child;
        loop {
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

    pub(crate) fn toggle_container_layout(&mut self) {
        let Some(focused) = self.workspaces.get(self.current).focused else {
            return;
        };
        let container_id = match focused {
            Child::Container(id) => id,
            Child::Window(_) => match self.get_parent_container(focused) {
                Some(id) => id,
                None => return,
            },
        };
        let container = self.containers.get_mut(container_id);
        container.is_tabbed = !container.is_tabbed;
        if container.is_tabbed
            && let Some(pos) = container.children.iter().position(|&c| c == focused)
        {
            container.active_tab = pos;
        }
        self.balance_workspace(self.current);
    }

    pub(crate) fn focus_left(&mut self) {
        tracing::info!("Focusing left");
        self.focus_in_direction(Direction::Horizontal, false);
    }

    pub(crate) fn focus_right(&mut self) {
        tracing::info!("Focusing right");
        self.focus_in_direction(Direction::Horizontal, true);
    }

    pub(crate) fn focus_up(&mut self) {
        tracing::info!("Focusing up");
        self.focus_in_direction(Direction::Vertical, false);
    }

    pub(crate) fn focus_down(&mut self) {
        tracing::info!("Focusing down");
        self.focus_in_direction(Direction::Vertical, true);
    }

    fn focus_in_direction(&mut self, direction: Direction, forward: bool) {
        let Some(focused) = self.workspaces.get(self.current).focused else {
            return;
        };
        let Some(mut container_id) = self.get_parent_container(focused) else {
            return;
        };
        let mut current_child = focused;
        let mut iterations = 0;
        loop {
            iterations += 1;
            if iterations > 1000 {
                panic!("focus_in_direction exceeded max iterations");
            }
            if self.containers.get(container_id).direction != direction {
                current_child = Child::Container(container_id);
                let Some(parent) = self.get_parent_container(current_child) else {
                    return;
                };
                container_id = parent;
                continue;
            }
            let container = self.containers.get(container_id);
            let Some(pos) = container.children.iter().position(|c| *c == current_child) else {
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
                let window_id = match sibling {
                    Child::Window(id) => id,
                    Child::Container(id) => {
                        if forward {
                            first_window(&self.containers, id)
                        } else {
                            last_window(&self.containers, id)
                        }
                    }
                };
                tracing::debug!("Changing focus to: {:?}", window_id);
                self.workspaces.get_mut(self.current).focused = Some(Child::Window(window_id));
                return;
            }
            current_child = Child::Container(container_id);
            let Some(parent) = self.get_parent_container(current_child) else {
                return;
            };
            container_id = parent;
        }
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

    fn move_in_direction(&mut self, direction: Direction, forward: bool) {
        let Some(focused) = self.workspaces.get(self.current).focused else {
            return;
        };
        let Parent::Container(old_parent_id) = self.get_parent(focused) else {
            return;
        };

        // Handle swap within same container
        let old_container = self.containers.get(old_parent_id);
        if old_container.direction == direction {
            let pos = old_container
                .children
                .iter()
                .position(|c| *c == focused)
                .unwrap();
            let target_pos = if forward {
                pos + 1
            } else {
                pos.saturating_sub(1)
            };
            if target_pos != pos && target_pos < old_container.children.len() {
                tracing::debug!(
                    "Swapping {focused:?} from pos {pos} to {target_pos} in {old_parent_id:?}"
                );
                self.containers
                    .get_mut(old_parent_id)
                    .children
                    .swap(pos, target_pos);
                self.balance_workspace(self.current);
                return;
            }
            // At edge, fall through to find ancestor
        }

        let mut current_anchor = Child::Container(old_parent_id);
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
                    if container.direction == direction {
                        let pos = container
                            .children
                            .iter()
                            .position(|c| *c == current_anchor)
                            .unwrap();
                        let insert_pos = if forward { pos + 1 } else { pos };

                        tracing::debug!(
                            "Moving {focused:?} from {old_parent_id:?} to {container_id:?} at pos {insert_pos}"
                        );
                        self.containers
                            .get_mut(container_id)
                            .children
                            .insert(insert_pos, focused);
                        self.set_parent(focused, Parent::Container(container_id));
                        self.remove_child_and_cleanup(old_parent_id, focused, self.current);
                        self.balance_workspace(self.current);
                        return;
                    }
                    current_anchor = Child::Container(container_id);
                }
                Parent::Workspace(workspace_id) => {
                    tracing::debug!("Moving {focused:?} to new root container in {workspace_id:?}");
                    self.remove_child_and_cleanup(old_parent_id, focused, workspace_id);
                    let root = self.workspaces.get(workspace_id).root.unwrap();
                    let screen = self.workspaces.get(workspace_id).screen;

                    let new_root_id = self.containers.allocate(Container::new(
                        Parent::Workspace(workspace_id),
                        screen,
                        direction,
                    ));
                    self.set_parent(root, Parent::Container(new_root_id));
                    self.set_parent(focused, Parent::Container(new_root_id));

                    let children = &mut self.containers.get_mut(new_root_id).children;
                    if forward {
                        children.push(root);
                        children.push(focused);
                    } else {
                        children.push(focused);
                        children.push(root);
                    }
                    self.workspaces.get_mut(workspace_id).root =
                        Some(Child::Container(new_root_id));
                    self.balance_workspace(workspace_id);
                    return;
                }
            }
        }
    }

    pub(crate) fn move_focused_to_workspace(&mut self, target_workspace: usize) -> Option<Child> {
        let focused = self.workspaces.get(self.current).focused?;

        let current_workspace_id = self.current;
        let target_workspace_id = match self.workspaces.find(|w| w.name == target_workspace) {
            Some(id) => id,
            None => self
                .workspaces
                .allocate(Workspace::new(self.screen, target_workspace)),
        };
        if current_workspace_id == target_workspace_id {
            return None;
        }

        // Remove from current workspace
        let parent = self.get_parent(focused);
        match parent {
            Parent::Container(parent_id) => {
                let new_focus = sibling_window(&self.containers, parent_id, focused);
                self.workspaces.get_mut(current_workspace_id).focused =
                    Some(Child::Window(new_focus));
                self.remove_child_and_cleanup(parent_id, focused, current_workspace_id);
            }
            Parent::Workspace(_) => {
                self.workspaces.get_mut(current_workspace_id).root = None;
                self.workspaces.get_mut(current_workspace_id).focused = None;
            }
        }

        // Insert into target workspace
        let (target_parent, insert_after) = self.get_insert_target(target_workspace_id);
        match target_parent {
            Parent::Container(container_id) => {
                if let Some(after) = insert_after {
                    self.containers
                        .get_mut(container_id)
                        .insert_after(focused, after);
                } else {
                    self.containers.get_mut(container_id).push_child(focused);
                }
            }
            Parent::Workspace(ws_id) => {
                self.workspaces.get_mut(ws_id).root = Some(focused);
            }
        }
        self.set_parent(focused, target_parent);
        self.workspaces.get_mut(target_workspace_id).focused = Some(focused);

        self.balance_workspace(current_workspace_id);
        self.balance_workspace(target_workspace_id);

        tracing::info!("Moved {focused:?} to workspace {target_workspace}");
        Some(focused)
    }

    /// Returns (parent, optional child to insert after)
    fn get_insert_target(&mut self, workspace_id: WorkspaceId) -> (Parent, Option<Child>) {
        let ws = self.workspaces.get(workspace_id);
        let focused = ws.focused.or(ws.root);
        match focused {
            Some(Child::Window(focused_id)) => {
                let focused_window = self.windows.get(focused_id);
                let new_window_direction = focused_window.new_window_direction;
                match focused_window.parent {
                    Parent::Container(container_id) => {
                        let container = self.containers.get(container_id);
                        let direction = container.direction;
                        let dimension = container.dimension;
                        if new_window_direction != direction {
                            let new_container_id = self.containers.allocate(Container::new(
                                Parent::Container(container_id),
                                dimension,
                                new_window_direction,
                            ));
                            self.containers.get_mut(container_id).replace_child(
                                Child::Window(focused_id),
                                Child::Container(new_container_id),
                            );
                            self.windows.get_mut(focused_id).parent =
                                Parent::Container(new_container_id);
                            self.containers
                                .get_mut(new_container_id)
                                .children
                                .push(Child::Window(focused_id));
                            (
                                Parent::Container(new_container_id),
                                Some(Child::Window(focused_id)),
                            )
                        } else {
                            (
                                Parent::Container(container_id),
                                Some(Child::Window(focused_id)),
                            )
                        }
                    }
                    Parent::Workspace(_) => {
                        let screen = self.workspaces.get(workspace_id).screen;
                        let container_id = self.containers.allocate(Container::new(
                            Parent::Workspace(workspace_id),
                            screen,
                            new_window_direction,
                        ));
                        self.windows.get_mut(focused_id).parent = Parent::Container(container_id);
                        self.containers
                            .get_mut(container_id)
                            .push_window(focused_id);
                        self.workspaces.get_mut(workspace_id).root =
                            Some(Child::Container(container_id));
                        (
                            Parent::Container(container_id),
                            Some(Child::Window(focused_id)),
                        )
                    }
                }
            }
            Some(Child::Container(container_id)) => {
                let container = self.containers.get(container_id);
                let new_window_direction = container.new_window_direction;
                let direction = container.direction;
                let parent = container.parent;
                let dimension = container.dimension;
                if new_window_direction != direction {
                    match parent {
                        Parent::Container(parent_id) => (
                            Parent::Container(parent_id),
                            Some(Child::Container(container_id)),
                        ),
                        Parent::Workspace(_) => {
                            let new_container_id = self.containers.allocate(Container::new(
                                Parent::Workspace(workspace_id),
                                dimension,
                                new_window_direction,
                            ));
                            self.containers.get_mut(container_id).parent =
                                Parent::Container(new_container_id);
                            self.containers
                                .get_mut(new_container_id)
                                .children
                                .push(Child::Container(container_id));
                            self.workspaces.get_mut(workspace_id).root =
                                Some(Child::Container(new_container_id));
                            (
                                Parent::Container(new_container_id),
                                Some(Child::Container(container_id)),
                            )
                        }
                    }
                } else {
                    (Parent::Container(container_id), None)
                }
            }
            None => (Parent::Workspace(workspace_id), None),
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
                let container = self.containers.get(container_id);

                // Tabbed: layout all children at full size (only active is visible)
                if container.is_tabbed {
                    let content_y = y + self.tab_bar_height;
                    let content_height = available_height - self.tab_bar_height;
                    for &child in container.children.clone().iter() {
                        self.distribute_available_space(
                            child,
                            x,
                            content_y,
                            available_width,
                            content_height,
                            cache,
                        );
                    }
                    self.containers.get_mut(container_id).dimension = Dimension {
                        x,
                        y,
                        width: available_width,
                        height: available_height,
                    };
                    return;
                }

                let ((free_h, free_v), _) = cache[&container_id];
                tracing::debug!("Number of freely resized nodes: horizontal: {free_h}, {free_v}");
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

        if container.is_tabbed {
            let mut max_horizontal = 1;
            let mut max_vertical = 1;
            for &child in &container.children {
                match child {
                    Child::Window(_) => {}
                    Child::Container(child_id) => {
                        let ((child_h, child_v), _) =
                            self.query_container_structure(child_id, cache);
                        max_horizontal = max_horizontal.max(child_h);
                        max_vertical = max_vertical.max(child_v);
                    }
                }
            }
            let result = ((max_horizontal, max_vertical), (0.0, 0.0));
            cache.insert(container_id, result);
            return result;
        }

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

    fn get_parent_container(&self, child: Child) -> Option<ContainerId> {
        match self.get_parent(child) {
            Parent::Container(c) => Some(c),
            Parent::Workspace(_) => None,
        }
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
                last_window(containers, c)
            } else {
                first_window(containers, c)
            }
        }
    }
}
