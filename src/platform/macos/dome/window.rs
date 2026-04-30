use std::time::{Duration, Instant};

use anyhow::Result;

use crate::core::{Dimension, MonitorId, WindowId, WindowRestrictions};
use crate::platform::macos::MonitorInfo;
use crate::platform::macos::accessibility::AXWindowApi;

use super::{DebounceBurst, Dome};

const MAX_ENFORCEMENT_RETRIES: u8 = 5;

#[derive(Clone, Copy)]
pub(super) enum WindowState {
    Positioned(PositionedState),
    /// Window is in a macOS native fullscreen Space.
    NativeFullscreen,
    /// Window was zoomed to fill the screen via the zoom button or similar.
    /// Distinct from native fullscreen — no separate Space is created.
    BorderlessFullscreen,
    /// Window is minimized by Dome because it can't be moved offscreen
    /// (e.g. borderless fullscreen windows). Windows minimized by users are
    /// tracked via `UserMinimized` instead.
    Minimized,
    /// Window was minimized by the user (Cmd+M or minimize button).
    /// Tracked in hub.minimized_windows for the picker.
    UserMinimized,
}

#[derive(Clone, Copy)]
pub(super) enum PositionedState {
    /// Window is moved offscreen by Dome. `actual` is the last observed position, may differ from
    /// the current hidden coordinates if monitors changed since the window was hidden.
    Offscreen(OffscreenPlacement),
    /// Window is in a tiling layout slot with drift correction.
    Tiling(Placement),
    /// Window is floating. Carries only the reconciled target rect and a
    /// stale-observation timestamp -- no retry/drift fields because floats
    /// accept the OS-reported position as ground truth.
    Float(FloatPlacement),
}

#[derive(Clone, Copy)]
pub(super) struct OffscreenPlacement {
    actual: RoundedDimension,
    retries: u8,
}

impl OffscreenPlacement {
    pub(super) fn new(actual: RoundedDimension) -> Self {
        Self { actual, retries: 0 }
    }

    /// Check if the window drifted from the hidden position. Updates `actual`
    /// unconditionally. Returns true if the window is NOT at the hidden
    /// position (i.e. it fought back). Increments retries on drift.
    fn record_drift(&mut self, new_actual: RoundedDimension, monitors: &[MonitorInfo]) -> bool {
        self.actual = new_actual;
        let (hidden_x, hidden_y) = hidden_position(monitors);
        if new_actual.x == hidden_x || new_actual.y == hidden_y {
            return false;
        }
        self.retries = self.retries.saturating_add(1);
        true
    }

    fn should_retry(&self) -> bool {
        self.retries <= MAX_ENFORCEMENT_RETRIES
    }

    fn just_gave_up(&self) -> bool {
        self.retries == MAX_ENFORCEMENT_RETRIES + 1
    }
}

#[derive(Clone, Copy)]
pub(super) struct Placement {
    target: RoundedDimension,
    actual: RoundedDimension,
    retries: u8,
    /// When the last placement was issued. AX position-change notifications
    /// generated before this timestamp reflect pre-placement state and are ignored.
    placed_at: Instant,
}

/// Lightweight placement state for floating windows. Floats accept the
/// OS-reported geometry as ground truth, so there is no `actual` (target IS
/// actual after each observation) and no retry/drift machinery.
#[derive(Clone, Copy)]
pub(super) struct FloatPlacement {
    /// Last rect reconciled with the OS -- the rect we most recently passed to
    /// `set_frame` or adopted from a drag observation. Used for outbound
    /// idempotence in `place_window` and to skip no-op observations in
    /// `window_moved`.
    pub(super) target: RoundedDimension,
    /// When `target` was last bumped by an outbound `set_frame`. The
    /// initial-placement stale filter in `window_moved` ignores AX bursts
    /// whose `observed_at.last` predates this timestamp. User-drag
    /// observations do NOT bump this: they write `target` without issuing
    /// `set_frame`, so the filter anchor stays on the last outbound call.
    placed_at: Instant,
}

impl FloatPlacement {
    fn new(target: RoundedDimension) -> Self {
        Self {
            target,
            placed_at: Instant::now(),
        }
    }

    /// Record a new target. Returns true if set_frame is needed.
    /// Bumps `placed_at` only when the target actually changes.
    fn set_target(&mut self, target: RoundedDimension) -> bool {
        if self.target == target {
            return false;
        }
        self.target = target;
        self.placed_at = Instant::now();
        true
    }
}

