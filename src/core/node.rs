use std::iter::Sum;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

use crate::config::WindowMatcher;
use crate::core::allocator::{Node, NodeId};
use crate::core::matcher::FloatFullscreenMatcherId;

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

/// Core is coordinate-system-agnostic: `work_area` holds whatever rect
/// the platform supplies in its own native frame (logical on macOS,
/// physical on Windows). Core never characterises or converts the
/// unit -- all layout math is unit-agnostic.
#[derive(Debug, Clone)]
pub(crate) struct Monitor {
    pub(super) name: String,
    pub(super) work_area: PixelRect,
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
    /// Each id's screen-absolute rect lives on the window itself, in
    /// `DisplayMode::Float`. Focusing a float moves it to the end.
    pub(super) float_windows: Vec<WindowId>,
    /// All fullscreen windows in this workspace, order by z-index with the last is the top most
    /// window. Only the top most fullscreen window is displayed.
    pub(super) fullscreen_windows: Vec<WindowId>,
    pub(super) float_matchers: Vec<FloatFullscreenMatcherId>,
    pub(super) fullscreen_matchers: Vec<FloatFullscreenMatcherId>,
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
            float_matchers: Vec::new(),
            fullscreen_matchers: Vec::new(),
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
pub(super) enum DisplayMode {
    #[default]
    Tiling,
    Float {
        border_box: PixelRect,
        occupy: Option<FloatFullscreenMatcherId>,
    },
    Fullscreen {
        occupy: Option<FloatFullscreenMatcherId>,
    },
}

impl std::fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tiling => write!(f, "tiling"),
            Self::Float { .. } => write!(f, "float"),
            Self::Fullscreen { .. } => write!(f, "fullscreen"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum WindowRestrictions {
    #[default]
    None,
    /// Blocks all user-initiated operations globally (Windows exclusive fullscreen).
    BlockAll,
    /// Protects platform-initiated fullscreen. Only the platform can undo it.
    ProtectFullscreen,
}

pub(crate) trait WindowMetadata:
    std::fmt::Display + std::fmt::Debug + Send + Sync + 'static
{
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
pub(crate) struct Window {
    pub(super) workspace: Option<WorkspaceId>,
    pub(super) mode: DisplayMode,
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
            mode: self.mode,
            restrictions: self.restrictions,
            is_minimized: self.is_minimized,
            limits: self.limits,
        }
    }
}

impl Window {
    /// None iff the window is minimized (is_minimized <=> workspace().is_none()).
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

    pub(super) fn tiling(workspace: WorkspaceId, metadata: Box<dyn WindowMetadata>) -> Self {
        Self {
            workspace: Some(workspace),
            mode: DisplayMode::Tiling,
            restrictions: WindowRestrictions::None,
            is_minimized: false,
            metadata,
            limits: SizeLimits::default(),
        }
    }

    pub(super) fn float(
        workspace: WorkspaceId,
        border_box: PixelRect,
        metadata: Box<dyn WindowMetadata>,
    ) -> Self {
        Self {
            workspace: Some(workspace),
            mode: DisplayMode::Float {
                border_box,
                occupy: None,
            },
            restrictions: WindowRestrictions::None,
            is_minimized: false,
            metadata,
            limits: SizeLimits::default(),
        }
    }

    pub(super) fn fullscreen(
        workspace: WorkspaceId,
        restrictions: WindowRestrictions,
        metadata: Box<dyn WindowMetadata>,
    ) -> Self {
        Self {
            workspace: Some(workspace),
            mode: DisplayMode::Fullscreen { occupy: None },
            restrictions,
            is_minimized: false,
            metadata,
            limits: SizeLimits::default(),
        }
    }

    pub(crate) fn limits(&self) -> SizeLimits {
        self.limits
    }

    pub(crate) fn title(&self) -> &str {
        self.metadata.title().unwrap_or("")
    }

    pub(crate) fn is_float(&self) -> bool {
        matches!(self.mode, DisplayMode::Float { .. })
    }

    pub(crate) fn is_fullscreen(&self) -> bool {
        matches!(self.mode, DisplayMode::Fullscreen { .. })
    }
}

/// Core-side twin of `MinimizedWindow` in `src/action.rs`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MinimizedWindowEntry {
    pub(crate) id: WindowId,
    pub(crate) title: String,
    pub(crate) app_name: Option<String>,
    pub(crate) bundle_id: Option<String>,
    pub(crate) executable_path: Option<String>,
}

