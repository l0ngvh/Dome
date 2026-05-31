use std::iter::Sum;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

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
    /// layout math on this monitor. Stored here so `SizeConstraint::resolve`
    /// can convert logical config values without re-reading platform state.
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
    /// When true, the focused window is float_windows.last().
    /// Wouldn't have any effect when any fullscreen window is present, but for consistency would be
    /// set to false in that case
    pub(super) is_float_focused: bool,
    /// Float ids in this workspace, ordered by z-index (last is topmost).
    /// Each id's screen-absolute dim lives on the window itself, in
    /// `DisplayMode::Float { dim }`. Focusing a float moves it to the end.
    pub(super) float_windows: Vec<WindowId>,
    /// All fullscreen windows in this workspace, order by z-index with the last is the top most
    /// window. Only the top most fullscreen window is displayed.
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum DisplayMode {
    #[default]
    Tiling,
    Float {
        dim: Dimension,
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
    pub(super) workspace: Option<WorkspaceId>,
    pub(super) mode: DisplayMode,
    pub(super) restrictions: WindowRestrictions,
    is_minimized: bool,
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
    /// Returns the workspace this window is attached to. None iff the
    /// window is minimized (is_minimized <=> workspace().is_none()).
    pub(crate) fn workspace(&self) -> Option<WorkspaceId> {
        self.workspace
    }

    pub(crate) fn is_minimized(&self) -> bool {
        self.is_minimized
    }

    pub(super) fn set_minimized(&mut self, v: bool) {
        self.is_minimized = v;
    }

    pub(super) fn set_workspace(&mut self, ws: Option<WorkspaceId>) {
        self.workspace = ws;
    }

    pub(super) fn tiling(workspace: WorkspaceId) -> Self {
        Self {
            workspace: Some(workspace),
            mode: DisplayMode::Tiling,
            restrictions: WindowRestrictions::None,
            is_minimized: false,
            title: String::new(),
            min_width: 0.0,
            min_height: 0.0,
            max_width: 0.0,
            max_height: 0.0,
        }
    }

    pub(super) fn float(workspace: WorkspaceId, dim: Dimension) -> Self {
        Self {
            workspace: Some(workspace),
            mode: DisplayMode::Float { dim },
            restrictions: WindowRestrictions::None,
            is_minimized: false,
            title: String::new(),
            min_width: 0.0,
            min_height: 0.0,
            max_width: 0.0,
            max_height: 0.0,
        }
    }

    pub(super) fn fullscreen(workspace: WorkspaceId, restrictions: WindowRestrictions) -> Self {
        Self {
            workspace: Some(workspace),
            mode: DisplayMode::Fullscreen,
            restrictions,
            is_minimized: false,
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
        matches!(self.mode, DisplayMode::Float { .. })
    }

    pub(crate) fn is_fullscreen(&self) -> bool {
        matches!(self.mode, DisplayMode::Fullscreen)
    }
}

/// Unit marker for rectangles expressed in **logical points** (DPI-independent).
/// Used on macOS (Accessibility API, NSWindow, NSScreen) and at the egui overlay
/// boundary on Windows.
pub(crate) struct Logical;

/// Unit marker for rectangles expressed in **physical pixels** (raw device coords).
/// Used on Windows (PMv2 context: `GetWindowRect`, `SetWindowPos`, `GetMonitorInfoW`,
/// DWM frame bounds). See `src/platform/windows/handle.rs::get_dimension` for the
/// cross-DPI virtualization rationale.
#[cfg_attr(
    not(target_os = "windows"),
    expect(
        dead_code,
        reason = "phantom marker used only as a type parameter on Windows"
    )
)]
pub(crate) struct Physical;

/// Per-target alias pinning core's `Dimension` to one concrete unit. `Hub` and every
/// core DTO keep the bare `Dimension` spelling and resolve to `Dimension<Unit>`.
#[cfg(target_os = "windows")]
pub(crate) type Unit = Physical;
#[cfg(not(target_os = "windows"))]
pub(crate) type Unit = Logical;