impl Placement {
    fn new(actual: RoundedDimension, target: RoundedDimension) -> Self {
        Self {
            target,
            actual,
            retries: 0,
            placed_at: Instant::now(),
        }
    }

    /// Record a new target. Returns true if set_frame is needed.
    fn set_target(&mut self, target: RoundedDimension) -> bool {
        let target_changed = self.target != target;
        self.target = target;
        if target_changed {
            self.retries = 0;
            self.placed_at = Instant::now();
        }
        target_changed
    }

    // FIXME: Change this to if new placement encompass the old placement
    //
    /// Edge-alignment predicate. Returns true if `new_actual` has at least
    /// one vertical *and* one horizontal edge misaligned with the target
    /// (i.e. this is drift, not just an edge-anchored size delta). Pure —
    /// no mutation; caller must follow up with `observe_drift` to consume a
    /// retry.
    fn has_drifted(&self, new_actual: RoundedDimension) -> bool {
        let target = self.target;
        let left = new_actual.x == target.x;
        let right = new_actual.x + new_actual.width == target.x + target.width;
        let top = new_actual.y == target.y;
        let bottom = new_actual.y + new_actual.height == target.y + target.height;
        !((left || right) && (top || bottom))
    }

    /// Record a drift observation. Bumps `retries`, updates `actual`, and
    /// returns the target to re-issue via `set_frame` while retries remain;
    /// returns `None` once the budget is exhausted (logging the give-up
    /// message once). Shared by the edge-based and late-event drift paths
    /// so a single helper owns the retry accounting and logging.
    fn observe_drift(&mut self, new_actual: RoundedDimension) -> Option<RoundedDimension> {
        self.retries = self.retries.saturating_add(1);
        self.actual = new_actual;
        if self.should_retry() {
            tracing::trace!(target = ?self.target, "window drifted, correcting");
            Some(self.target)
        } else {
            if self.just_gave_up() {
                tracing::debug!("window can't be moved to {:?}", self.target);
            }
            None
        }
    }

    /// Whether drift retries are not yet exhausted.
    fn should_retry(&self) -> bool {
        self.retries <= MAX_ENFORCEMENT_RETRIES
    }

    /// Whether we just crossed the retry limit (for one-time logging).
    fn just_gave_up(&self) -> bool {
        self.retries == MAX_ENFORCEMENT_RETRIES + 1
    }

    /// Compare actual vs target, return constraint if size mismatched.
    fn detect_constraint(&self) -> Option<RawConstraint> {
        let (actual, target) = (self.actual, self.target);
        let min_w = (actual.width > target.width).then_some(actual.width as f32);
        let min_h = (actual.height > target.height).then_some(actual.height as f32);
        let max_w = (actual.width < target.width).then_some(actual.width as f32);
        let max_h = (actual.height < target.height).then_some(actual.height as f32);
        if min_w.is_some() || min_h.is_some() || max_w.is_some() || max_h.is_some() {
            tracing::trace!(
                ?target,
                ?actual,
                ?min_w,
                ?min_h,
                ?max_w,
                ?max_h,
                "window constrained"
            );
            Some(RawConstraint {
                min_width: min_w,
                min_height: min_h,
                max_width: max_w,
                max_height: max_h,
            })
        } else {
            None
        }
    }
}

pub(super) fn apply_inset(dim: Dimension, border: f32) -> Dimension {
    Dimension {
        x: dim.x + border,
        y: dim.y + border,
        width: (dim.width - 2.0 * border).max(0.0),
        height: (dim.height - 2.0 * border).max(0.0),
    }
}

/// Inverse of `apply_inset`: converts an observed content rect (post-inset, i32)
/// back to the outer frame stored in core's `float_windows`.
// TODO: revisit if config.border_size is ever non-integer -- round-trip can drift by +/-1 px per edge
fn reverse_inset(rounded: RoundedDimension, border: f32) -> Dimension {
    Dimension {
        x: rounded.x as f32 - border,
        y: rounded.y as f32 - border,
        width: rounded.width as f32 + 2.0 * border,
        height: rounded.height as f32 + 2.0 * border,
    }
}

