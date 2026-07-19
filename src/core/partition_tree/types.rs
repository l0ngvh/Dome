use super::preferred_layout::{PreferredSlot, PreferredWindowSlotId};
use crate::config::SplitMode;
use crate::core::hub::SpawnIndicator;
use crate::core::node::Child;
use crate::core::node::{ContainerId, Dimension, Direction, Length, WorkspaceId};

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

/// Per-workspace tiling state owned by the strategy.
#[derive(Debug, Default)]
pub(super) struct WorkspaceTilingState {
    pub(super) root: Option<Child>,
    /// Tiling focus pointer. Usually a `Child::Window` (the focused window). Can be
    /// `Child::Container` for container-highlight mode, where
    /// `focused_tiling_window()` returns `None`. Can only be None in an empty workspace.
    ///
    /// Anchors invariant 3 of `Container`: when this is `Some(X)`, every ancestor
    /// container of X has `focused == X`. Established by `set_focus_child`,
    /// preserved by `replace_child_focus`.
    pub(super) focused_tiling: Option<Child>,
    /// Root of the static preferred layout tree. `None` when no layout is configured.
    pub(super) preferred_root: Option<PreferredSlot>,
    /// The highest occupied node in the preferred layout tree. `None` when no
    /// matched window has been placed.
    pub(super) occupied_preferred_root: Option<PreferredSlot>,
    pub(super) viewport_offset: (Length, Length),
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
