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

    pub(crate) fn insert_window(&mut self) -> WindowId {
        let current_workspace = self.workspaces.get_mut(self.current);
        let focused_node = current_workspace.focused.or(current_workspace.root);

        let window_id = match focused_node {
            Some(node_id) => match node_id {
                // Push to existing container
                Child::Container(container_id) => {
                    let container = self.containers.get_mut(container_id);
                    let window_id = self
                        .windows
                        .allocate(Window::new(Parent::Container(container_id)));
                    container.push_window(window_id);
                    self.balance_container(container_id);
                    window_id
                }
                // Push to window's parent container. Create the parent container if necessary
                Child::Window(focused_window_id) => {
                    let focused_window = self.windows.get_mut(focused_window_id);
                    let container_id = match focused_window.parent {
                        Parent::Container(container_id) => container_id,
                        Parent::Workspace(workspace_id) => {
                            let workspace = self.workspaces.get_mut(workspace_id);
                            let screen = workspace.screen;
                            let container_id = self
                                .containers
                                .allocate(Container::new(Parent::Workspace(workspace_id), screen));
                            focused_window.parent = Parent::Container(container_id);
                            self.containers
                                .get_mut(container_id)
                                .push_window(focused_window_id);
                            workspace.root = Some(Child::Container(container_id));
                            container_id
                        }
                    };
                    let window_id = self
                        .windows
                        .allocate(Window::new(Parent::Container(container_id)));
                    self.containers.get_mut(container_id).push_window(window_id);
                    self.balance_container(container_id);
                    window_id
                }
            },
            None => {
                tracing::trace!("Inserting window in empty workspace");
                let window_id = self
                    .windows
                    .allocate(Window::new(Parent::Workspace(self.current)));
                // TODO: set window size to workspace's size
                current_workspace.root = Some(Child::Window(window_id));
                let screen = current_workspace.screen;
                self.windows.get_mut(window_id).dimension = screen;
                window_id
            }
        };

        self.workspaces.get_mut(self.current).focused = Some(Child::Window(window_id));
        window_id
    }

    pub(crate) fn delete_window(&mut self, id: WindowId) {
        let parent = self.windows.get(id).parent;
        self.windows.delete(id);
        match parent {
            Parent::Container(parent_id) => {
                let parent = self.containers.get_mut(parent_id);
                parent.remove_window(id);
                // Valid containers must have at least 2 children
                if parent.children.len() == 1 {
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
                            self.balance_container(grandparent);
                        }
                        Parent::Workspace(workspace) => {
                            let screen = self.workspaces.get(workspace).screen;
                            match parent_last_child {
                                Child::Window(window_id) => {
                                    self.windows.get_mut(window_id).dimension = screen
                                }
                                Child::Container(container_id) => self.adjust(container_id, screen),
                            }
                            self.workspaces.get_mut(workspace).root = Some(parent_last_child);
                        }
                    }
                } else {
                    self.balance_container(parent_id);
                }
            }
            Parent::Workspace(workspace_id) => {
                if workspace_id != self.current {
                    self.workspaces.delete(workspace_id);
                }
            }
        }
    }

    fn balance_container(&mut self, container_id: ContainerId) {
        let container = self.containers.get(container_id);
        self.adjust(container_id, container.dimension);
    }

    fn adjust(&mut self, container_id: ContainerId, dimension: Dimension) {
        tracing::debug!("Adjusting container {container_id} with dimension {dimension:?}");
        let (fixed_width, fixed_height) = self.query_fixed(Child::Container(container_id));
        let container = self.containers.get_mut(container_id);
        container.dimension = dimension;

        let available_width = dimension.width - fixed_width;
        let available_height = dimension.height - fixed_height;
        self.allocate_available_width(
            Child::Container(container_id),
            dimension.x,
            dimension.y,
            available_width,
            available_height,
        );
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

    fn allocate_available_width(
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
                        for child_id in container.children.clone() {
                            self.allocate_available_width(
                                child_id,
                                current_x,
                                y,
                                column_width,
                                available_height,
                            );
                            current_x += match child_id {
                                Child::Window(window) => self.windows.get(window).dimension.width,
                                Child::Container(container) => {
                                    self.containers.get(container).dimension.width
                                }
                            }
                        }
                    }
                    Direction::Vertical => {
                        // TODO: filter out fixed height windows/containers in width calculation
                        let row_height = available_height / container.children.len() as f32;
                        let mut current_y = y;
                        for child_id in container.children.clone() {
                            self.allocate_available_width(
                                child_id,
                                x,
                                current_y,
                                available_width,
                                row_height,
                            );
                            current_y += match child_id {
                                Child::Window(window) => self.windows.get(window).dimension.height,
                                Child::Container(container) => {
                                    self.containers.get(container).dimension.height
                                }
                            }
                        }
                    }
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
}

impl Node for Container {
    type Id = ContainerId;
}

