use crate::core::allocator::{Node, NodeId};

#[derive(Debug, Clone)]
pub(crate) struct Workspace {
    pub(super) name: usize,
    pub(super) screen: Dimension,
    // TODO: Add list of float windows
    pub(super) root: Option<Child>,
    pub(super) focused: Option<Child>,
}

impl Node for Workspace {
    type Id = WorkspaceId;
}

impl Workspace {
    pub(super) fn new(screen: Dimension, name: usize) -> Self {
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

/// Contain the windows, dimension including borders
#[derive(Debug, Clone)]
pub(crate) struct Container {
    pub(super) parent: Parent,
    pub(super) children: Vec<Child>,
    pub(super) dimension: Dimension,
    pub(super) direction: Direction,
    pub(super) new_window_direction: Direction,
}

impl Node for Container {
    type Id = ContainerId;
}

impl Container {
    pub(super) fn new(parent: Parent, dimension: Dimension, direction: Direction) -> Self {
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

    pub(crate) fn dimension(&self) -> Dimension {
        self.dimension
    }

    pub(crate) fn new_window_direction(&self) -> Direction {
        self.new_window_direction
    }

    pub(super) fn push_window(&mut self, window_id: WindowId) {
        self.children.push(Child::Window(window_id));
    }

    pub(super) fn push_child(&mut self, child: Child) {
        self.children.push(child);
    }

    pub(super) fn insert_window_after(&mut self, window_id: WindowId, after: Child) {
        if let Some(pos) = self.children.iter().position(|c| *c == after) {
            self.children.insert(pos + 1, Child::Window(window_id));
        } else {
            self.children.push(Child::Window(window_id));
        }
    }

    pub(super) fn insert_after(&mut self, child: Child, after: Child) {
        if let Some(pos) = self.children.iter().position(|c| *c == after) {
            self.children.insert(pos + 1, child);
        } else {
            self.children.push(child);
        }
    }

    pub(super) fn remove_child(&mut self, child: Child) {
        self.children.retain(|c| *c != child);
    }

    pub(super) fn replace_child(&mut self, old: Child, new: Child) {
        if let Some(pos) = self.children.iter().position(|c| *c == old) {
            self.children[pos] = new;
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
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
pub(crate) enum Parent {
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

/// Represents a single application window, dimension doesn't account for border
#[derive(Debug, Clone)]
pub(crate) struct Window {
    pub(super) parent: Parent,
    pub(super) dimension: Dimension,
    pub(super) new_window_direction: Direction,
}

impl Node for Window {
    type Id = WindowId;
}

impl Window {
    pub(super) fn new(parent: Parent, new_window_direction: Direction) -> Self {
        Self {
            parent,
            dimension: Dimension::default(),
            new_window_direction,
        }
    }

    pub(crate) fn dimension(&self) -> Dimension {
        self.dimension
    }

    pub(crate) fn new_window_direction(&self) -> Direction {
        self.new_window_direction
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct WindowId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ContainerId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