/// Trait encoding the logical-to-target conversion for each unit marker.
/// `Logical::from_logical` is identity; `Physical::from_logical` multiplies by scale.
/// Dispatch is on the target unit (not the input) so adding a new target (e.g. Linux)
/// is just `impl UnitKind for NewMarker` plus a cfg arm on `Unit`.
pub(crate) trait UnitKind {
    fn from_logical(logical: f32, scale: f32) -> f32;
}

impl UnitKind for Logical {
    fn from_logical(l: f32, _s: f32) -> f32 {
        l
    }
}

impl UnitKind for Physical {
    fn from_logical(l: f32, s: f32) -> f32 {
        l * s
    }
}

/// 1D length tagged with a unit. `Length<Logical>` is the config unit;
/// `Length<Unit>` is the binary's target unit (= `Logical` on macOS,
/// `Physical` on Windows). Inner `f32` is private to force every core
/// consumer to cross the logical-to-unit boundary via `to_unit(scale)`.
pub(crate) struct Length<U = Unit> {
    v: f32,
    _unit: PhantomData<fn() -> U>,
}

impl<U> Clone for Length<U> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<U> Copy for Length<U> {}

impl<U> std::fmt::Debug for Length<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Length").field("v", &self.v).finish()
    }
}

impl<U> PartialEq for Length<U> {
    fn eq(&self, other: &Self) -> bool {
        self.v == other.v
    }
}

impl<U> PartialOrd for Length<U> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.v.partial_cmp(&other.v)
    }
}

// Intentionally no Add<f32>/Sub<f32>/From<f32>: raw scalars cross via Length::new / .value().
impl<U> Length<U> {
    pub(crate) const ZERO: Self = Self::new(0.0);

    pub(crate) const fn new(v: f32) -> Self {
        Self {
            v,
            _unit: PhantomData,
        }
    }

    pub(crate) fn max(self, other: Self) -> Self {
        Self::new(self.v.max(other.v))
    }

    pub(crate) fn min(self, other: Self) -> Self {
        Self::new(self.v.min(other.v))
    }

    #[cfg(test)]
    pub(crate) fn abs(self) -> Self {
        Self::new(self.v.abs())
    }

    pub(crate) fn round(self) -> Self {
        Self::new(self.v.round())
    }

    pub(crate) fn clamp(self, lo: Self, hi: Self) -> Self {
        Self::new(self.v.clamp(lo.v, hi.v))
    }
}

impl<U> Sum for Length<U> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |acc, x| acc + x)
    }
}

impl<'a, U: 'a> Sum<&'a Self> for Length<U> {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |acc, &x| acc + x)
    }
}

impl<U> Default for Length<U> {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl<U> std::fmt::Display for Length<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.v, f)
    }
}

impl Length<Logical> {
    /// Convert a logical config length into the binary's `Unit`. Identity on
    /// macOS; multiplies by `scale` on Windows. This is the only method that
    /// crosses the logical-to-unit boundary; core arithmetic reads go through it.
    pub(crate) fn to_unit(self, scale: f32) -> Length<Unit> {
        Length::new(<Unit as UnitKind>::from_logical(self.v, scale))
    }

    /// Raw `f32` accessor for callers that stay in logical space (config
    /// validation, platform shells bridging to egui's raw-f32 logical-point
    /// coordinate space). Not for core code that mixes with `Unit`-space
    /// rectangles; use `to_unit(scale).value()` instead. Greppable escape
    /// hatch: should never appear in `src/core/**`.
    pub(crate) fn logical(self) -> f32 {
        self.v
    }
}

impl Length<Unit> {
    pub(crate) fn value(self) -> f32 {
        self.v
    }
}

impl<U> Add for Length<U> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.v + rhs.v)
    }
}

impl<U> Sub for Length<U> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.v - rhs.v)
    }
}

impl<U> Mul<f32> for Length<U> {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.v * rhs)
    }
}

impl<U> Mul<Length<U>> for f32 {
    type Output = Length<U>;
    fn mul(self, rhs: Length<U>) -> Length<U> {
        Length::new(self * rhs.v)
    }
}

