use crate::core::allocator::Node;
use crate::core::node::{ContainerId, Dimension, Direction, Length, WindowId, WorkspaceId};

/// Effective per-child layout constraints in the tree's `Length` unit.
///
/// `Length::ZERO` on a `max_*` field means "unbounded" on that axis. This
/// matches the platform-side encoding of `Window::max_size` (zero means
/// "no max"). Containers always set both maxes to `Length::ZERO`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Constraints {
    pub(crate) min_width: Length,
    pub(crate) min_height: Length,
    pub(crate) max_width: Length,
    pub(crate) max_height: Length,
}

/// Internal node of the partition tree. Holds an ordered list of `Child`
/// nodes (windows or sub-containers) and either a split direction or the
/// tabbed flag.
///
/// Invariants:
/// 1. `children.len() >= 2`. Containers with one or zero children are
///    collapsed by `tree.rs` on detach.
/// 2. A non-tabbed container's `direction` differs from its non-tabbed
///    parent's direction. A tabbed container is exempt: `direction()`
///    returns `None` for it, so the alternation rule does not apply
///    across a tabbed boundary. `validate_container_direction`
///    (`validate.rs`) enforces this.
/// 3. Focus-chain invariant: every container on the path from workspace
///    root to the focused leaf has `focused` set to the same `Child`
///    value (the focused leaf, or a `Child::Container` after
///    `focus_parent`). See `WorkspaceTilingState::focused_tiling`
///    (`mod.rs`) for the workspace-level pointer that anchors this
///    chain.
#[derive(Debug, Clone)]
pub(crate) struct Container {
    pub(super) parent: Parent,
    pub(super) workspace: WorkspaceId,
    pub(super) children: Vec<Child>,
    /// The focused descendant per invariant 3 above. Not necessarily a
    /// direct child: in a chain `root -> A -> B -> W`, all of
    /// `root.focused`, `A.focused`, `B.focused` equal `Child::Window(W)`.
    pub(super) focused: Child,
    pub(super) dimension: Dimension,
    /// Split axis. Read through `direction()`, which returns `None` when
    /// `is_tabbed` is set. A value is stored while tabbed to keep the field
    /// initialised, but it is unused until the container converts back to split.
    direction: Direction,
    /// Spawn mode for new children inserted under this container. Mutate via
    /// `set_spawn_mode_reset` (drops history) or `set_spawn_mode_keep_history`
    /// (preserves history). Direct field write would lose the `H <-> V <-> Tab`
    /// rotation state.
    spawn_mode: SpawnMode,
    pub(super) is_tabbed: bool,
    pub(super) active_tab_index: usize,
    pub(super) min_width: Length,
    pub(super) min_height: Length,
}

impl Node for Container {
    type Id = ContainerId;
}

impl Container {
    /// Build a split container. `focused` must be one of `children` (or a
    /// descendant under one of them) so invariant 3 holds at construction.
    /// `direction` seeds `spawn_mode` to match.
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
            min_width: Length::ZERO,
            min_height: Length::ZERO,
        }
    }

    /// Build a tabbed container. Same `focused` precondition as `split`.
    /// `direction` is stored as `Horizontal` for layout fallback but is
    /// hidden by `direction()` while `is_tabbed` is set.
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
            min_width: Length::ZERO,
            min_height: Length::ZERO,
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

    pub(crate) fn set_active_tab_to_child(&mut self, child: Child) {
        if !self.is_tabbed {
            panic!("Calling set_active_tab_to_child on split container");
        }
        self.active_tab_index = self.children.iter().position(|c| *c == child).unwrap();
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

    pub(crate) fn min_size(&self) -> (Length, Length) {
        (self.min_width, self.min_height)
    }

    pub(super) fn direction(&self) -> Option<Direction> {
        if self.is_tabbed {
            None
        } else {
            Some(self.direction)
        }
    }

    pub(super) fn can_accommodate(&self, spawn_mode: SpawnMode) -> bool {
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

    pub(super) fn set_spawn_mode_reset(&mut self, spawn_mode: SpawnMode) {
        self.spawn_mode = SpawnMode::without_history(spawn_mode)
    }

    pub(crate) fn set_spawn_mode_keep_history(&mut self, spawn_mode: SpawnMode) {
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

    pub(super) fn replace_child_if_present(&mut self, old: Child, new: Child) {
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

/// Spawn mode of a container or window: where the next sibling will be
/// inserted relative to it. The two-field model (`current`, `previous`)
/// enables `toggle` to implement a three-cycle: toggling twice from the same
/// axis visits Tab, while a single toggle just flips H <-> V.
///
/// `set_spawn_mode_reset` collapses history (calls `without_history`).
/// `set_spawn_mode_keep_history` preserves it. The history-aware path is what
/// lets alternating toggles (`H -> V -> H -> V`) eventually reach Tab, while a
/// fresh assignment (`H -> V`) does not.
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

    /// Build a no-history `SpawnMode` from a `Direction`.
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

    /// Advance through the three-cycle. Rotation table (`(previous, current)
    /// -> next`):
    ///
    /// ```text
    /// prev \ curr   H        V        Tab
    ///     H         V       Tab        V
    ///     V        Tab        H        H
    ///     Tab       V        H         H
    /// ```
    ///
    /// From H or V, toggling flips axis unless the previous state was the
    /// opposite axis (meaning the user already flipped once), in which case it
    /// advances to Tab. From Tab, return to whichever axis was not the
    /// immediate predecessor.
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

    /// Build a `SpawnMode` with `previous == current`, dropping rotation
    /// history. Prevents stale history from leaking into the next `toggle`.
    pub(crate) fn without_history(other: SpawnMode) -> Self {
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

/// Parent role in the partition tree. A `Container` can be a parent of other
/// nodes. A `Workspace` can be a parent only of the root node. Windows are
/// never parents.
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

/// Child role in the partition tree. A `Window` is always a leaf. A
/// `Container` is a child of either another container or the workspace.
/// Workspaces are never children.
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
