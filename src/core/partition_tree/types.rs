use crate::core::allocator::Node;
use crate::core::node::{ContainerId, Dimension, Direction, WindowId, WorkspaceId};

/// Contain the windows
/// Must maintain these invariants:
/// 1. All containers must have at least 2 children.
/// 2. Parent container and child container must differ in direction, unless one of them are tabbed
/// 3. A container's focus must either match a child's focus or point directly to a child.
///    Because `set_focus_child` writes the same target to every ancestor, all containers on
///    the path from root to the focused node share the same `focused` value.
#[derive(Debug, Clone)]
pub(crate) struct Container {
    pub(super) parent: Parent,
    pub(super) workspace: WorkspaceId,
    pub(super) children: Vec<Child>,
    /// The last focused node in this subtree. Can be a `Child::Window` or
    /// `Child::Container` (e.g. after `focus_parent`). Not the immediate child --
    /// `set_focus_child` writes the same target to every ancestor container.
    /// Walking `container.focused` from root reaches the focused node directly
    /// without needing to recurse through intermediate containers.
    pub(super) focused: Child,
    pub(super) dimension: Dimension,
    direction: Direction,
    // Don't allow directly set spawn_mode, otherwise that spawn mode will carry over other
    // spawn mode history
    spawn_mode: SpawnMode,
    pub(super) is_tabbed: bool,
    pub(super) active_tab_index: usize,
    pub(super) min_width: f32,
    pub(super) min_height: f32,
}

impl Node for Container {
    type Id = ContainerId;
}

impl Container {
    pub(super) fn split(
        parent: Parent,
        workspace: WorkspaceId,
        children: Vec<Child>,
        focused: Child,
        dimension: Dimension,
        direction: Direction,
    ) -> Self {
        let spawn_mode = match direction {
            Direction::Horizontal => SpawnMode::horizontal(),
            Direction::Vertical => SpawnMode::vertical(),
        };
        Self {
            children,
            focused,
            parent,
            workspace,
            dimension,
            direction,
            spawn_mode,
            is_tabbed: false,
            active_tab_index: 0,
            min_width: 0.0,
            min_height: 0.0,
        }
    }

    pub(super) fn tabbed(
        parent: Parent,
        workspace: WorkspaceId,
        children: Vec<Child>,
        focused: Child,
        dimension: Dimension,
    ) -> Self {
        Self {
            children,
            focused,
            parent,
            workspace,
            dimension,
            direction: Direction::Horizontal,
            spawn_mode: SpawnMode::tabbed(),
            is_tabbed: true,
            active_tab_index: 0,
            min_width: 0.0,
            min_height: 0.0,
        }
    }

    pub(crate) fn is_tabbed(&self) -> bool {
        self.is_tabbed
    }

    pub(crate) fn active_tab_index(&self) -> usize {
        self.active_tab_index
    }

    pub(crate) fn active_tab(&self) -> Option<Child> {
        if self.is_tabbed {
            Some(self.children[self.active_tab_index])
        } else {
            None
        }
    }

    pub(crate) fn set_active_tab(&mut self, tab: Child) {
        if !self.is_tabbed {
            panic!("Calling set_active_tab on split container");
        }
        self.active_tab_index = self.children.iter().position(|c| *c == tab).unwrap();
    }

    pub(super) fn switch_tab(&mut self, forward: bool) -> Option<Child> {
        if !self.is_tabbed {
            return None;
        }
        let len = self.children.len();
        let current = self.active_tab_index;
        let new_tab = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        self.active_tab_index = new_tab;
        Some(self.children[new_tab])
    }

    pub(super) fn set_active_tab_by_index(&mut self, index: usize) -> Option<Child> {
        if !self.is_tabbed || index >= self.children.len() {
            return None;
        }
        self.active_tab_index = index;
        Some(self.children[index])
    }

    pub(crate) fn children(&self) -> &[Child] {
        &self.children
    }

    pub(crate) fn min_size(&self) -> (f32, f32) {
        (self.min_width, self.min_height)
    }

    pub(super) fn direction(&self) -> Option<Direction> {
        if self.is_tabbed {
            None
        } else {
            Some(self.direction)
        }
    }