impl<U> Div<f32> for Length<U> {
    type Output = Self;
    fn div(self, rhs: f32) -> Self {
        Self::new(self.v / rhs)
    }
}

impl<U> AddAssign for Length<U> {
    fn add_assign(&mut self, rhs: Self) {
        self.v += rhs.v;
    }
}

impl<U> SubAssign for Length<U> {
    fn sub_assign(&mut self, rhs: Self) {
        self.v -= rhs.v;
    }
}

impl<'de> serde::Deserialize<'de> for Length<Logical> {
    /// Deserializes a non-negative `f32` from TOML/serde into `Length<Logical>`.
    /// Lives next to the type definition to keep serialisation coherent with the type.
    /// Only `serde::Deserialize`/`Deserializer`/`Error::custom` are used -- no OS deps.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = f32::deserialize(d)?;
        if v < 0.0 {
            return Err(serde::de::Error::custom("length must be non-negative"));
        }
        Ok(Length::new(v))
    }
}

/// A rectangle tagged with a compile-time unit marker (`Logical` or `Physical`).
/// The default type parameter `Unit` is cfg-aliased per target so core code can
/// spell plain `Dimension` without an explicit generic.
///
/// `PhantomData<fn() -> U>` keeps `Dimension` `Send + Sync` regardless of `U`
/// and makes `U` invariant, which is the idiomatic spelling for zero-sized tag
/// types we never want the compiler to silently widen.
pub(crate) struct Dimension<U = Unit> {
    pub(crate) width: Length<U>,
    pub(crate) height: Length<U>,
    pub(crate) x: Length<U>,
    pub(crate) y: Length<U>,
    _unit: PhantomData<fn() -> U>,
}

// Manual Debug avoids a `U: Debug` bound that #[derive(Debug)] would infer.
// The phantom field contributes nothing to the output.
impl<U> std::fmt::Debug for Dimension<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Reaches into Length's private v field to keep inline @"" snapshots byte-stable
        // across the Dimension->Length retag; .value() is not available for Length<Logical>.
        f.debug_struct("Dimension")
            .field("x", &self.x.v)
            .field("y", &self.y.v)
            .field("width", &self.width.v)
            .field("height", &self.height.v)
            .finish()
    }
}

// Manual PartialEq avoids a `U: PartialEq` bound that #[derive(PartialEq)]
// would infer. Only the Length fields matter; the phantom tag is zero-sized.
impl<U> PartialEq for Dimension<U> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x
            && self.y == other.y
            && self.width == other.width
            && self.height == other.height
    }
}

// Manual Copy/Clone impls avoid a `U: Copy`/`U: Clone` bound that
// #[derive(Copy, Clone)] would infer. PhantomData<fn() -> U> is
// unconditionally Copy+Clone.
impl<U> Copy for Dimension<U> {}
impl<U> Clone for Dimension<U> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<U> Dimension<U> {
    /// Four positional `Length<U>` args. Does not catch positional swaps
    /// (e.g. x vs width) since all share the same type; a builder would
    /// be needed for that, which is out of scope.
    pub(crate) const fn new(
        x: Length<U>,
        y: Length<U>,
        width: Length<U>,
        height: Length<U>,
    ) -> Self {
        Self {
            x,
            y,
            width,
            height,
            _unit: PhantomData,
        }
    }

    /// Pixel-snap all four fields via `Length::round`. This is a semantics
    /// choice ("snap placement to whole pixels before writing to the OS"),
    /// not a unit crossing. Keeping it here rather than inside the FFI
    /// wrapper lets the shell decide when to snap and preserves
    /// `Dimension<U>` as the shared boundary currency.
    pub(crate) fn round(self) -> Self {
        Self::new(
            self.x.round(),
            self.y.round(),
            self.width.round(),
            self.height.round(),
        )
    }
}

/// Manual `Default` avoids a `U: Default` bound that `#[derive(Default)]` would
/// infer. Zero rectangle is meaningful as an initial placeholder.
impl<U> Default for Dimension<U> {
    fn default() -> Self {
        Self::new(Length::ZERO, Length::ZERO, Length::ZERO, Length::ZERO)
    }
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
