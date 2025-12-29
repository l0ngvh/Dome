use crate::core::allocator::{Node, NodeId};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub(crate) struct Workspace {
    pub(super) name: usize,
    pub(super) screen: Dimension,
    pub(super) root: Option<Child>,
    pub(super) focused: Option<Focus>,
    pub(super) float_windows: Vec<FloatWindowId>,
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
            float_windows: Vec::new(),
        }
    }

    pub(crate) fn root(&self) -> Option<Child> {
        self.root
    }

    pub(crate) fn focused(&self) -> Option<Focus> {
        self.focused
    }

    pub(crate) fn float_windows(&self) -> &[FloatWindowId] {
        &self.float_windows
    }
}

/// Contain the windows, dimension including borders
#[derive(Debug, Clone)]
pub(crate) struct Container {
    pub(super) parent: Parent,
    pub(super) workspace: WorkspaceId,
    pub(super) children: Vec<Child>,
    pub(super) focused: Child,
    pub(super) title: String,
    pub(super) dimension: Dimension,
    pub(super) direction: Direction,
    pub(super) spawn_direction: Direction,
    pub(super) is_tabbed: bool,
    pub(super) active_tab: usize,
    pub(super) focused_by: HashSet<ContainerId>,
}

impl Node for Container {
    type Id = ContainerId;
}

impl Container {
    pub(super) fn new(
        parent: Parent,
        workspace: WorkspaceId,
        children: Vec<Child>,
        focused: Child,
        title: String,
        dimension: Dimension,
        direction: Direction,
    ) -> Self {
        Self {
            children,
            focused,
            title,
            parent,
            workspace,
            dimension,
            direction,
            spawn_direction: direction,
            is_tabbed: false,
            active_tab: 0,
            focused_by: HashSet::new(),
        }
    }

    pub(crate) fn focused(&self) -> Child {
        self.focused
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn is_tabbed(&self) -> bool {
        self.is_tabbed
    }

    pub(crate) fn active_tab(&self) -> usize {
        self.active_tab
    }

    pub(crate) fn children(&self) -> &[Child] {
        &self.children
    }

    pub(crate) fn dimension(&self) -> Dimension {
        self.dimension
    }

    pub(crate) fn spawn_direction(&self) -> Direction {
        self.spawn_direction
    }

    pub(super) fn window_position(&self, window_id: WindowId) -> usize {
        self.children
            .iter()
            .position(|c| *c == Child::Window(window_id))
            .unwrap()
    }

    pub(super) fn container_position(&self, container_id: ContainerId) -> usize {
        self.children
            .iter()
            .position(|c| *c == Child::Container(container_id))
            .unwrap()
    }

    pub(super) fn remove_child(&mut self, child: Child) {
        if let Some(pos) = self.children.iter().position(|c| *c == child) {
            self.children.remove(pos);
            if self.active_tab > 0 && pos <= self.active_tab {
                self.active_tab -= 1;
            }
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub(super) workspace: WorkspaceId,
    pub(super) dimension: Dimension,
    pub(super) spawn_direction: Direction,
    pub(super) title: String,
    pub(super) focused_by: HashSet<ContainerId>,
}

impl Node for Window {
    type Id = WindowId;
}

impl Window {
    pub(super) fn new(
        parent: Parent,
        workspace: WorkspaceId,
        spawn_direction: Direction,
        title: String,
    ) -> Self {
        Self {
            parent,
            workspace,
            dimension: Dimension::default(),
            spawn_direction,
            title,
            focused_by: HashSet::new(),
        }
    }

    pub(crate) fn dimension(&self) -> Dimension {
        self.dimension
    }

    pub(crate) fn spawn_direction(&self) -> Direction {
        self.spawn_direction
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FloatWindow {
    pub(super) workspace: WorkspaceId,
    pub(super) dimension: Dimension,
    pub(super) title: String,
}

impl Node for FloatWindow {
    type Id = FloatWindowId;
}

impl FloatWindow {
    pub(super) fn new(workspace: WorkspaceId, dimension: Dimension, title: String) -> Self {
        Self {
            workspace,
            dimension,
            title,
        }
    }

    pub(crate) fn dimension(&self) -> Dimension {
        self.dimension
    }

    #[expect(unused)]
    pub(crate) fn title(&self) -> &str {
        &self.title
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Focus {
    Tiling(Child),
    Float(FloatWindowId),
}

impl Focus {
    pub(crate) fn window(id: WindowId) -> Self {
        Focus::Tiling(Child::Window(id))
    }
}

impl std::fmt::Display for Child {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Child::Window(id) => write!(f, "{}", id),
            Child::Container(id) => write!(f, "{}", id),
        }
    }
}

impl std::fmt::Display for Focus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Focus::Tiling(child) => write!(f, "{}", child),
            Focus::Float(id) => write!(f, "{}", id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct WindowId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FloatWindowId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ContainerId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct WorkspaceId(usize);

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WindowId({})", self.0)
    }
}

impl std::fmt::Display for FloatWindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FloatWindowId({})", self.0)
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

impl NodeId for FloatWindowId {
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
