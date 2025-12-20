use anyhow::Result;

use crate::window::OsWindow;

type NodeId = usize;

#[derive(Debug)]
enum Node {
    Workspace(Workspace),
    Container(Container),
    Window(Window),
    Tombstone,
}

#[derive(Debug)]
struct Container {
    parent: Parent,
    children: Vec<Child>,
    width: f32,
    height: f32,
    x: f32,
    y: f32,
    direction: Direction,
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
}

#[derive(Debug)]
enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy)]
enum Parent {
    Container(NodeId),
    Workspace(NodeId),
}

#[derive(Debug)]
struct Window {
    inner: OsWindow,
    parent: Parent,
    width: f32,
    height: f32,
    x: f32,
    y: f32,
}

impl Window {
    fn new(inner: OsWindow, parent: Parent) -> Self {
        Self {
            inner,
            parent,
            width: 0.0,
            height: 0.0,
            x: 0.0,
            y: 0.0,
        }
    }

    fn set_position(&mut self, x: f32, y: f32) -> Result<()> {
        self.x = x;
        self.y = y;
        self.inner.set_position(x, y)
    }

    fn set_size(&mut self, width: f32, height: f32) -> Result<()> {
        self.width = width;
        self.height = height;
        self.inner.set_size(width, height)
    }

    fn hide(&mut self) -> Result<()> {
        self.inner.hide()
    }