/// Unit marker for rectangles expressed in **logical points** (DPI-independent).
/// Used on macOS (Accessibility API, NSWindow, NSScreen) and at the egui overlay
/// boundary on Windows.
pub(crate) struct Logical;

/// Unit marker for rectangles expressed in **physical pixels** (raw device coords).
/// Used on Windows (PMv2 context: `GetWindowRect`, `SetWindowPos`, `GetMonitorInfoW`,
/// DWM frame bounds).
#[cfg_attr(
    not(target_os = "windows"),
    expect(
        dead_code,
        reason = "phantom marker used only as a type parameter on Windows"
    )
)]
pub(crate) struct Physical;

#[cfg(target_os = "windows")]
pub(crate) type Unit = Physical;
#[cfg(not(target_os = "windows"))]
pub(crate) type Unit = Logical;

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

    pub(crate) fn clamp(self, lo: Self, hi: Self) -> Self {
        Self::new(self.v.clamp(lo.v, hi.v))
    }

    pub(crate) fn from_pixels(px: Pixels<U>) -> Self {
        Self::new(px.v as f32)
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

/// Effective per-child layout constraints in the `Length` unit, in border-box
/// space.
///
/// `Length::ZERO` on a `max_*` field means "unbounded" on that axis. Containers
/// always set both maxes to `Length::ZERO`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Constraints {
    pub(crate) min_width: Length,
    pub(crate) min_height: Length,
    pub(crate) max_width: Length,
    pub(crate) max_height: Length,
}

/// OS-reported size limits, in content-box space, which is what an app's stated
/// minimum or maximum describes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct SizeLimits {
    pub(crate) min_width: Option<Length<Unit>>,
    pub(crate) min_height: Option<Length<Unit>>,
    pub(crate) max_width: Option<Length<Unit>>,
    pub(crate) max_height: Option<Length<Unit>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) enum LimitUpdate {
    #[default]
    Unchanged,
    #[cfg_attr(
        all(not(test), not(target_os = "windows")),
        expect(
            dead_code,
            reason = "only the Windows shell reports a genuine no-limit observation"
        )
    )]
    Cleared,
    Set(Length<Unit>),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct LimitObservation {
    pub(crate) min_width: LimitUpdate,
    pub(crate) min_height: LimitUpdate,
    pub(crate) max_width: LimitUpdate,
    pub(crate) max_height: LimitUpdate,
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
    /// Lives next to the type definition to keep serialisation coherent with the type.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = f32::deserialize(d)?;
        if !v.is_finite() || v < 0.0 {
            return Err(serde::de::Error::custom(
                "length must be a finite non-negative number",
            ));
        }
        Ok(Length::new(v))
    }
}

/// A whole number of units, tagged with the same unit marker as `Length`. Where a
/// `Length` holds any `f32`, this holds only a value already on the pixel grid.
pub(crate) struct Pixels<U = Unit> {
    v: i32,
    _unit: PhantomData<fn() -> U>,
}

// Manual impls avoid the `U: Trait` bounds a derive would infer, as on `Length` above.
impl<U> Clone for Pixels<U> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<U> Copy for Pixels<U> {}

impl<U> std::fmt::Debug for Pixels<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pixels").field("v", &self.v).finish()
    }
}

impl<U> PartialEq for Pixels<U> {
    fn eq(&self, other: &Self) -> bool {
        self.v == other.v
    }
}

impl<U> Eq for Pixels<U> {}

impl<U> PartialOrd for Pixels<U> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<U> Ord for Pixels<U> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.v.cmp(&other.v)
    }
}

// Intentionally no From<i32> and no From<Pixels> for Length. An Into would put the two
// named crossings, Pixels::round and Length::from_pixels, back out of sight.
impl<U> Pixels<U> {
    pub(crate) const ZERO: Self = Self::new(0);

    pub(crate) const fn new(v: i32) -> Self {
        Self {
            v,
            _unit: PhantomData,
        }
    }

    pub(crate) fn round(length: Length<U>) -> Self {
        Self::new(length.v.round() as i32)
    }

