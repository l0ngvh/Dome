use crate::core::allocator::{Node, NodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MonitorId(usize);

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

/// Core is coordinate-system-agnostic: `dimension` holds whatever rect
/// the platform supplies in its own native frame (logical on macOS,
/// physical on Windows). Core never characterises or converts the
/// unit -- all layout math is unit-agnostic.
#[derive(Debug, Clone)]
pub(crate) struct Monitor {
    pub(super) name: String,
    pub(super) dimension: Dimension,
    /// Multiplier applied to config-denominated lengths before use in
    /// layout math on this monitor.
    ///
    /// - macOS: always `1.0`. AppKit, AX, and Core Graphics all express
    ///   window geometry in logical points, which is also the config unit.
    /// - Windows: the monitor's DPI scale (e.g. `1.5` at 150%). PMv2
    ///   reports rects in physical pixels, but config values are logical
    ///   pixels, so they must be multiplied to reach the frame unit.
    pub(super) scale: f32,
    pub(super) active_workspace: WorkspaceId,
}

impl Node for Monitor {
    type Id = MonitorId;
}

#[derive(Debug, Clone)]
pub(crate) struct Workspace {
    pub(super) name: String,
    pub(super) monitor: MonitorId,
    /// When true, the focused window is float_windows.last() (z-ordered, last = topmost).
    /// Must be false when float_windows is empty.
    pub(super) is_float_focused: bool,
    /// All floats in this workspace, with their screen-absolute dimensions.
    /// This is the authoritative source for float screen position -- not window.dimension.
    pub(super) float_windows: Vec<(WindowId, Dimension)>,
    /// All fullscreen windows in this workspace. Last element is topmost (highest z-order).
    pub(super) fullscreen_windows: Vec<WindowId>,
}

impl Node for Workspace {
    type Id = WorkspaceId;
}

impl Workspace {
    pub(super) fn new(name: String, monitor: MonitorId) -> Self {
        Self {
            is_float_focused: false,
            name,
            monitor,
            float_windows: Vec::new(),
            fullscreen_windows: Vec::new(),
        }
    }

    /// Computes effective non-tiling focus: fullscreen > float.
    /// Returns None if neither fullscreen nor float is focused, meaning
    /// tiling focus should be consulted via the strategy.
    pub(crate) fn focused_non_tiling(&self) -> Option<WindowId> {
        if let Some(&id) = self.fullscreen_windows.last() {
            return Some(id);
        }
        if self.is_float_focused
            && let Some(&(id, _)) = self.float_windows.last()
        {
            return Some(id);
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn is_float_focused(&self) -> bool {
        self.is_float_focused
    }

    #[cfg(test)]
    pub(crate) fn float_windows(&self) -> &[(WindowId, Dimension)] {
        &self.float_windows
    }

    #[cfg(test)]
    pub(crate) fn fullscreen_windows(&self) -> &[WindowId] {
        &self.fullscreen_windows
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DisplayMode {
    #[default]
    Tiling,
    Float,
    Fullscreen,
    Minimized,
}

impl std::fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tiling => write!(f, "tiling"),
            Self::Float => write!(f, "float"),
            Self::Fullscreen => write!(f, "fullscreen"),
            Self::Minimized => write!(f, "minimized"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum WindowRestrictions {
    #[default]
    None,
    /// Blocks all user-initiated operations globally (Windows exclusive fullscreen).
    BlockAll,
    /// Blocks toggle_fullscreen, toggle_float, move_to_monitor on this window.
    /// Allows move_to_workspace — fullscreen windows can move across workspaces.
    /// Protects platform-initiated fullscreen — only the platform can undo it.
    ProtectFullscreen,
}

/// Represents a single application window
#[derive(Debug, Clone)]
pub(crate) struct Window {
    pub(super) workspace: WorkspaceId,
    pub(super) mode: DisplayMode,
    pub(super) restrictions: WindowRestrictions,
    pub(super) title: String,
    pub(super) min_width: f32,
    pub(super) min_height: f32,
    pub(super) max_width: f32,
    pub(super) max_height: f32,
}

impl Node for Window {
    type Id = WindowId;
}

impl Window {
    pub(super) fn tiling(workspace: WorkspaceId) -> Self {
        Self {
            workspace,
            mode: DisplayMode::Tiling,
            restrictions: WindowRestrictions::None,
            title: String::new(),
            min_width: 0.0,
            min_height: 0.0,
            max_width: 0.0,
            max_height: 0.0,
        }
    }

    pub(super) fn float(workspace: WorkspaceId) -> Self {
        Self {
            workspace,
            mode: DisplayMode::Float,
            restrictions: WindowRestrictions::None,
            title: String::new(),
            min_width: 0.0,
            min_height: 0.0,
            max_width: 0.0,
            max_height: 0.0,
        }
    }

    pub(super) fn fullscreen(workspace: WorkspaceId, restrictions: WindowRestrictions) -> Self {
        Self {
            workspace,
            mode: DisplayMode::Fullscreen,
            restrictions,
            title: String::new(),
            min_width: 0.0,
            min_height: 0.0,
            max_width: 0.0,
            max_height: 0.0,
        }
    }

    pub(crate) fn min_size(&self) -> (f32, f32) {
        (self.min_width, self.min_height)
    }

    pub(crate) fn max_size(&self) -> (f32, f32) {
        (self.max_width, self.max_height)
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn is_float(&self) -> bool {
        self.mode == DisplayMode::Float
    }

    pub(crate) fn is_fullscreen(&self) -> bool {
        self.mode == DisplayMode::Fullscreen
    }

    pub(crate) fn is_minimized(&self) -> bool {
        self.mode == DisplayMode::Minimized
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Dimension {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) x: f32,
    pub(crate) y: f32,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct WindowId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