impl Container {
    fn new(parent: Parent, dimension: Dimension) -> Self {
        Self {
            children: Vec::new(),
            parent,
            dimension,
            direction: Direction::default(),
        }
    }

    pub(crate) fn children(&self) -> &[Child] {
        &self.children
    }

    fn push_window(&mut self, window_id: WindowId) {
        self.children.push(Child::Window(window_id));
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

#[derive(Debug)]
pub(crate) struct Window {
    parent: Parent,
    dimension: Dimension,
}

impl Node for Window {
    type Id = WindowId;
}

impl Window {
    fn new(parent: Parent) -> Self {
        Self {
            parent,
            dimension: Dimension::default(),
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
            .expect("Node not found")
            .as_ref()
            .expect("Node was deleted")
    }

    fn get_mut(&mut self, id: T::Id) -> &mut T {
        self.storage
            .get_mut(id.get())
            .expect("Node not found")
            .as_mut()
            .expect("Node was deleted")
    }

    fn find(&self, f: impl Fn(&T) -> bool) -> Option<T::Id> {
        self.storage
            .iter()
            .position(|node| node.as_ref().is_some_and(&f))
            .map(T::Id::new)
    }
}

trait Node {
    type Id: NodeId;
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
        let screen = Dimension {
            x: 2.0,
            y: 1.0,
            width: 20.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);
        hub.insert_window();
        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=2 y=1 w=20 h=10),
          Workspace(id=0, name=0, focused=WindowId(0),
            Window(id=0, x=2, y=1, w=20, h=10)
          )
        )
        ");
    }

    #[test]
    fn split_window_evenly() {
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
        Hub(focused=0, screen=(x=2 y=1 w=20 h=10),
          Workspace(id=0, name=0, focused=WindowId(3),
            Container(id=0, x=2, y=1, w=20, h=10,
              Window(id=0, x=2, y=1, w=5, h=10)
              Window(id=1, x=7, y=1, w=5, h=10)
              Window(id=2, x=12, y=1, w=5, h=10)
              Window(id=3, x=17, y=1, w=5, h=10)
            )
          )
        )
        ");
    }

    #[test]
    fn delete_window_removes_from_container() {
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
        Hub(focused=0, screen=(x=0 y=0 w=12 h=10),
          Workspace(id=0, name=0, focused=WindowId(2),
            Container(id=0, x=0, y=0, w=12, h=10,
              Window(id=0, x=0, y=0, w=6, h=10)
              Window(id=2, x=6, y=0, w=6, h=10)
            )
          )
        )
        ");
    }

    #[test]
    fn delete_window_removes_parent_container() {
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
        Hub(focused=0, screen=(x=0 y=0 w=10 h=10),
          Workspace(id=0, name=0, focused=WindowId(1),
            Container(id=0, x=0, y=0, w=10, h=10,
              Window(id=0, x=0, y=0, w=5, h=10)
              Window(id=1, x=5, y=0, w=5, h=10)
            )
          )
        )
        ");

        hub.delete_window(w2);

        assert_snapshot!(snapshot(&hub), @r"
        Hub(focused=0, screen=(x=0 y=0 w=10 h=10),
          Workspace(id=0, name=0, focused=WindowId(1),
            Window(id=0, x=0, y=0, w=10, h=10)
          )
        )
        ");
    }

    #[test]
    fn switch_workspace_attaches_windows_correctly() {
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
        Hub(focused=0, screen=(x=0 y=0 w=12 h=10),
          Workspace(id=0, name=0, focused=WindowId(4),
            Container(id=0, x=0, y=0, w=12, h=10,
              Window(id=0, x=0, y=0, w=4, h=10)
              Window(id=1, x=4, y=0, w=4, h=10)
              Window(id=4, x=8, y=0, w=4, h=10)
            )
          )
          Workspace(id=1, name=1, focused=WindowId(3),
            Container(id=1, x=0, y=0, w=12, h=10,
              Window(id=2, x=0, y=0, w=6, h=10)
              Window(id=3, x=6, y=0, w=6, h=10)
            )
          )
        )
        ");
    }

    // TODO: test unfocus then insert new window
    // TODO: test cleanup workspace + delete all without clean up

    fn snapshot(hub: &Hub) -> String {
        let mut s = format!(
            "Hub(focused={}, screen=(x={} y={} w={} h={}),\n",
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
                    "{}Window(id={}, x={}, y={}, w={}, h={})\n",
                    prefix, id.0, dim.x, dim.y, dim.width, dim.height
                ));
            }
            Child::Container(id) => {
                let c = hub.get_container(id);
                s.push_str(&format!(
                    "{}Container(id={}, x={}, y={}, w={}, h={},\n",
                    prefix,
                    id.0,
                    c.dimension.x,
                    c.dimension.y,
                    c.dimension.width,
                    c.dimension.height
                ));
                for &child in c.children() {
                    fmt_child_str(hub, s, child, indent + 1);
                }
                s.push_str(&format!("{})\n", prefix));
            }
        }
    }
}