    pub(crate) const fn value(self) -> i32 {
        self.v
    }

    #[cfg_attr(
        target_os = "windows",
        expect(
            dead_code,
            reason = "only the macOS borderless-fullscreen tolerance check compares distances"
        )
    )]
    pub(crate) fn abs(self) -> Self {
        Self::new(self.v.abs())
    }

    pub(crate) fn max(self, other: Self) -> Self {
        Self::new(self.v.max(other.v))
    }

    pub(crate) fn min(self, other: Self) -> Self {
        Self::new(self.v.min(other.v))
    }
}

impl<U> Add for Pixels<U> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.v + rhs.v)
    }
}

impl<U> Sub for Pixels<U> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.v - rhs.v)
    }
}

impl<U> Mul<i32> for Pixels<U> {
    type Output = Self;
    fn mul(self, rhs: i32) -> Self {
        Self::new(self.v * rhs)
    }
}

impl<U> Mul<Pixels<U>> for i32 {
    type Output = Pixels<U>;
    fn mul(self, rhs: Pixels<U>) -> Pixels<U> {
        Pixels::new(self * rhs.v)
    }
}

impl<U> Div<i32> for Pixels<U> {
    type Output = Self;
    fn div(self, rhs: i32) -> Self {
        Self::new(self.v / rhs)
    }
}

/// A rectangle tagged with a compile-time unit marker (`Logical` or `Physical`).
/// The default type parameter `Unit` is cfg-aliased per target so core code can
/// spell plain `Dimension` without an explicit generic.
pub(crate) struct Dimension<U = Unit> {
    pub(crate) width: Length<U>,
    pub(crate) height: Length<U>,
    pub(crate) x: Length<U>,
    pub(crate) y: Length<U>,
}

// Manual Debug avoids a `U: Debug` bound that #[derive(Debug)] would infer.
impl<U> std::fmt::Debug for Dimension<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dimension")
            .field("x", &self.x.v)
            .field("y", &self.y.v)
            .field("width", &self.width.v)
            .field("height", &self.height.v)
            .finish()
    }
}

impl<U> PartialEq for Dimension<U> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x
            && self.y == other.y
            && self.width == other.width
            && self.height == other.height
    }
}

impl<U> Copy for Dimension<U> {}
impl<U> Clone for Dimension<U> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<U> Dimension<U> {
    /// Does not catch positional swaps (e.g. x vs width) since all four args share
    /// the same type. A builder would be needed for that, which is out of scope.
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
        }
    }
}

/// Zero rectangle is meaningful as an initial placeholder.
impl<U> Default for Dimension<U> {
    fn default() -> Self {
        Self::new(Length::ZERO, Length::ZERO, Length::ZERO, Length::ZERO)
    }
}

/// A rectangle whose four edges lie on integer unit boundaries, so the on-grid
/// invariant is carried by the representation rather than by a rounding call at
/// each producer. The integer backing is load-bearing: placements are compared
/// for exact equality after an OS round-trip, which is not sound on `f32`.
pub(crate) struct PixelRect<U = Unit> {
    x: Pixels<U>,
    y: Pixels<U>,
    width: Pixels<U>,
    height: Pixels<U>,
}

// Manual impls avoid the `U: Trait` bounds a derive would infer, as on Dimension above.
impl<U> std::fmt::Debug for PixelRect<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PixelRect")
            .field("x", &self.x.v)
            .field("y", &self.y.v)
            .field("width", &self.width.v)
            .field("height", &self.height.v)
            .finish()
    }
}

impl<U> PartialEq for PixelRect<U> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x
            && self.y == other.y
            && self.width == other.width
            && self.height == other.height
    }
}

impl<U> Eq for PixelRect<U> {}

impl<U> Copy for PixelRect<U> {}
impl<U> Clone for PixelRect<U> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<U> PixelRect<U> {
    pub(crate) const ZERO: Self = Self::new(0, 0, 0, 0);

