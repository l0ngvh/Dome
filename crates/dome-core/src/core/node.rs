pub(crate) use crate::geometry::{
    Dimension, FractionalLength, Length, Logical, PixelRect, Unit, pixels_from_lua_number,
};

use crate::core::allocator::{Node, NodeId};
use crate::core::matcher::WindowMatcher;

pub use dome_ipc::WindowId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MonitorId(usize);

impl NodeId for MonitorId {
    fn new(id: usize) -> Self {
        Self(id)
    }
    fn get(self) -> usize {
        self.0
    }
}

impl std::fmt::Display for MonitorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonitorId({})", self.0)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) enum DisplayMode {
    #[default]
    Tiling,
    Float {
        border_box: PixelRect,
    },
    Fullscreen,
}

impl std::fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tiling => write!(f, "tiling"),
            Self::Float { .. } => write!(f, "float"),
            Self::Fullscreen => write!(f, "fullscreen"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WindowRestrictions {
    #[default]
    None,
    /// Blocks restricted navigation, movement and display-mode actions while this window
    /// has focus (Windows exclusive fullscreen).
    BlockAll,
    /// Protects platform-initiated fullscreen. Only the platform can undo it.
    ProtectFullscreen,
}

pub trait WindowMetadata: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static {
    fn app_name(&self) -> Option<String>;
    fn title(&self) -> Option<&str>;
    fn set_title(&mut self, title: String);
    fn clone_box(&self) -> Box<dyn WindowMetadata>;

    fn matches_window_matcher(&self, matcher: &WindowMatcher) -> bool;

    /// Every populated platform field is included for maximum specificity.
    fn to_window_matcher(&self) -> WindowMatcher;

    fn bundle_id(&self) -> Option<String> {
        None
    }

    fn executable_path(&self) -> Option<String> {
        None
    }
}

#[derive(Debug)]
pub struct Window {
    pub(super) workspace: Option<WorkspaceId>,
    pub(super) restrictions: WindowRestrictions,
    is_minimized: bool,
    pub(super) metadata: Box<dyn WindowMetadata>,
    pub(super) limits: SizeLimits,
}

impl Node for Window {
    type Id = WindowId;
}

impl Clone for Window {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone_box(),
            workspace: self.workspace,
            restrictions: self.restrictions,
            is_minimized: self.is_minimized,
            limits: self.limits,
        }
    }
}

impl Window {
    /// None if the window is minimized (is_minimized <=> workspace().is_none()).
    pub fn workspace(&self) -> Option<WorkspaceId> {
        self.workspace
    }

    pub fn is_minimized(&self) -> bool {
        self.is_minimized
    }

    pub(super) fn set_minimized(&mut self, v: bool) {
        self.is_minimized = v;
    }

    pub(super) fn set_workspace(&mut self, ws: Option<WorkspaceId>) {
        self.workspace = ws;
    }

    pub(super) fn new(
        workspace: WorkspaceId,
        restrictions: WindowRestrictions,
        metadata: Box<dyn WindowMetadata>,
    ) -> Self {
        Self {
            workspace: Some(workspace),
            restrictions,
            is_minimized: false,
            metadata,
            limits: SizeLimits::default(),
        }
    }

    pub fn limits(&self) -> SizeLimits {
        self.limits
    }

    pub fn title(&self) -> &str {
        self.metadata.title().unwrap_or("")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MinimizedWindowEntry {
    pub id: WindowId,
    pub title: String,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    pub executable_path: Option<String>,
}

/// Effective per-child layout constraints in border-box space.
///
/// `FractionalLength::ZERO` on a `max_*` field means "unbounded" on that axis.
#[derive(Debug, Clone, Copy)]
pub struct Constraints {
    pub min_width: FractionalLength,
    pub min_height: FractionalLength,
    pub max_width: FractionalLength,
    pub max_height: FractionalLength,
}

/// OS-reported size limits, in content-box space, which is what an app's stated
/// minimum or maximum describes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SizeLimits {
    pub min_width: Option<FractionalLength<Unit>>,
    pub min_height: Option<FractionalLength<Unit>>,
    pub max_width: Option<FractionalLength<Unit>>,
    pub max_height: Option<FractionalLength<Unit>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum LimitUpdate {
    #[default]
    Unchanged,
    Cleared,
    Set(FractionalLength<Unit>),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LimitObservation {
    pub min_width: LimitUpdate,
    pub min_height: LimitUpdate,
    pub max_width: LimitUpdate,
    pub max_height: LimitUpdate,
}

impl SizeLimits {
    /// Applies each axis's minimum before its maximum. A new minimum above the stored maximum
    /// raises the maximum, and a new maximum below the stored minimum lowers the minimum.
    pub(super) fn apply_observation(&mut self, observed: LimitObservation) {
        let update = |name: &str,
                      min: &mut Option<FractionalLength<Unit>>,
                      max: &mut Option<FractionalLength<Unit>>,
                      new_min: LimitUpdate,
                      new_max: LimitUpdate| {
            match new_min {
                LimitUpdate::Unchanged => {}
                LimitUpdate::Cleared => *min = None,
                LimitUpdate::Set(new_min) => {
                    *min = Some(new_min);
                    if max.is_some_and(|m| m < new_min) {
                        tracing::debug!(
                            "{name}: existing max {:.2} < new min {:.2}, raising max",
                            max.unwrap_or(FractionalLength::ZERO).value(),
                            new_min.value()
                        );
                        *max = Some(new_min);
                    }
                }
            }
            match new_max {
                LimitUpdate::Unchanged => {}
                LimitUpdate::Cleared => *max = None,
                LimitUpdate::Set(new_max) => {
                    *max = Some(new_max);
                    if min.is_some_and(|m| m > new_max) {
                        tracing::debug!(
                            "{name}: existing min {:.2} > new max {:.2}, lowering min",
                            min.unwrap_or(FractionalLength::ZERO).value(),
                            new_max.value()
                        );
                        *min = Some(new_max);
                    }
                }
            }
        };

        update(
            "width",
            &mut self.min_width,
            &mut self.max_width,
            observed.min_width,
            observed.max_width,
        );
        update(
            "height",
            &mut self.min_height,
            &mut self.max_height,
            observed.min_height,
            observed.max_height,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContainerId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(usize);

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

/// Child role in a tiling tree. A `Window` is always a leaf. A `Container`
/// is child of either another container or the workspace. Workspaces are
/// never children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Child {
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

/// An opaque, ordered collection of children. Strategies define what may go inside
/// a container and own any per-container arrangement state.
#[derive(Debug, Clone)]
pub struct Container {
    pub(super) children: Vec<Child>,
    pub(super) workspace: WorkspaceId,
}

impl Node for Container {
    type Id = ContainerId;
}

impl Container {
    pub(super) fn children(&self) -> &[Child] {
        &self.children
    }

    pub(super) fn position_of(&self, child: Child) -> usize {
        self.children.iter().position(|c| *c == child).unwrap()
    }

    pub(super) fn replace_child_if_present(&mut self, old: Child, new: Child) {
        if let Some(pos) = self.children.iter().position(|c| *c == old) {
            self.children[pos] = new;
        }
    }
}

impl NodeId for WindowId {
    fn new(id: usize) -> Self {
        WindowId::new(id)
    }
    fn get(self) -> usize {
        WindowId::get(self)
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
