use super::preferred_layout::{PreferredContainerSlotId, PreferredSlot, PreferredWindowSlotId};
use crate::config::SplitMode;
use crate::core::hub::SpawnIndicator;
use crate::core::node::Child;
use crate::core::node::{ContainerId, Dimension, Direction, Length, WindowId, WorkspaceId};

/// Spawn mode of a container or window: where the next sibling will be
/// inserted relative to it.
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

impl From<crate::config::SplitMode> for SpawnMode {
    fn from(split: crate::config::SplitMode) -> Self {
        match split {
            crate::config::SplitMode::Horizontal => SpawnMode::horizontal(),
            crate::config::SplitMode::Vertical => SpawnMode::vertical(),
            crate::config::SplitMode::Tabbed => SpawnMode::tabbed(),
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

/// Per-window tiling state.
#[derive(Debug)]
pub(super) struct TilingWindowData {
    pub(super) parent: Parent,
    pub(super) dimension: Dimension,
    pub(super) spawn_mode: SpawnMode,
    pub(super) occupy: Option<PreferredWindowSlotId>,
}

impl TilingWindowData {
    pub(super) fn new(workspace: WorkspaceId) -> Self {
        TilingWindowData {
            parent: Parent::Workspace(workspace),
            // Zero placeholder -- layout_workspace at the end of this function
            // computes the real rect before any reader observes this entry.
            dimension: Dimension::default(),
            spawn_mode: SpawnMode::default(),
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
    /// Spawn mode for new children inserted under this container. Mutate via
    /// `set_spawn_mode_reset` (drops history) or `set_spawn_mode_keep_history`
    /// (preserves history). Direct field write would lose the `H <-> V <-> Tab`
    /// rotation state.
    spawn_mode: SpawnMode,
    pub(super) is_tabbed: bool,
    pub(super) active_tab_index: usize,
    pub(super) min_width: Length,
    pub(super) min_height: Length,
    /// Preferred container slot this live container materializes, if any.
    pub(super) occupy: Option<PreferredContainerSlotId>,
}

impl TilingContainerData {
    pub(super) fn new(parent: Parent, workspace: WorkspaceId, split_mode: SplitMode) -> Self {
        let (direction, spawn_mode, is_tabbed) = match split_mode {
            SplitMode::Horizontal => (Direction::Horizontal, SpawnMode::horizontal(), false),
            SplitMode::Vertical => (Direction::Vertical, SpawnMode::vertical(), false),
            SplitMode::Tabbed => (Direction::Horizontal, SpawnMode::tabbed(), true),
        };
        Self {
            parent,
            workspace,
            dimension: Dimension::default(),
            direction,
            spawn_mode,
            is_tabbed,
            active_tab_index: 0,
            min_width: Length::ZERO,
            min_height: Length::ZERO,
            occupy: None,
        }
    }

    pub(super) fn is_tabbed(&self) -> bool {
        self.is_tabbed
    }

    pub(super) fn active_tab_index(&self) -> usize {
        self.active_tab_index
    }

    pub(super) fn min_size(&self) -> (Length, Length) {
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

    pub(super) fn spawn_mode(&self) -> SpawnMode {
        self.spawn_mode
    }

    pub(super) fn set_spawn_mode_reset(&mut self, spawn_mode: SpawnMode) {
        self.spawn_mode = SpawnMode::without_history(spawn_mode)
    }

    pub(super) fn set_spawn_mode_keep_history(&mut self, spawn_mode: SpawnMode) {
        self.spawn_mode = self.spawn_mode.switch_to(spawn_mode)
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
    pub(super) viewport_offset: (Length, Length),
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

impl From<SpawnMode> for SpawnIndicator {
    fn from(mode: SpawnMode) -> Self {
        Self {
            top: mode.is_tab(),
            right: mode.is_horizontal(),
            bottom: mode.is_vertical(),
            left: false,
        }
    }
}

impl From<SpawnMode> for SplitMode {
    fn from(mode: SpawnMode) -> Self {
        match mode.current {
            SpawnState::Horizontal => SplitMode::Horizontal,
            SpawnState::Vertical => SplitMode::Vertical,
            SpawnState::Tab => SplitMode::Tabbed,
        }
    }
}
