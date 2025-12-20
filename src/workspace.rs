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
            Some(node_id) => {
                match node_id {
                    // Push to existing container
                    Child::Container(container_id) => {
                        let container = self.containers.get(container_id);
                        tracing::debug!("Inserting new window to {container:?}");
                        let parent = container.parent;
                        let dimension = container.dimension;
                        let new_window_direction = container.new_window_direction;
                        let direction = container.direction;
                        // Create new container if direction doesn't match
                        let container_id = if new_window_direction != direction {
                            tracing::debug!(
                                "Creating new parent container with direction {:?}",
                                new_window_direction
                            );
                            let new_container_id = self.containers.allocate(Container::new(
                                parent,
                                dimension,
                                new_window_direction,
                            ));
                            self.insert_parent(Child::Container(container_id), new_container_id);
                            new_container_id
                        } else {
                            container_id
                        };
                        let container = self.containers.get_mut(container_id);
                        let window_id = self.windows.allocate(Window::new(
                            Parent::Container(container_id),
                            container.direction,
                        ));
                        container.push_window(window_id);
                        window_id
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
                                    self.insert_parent(
                                        Child::Window(focused_window_id),
                                        new_container_id,
                                    );
                                    new_container_id
                                } else {
                                    container_id
                                }
                            }
                            Parent::Workspace(workspace_id) => {
                                let workspace = self.workspaces.get_mut(workspace_id);
                                let screen = workspace.screen;
                                let container_id = self.containers.allocate(Container::new(
                                    Parent::Workspace(workspace_id),
                                    screen,
                                    Direction::default(),
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
                        self.containers.get_mut(container_id).push_window(window_id);
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
        self.windows.delete(id);
        match parent {
            Parent::Container(parent_id) => {
                let parent = self.containers.get_mut(parent_id);
                parent.remove_window(id);
                // Valid containers must have at least 2 children
                let workspace = if parent.children.len() == 1 {
                    let grandparent = parent.parent;
                    let parent_last_child = parent.children.pop().unwrap();
                    match parent_last_child {
                        Child::Window(id) => self.windows.get_mut(id).parent = grandparent,

                        Child::Container(id) => self.containers.get_mut(id).parent = grandparent,
                    }
                    self.containers.delete(parent_id);
                    match grandparent {
                        Parent::Container(grandparent) => {
                            self.containers
                                .get_mut(grandparent)
                                .remove_container(parent_id);
                            self.containers
                                .get_mut(grandparent)
                                .children
                                .push(parent_last_child);
                            self.get_containing_workspace(Child::Container(grandparent))
                        }
                        Parent::Workspace(workspace) => {
                            self.workspaces.get_mut(workspace).root = Some(parent_last_child);
                            workspace
                        }
                    }
                } else {
                    self.get_containing_workspace(Child::Container(parent_id))
                };
                self.balance_workspace(workspace);
                workspace
            }
            Parent::Workspace(workspace_id) => {
                self.workspaces.get_mut(workspace_id).root = None;
                self.workspaces.get_mut(workspace_id).focused = None;

                if workspace_id != self.current {
                    self.workspaces.delete(workspace_id);
                }
                workspace_id
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_new_window_direction(&mut self) {
        tracing::info!("Toggling new window inserting direction for focused node");
        let current_workspace = self.workspaces.get_mut(self.current);
        if let Some(focused) = current_workspace.focused {
            match focused {
                Child::Container(container_id) => {
                    let container = self.containers.get_mut(container_id);
                    container.new_window_direction = match container.new_window_direction {
                        Direction::Horizontal => Direction::Vertical,
                        Direction::Vertical => Direction::Horizontal,
                    };
                }
                Child::Window(window_id) => {
                    let window = self.windows.get_mut(window_id);
                    window.new_window_direction = match window.new_window_direction {
                        Direction::Horizontal => Direction::Vertical,
                        Direction::Vertical => Direction::Horizontal,
                    };
                }
            }
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

    #[tracing::instrument(skip(self))]
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

    fn insert_parent(&mut self, child: Child, new_parent: ContainerId) {
        let current_parent = match child {
            Child::Window(window_id) => self.windows.get(window_id).parent,
            Child::Container(container_id) => self.containers.get(container_id).parent,
        };

        match current_parent {
            Parent::Container(parent_id) => {
                let grandparent = self.containers.get_mut(parent_id);
                grandparent.children.retain(|c| *c != child);
                grandparent.push_container(new_parent);
            }
            Parent::Workspace(workspace_id) => {
                self.workspaces.get_mut(workspace_id).root = Some(Child::Container(new_parent));
            }
        }

        match child {
            Child::Window(window_id) => {
                self.windows.get_mut(window_id).parent = Parent::Container(new_parent)
            }
            Child::Container(container_id) => {
                self.containers.get_mut(container_id).parent = Parent::Container(new_parent)
            }
        }

        self.containers.get_mut(new_parent).parent = current_parent;
        self.containers.get_mut(new_parent).children.push(child);
    }

    #[tracing::instrument(skip(self))]
    fn get_containing_workspace(&self, child: Child) -> WorkspaceId {
        let mut parent = match child {
            Child::Window(window_id) => self.windows.get(window_id).parent,
            Child::Container(container_id) => self.containers.get(container_id).parent,
        };
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

    fn push_container(&mut self, container_id: ContainerId) {
        self.children.push(Child::Container(container_id));
    }

    fn remove_window(&mut self, window_id: WindowId) {
        self.children
            .retain(|child| !child.is_window_and(|id| id == window_id));
    }

    fn remove_container(&mut self, container_id: ContainerId) {
        self.children
            .retain(|child| !child.is_container_and(|id| id == container_id));
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Direction {
    #[default]
    Horizontal,
    Vertical,
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

    fn is_container_and(&self, f: impl Fn(ContainerId) -> bool) -> bool {
        if let Child::Container(id) = self {
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
          Workspace(id=0, name=0, focused=WindowId(1),
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
          Workspace(id=0, name=0, focused=WindowId(2),
            Container(id=0, parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
              Window(id=0, parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
              Window(id=1, parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
            )
          )
        )
        ");
    }

    // TODO: test unfocus then insert new window
    // TODO: test cleanup workspace + delete all without clean up

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