    fn show(&mut self) -> Result<()> {
        self.inner.show()
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
enum Child {
    Window(NodeId),
    Container(NodeId),
}

impl Child {
    fn is_window_and(&self, f: impl Fn(NodeId) -> bool) -> bool {
        if let Child::Window(id) = self {
            f(*id)
        } else {
            false
        }
    }
}

#[derive(Debug)]
struct Workspace {
    name: usize,
    screen: Screen,
    // TODO: Add list of float windows
    root: Option<Child>,
    current: Option<Child>,
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
}

#[derive(Debug)]
pub(crate) struct Hub {
    screen: Screen,
    current: usize,

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
    // TODO: Criteria
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

        if let Some(node) = self.workspaces.get(self.current).root {
            self.hide(node);
        }

        if let Some(node) = self.workspaces.get(workspace_id).root {
            self.show(node);
        }

        self.current = workspace_id
    }

    pub(crate) fn insert_window(&mut self, os_window: OsWindow) -> NodeId {
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
                        .allocate(Window::new(os_window, Parent::Container(container_id)));
                    self.containers
                        .get_mut(container_id)
                        .children
                        .push(Child::Window(window_id));
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
                                .children
                                .push(Child::Window(root_window));
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
                        .allocate(Window::new(os_window, Parent::Container(container_id)));
                    self.containers
                        .get_mut(container_id)
                        .children
                        .push(Child::Window(window_id));
                    self.balance_container(container_id);
                    window_id
                }
            },
            None => {
                tracing::trace!("Inserting window in empty workspace");
                let window_id = self
                    .windows
                    .allocate(Window::new(os_window, Parent::Workspace(self.current)));
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

    pub(crate) fn delete_window(&mut self, id: NodeId) {
        let parent = self.windows.get(id).parent;
        self.windows.delete(id);
        match parent {
            Parent::Container(container_id) => {
                self.containers
                    .get_mut(container_id)
                    .children
                    .retain(|child| !child.is_window_and(|child| id == child));
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
                        Parent::Container(container) => self.balance_container(container),
                        Parent::Workspace(workspace) => {
                            let screen = self.workspaces.get(workspace).screen;
                            self.set_size(child, screen.width, screen.height);
                            self.set_position(child, screen.x, screen.y);
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

    fn balance_container(&mut self, container: NodeId) {
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

                    if let Err(e) = self.set_position(node_id, x, container_y) {
                        tracing::info!("Failed to set position for node {node_id:?}: {e:#}")
                    }
                    if let Err(e) = self.set_size(node_id, column_width, container_height) {
                        tracing::info!("Failed to set size for window {node_id:?}: {e:#}")
                    }
                }
            }
            Direction::Vertical => todo!(),
        }
    }

    fn set_position(&mut self, child: Child, x: f32, y: f32) -> Result<()> {
        match child {
            Child::Window(window) => self.windows.get_mut(window).set_position(x, y),
            Child::Container(container) => {
                self.containers.get_mut(container).x = x;
                self.containers.get_mut(container).y = y;

                match self.containers.get(container).direction {
                    Direction::Horizontal => {
                        let mut current_x = x;
                        for child_id in self.containers.get(container).children.clone() {
                            self.set_position(child_id, current_x, y)?;
                            current_x += match child_id {
                                Child::Window(window) => self.windows.get(window).width,
                                Child::Container(container) => self.containers.get(container).width,
                            }
                        }
                    }
                    Direction::Vertical => {
                        let mut current_y = y;
                        for child_id in self.containers.get(container).children.clone() {
                            self.set_position(child_id, x, current_y)?;
                            current_y += match child_id {
                                Child::Window(window) => self.windows.get(window).height,
                                Child::Container(container) => {
                                    self.containers.get(container).height
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }

    #[tracing::instrument]
    fn set_size(&mut self, child: Child, width: f32, height: f32) -> Result<()> {
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
                            self.set_size(child, child_width, height)?;
                        }
                    }
                    Direction::Vertical => {
                        let child_height = height / child_count;
                        for child in children {
                            self.set_size(child, width, child_height)?;
                        }
                    }
                }
                Ok(())
            }
        }
    }

    // Low level hide, don't resize adjacent nodes
    fn hide(&mut self, node_id: Child) -> Result<()> {
        match node_id {
            Child::Window(window_id) => self.windows.get_mut(window_id).hide(),
            Child::Container(container_id) => {
                for child in self.containers.get(container_id).children.clone() {
                    self.hide(child);
                }
                Ok(())
            }
        }
    }

    // Low level show, don't resize adjacent nodes
    fn show(&mut self, node_id: Child) -> Result<()> {
        match node_id {
            Child::Window(window_id) => self.windows.get_mut(window_id).show(),
            Child::Container(container_id) => {
                for child in self.containers.get(container_id).children.clone() {
                    self.show(child);
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug)]
struct Allocator<T> {
    storage: Vec<Option<T>>,
    free_list: Vec<NodeId>,
}

impl<T: std::fmt::Debug> Allocator<T> {
    fn new() -> Self {
        Self {
            storage: Vec::new(),
            free_list: Vec::new(),
        }
    }

    fn allocate(&mut self, node: T) -> NodeId {
        if let Some(free) = self.free_list.pop() {
            self.storage[free] = Some(node);
            free
        } else {
            let id = self.storage.len();
            self.storage.push(Some(node));
            id
        }
    }

    fn delete(&mut self, id: NodeId) {
        if let Some(slot) = self.storage.get_mut(id) {
            *slot = None;
            self.free_list.push(id);
        }
    }

    fn get(&self, id: NodeId) -> &T {
        self.storage
            .get(id)
            // Safety: The assumption is that all nodes must be valid. If any of these happens it's
            // undefined behavior
            .expect("Node not found {id}")
            .as_ref()
            .expect("Node was deleted {id}")
    }

    fn get_mut(&mut self, id: NodeId) -> &mut T {
        self.storage
            .get_mut(id)
            // Safety: The assumption is that all nodes must be valid. If any of these happens it's
            // undefined behavior
            .expect("Node not found {id}")
            .as_mut()
            .expect("Node was deleted {id}")
    }

    fn find(&self, f: impl Fn(&T) -> bool) -> Option<NodeId> {
        self.storage
            .iter()
            .position(|node| node.as_ref().is_some_and(&f))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        window::MockWindow,
        workspace::{Child, Hub, Screen, Workspace},
    };

    #[test]
    fn focus_default_workspace() {
        let screen = Screen {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
        let hub = Hub::new(screen);
        assert_eq!(hub.current, hub.workspaces.find(|w| w.name == 0).unwrap());
    }

    #[test]
    fn focus_inserted_window() {
        let screen = Screen {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);
        let window_id = hub.insert_window(MockWindow::default());
        assert_eq!(get_workspace(&hub, 0).root, Some(Child::Window(window_id)));
        assert_eq!(
            get_workspace(&hub, 0).current,
            Some(Child::Window(window_id))
        );
    }

    #[test]
    fn initial_window_cover_full_screen() {
        let screen = Screen {
            x: 2.0,
            y: 1.0,
            width: 20.0,
            height: 10.0,
        };
        let mut hub = Hub::new(screen);
        let window_id = hub.insert_window(MockWindow::default());
        let window = hub.windows.get(window_id);
        assert_eq!(window.width, 20.0);
        assert_eq!(window.height, 10.0);
        assert_eq!(window.x, 2.0);
        assert_eq!(window.y, 1.0);
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

        let mut window_ids = Vec::new();
        for _ in 0..4 {
            window_ids.push(hub.insert_window(MockWindow::default()));
        }

        // Each window should have 1/4 of the screen width (20.0 / 4 = 5.0)
        for (i, &window_id) in window_ids.iter().enumerate() {
            let window = hub.windows.get(window_id);
            assert_eq!(window.width, 5.0);
            assert_eq!(window.height, 10.0);
            assert_eq!(window.x, 2.0 + (i as f32 * 5.0));
            assert_eq!(window.y, 1.0);
        }
    }

    #[test]
    fn delete_window_removes_from_container() {
        let screen = Screen { x: 0.0, y: 0.0, width: 12.0, height: 10.0 };
        let mut hub = Hub::new(screen);
        
        let w1 = hub.insert_window(MockWindow::default());
        let w2 = hub.insert_window(MockWindow::default());
        let w3 = hub.insert_window(MockWindow::default());
        
        hub.delete_window(w2);
        
        // Verify w2 is deleted
        assert!(hub.windows.storage.get(w2).unwrap().is_none());
        
        // Verify w1 and w3 still exist and are resized correctly (12.0 / 2 = 6.0)
        let window1 = hub.windows.get(w1);
        assert_eq!(window1.width, 6.0);
        assert_eq!(window1.height, 10.0);
        assert_eq!(window1.x, 0.0);
        assert_eq!(window1.y, 0.0);
        
        let window3 = hub.windows.get(w3);
        assert_eq!(window3.width, 6.0);
        assert_eq!(window3.height, 10.0);
        assert_eq!(window3.x, 6.0);
        assert_eq!(window3.y, 0.0);
    }

    // TODO: test unfocus then insert new window

    fn get_workspace(hub: &Hub, name: usize) -> &Workspace {
        let id = hub.workspaces.find(|w| w.name == name).unwrap();
        hub.workspaces.get(id)
    }
}