    /// Deliberately stays on `i32`. This is the raw entry point, where a caller asserts
    /// gridness by choosing it, and `from_pixels` is the sibling for typed callers.
    pub(crate) const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x: Pixels::new(x),
            y: Pixels::new(y),
            width: Pixels::new(width),
            height: Pixels::new(height),
        }
    }

    pub(crate) const fn from_pixels(
        x: Pixels<U>,
        y: Pixels<U>,
        width: Pixels<U>,
        height: Pixels<U>,
    ) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Rounds every edge to nearest. The far edges are derived from `x + width` rather
    /// than by rounding the extents, because `round(x) + round(width)` can disagree with
    /// `round(x + width)`, which opens gaps between adjacent boxes and overshoots the
    /// monitor on the last one.
    pub(crate) fn from_dimension(dim: Dimension<U>) -> Self {
        let left = dim.x.v.round() as i32;
        let top = dim.y.v.round() as i32;
        let right = (dim.x + dim.width).v.round() as i32;
        let bottom = (dim.y + dim.height).v.round() as i32;
        Self::new(left, top, right - left, bottom - top)
    }

    /// The largest grid-aligned rectangle contained by `dim`: the origin moves up to
    /// the next whole unit and the far edge back to the previous one, so the result can
    /// only shrink. Used for the monitor work area, which is a region a window must stay
    /// inside. Rounding to nearest would let a window cover a fraction of a pixel a
    /// status bar reserved.
    pub(crate) fn from_dimension_inward(dim: Dimension<U>) -> Self {
        let left = dim.x.v.ceil() as i32;
        let top = dim.y.v.ceil() as i32;
        let right = (dim.x + dim.width).v.floor() as i32;
        let bottom = (dim.y + dim.height).v.floor() as i32;
        Self::new(left, top, (right - left).max(0), (bottom - top).max(0))
    }

    pub(crate) fn to_dimension(self) -> Dimension<U> {
        Dimension::new(
            Length::from_pixels(self.x),
            Length::from_pixels(self.y),
            Length::from_pixels(self.width),
            Length::from_pixels(self.height),
        )
    }

    pub(crate) const fn x(self) -> Pixels<U> {
        self.x
    }

    pub(crate) const fn y(self) -> Pixels<U> {
        self.y
    }

    pub(crate) const fn width(self) -> Pixels<U> {
        self.width
    }

    pub(crate) const fn height(self) -> Pixels<U> {
        self.height
    }

    // Adds through the private fields because the `Add` impl is not const-callable.
    pub(crate) const fn right(self) -> Pixels<U> {
        Pixels::new(self.x.v + self.width.v)
    }

    pub(crate) const fn bottom(self) -> Pixels<U> {
        Pixels::new(self.y.v + self.height.v)
    }

    /// Mirrors `strategy::clip`, including returning `None` on an empty intersection,
    /// so the two cannot drift apart in meaning. Exact on integers: an intersection of
    /// two grid-aligned rectangles is grid-aligned, so nothing needs rounding after.
    pub(crate) fn clip(self, bounds: Self) -> Option<Self> {
        let x1 = self.x.max(bounds.x);
        let y1 = self.y.max(bounds.y);
        let x2 = self.right().min(bounds.right());
        let y2 = self.bottom().min(bounds.bottom());
        if x1 >= x2 || y1 >= y2 {
            return None;
        }
        Some(Self::from_pixels(x1, y1, x2 - x1, y2 - y1))
    }

    /// `<=` rather than `==` so an inverted extent counts as empty, matching what
    /// `strategy::clip` rejects.
    pub(crate) const fn is_empty(self) -> bool {
        self.width.v <= 0 || self.height.v <= 0
    }

    /// Clamps extent at zero rather than going negative. The origin is still pushed
    /// inward, so a box narrower than `2 * border` ends up empty at an origin past
    /// its own far edge.
    pub(crate) fn inset_by(self, border: Pixels<U>) -> Self {
        Self::from_pixels(
            self.x + border,
            self.y + border,
            (self.width - border * 2).max(Pixels::ZERO),
            (self.height - border * 2).max(Pixels::ZERO),
        )
    }

    /// The exact left inverse of `inset_by` for any box wider and taller than
    /// `2 * border`. Below that `inset_by` clamps its extents and the original is lost.
    pub(crate) fn outset_by(self, border: Pixels<U>) -> Self {
        Self::from_pixels(
            self.x - border,
            self.y - border,
            self.width + border * 2,
            self.height + border * 2,
        )
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

/// Child role in a tiling tree. A `Window` is always a leaf. A `Container`
/// is child of either another container or the workspace. Workspaces are
/// never children.
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