struct RawConstraint {
    min_width: Option<f32>,
    min_height: Option<f32>,
    max_width: Option<f32>,
    max_height: Option<f32>,
}

/// Window position/size with integer coordinates. Integers are used for
/// pixel-exact comparison — floating-point coordinates would introduce rounding
/// ambiguity in drift detection.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) struct RoundedDimension {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

fn round_dim(dim: Dimension) -> RoundedDimension {
    RoundedDimension {
        x: dim.x.round() as i32,
        y: dim.y.round() as i32,
        width: dim.width.round() as i32,
        height: dim.height.round() as i32,
    }
}

/// Clip rect to bounds. Returns None if fully outside.
pub(super) fn clip_to_bounds(rect: Dimension, bounds: Dimension) -> Option<Dimension> {
    if rect.x >= bounds.x + bounds.width
        || rect.y >= bounds.y + bounds.height
        || rect.x + rect.width <= bounds.x
        || rect.y + rect.height <= bounds.y
    {
        return None;
    }
    let x = rect.x.max(bounds.x);
    let y = rect.y.max(bounds.y);
    let right = (rect.x + rect.width).min(bounds.x + bounds.width);
    let bottom = (rect.y + rect.height).min(bounds.y + bounds.height);
    Some(Dimension {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

pub(super) fn move_offscreen(
    monitors: &[MonitorInfo],
    actual: &RoundedDimension,
    ax: &dyn AXWindowApi,
) -> Result<()> {
    let (hidden_x, hidden_y) = hidden_position(monitors);
    // When spaces change or monitors are connected/disconnected, hidden windows
    // may be moved to visible state, so we need to re-hide them
    if actual.x == hidden_x || actual.y == hidden_y {
        return Ok(());
    }
    ax.hide_at(hidden_x, hidden_y)
}

/// Returns the monitor used for hiding windows offscreen.
/// We pick the monitor whose bottom-right corner is furthest from origin,
/// ensuring hidden windows are placed at a valid screen position that is
/// not visible on any other screen.
pub(super) fn hidden_monitor(monitors: &[MonitorInfo]) -> &MonitorInfo {
    monitors
        .iter()
        .max_by_key(|m| {
            (m.dimension.x + m.dimension.width) as i32 + (m.dimension.y + m.dimension.height) as i32
        })
        .unwrap()
}

fn hidden_position(monitors: &[MonitorInfo]) -> (i32, i32) {
    // MacOS doesn't allow completely set windows offscreen, so we need to leave at
    // least one pixel left
    // https://nikitabobko.github.io/AeroSpace/guide#emulation-of-virtual-workspaces
    let d = &hidden_monitor(monitors).dimension;
    ((d.x + d.width - 1.0) as i32, (d.y + d.height - 1.0) as i32)
}

impl Dome {
    #[tracing::instrument(skip(self))]
    pub(super) fn place_window(&mut self, window_id: WindowId, dim: Dimension) {
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        if window.is_moving {
            return;
        }
        let target = round_dim(dim);
        let is_float = self.hub.get_window(window_id).is_float();
        match &mut window.state {
            WindowState::Positioned(PositionedState::Tiling(p)) if !is_float => {
                if p.set_target(target)
                    && let Err(e) =
                        window
                            .ax
                            .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ax);
                }
            }
            WindowState::Positioned(PositionedState::Float(fp)) if is_float => {
                if fp.set_target(target)
                    && let Err(e) =
                        window
                            .ax
                            .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ax);
                }
            }
            // Cross-transitions: hub says float but platform is tiling, or vice versa.
            // Rebuild the placement state to match the hub's view.
            WindowState::Positioned(PositionedState::Tiling(_) | PositionedState::Float(_)) => {
                if is_float {
                    window.state = WindowState::Positioned(PositionedState::Float(
                        FloatPlacement::new(target),
                    ));
                } else {
                    window.state = WindowState::Positioned(PositionedState::Tiling(
                        Placement::new(target, target),
                    ));
                }
                if let Err(e) = window
                    .ax
                    .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ax);
                }
            }
            WindowState::Positioned(PositionedState::Offscreen(offscreen)) => {
                let actual = offscreen.actual;
                if is_float {
                    window.state = WindowState::Positioned(PositionedState::Float(
                        FloatPlacement::new(target),
                    ));
                } else {
                    window.state = WindowState::Positioned(PositionedState::Tiling(
                        Placement::new(actual, target),
                    ));
                }
                if let Err(e) = window
                    .ax
                    .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ax);
                }
            }
            _ => {
                debug_assert!(
                    false,
                    "We can only position windows in Positioned state, it seems core's state and platform's state differ"
                );
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn place_fullscreen_window(&mut self, window_id: WindowId, monitor_id: MonitorId) {
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        let monitor = self.monitor_registry.get_entry_mut(monitor_id);
        let screen_dim = monitor.screen.dimension;
        match &mut window.state {
            WindowState::Minimized | WindowState::UserMinimized => {
                if let Err(err) = window.ax.unminimize() {
                    tracing::trace!("Failed to unminimize window: {err:#}");
                }
                window.state = WindowState::BorderlessFullscreen
            }
            WindowState::Positioned(PositionedState::Offscreen(offscreen)) => {
                let actual = offscreen.actual;
                let target = round_dim(screen_dim);
                // Fullscreen is tiling-shaped: always use Tiling placement
                window.state = WindowState::Positioned(PositionedState::Tiling(Placement::new(
                    actual, target,
                )));
                if let Err(err) =
                    window
                        .ax
                        .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Failed to set fullscreen frame: {err:#}");
                }
            }
            WindowState::Positioned(PositionedState::Tiling(p)) => {
                let target = round_dim(screen_dim);
                if p.set_target(target)
                    && let Err(err) =
                        window
                            .ax
                            .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Failed to set fullscreen frame: {err:#}");
                }
            }
            WindowState::Positioned(PositionedState::Float(fp)) => {
                let target = round_dim(screen_dim);
                if fp.set_target(target)
                    && let Err(err) =
                        window
                            .ax
                            .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Failed to set fullscreen frame: {err:#}");
                }
            }
            // We don't touch OS managed fullscreen windows
            _ => {}
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn window_entered_native_fullscreen(&mut self, window_id: WindowId) {
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        window.state = WindowState::NativeFullscreen;
        self.hub
            .set_fullscreen(window.window_id, WindowRestrictions::ProtectFullscreen);
    }

    #[tracing::instrument(skip(self), fields(window = tracing::field::Empty))]
    pub(super) fn window_moved(
        &mut self,
        window_id: WindowId,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        observed_at: DebounceBurst,
    ) {
        let new_placement = RoundedDimension {
            x,
            y,
            width: w,
            height: h,
        };
        let monitors = self.monitor_registry.all_screens();
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        let monitor = self
            .monitor_registry
            .find_monitor_at(new_placement.x as f32, new_placement.y as f32);
        let is_borderless_fullscreen = monitor.is_some_and(|m| {
            let mon = &m.dimension;
            let tolerance = 2;
            (new_placement.x - mon.x as i32).abs() <= tolerance
                && (new_placement.y - mon.y as i32).abs() <= tolerance
                && (new_placement.width - mon.width as i32).abs() <= tolerance
                && (new_placement.height - mon.height as i32).abs() <= tolerance
        });

        tracing::Span::current().record("window", window.to_string());

        match &mut window.state {
            WindowState::Positioned(PositionedState::Offscreen(offscreen)) => {
                if is_borderless_fullscreen {
                    // Window turned fullscreen, but not visible, so we hide them again
                    self.hub
                        .set_fullscreen(window_id, WindowRestrictions::ProtectFullscreen);
                    window.state = WindowState::Minimized;
                    if let Err(e) = window.ax.minimize() {
                        tracing::trace!("Failed to minimize window: {e:#}");
                    }
                } else if offscreen.record_drift(new_placement, &monitors) {
                    if offscreen.should_retry() {
                        if let Err(e) = move_offscreen(&monitors, &offscreen.actual, &*window.ax) {
                            tracing::trace!("re-hide failed: {e}");
                        }
                    } else if offscreen.just_gave_up() {
                        tracing::debug!("Window {window} exhausted hide retries, giving up");
                    }
                }
            }
            WindowState::Positioned(PositionedState::Tiling(p)) => {
                // Stale check: if even the latest notification predates the
                // last placement, the burst carries only pre-placement state
                // and must be ignored. A burst that straddles placed_at
                // (observed_at.first < placed_at <= observed_at.last) is kept, since
                // at least one notification fired post-placement.
                if observed_at.last < p.placed_at {
                    tracing::trace!(placed_at = ?p.placed_at, "stale observation, ignoring");
                    return;
                }

                if new_placement == p.target {
                    p.actual = new_placement;
                    return;
                }

                if is_borderless_fullscreen {
                    window.state = WindowState::BorderlessFullscreen;
                    self.hub
                        .set_fullscreen(window_id, WindowRestrictions::ProtectFullscreen);
                    return;
                }

                // If the debounced events start within 1s of set_frame call, this is likely to be
                // caused by the set_frame call, or at least the set_frame call was debounced
                // alongside a previous burst, which is essentially the same.
                if observed_at.first <= p.placed_at + Duration::from_secs(1) {
                    if p.has_drifted(new_placement) {
                        if let Some(target) = p.observe_drift(new_placement)
                            && let Err(e) =
                                window
                                    .ax
                                    .set_frame(target.x, target.y, target.width, target.height)
                        {
                            tracing::trace!("Window {} set_frame failed: {e}", window);
                        }
                        return;
                    }

                    p.actual = new_placement;
                    let Some(c) = p.detect_constraint() else {
                        return;
                    };
                    // Convert actual window size back to frame size by adding border back.
                    // Frame dimensions have border inset applied. If in the original frame,
                    // window width is smaller than sum of borders, then we will request a size
                    // that can accommodate the borders here.
                    let remove_inset = |v: f32| v + 2.0 * self.config.border_size;
                    self.hub.set_window_constraint(
                        window_id,
                        c.min_width.map(remove_inset),
                        c.min_height.map(remove_inset),
                        c.max_width.map(remove_inset),
                        c.max_height.map(remove_inset),
                    );
                } else {
                    // This is likely not caused by Dome calling AX's set_frame but by app
                    // resizing itself or user move actions.
                    if let Some(target) = p.observe_drift(new_placement)
                        && let Err(e) =
                            window
                                .ax
                                .set_frame(target.x, target.y, target.width, target.height)
                    {
                        tracing::trace!("Window {} set_frame failed: {e}", window);
                    }
                }
            }
            WindowState::Positioned(PositionedState::Float(fp)) => {
                // Stale check against the last outbound set_frame timestamp.
                if observed_at.last < fp.placed_at {
                    tracing::trace!(placed_at = ?fp.placed_at, "stale observation, ignoring");
                    return;
                }

                if new_placement == fp.target {
                    return;
                }

                if is_borderless_fullscreen {
                    window.state = WindowState::BorderlessFullscreen;
                    self.hub
                        .set_fullscreen(window_id, WindowRestrictions::ProtectFullscreen);
                    return;
                }

                // Float accepts the OS-reported position as ground truth.
                // Write target directly -- placed_at is NOT bumped because
                // this is an observation, not an outbound set_frame.
                fp.target = new_placement;
                let outer_dim = reverse_inset(new_placement, self.config.border_size);
                self.hub.update_float_dimension(window_id, outer_dim);
            }
            WindowState::Minimized => {
                // Window somehow got brought back to screen, maybe through window focused but the
                // notification was not fired
                tracing::trace!("Previously minimized borderless fullscreen window reappeared");
                if is_borderless_fullscreen {
                    if let Err(e) = window.ax.minimize() {
                        tracing::trace!("Failed to minimize window: {e:#}");
                    }
                }
                // No longer fullscreen borderless, so bring them back and put in offscreen
                else {
                    if let Err(e) = window.ax.unminimize() {
                        tracing::debug!("Failed to unminimize window: {e:#}");
                    }
                    let offscreen = OffscreenPlacement::new(new_placement);
                    if let Err(e) = move_offscreen(&monitors, &offscreen.actual, &*window.ax) {
                        tracing::trace!("hide after unminimize failed: {e}");
                    }
                    window.state = WindowState::Positioned(PositionedState::Offscreen(offscreen));
                    self.hub.unset_fullscreen(window_id);
                }
            }
            WindowState::BorderlessFullscreen => {
                // No longer border borderless fullscreen. Move to offscreen position as these
                // windows might now be inserted offscreen, which will be put back into view later
                // if it's in view
                if !is_borderless_fullscreen {
                    window.state = WindowState::Positioned(PositionedState::Offscreen(
                        OffscreenPlacement::new(new_placement),
                    ));
                    self.hub.unset_fullscreen(window_id);
                }
            }
            WindowState::NativeFullscreen => {
                if is_borderless_fullscreen {
                    if self.monitor_registry.is_displayed(window_id) {
                        window.state = WindowState::BorderlessFullscreen;
                    } else {
                        // Window exited native fullscreen on an unfocused workspace.
                        // Minimize it so it doesn't stay visible over the active workspace.
                        window.state = WindowState::Minimized;
                        if let Err(e) = window.ax.minimize() {
                            tracing::trace!("Failed to minimize window: {e:#}");
                        }
                    }
                } else {
                    window.state = WindowState::Positioned(PositionedState::Offscreen(
                        OffscreenPlacement::new(new_placement),
                    ));
                    self.hub.unset_fullscreen(window_id);
                }
            }
            // User-minimized windows should not trigger move logic.
            WindowState::UserMinimized => {}
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn hide_window(&mut self, window_id: WindowId) {
        let monitors = self.monitor_registry.all_screens();
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        // Minimize borderless fullscreen windows instead of moving offscreen:
        // 1. User-zoomed windows maintain their fullscreen state, so moving them is futile
        // 2. Moving offscreen triggers handle_window_moved which detects fullscreen exit
        // Native fullscreen windows are on a separate Space and don't need hiding.
        let result = match &window.state {
            WindowState::BorderlessFullscreen => {
                window.state = WindowState::Minimized;
                window.ax.minimize()
            }
            WindowState::NativeFullscreen | WindowState::Minimized | WindowState::UserMinimized => {
                Ok(())
            }
            WindowState::Positioned(positioned_state) => match positioned_state {
                PositionedState::Tiling(placement) => {
                    let offscreen = OffscreenPlacement::new(placement.actual);
                    let result = move_offscreen(&monitors, &offscreen.actual, &*window.ax);
                    window.state = WindowState::Positioned(PositionedState::Offscreen(offscreen));
                    result
                }
                PositionedState::Float(fp) => {
                    // Post-sync: fp.target is the last observed rect
                    let offscreen = OffscreenPlacement::new(fp.target);
                    let result = move_offscreen(&monitors, &offscreen.actual, &*window.ax);
                    window.state = WindowState::Positioned(PositionedState::Offscreen(offscreen));
                    result
                }
                PositionedState::Offscreen(offscreen) => {
                    move_offscreen(&monitors, &offscreen.actual, &*window.ax)
                }
            },
        };
        if let Err(e) = result {
            tracing::trace!("Failed to hide window: {e:#}");
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn move_window_offscreen(&mut self, window_id: WindowId) {
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        let WindowState::Positioned(positioned_state) = window.state else {
            debug_assert!(
                false,
                "Can only move windows which dome control the positions offscreen"
            );
            return;
        };
        let monitors = self.monitor_registry.all_screens();
        match positioned_state {
            PositionedState::Tiling(placement) => {
                let offscreen = OffscreenPlacement::new(placement.actual);
                if let Err(e) = move_offscreen(&monitors, &offscreen.actual, &*window.ax) {
                    tracing::debug!(%window_id, "Failed to move window offscreen: {e}");
                }
                window.state = WindowState::Positioned(PositionedState::Offscreen(offscreen));
            }
            PositionedState::Float(fp) => {
                // Post-sync: fp.target is the last observed rect
                let offscreen = OffscreenPlacement::new(fp.target);
                if let Err(e) = move_offscreen(&monitors, &offscreen.actual, &*window.ax) {
                    tracing::debug!(%window_id, "Failed to move window offscreen: {e}");
                }
                window.state = WindowState::Positioned(PositionedState::Offscreen(offscreen));
            }
            PositionedState::Offscreen(offscreen) => {
                if let Err(e) = move_offscreen(&monitors, &offscreen.actual, &*window.ax) {
                    tracing::debug!(%window_id, "Failed to move window offscreen: {e}");
                }
            }
        }
    }

    pub(super) fn rehide_offscreen_windows(&self, screens: &[MonitorInfo]) {
        for (_, entry) in self.registry.iter() {
            if let WindowState::Positioned(PositionedState::Offscreen(offscreen)) = &entry.state
                && let Err(e) = move_offscreen(screens, &offscreen.actual, &*entry.ax)
            {
                tracing::trace!("Failed to re-hide window: {e:#}");
            }
        }
    }
}
