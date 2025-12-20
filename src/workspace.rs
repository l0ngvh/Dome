#[derive(Debug)]
pub(crate) struct Hub {
    screen: Screen,
    current: WorkspaceId,

    workspaces: Allocator<Workspace>,
    windows: Allocator<Window>,
    containers: Allocator<Container>,
}

impl Hub {
    pub(crate) fn new(screen: Screen) -> Self {
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
        let focused_node = self
            .workspaces
            .get(self.current)
            .current
            .or(self.workspaces.get(self.current).root);

        let window_id = match focused_node {
            Some(node_id) => match node_id {
                // Push to existing container
                Child::Container(container_id) => {
                    let window_id = self
                        .windows
                        .allocate(Window::new(Parent::Container(container_id)));
                    self.containers.get_mut(container_id).push_window(window_id);
                    self.balance_container(container_id);
                    window_id
                }
                // Push to window's parent container. Create the parent container if necessary
                Child::Window(root_window) => {
                    let parent = self.windows.get_mut(root_window).parent;
                    let container_id = match parent {
                        Parent::Container(container_id) => container_id,
                        Parent::Workspace(workspace_id) => {
                            let container_id = self
                                .containers
                                .allocate(Container::new(Parent::Workspace(workspace_id)));
                            self.windows.get_mut(root_window).parent =
                                Parent::Container(container_id);
                            self.containers
                                .get_mut(container_id)
                                .push_window(root_window);
                            self.workspaces.get_mut(workspace_id).root =
                                Some(Child::Container(container_id));
                            let screen = self.workspaces.get(workspace_id).screen;
                            self.set_position(Child::Container(container_id), screen.x, screen.y);
                            self.set_size(
                                Child::Container(container_id),
                                screen.width,
                                screen.height,
                            );
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
                self.workspaces.get_mut(self.current).root = Some(Child::Window(window_id));
                let screen = self.workspaces.get(self.current).screen;
                self.set_size(Child::Window(window_id), screen.width, screen.height);
                self.set_position(Child::Window(window_id), screen.x, screen.y);
                window_id
            }
        };

        self.focus(Child::Window(window_id));
        window_id
    }

    pub(crate) fn delete_window(&mut self, id: WindowId) {
        let parent = self.windows.get(id).parent;
        self.windows.delete(id);
        match parent {
            Parent::Container(container_id) => {
                self.containers.get_mut(container_id).remove_window(id);
                // Balance, containers must have at least 2 children
                if self.containers.get(container_id).children.len() == 1 {
                    let parent = self.containers.get(container_id).parent;
                    let child = self
                        .containers
                        .get_mut(container_id)
                        .children
                        .pop()
                        .unwrap();
                    match child {
                        Child::Window(id) => self.windows.get_mut(id).parent = parent,

                        Child::Container(id) => self.containers.get_mut(id).parent = parent,
                    }
                    self.containers.delete(container_id);
                    match parent {
                        Parent::Container(parent) => {
                            self.containers
                                .get_mut(parent)
                                .remove_container(container_id);
                            self.balance_container(parent);
                        }
                        Parent::Workspace(workspace) => {
                            let screen = self.workspaces.get(workspace).screen;
                            self.set_size(child, screen.width, screen.height);
                            self.set_position(child, screen.x, screen.y);
                            self.workspaces.get_mut(workspace).root = Some(child);
                        }
                    }
                } else {
                    self.balance_container(container_id);
                }
            }
            Parent::Workspace(workspace_id) => {
                if workspace_id != self.current {
                    self.workspaces.delete(workspace_id);
                }
            }
        }
    }

    fn focus(&mut self, child: Child) {
        // TODO: Unfocused the last container/window
        self.workspaces.get_mut(self.current).current = Some(child)
    }

    fn balance_container(&mut self, container: ContainerId) {
        let container = self.containers.get(container);
        let nodes = &container.children;
        let node_count = nodes.len() as f32;
        match container.direction {
            Direction::Horizontal => {
                let column_width = container.width / node_count;
                let container_x = container.x;
                let container_y = container.y;
                let container_height = container.height;
                for (i, node_id) in nodes.clone().into_iter().enumerate() {
                    // TODO: Might revisit this when resize mode is implemented. Depend on when
                    // balance is user's intention or just container's dynamic tiling behavior
                    // (i.e. resize adjacent containers)
                    let x = container_x + (i as f32 * column_width);

                    self.set_position(node_id, x, container_y);
                    self.set_size(node_id, column_width, container_height);
                }
            }
            Direction::Vertical => todo!(),
        }
    }

    fn set_position(&mut self, child: Child, x: f32, y: f32) {
        match child {
            Child::Window(window) => self.windows.get_mut(window).set_position(x, y),
            Child::Container(container) => {
                self.containers.get_mut(container).x = x;
                self.containers.get_mut(container).y = y;

                match self.containers.get(container).direction {
                    Direction::Horizontal => {
                        let mut current_x = x;
                        for child_id in self.containers.get(container).children.clone() {
                            self.set_position(child_id, current_x, y);
                            current_x += match child_id {
                                Child::Window(window) => self.windows.get(window).width,
                                Child::Container(container) => self.containers.get(container).width,
                            }
                        }
                    }
                    Direction::Vertical => {
                        let mut current_y = y;
                        for child_id in self.containers.get(container).children.clone() {
                            self.set_position(child_id, x, current_y);
                            current_y += match child_id {
                                Child::Window(window) => self.windows.get(window).height,
                                Child::Container(container) => {
                                    self.containers.get(container).height
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[tracing::instrument]
    fn set_size(&mut self, child: Child, width: f32, height: f32) {
        match child {
            Child::Window(window) => self.windows.get_mut(window).set_size(width, height),
            Child::Container(container) => {
                self.containers.get_mut(container).width = width;
                self.containers.get_mut(container).height = height;

                let children = self.containers.get(container).children.clone();
                let child_count = children.len() as f32;

                match self.containers.get(container).direction {
                    Direction::Horizontal => {
                        let child_width = width / child_count;
                        for child in children {
                            self.set_size(child, child_width, height);
                        }
                    }
                    Direction::Vertical => {
                        let child_height = height / child_count;
                        for child in children {
                            self.set_size(child, width, child_height);
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
    screen: Screen,
    // TODO: Add list of float windows
    root: Option<Child>,
    current: Option<Child>,
}

impl Node for Workspace {
    type Id = WorkspaceId;
}

impl Workspace {
    fn new(screen: Screen, name: usize) -> Self {
        Self {
            root: None,
            current: None,
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
    width: f32,
    height: f32,
    x: f32,
    y: f32,
    direction: Direction,
}

impl Node for Container {
    type Id = ContainerId;
}

impl Container {
    fn new(parent: Parent) -> Self {
        Self {
            children: Vec::new(),
            parent,
            width: 0.0,
            height: 0.0,
            x: 0.0,
            y: 0.0,
            direction: Direction::Horizontal,
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

#[derive(Debug)]
enum Direction {
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
    width: f32,
    height: f32,
    x: f32,
    y: f32,
}

impl Node for Window {
    type Id = WindowId;
}

impl Window {
    fn new(parent: Parent) -> Self {
        Self {
            parent,
            width: 0.0,
            height: 0.0,
            x: 0.0,
            y: 0.0,
        }
    }

    pub(crate) fn x(&self) -> f32 {
        self.x
    }

    pub(crate) fn y(&self) -> f32 {
        self.y
    }

    pub(crate) fn width(&self) -> f32 {
        self.width
    }

    pub(crate) fn height(&self) -> f32 {
        self.height
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn set_size(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Screen {
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
    use crate::workspace::{Child, Hub, Screen};
    use insta::assert_snapshot;

    #[test]
    fn initial_window_cover_full_screen() {
        let screen = Screen {
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
        let screen = Screen {
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
        let screen = Screen {
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
        let screen = Screen {
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
        let screen = Screen {
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
                let focused = if let Some(current) = ws.current {
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
                s.push_str(&format!(
                    "{}Window(id={}, x={}, y={}, w={}, h={})\n",
                    prefix,
                    id.0,
                    w.x(),
                    w.y(),
                    w.width(),
                    w.height()
                ));
            }
            Child::Container(id) => {
                let c = hub.get_container(id);
                s.push_str(&format!(
                    "{}Container(id={}, x={}, y={}, w={}, h={},\n",
                    prefix, id.0, c.x, c.y, c.width, c.height
                ));
                for &child in c.children() {
                    fmt_child_str(hub, s, child, indent + 1);
                }
                s.push_str(&format!("{})\n", prefix));
            }
        }
    }
}
