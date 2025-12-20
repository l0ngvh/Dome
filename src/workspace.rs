#[derive(Debug)]
pub(crate) struct Hub {
    screen: Dimension,
    current: WorkspaceId,

    workspaces: Allocator<Workspace>,
    windows: Allocator<Window>,
    containers: Allocator<Container>,
}

impl Hub {
    pub(crate) fn new(screen: Dimension) -> Self {
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

    #[cfg(not(test))]
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
            Child::Window(window_id) => self.windows.get_mut(window_id).dimension = screen,
            Child::Container(container_id) => {
                let (fixed_width, fixed_height) = self.query_fixed(Child::Container(container_id));
                tracing::debug!(
                    "Container {container_id}'s fixed size {fixed_width} {fixed_height}"
                );
                let container = self.containers.get_mut(container_id);
                container.dimension = screen;

                let available_width = screen.width - fixed_width;
                let available_height = screen.height - fixed_height;
                tracing::debug!(
                    "Distributing available space ({available_width}, {available_height}) to {container_id}"
                );
                self.distribute_available_space(
                    Child::Container(container_id),
                    screen.x,
                    screen.y,
                    available_width,
                    available_height,
                );
            }
        }
    }

    fn query_fixed(&self, child: Child) -> (f32, f32) {
        match child {
            Child::Window(_window_id) => (0.0, 0.0), // TODO: query fixed size
            Child::Container(container_id) => {
                let container = self.containers.get(container_id);
                let child_sizes = container
                    .children
                    .iter()
                    .map(|child| self.query_fixed(*child));
                match container.direction {
                    Direction::Horizontal => child_sizes
                        .reduce(|(width_1, height_1), (width_2, height_2)| {
                            (width_1 + width_2, height_1.max(height_2))
                        })
                        .unwrap_or_default(),
                    Direction::Vertical => child_sizes
                        .reduce(|(width_1, height_1), (width_2, height_2)| {
                            (width_1.max(width_2), height_1 + height_2)
                        })
                        .unwrap_or_default(),
                }
            }
        }
    }

    fn distribute_available_space(
        &mut self,
        child: Child,
        x: f32,
        y: f32,
        available_width: f32,
        available_height: f32,
    ) {
        match child {
            Child::Window(window_id) => {
                let window = self.windows.get_mut(window_id);
                window.dimension.x = x;
                window.dimension.y = y;
                // TODO: ignore when window is fixed size
                window.dimension.width = available_width;
                window.dimension.height = available_height;
            }
            Child::Container(container_id) => {
                let container = self.containers.get(container_id);
                match container.direction {
                    Direction::Horizontal => {
                        // TODO: filter out fixed width windows/containers in width calculation
                        let column_width = available_width / container.children.len() as f32;
                        let mut current_x = x;
                        let mut actual_height = available_height;
                        for child_id in container.children.clone() {
                            self.distribute_available_space(
                                child_id,
                                current_x,
                                y,
                                column_width,
                                available_height,
                            );
                            match child_id {
                                Child::Window(window) => {
                                    let window = self.windows.get(window);
                                    current_x += window.dimension.width;
                                    actual_height = actual_height.max(window.dimension.height)
                                }
                                Child::Container(container) => {
                                    let container = self.containers.get(container);
                                    current_x += container.dimension.width;
                                    actual_height = actual_height.max(container.dimension.height)
                                }
                            }
                        }
                        let container = self.containers.get_mut(container_id);
                        container.dimension.x = x;
                        container.dimension.y = y;
                        container.dimension.width = current_x - x;
                        container.dimension.height = actual_height;
                    }
                    Direction::Vertical => {
                        // TODO: filter out fixed height windows/containers in width calculation
                        let row_height = available_height / container.children.len() as f32;
                        let mut current_y = y;
                        let mut actual_width = available_width;
                        for child_id in container.children.clone() {
                            self.distribute_available_space(
                                child_id,
                                x,
                                current_y,
                                available_width,
                                row_height,
                            );
                            match child_id {
                                Child::Window(window) => {
                                    let window = self.windows.get(window);
                                    current_y += window.dimension.height;
                                    actual_width = actual_width.max(window.dimension.width)
                                }
                                Child::Container(container) => {
                                    let container = self.containers.get(container);
                                    current_y += container.dimension.height;
                                    actual_width = actual_width.max(container.dimension.width)
                                }
                            }
                        }
                        let container = self.containers.get_mut(container_id);
                        container.dimension.x = x;
                        container.dimension.y = y;
                        container.dimension.width = actual_width;
                        container.dimension.height = current_y - y;
                    }
                }
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

#[derive(Debug)]
pub(crate) struct Workspace {
    name: usize,
    screen: Dimension,
    // TODO: Add list of float windows
    root: Option<Child>,
    focused: Option<Child>,
}

impl Node for Workspace {
    type Id = WorkspaceId;
}

impl Workspace {
    fn new(screen: Dimension, name: usize) -> Self {
        Self {
            root: None,
            focused: None,
            screen,
            name,
        }
    }

    pub(crate) fn root(&self) -> Option<Child> {
        self.root
    }

    pub(crate) fn focused(&self) -> Option<Child> {
        self.focused
    }
}

#[derive(Debug)]
pub(crate) struct Container {
    parent: Parent,
    children: Vec<Child>,
    dimension: Dimension,
    direction: Direction,
    new_window_direction: Direction,
}

impl Node for Container {
    type Id = ContainerId;
}

impl Container {
    fn new(parent: Parent, dimension: Dimension, direction: Direction) -> Self {
        Self {
            children: Vec::new(),
            parent,
            dimension,
            direction,
            new_window_direction: direction,
        }
    }

    pub(crate) fn children(&self) -> &[Child] {
        &self.children
    }

    fn push_window(&mut self, window_id: WindowId) {
        self.children.push(Child::Window(window_id));
    }

    fn insert_window_after(&mut self, window_id: WindowId, after: Child) {
        if let Some(pos) = self.children.iter().position(|c| *c == after) {
            self.children.insert(pos + 1, Child::Window(window_id));
        } else {
            self.children.push(Child::Window(window_id));
        }
    }

    fn remove_window(&mut self, window_id: WindowId) {
        self.children
            .retain(|child| !child.is_window_and(|id| id == window_id));
    }

    fn replace_child(&mut self, old: Child, new: Child) {
        if let Some(pos) = self.children.iter().position(|c| *c == old) {
            self.children[pos] = new;
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Direction {
    #[default]
    Horizontal,
    Vertical,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Horizontal => write!(f, "Horizontal"),
            Direction::Vertical => write!(f, "Vertical"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Parent {
    Container(ContainerId),
    Workspace(WorkspaceId),
}

impl std::fmt::Display for Parent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Parent::Container(id) => write!(f, "{}", id),
            Parent::Workspace(id) => write!(f, "{}", id),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Window {
    parent: Parent,
    dimension: Dimension,
    new_window_direction: Direction,
}

impl Node for Window {
    type Id = WindowId;
}

impl Window {
    fn new(parent: Parent, new_window_direction: Direction) -> Self {
        Self {
            parent,
            dimension: Dimension::default(),
            new_window_direction,
        }
    }

    pub(crate) fn dimension(&self) -> Dimension {
        self.dimension
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Dimension {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) x: f32,
    pub(crate) y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Child {
    Window(WindowId),
    Container(ContainerId),
}

impl std::fmt::Display for Child {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Child::Window(id) => write!(f, "{}", id),
            Child::Container(id) => write!(f, "{}", id),
        }
    }
}

impl Child {
    fn is_window_and(&self, f: impl Fn(WindowId) -> bool) -> bool {
        if let Child::Window(id) = self {
            f(*id)
        } else {
            false
        }
    }
}

#[derive(Debug)]
struct Allocator<T: Node> {
    storage: Vec<Option<T>>,
    free_list: Vec<usize>,
}

impl<T: std::fmt::Debug + Node> Allocator<T> {
    fn new() -> Self {
        Self {
            storage: Vec::new(),
            free_list: Vec::new(),
        }
    }

    #[tracing::instrument(skip(self))]
    fn allocate(&mut self, node: T) -> T::Id {
        if let Some(free) = self.free_list.pop() {
            self.storage[free] = Some(node);
            T::Id::new(free)
        } else {
            let id = self.storage.len();
            self.storage.push(Some(node));
            T::Id::new(id)
        }
    }

    #[tracing::instrument(skip(self))]
    fn delete(&mut self, id: T::Id) {
        let idx = id.get();
        if let Some(slot) = self.storage.get_mut(idx) {
            *slot = None;
            self.free_list.push(idx);
        }
    }

    fn get(&self, id: T::Id) -> &T {
        self.storage
            .get(id.get())
            // TODO: dump everything here?
            .expect(format!("Node {id:?} not found").as_str())
            .as_ref()
            .expect(format!("Node {id:?} was deleted").as_str())
    }

    fn get_mut(&mut self, id: T::Id) -> &mut T {
        self.storage
            .get_mut(id.get())
            .expect(format!("Node {id:?} not found").as_str())
            .as_mut()
            .expect(format!("Node {id:?} was deleted").as_str())
    }

    fn find(&self, f: impl Fn(&T) -> bool) -> Option<T::Id> {
        self.storage
            .iter()
            .position(|node| node.as_ref().is_some_and(&f))
            .map(T::Id::new)
    }
}

trait Node {
    type Id: NodeId + std::fmt::Debug;
}

trait NodeId: Copy {
    fn new(id: usize) -> Self;
    fn get(self) -> usize;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct WindowId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContainerId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkspaceId(usize);

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WindowId({})", self.0)
    }
}

impl std::fmt::Display for ContainerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ContainerId({})", self.0)
    }
}

impl std::fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WorkspaceId({})", self.0)
    }
}

impl NodeId for WindowId {
    fn new(id: usize) -> Self {
        Self(id)
    }
    fn get(self) -> usize {
        self.0
    }
}

impl NodeId for ContainerId {
    fn new(id: usize) -> Self {
        Self(id)
    }
    fn get(self) -> usize {
        self.0
    }
}

impl NodeId for WorkspaceId {
    fn new(id: usize) -> Self {
        Self(id)
    }
    fn get(self) -> usize {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::{Child, Dimension, Hub};
    use insta::assert_snapshot;

    #[test]
    fn initial_window_cover_full_screen() {
        setup_logger();
        let screen = Dimension {
            x: 2.0,
            y: 1.0,
            width: 20.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);
        hub.insert_window();
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=2.00 y=1.00 w=20.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(0),
            Window(id=0, parent=WorkspaceId(0), x=2.00, y=1.00, w=20.00, h=10.00)
          )
        )
        ");
    }

    #[test]
    fn split_window_evenly() {
        setup_logger();
        let screen = Dimension {
            x: 2.0,
            y: 1.0,
            width: 20.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        for _ in 0..4 {
            hub.insert_window();
        }

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=2.00 y=1.00 w=20.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(3),
            Container(id=0, parent=WorkspaceId(0), x=2.00, y=1.00, w=20.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=2.00, y=1.00, w=5.00, h=10.00)
              Window(id=1, parent=ContainerId(0), x=7.00, y=1.00, w=5.00, h=10.00)
              Window(id=2, parent=ContainerId(0), x=12.00, y=1.00, w=5.00, h=10.00)
              Window(id=3, parent=ContainerId(0), x=17.00, y=1.00, w=5.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn delete_window_removes_from_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 12.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        let w2 = hub.insert_window();
        hub.insert_window();

        hub.delete_window(w2);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=12.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(2),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=6.00, h=10.00)
              Window(id=2, parent=ContainerId(0), x=6.00, y=0.00, w=6.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn delete_window_removes_parent_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        let w2 = hub.insert_window();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Window(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
            )
          )
        )
        ");

        hub.delete_window(w2);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(0),
            Window(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00)
          )
        )
        ");
    }

    #[test]
    fn delete_all_windows() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        let w1 = hub.insert_window();
        let w2 = hub.insert_window();
        let w3 = hub.insert_window();

        hub.delete_window(w1);
        hub.delete_window(w2);
        hub.delete_window(w3);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0)
        )
        ");
    }

    #[test]
    fn delete_all_windows_cleanup_unfocused_workspace() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        let w1 = hub.insert_window();
        let w2 = hub.insert_window();

        hub.focus_workspace(1);
        hub.delete_window(w1);
        hub.delete_window(w2);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=1, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=1, name=1)
        )
        ");
    }

    #[test]
    fn switch_workspace_attaches_windows_correctly() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 12.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();

        hub.focus_workspace(1);

        hub.insert_window();
        hub.insert_window();

        hub.focus_workspace(0);

        hub.insert_window();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=12.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(4),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=4.00, h=10.00)
              Window(id=1, parent=ContainerId(0), x=4.00, y=0.00, w=4.00, h=10.00)
              Window(id=4, parent=ContainerId(0), x=8.00, y=0.00, w=4.00, h=10.00)
            )
          )
          Workspace(id=1, name=1, focused=WindowId(3),
            Container(id=1, parent=WorkspaceId(1), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
              Window(id=2, parent=ContainerId(1), x=0.00, y=0.00, w=6.00, h=10.00)
              Window(id=3, parent=ContainerId(1), x=6.00, y=0.00, w=6.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn focus_same_workspace() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        let initial_workspace = hub.current_workspace();
        hub.focus_workspace(0);

        assert_eq!(hub.current_workspace(), initial_workspace);
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(0),
            Window(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00)
          )
        )
        ");
    }

    #[test]
    fn toggle_new_window_direction_creates_new_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(2),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Container(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
                Window(id=1, parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=5.00)
                Window(id=2, parent=ContainerId(1), x=5.00, y=5.00, w=5.00, h=5.00)
              )
            )
          )
        )
        ");
    }

    #[test]
    fn delete_window_after_orientation_change() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();
        hub.toggle_new_window_direction();
        let w3 = hub.insert_window();
        hub.delete_window(w3);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Window(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn toggle_new_window_direction_in_single_window_workspace_creates_vertical_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
              Window(id=1, parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
            )
          )
        )
        ");
    }

    #[test]
    fn toggle_new_window_direction_in_vertical_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(3),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Container(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
                Window(id=1, parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=5.00)
                Container(id=2, parent=ContainerId(1), x=5.00, y=5.00, w=5.00, h=5.00, direction=Horizontal,
                  Window(id=2, parent=ContainerId(2), x=5.00, y=5.00, w=2.50, h=5.00)
                  Window(id=3, parent=ContainerId(2), x=7.50, y=5.00, w=2.50, h=5.00)
                )
              )
            )
          )
        )
        ");
    }

    #[test]
    fn focus_parent_twice_nested_containers() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        // Create nested containers
        hub.insert_window();
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();

        hub.focus_parent();
        hub.focus_parent();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=ContainerId(0),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Container(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
                Window(id=1, parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=5.00)
                Window(id=2, parent=ContainerId(1), x=5.00, y=5.00, w=5.00, h=5.00)
              )
            )
          )
        )
        ");
    }

    #[test]
    fn focus_parent_twice_single_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();

        hub.focus_parent();
        hub.focus_parent();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=ContainerId(0),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Window(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn insert_window_after_focusing_parent() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();

        hub.focus_parent();

        hub.insert_window();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(2),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
              Window(id=1, parent=ContainerId(0), x=3.33, y=0.00, w=3.33, h=10.00)
              Window(id=2, parent=ContainerId(0), x=6.67, y=0.00, w=3.33, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn new_container_preserves_wrapped_window_position() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 12.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();
        hub.insert_window();
        // Focus w1 (middle)
        hub.focus_left();
        hub.toggle_new_window_direction();
        hub.insert_window();

        // New container wrapping w1 should be in the middle position
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=12.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(3),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=4.00, h=10.00)
              Container(id=1, parent=ContainerId(0), x=4.00, y=0.00, w=4.00, h=10.00, direction=Vertical,
                Window(id=1, parent=ContainerId(1), x=4.00, y=0.00, w=4.00, h=5.00)
                Window(id=3, parent=ContainerId(1), x=4.00, y=5.00, w=4.00, h=5.00)
              )
              Window(id=2, parent=ContainerId(0), x=8.00, y=0.00, w=4.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn insert_window_after_focused_window() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 12.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();
        hub.insert_window();
        // Focus w1 (middle)
        hub.focus_left();
        hub.insert_window();

        // w3 should be inserted right after w1, not at the end
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=12.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(3),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=3.00, h=10.00)
              Window(id=1, parent=ContainerId(0), x=3.00, y=0.00, w=3.00, h=10.00)
              Window(id=3, parent=ContainerId(0), x=6.00, y=0.00, w=3.00, h=10.00)
              Window(id=2, parent=ContainerId(0), x=9.00, y=0.00, w=3.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn insert_window_after_focused_container_in_parent() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 12.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        // Create: [w0] [w1, w2] [w3]
        hub.insert_window();
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_parent();
        hub.toggle_new_window_direction();
        hub.insert_window();

        // Focus the middle container and toggle direction
        hub.focus_left();
        hub.focus_parent();
        hub.insert_window();

        // w4 should be inserted right after the focused container, not at the end
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=12.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(4),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=3.00, h=10.00)
              Container(id=1, parent=ContainerId(0), x=3.00, y=0.00, w=3.00, h=10.00, direction=Vertical,
                Window(id=1, parent=ContainerId(1), x=3.00, y=0.00, w=3.00, h=5.00)
                Window(id=2, parent=ContainerId(1), x=3.00, y=5.00, w=3.00, h=5.00)
              )
              Window(id=4, parent=ContainerId(0), x=6.00, y=0.00, w=3.00, h=10.00)
              Window(id=3, parent=ContainerId(0), x=9.00, y=0.00, w=3.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn insert_to_new_container_when_focused_container_window_insert_direction_differ_and_no_parent()
    {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();
        hub.insert_window();

        hub.focus_parent();
        hub.toggle_new_window_direction();

        hub.insert_window();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(3),
            Container(id=1, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Container(id=0, parent=ContainerId(1), x=0.00, y=0.00, w=10.00, h=5.00, direction=Horizontal,
                Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=5.00)
                Window(id=1, parent=ContainerId(0), x=3.33, y=0.00, w=3.33, h=5.00)
                Window(id=2, parent=ContainerId(0), x=6.67, y=0.00, w=3.33, h=5.00)
              )
              Window(id=3, parent=ContainerId(1), x=0.00, y=5.00, w=10.00, h=5.00)
            )
          )
        )
        ");
    }

    #[test]
    fn insert_to_parent_when_focused_container_window_insert_direction_differ_but_has_parent() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        // Creating [w0, [w1, w2], w3]
        hub.insert_window();
        hub.insert_window();
        hub.insert_window();

        hub.focus_left();
        hub.toggle_new_window_direction();
        hub.insert_window();

        hub.focus_parent();
        hub.toggle_new_window_direction();

        // Should be inserted in the root container
        hub.insert_window();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(4),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=2.50, h=10.00)
              Container(id=1, parent=ContainerId(0), x=2.50, y=0.00, w=2.50, h=10.00, direction=Vertical,
                Window(id=1, parent=ContainerId(1), x=2.50, y=0.00, w=2.50, h=5.00)
                Window(id=3, parent=ContainerId(1), x=2.50, y=5.00, w=2.50, h=5.00)
              )
              Window(id=4, parent=ContainerId(0), x=5.00, y=0.00, w=2.50, h=10.00)
              Window(id=2, parent=ContainerId(0), x=7.50, y=0.00, w=2.50, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn clean_up_parent_container_when_only_child_is_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        let w1 = hub.insert_window();
        hub.insert_window();

        // Create new child container
        hub.toggle_new_window_direction();
        hub.insert_window();

        hub.focus_parent();
        hub.toggle_new_window_direction();

        // Should be inserted in the root container
        let w4 = hub.insert_window();
        hub.delete_window(w1);
        hub.delete_window(w4);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(2),
            Container(id=1, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Window(id=1, parent=ContainerId(1), x=0.00, y=0.00, w=10.00, h=5.00)
              Window(id=2, parent=ContainerId(1), x=0.00, y=5.00, w=10.00, h=5.00)
            )
          )
        )
        ");
    }

    #[test]
    fn delete_focused_window_change_focus_to_previous_window() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        let w2 = hub.insert_window();
        hub.insert_window();
        hub.focus_left();

        hub.delete_window(w2);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(0),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Window(id=2, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn delete_focused_window_change_focus_to_next_window() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        let w1 = hub.insert_window();
        hub.insert_window();
        hub.focus_left();

        hub.delete_window(w1);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Window(id=1, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00)
          )
        )
        ");
    }

    #[test]
    fn delete_focused_window_focus_last_window_of_preceding_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_parent();
        hub.toggle_new_window_direction();
        let w3 = hub.insert_window();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(2),
            Container(id=1, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Container(id=0, parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
                Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=5.00)
                Window(id=1, parent=ContainerId(0), x=0.00, y=5.00, w=5.00, h=5.00)
              )
              Window(id=2, parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=10.00)
            )
          )
        )
        ");

        hub.delete_window(w3);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
              Window(id=1, parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
            )
          )
        )
        ");
    }

    #[test]
    fn delete_focused_window_focus_first_window_of_following_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        let w1 = hub.insert_window();
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_left();
        hub.focus_left();

        hub.delete_window(w1);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=1, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Window(id=1, parent=ContainerId(1), x=0.00, y=0.00, w=10.00, h=5.00)
              Window(id=2, parent=ContainerId(1), x=0.00, y=5.00, w=10.00, h=5.00)
            )
          )
        )
        ");
    }

    #[test]
    fn delete_window_when_parent_focused_gives_focus_to_last_child() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        let w1 = hub.insert_window();
        hub.insert_window();
        hub.focus_parent();

        hub.delete_window(w1);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Window(id=1, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00)
          )
        )
        ");
    }

    // TODO: test unfocus then insert new window

    #[test]
    fn container_replaced_by_child_keeps_position_in_parent() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 12.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        // Create: [w0] [w1, w2] [w3]
        hub.insert_window();
        let w1 = hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_parent();
        hub.toggle_new_window_direction();
        hub.insert_window();

        hub.delete_window(w1);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=12.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(3),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=4.00, h=10.00)
              Window(id=2, parent=ContainerId(0), x=4.00, y=0.00, w=4.00, h=10.00)
              Window(id=3, parent=ContainerId(0), x=8.00, y=0.00, w=4.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn focus_left_right_in_horizontal_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();
        hub.insert_window();

        hub.focus_left();
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
              Window(id=1, parent=ContainerId(0), x=3.33, y=0.00, w=3.33, h=10.00)
              Window(id=2, parent=ContainerId(0), x=6.67, y=0.00, w=3.33, h=10.00)
            )
          )
        )
        ");

        hub.focus_right();
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(2),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
              Window(id=1, parent=ContainerId(0), x=3.33, y=0.00, w=3.33, h=10.00)
              Window(id=2, parent=ContainerId(0), x=6.67, y=0.00, w=3.33, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn focus_up_down_in_vertical_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.insert_window();

        hub.focus_up();
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=3.33)
              Window(id=1, parent=ContainerId(0), x=0.00, y=3.33, w=10.00, h=3.33)
              Window(id=2, parent=ContainerId(0), x=0.00, y=6.67, w=10.00, h=3.33)
            )
          )
        )
        ");

        hub.focus_down();
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(2),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=3.33)
              Window(id=1, parent=ContainerId(0), x=0.00, y=3.33, w=10.00, h=3.33)
              Window(id=2, parent=ContainerId(0), x=0.00, y=6.67, w=10.00, h=3.33)
            )
          )
        )
        ");
    }

    #[test]
    fn focus_right_selects_first_child_of_next_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_up();
        hub.toggle_new_window_direction();
        hub.insert_window();

        // Focus w0
        hub.focus_left();

        // focus_right should select w2 (first child of first nested container)
        hub.focus_right();
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(3),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Container(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
                Container(id=2, parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=5.00, direction=Horizontal,
                  Window(id=1, parent=ContainerId(2), x=5.00, y=0.00, w=2.50, h=5.00)
                  Window(id=3, parent=ContainerId(2), x=7.50, y=0.00, w=2.50, h=5.00)
                )
                Window(id=2, parent=ContainerId(1), x=5.00, y=5.00, w=5.00, h=5.00)
              )
            )
          )
        )
        ");
    }

    #[test]
    fn focus_left_selects_last_child_of_previous_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        // Create: [w0, w1] [w2]
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_parent();
        hub.toggle_new_window_direction();
        hub.insert_window();

        // focus_left from w2 should select w1 (last child of previous container)
        hub.focus_left();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=1, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Container(id=0, parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
                Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=5.00)
                Window(id=1, parent=ContainerId(0), x=0.00, y=5.00, w=5.00, h=5.00)
              )
              Window(id=2, parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn focus_left_from_nested_container_goes_to_grandparent_previous() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        // Create: [w0, [w1, [w2, w3]]]
        hub.insert_window();
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();

        hub.focus_left();
        hub.focus_left();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(0),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Container(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
                Window(id=1, parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=5.00)
                Container(id=2, parent=ContainerId(1), x=5.00, y=5.00, w=5.00, h=5.00, direction=Horizontal,
                  Window(id=2, parent=ContainerId(2), x=5.00, y=5.00, w=2.50, h=5.00)
                  Window(id=3, parent=ContainerId(2), x=7.50, y=5.00, w=2.50, h=5.00)
                )
              )
            )
          )
        )
        ");
    }

    #[test]
    fn focus_down_from_nested_container_goes_to_grandparent_next() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        // Create: [[[w0, w1], w2], w3]
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_up();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_left();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_down();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Container(id=1, parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00, direction=Horizontal,
                Container(id=2, parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=5.00, direction=Vertical,
                  Window(id=0, parent=ContainerId(2), x=0.00, y=0.00, w=5.00, h=2.50)
                  Window(id=3, parent=ContainerId(2), x=0.00, y=2.50, w=5.00, h=2.50)
                )
                Window(id=2, parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=5.00)
              )
              Window(id=1, parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
            )
          )
        )
        ");
    }

    #[test]
    fn focus_right_from_last_child_goes_to_next_sibling_in_parent() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        // Create: [w0, w1] [w2]
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_parent();
        hub.toggle_new_window_direction();
        hub.insert_window();

        // Focus w1 (last in nested container)
        hub.focus_left();

        // focus_right from w1 should go to w2 (next sibling in parent)
        hub.focus_right();

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(2),
            Container(id=1, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Container(id=0, parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
                Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=5.00)
                Window(id=1, parent=ContainerId(0), x=0.00, y=5.00, w=5.00, h=5.00)
              )
              Window(id=2, parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn focus_down_into_horizontal_nested_container() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.insert_window();

        // Focus window 0 (top)
        hub.focus_up();
        hub.focus_up();

        hub.focus_down();
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
              Container(id=1, parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00, direction=Horizontal,
                Window(id=1, parent=ContainerId(1), x=0.00, y=5.00, w=3.33, h=5.00)
                Window(id=2, parent=ContainerId(1), x=3.33, y=5.00, w=3.33, h=5.00)
                Window(id=3, parent=ContainerId(1), x=6.67, y=5.00, w=3.33, h=5.00)
              )
            )
          )
        )
        ");
    }

    #[test]
    fn focus_left_at_boundary_does_nothing() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();
        hub.focus_left();
        hub.focus_left(); // Already at leftmost

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(0),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Window(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn focus_right_at_boundary_does_nothing() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.insert_window();
        hub.focus_right(); // Already at rightmost

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Window(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
            )
          )
        )
        ");
    }

    #[test]
    fn focus_up_at_boundary_does_nothing() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_up();
        hub.focus_up(); // Already at topmost

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(0),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
              Window(id=1, parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
            )
          )
        )
        ");
    }

    #[test]
    fn focus_down_at_boundary_does_nothing() {
        setup_logger();
        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);

        hub.insert_window();
        hub.toggle_new_window_direction();
        hub.insert_window();
        hub.focus_down(); // Already at bottommost

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0.00 y=0.00 w=10.00 h=10.00),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
              Window(id=1, parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
            )
          )
        )
        ");
    }

    fn snapshot(hub: &Hub) -> String {
        let mut s = format!(
            "Hub(focused={}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}),\n",
            hub.current_workspace().0,
            hub.screen.x,
            hub.screen.y,
            hub.screen.width,
            hub.screen.height
        );
        for (idx, workspace) in hub.workspaces.storage.iter().enumerate() {
            if let Some(ws) = workspace {
                let focused = if let Some(current) = ws.focused {
                    format!(", focused={}", current)
                } else {
                    String::new()
                };
                if ws.root().is_none() {
                    s.push_str(&format!(
                        "  Workspace(id={}, name={}{})\n",
                        idx, ws.name, focused
                    ));
                } else {
                    s.push_str(&format!(
                        "  Workspace(id={}, name={}{},\n",
                        idx, ws.name, focused
                    ));
                    fmt_child_str(hub, &mut s, ws.root().unwrap(), 2);
                    s.push_str("  )\n");
                }
            }
        }
        s.push_str(")\n");
        s
    }

    fn fmt_child_str(hub: &Hub, s: &mut String, child: Child, indent: usize) {
        let prefix = "  ".repeat(indent);
        match child {
            Child::Window(id) => {
                let w = hub.get_window(id);
                let dim = w.dimension();
                s.push_str(&format!(
                    "{}Window(id={}, parent={}, x={:.2}, y={:.2}, w={:.2}, h={:.2})\n",
                    prefix, id.0, w.parent, dim.x, dim.y, dim.width, dim.height
                ));
            }
            Child::Container(id) => {
                let c = hub.get_container(id);
                s.push_str(&format!(
                    "{}Container(id={}, parent={}, x={:.2}, y={:.2}, w={:.2}, h={:.2}, direction={:?},\n",
                    prefix,
                    id.0,
                    c.parent,
                    c.dimension.x,
                    c.dimension.y,
                    c.dimension.width,
                    c.dimension.height,
                    c.direction,
                ));
                for &child in c.children() {
                    fmt_child_str(hub, s, child, indent + 1);
                }
                s.push_str(&format!("{})\n", prefix));
            }
        }
    }

    fn setup_logger() {
        use tracing_subscriber::fmt::format::FmtSpan;
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_span_events(FmtSpan::ENTER)
            .try_init();
        std::panic::set_hook(Box::new(|panic_info| {
            let backtrace = backtrace::Backtrace::new();
            tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
        }));
    }
}
