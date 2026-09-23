use super::preferred_layout::{PreferredContainerSlotId, PreferredSlot, PreferredWindowSlotId};
use crate::config::lua::deserializer::string_enum;
use crate::core::node::Child;
use crate::core::node::{
    ContainerId, Dimension, Direction, Logical, Pixels, WindowId, WorkspaceId,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PartitionTreeConfig {
    pub(crate) tab_bar_height: Pixels<Logical>,
    pub(crate) automatic_tiling: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SplitMode {
    Horizontal,
    Vertical,
    Tabbed,
}

string_enum!(
    SplitMode,
    "\"horizontal\", \"vertical\" or \"tabbed\"",
    "horizontal" => SplitMode::Horizontal,
    "vertical" => SplitMode::Vertical,
    "tabbed" => SplitMode::Tabbed,
);

/// Parent role in the partition tree. A `Container` parents other nodes. A
/// `Workspace` parents only the root node.
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

/// Per-window tiling state.
#[derive(Debug)]
pub(super) struct TilingWindowData {
    pub(super) parent: Parent,
    pub(super) dimension: Dimension,
    pub(super) spawn_direction: Direction,
    pub(super) occupy: Option<PreferredWindowSlotId>,
}

impl TilingWindowData {
    pub(super) fn new(workspace: WorkspaceId) -> Self {
        Self::with_parent(Parent::Workspace(workspace))
    }

    pub(super) fn in_container(container: ContainerId) -> Self {
        Self::with_parent(Parent::Container(container))
    }

    fn with_parent(parent: Parent) -> Self {
        TilingWindowData {
            parent,
            dimension: Dimension::default(),
            spawn_direction: Direction::default(),
            occupy: None,
        }
    }
}

/// Per-container tiling state.
///
/// Invariant: a non-tabbed container's `direction` differs from its non-tabbed
/// parent's direction. A tabbed container is exempt: `direction()` returns
/// `None` for it, so the alternation rule does not apply across a tabbed
/// boundary. `validate_container_direction` enforces this.
#[derive(Debug)]
pub(super) struct TilingContainerData {
    pub(super) parent: Parent,
    pub(super) workspace: WorkspaceId,
    pub(super) dimension: Dimension,
    /// Split axis. Read through `direction()`, which returns `None` when
    /// `is_tabbed` is set. A value is stored while tabbed to keep the field
    /// initialised, but it is unused until the container converts back to split.
    direction: Direction,
    /// Direction the next child extends. Automatic tiling derives it from the
    /// container's shape, so it can differ from `direction`.
    spawn_direction: Direction,
    pub(super) is_tabbed: bool,
    pub(super) active_tab_index: usize,
    pub(super) occupy: Option<PreferredContainerSlotId>,
}

impl TilingContainerData {
    pub(super) fn new(parent: Parent, workspace: WorkspaceId, split_mode: SplitMode) -> Self {
        let (direction, is_tabbed) = match split_mode {
            SplitMode::Horizontal => (Direction::Horizontal, false),
            SplitMode::Vertical => (Direction::Vertical, false),
            SplitMode::Tabbed => (Direction::Horizontal, true),
        };
        Self {
            parent,
            workspace,
            dimension: Dimension::default(),
            direction,
            spawn_direction: direction,
            is_tabbed,
            active_tab_index: 0,
            occupy: None,
        }
    }

    pub(super) fn is_tabbed(&self) -> bool {
        self.is_tabbed
    }

    pub(super) fn active_tab_index(&self) -> usize {
        self.active_tab_index
    }

    pub(super) fn direction(&self) -> Option<Direction> {
        if self.is_tabbed {
            None
        } else {
            Some(self.direction)
        }
    }

    pub(super) fn has_direction(&self, direction: Direction) -> bool {
        if self.is_tabbed {
            false
        } else {
            self.direction == direction
        }
    }

    pub(super) fn spawn_direction(&self) -> Direction {
        self.spawn_direction
    }

    pub(super) fn set_spawn_direction(&mut self, spawn_direction: Direction) {
        self.spawn_direction = spawn_direction
    }

    pub(super) fn toggle_direction(&mut self) -> Direction {
        self.direction = match self.direction {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal,
        };
        self.direction
    }
}

/// Per-workspace tiling state owned by the strategy.
#[derive(Debug, Default)]
pub(super) struct WorkspaceTilingState {
    pub(super) root: Option<Child>,
    /// Tiling focus pointer. Usually a `Child::Window` (the focused window). Can be
    /// `Child::Container` for container-highlight mode, where
    /// `focused_tiling_window()` returns `None`. Can only be None in an empty workspace.
    pub(super) focused_tiling: Option<Child>,
    /// Windows of this workspace from most to least recently focused. Covers every
    /// tiling window of the workspace.
    pub(super) focus_history: Vec<WindowId>,
    /// Root of the static preferred layout tree. `None` when no layout is configured.
    pub(super) preferred_root: Option<PreferredSlot>,
    /// The highest occupied node in the preferred layout tree. `None` when no
    /// matched window has been placed.
    pub(super) occupied_preferred_root: Option<PreferredSlot>,
}

impl WorkspaceTilingState {
    pub(super) fn record_focus(&mut self, window_id: WindowId) {
        self.drop_from_history(window_id);
        self.focus_history.insert(0, window_id);
    }

    /// Enrolls as least recently focused without claiming focus. Idempotent, so a
    /// window that never left the workspace keeps its place.
    pub(super) fn add_to_history(&mut self, window_id: WindowId) {
        if !self.focus_history.contains(&window_id) {
            self.focus_history.push(window_id);
        }
    }

    pub(super) fn drop_from_history(&mut self, window_id: WindowId) {
        if let Some(pos) = self.focus_history.iter().position(|&w| w == window_id) {
            self.focus_history.remove(pos);
        }
    }
}

impl From<Direction> for SplitMode {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Horizontal => SplitMode::Horizontal,
            Direction::Vertical => SplitMode::Vertical,
        }
    }
}