    pub(super) fn can_accomodate(&self, spawn_mode: SpawnMode) -> bool {
        spawn_mode
            .as_direction()
            .is_some_and(|d| self.has_direction(d))
            || (spawn_mode.is_tab() && self.is_tabbed())
    }

    pub(super) fn has_direction(&self, direction: Direction) -> bool {
        if self.is_tabbed {
            false
        } else {
            self.direction == direction
        }
    }

    pub(crate) fn spawn_mode(&self) -> SpawnMode {
        self.spawn_mode
    }

    // Reset spawn mode
    pub(super) fn set_spawn_mode(&mut self, spawn_mode: SpawnMode) {
        self.spawn_mode = SpawnMode::clean(spawn_mode)
    }

    /// Keep history
    pub(crate) fn switch_spawn_mode(&mut self, spawn_mode: SpawnMode) {
        self.spawn_mode = self.spawn_mode.switch_to(spawn_mode)
    }

    pub(super) fn position_of(&self, child: Child) -> usize {
        self.children.iter().position(|c| *c == child).unwrap()
    }

    pub(super) fn remove_child(&mut self, child: Child) {
        let pos = self.children.iter().position(|c| *c == child).unwrap();
        self.children.remove(pos);
        if self.is_tabbed && pos <= self.active_tab_index {
            self.active_tab_index = self.active_tab_index.saturating_sub(1);
        }
    }

    pub(super) fn replace_child(&mut self, old: Child, new: Child) {
        if let Some(pos) = self.children.iter().position(|c| *c == old) {
            self.children[pos] = new;
        }
    }

    pub(super) fn toggle_direction(&mut self) -> Direction {
        self.direction = match self.direction {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal,
        };
        self.direction
    }
}

/// After toggling spawn mode of one descendant to tab, all descendants of a tabbed container must
/// also have spawn mode of tabbed, except from descendants of type tabbed container. The same also
/// applies to when toggling spawn mode from tabbed to split.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpawnMode {
    current: SpawnState,
    previous: SpawnState,
}

impl SpawnMode {
    pub(crate) fn horizontal() -> Self {
        Self {
            current: SpawnState::Horizontal,
            previous: SpawnState::Horizontal,
        }
    }

    pub(crate) fn vertical() -> Self {
        Self {
            current: SpawnState::Vertical,
            previous: SpawnState::Vertical,
        }
    }

    pub(crate) fn tabbed() -> Self {
        Self {
            current: SpawnState::Tab,
            previous: SpawnState::Tab,
        }
    }

    pub(crate) fn from_direction(direction: Direction) -> Self {
        match direction {
            Direction::Horizontal => Self::horizontal(),
            Direction::Vertical => Self::vertical(),
        }
    }

    pub(crate) fn is_tab(&self) -> bool {
        self.current == SpawnState::Tab
    }

    pub(crate) fn is_horizontal(&self) -> bool {
        self.current == SpawnState::Horizontal
    }

    pub(crate) fn is_vertical(&self) -> bool {
        self.current == SpawnState::Vertical
    }

    pub(crate) fn as_direction(&self) -> Option<Direction> {
        match self.current {
            SpawnState::Horizontal => Some(Direction::Horizontal),
            SpawnState::Vertical => Some(Direction::Vertical),
            SpawnState::Tab => None,
        }
    }

    pub(crate) fn switch_to(&self, other: SpawnMode) -> Self {
        Self {
            current: other.current,
            previous: self.current,
        }
    }

    pub(crate) fn toggle(self) -> Self {
        use SpawnState::*;
        let next = match self.current {
            Horizontal => {
                if matches!(self.previous, Vertical) {
                    Tab
                } else {
                    Vertical
                }
            }
            Vertical => {
                if matches!(self.previous, Horizontal) {
                    Tab
                } else {
                    Horizontal
                }
            }
            Tab => match self.previous {
                Horizontal => Vertical,
                Vertical => Horizontal,
                Tab => Horizontal,
            },
        };
        Self {
            current: next,
            previous: self.current,
        }
    }

    pub(crate) fn clean(other: SpawnMode) -> Self {
        Self {
            current: other.current,
            previous: other.current,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpawnState {
    #[default]
    Horizontal,
    Vertical,
    Tab,
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
