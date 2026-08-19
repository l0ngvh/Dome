use std::time::{Duration, Instant};

use anyhow::Result;

use crate::core::{
    Length, LimitObservation, LimitUpdate, MonitorId, PixelRect, Pixels, WindowId,
    WindowRestrictions,
};
use crate::platform::macos::MonitorInfo;
use crate::platform::macos::accessibility::ExternalWindow;

use super::{DebounceBurst, Dome, NewWindow};

const MAX_ENFORCEMENT_RETRIES: u8 = 5;

#[derive(Clone, Copy)]
pub(super) enum WindowState {
    Positioned(PositionedState),
    /// Window is in a macOS native fullscreen Space.
    NativeFullscreen,
    /// Window was zoomed to fill the screen via the zoom button or similar.
    /// Distinct from native fullscreen — no separate Space is created.
    BorderlessFullscreen,
    /// Borderless-fullscreen window currently minimized by Dome because its workspace is inactive.
    BorderlessMinimized {
        retries: u8,
    },
}

#[derive(Clone, Copy)]
pub(super) enum PositionedState {
    /// Window is moved offscreen by Dome. `actual` is the last observed position, may differ from
    /// the current hidden coordinates if monitors changed since the window was hidden.
    Offscreen(OffscreenPlacement),
    Tiling(Placement),
    Float(FloatPlacement),
}

#[derive(Clone, Copy)]
pub(super) struct OffscreenPlacement {
    actual: PixelRect,
    retries: u8,
}

impl OffscreenPlacement {
    pub(super) fn new(actual: PixelRect) -> Self {
        Self { actual, retries: 0 }
    }

    /// Updates `actual` unconditionally. Returns true if the window is NOT at
    /// the hidden position (i.e. it fought back).
    fn record_drift(&mut self, new_actual: PixelRect, monitors: &[MonitorInfo]) -> bool {
        self.actual = new_actual;
        let (hidden_x, hidden_y) = hidden_position(monitors);
        if new_actual.x() == hidden_x || new_actual.y() == hidden_y {
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
    target: PixelRect,
    actual: PixelRect,
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
    /// `set_frame` or adopted from a drag observation.
    pub(super) target: PixelRect,
    /// When `target` was last bumped by an outbound `set_frame`. User-drag
    /// observations do NOT bump this: they write `target` without issuing
    /// `set_frame`, so the filter anchor stays on the last outbound call.
    placed_at: Instant,
}

impl FloatPlacement {
    pub(super) fn new(target: PixelRect) -> Self {
        Self {
            target,
            placed_at: Instant::now(),
        }
    }

    /// Returns true if set_frame is needed.
    fn set_target(&mut self, target: PixelRect) -> bool {
        if self.target == target {
            return false;
        }
        self.target = target;
        self.placed_at = Instant::now();
        true
    }
}

impl Placement {
    fn new(actual: PixelRect, target: PixelRect) -> Self {
        Self {
            target,
            actual,
            retries: 0,
            placed_at: Instant::now(),
        }
    }

    /// Returns true if set_frame is needed.
    fn set_target(&mut self, target: PixelRect) -> bool {
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
    /// Returns true if `new_actual` has at least one vertical *and* one
    /// horizontal edge misaligned with the target (i.e. this is drift, not
    /// just an edge-anchored size delta). The caller must follow up with
    /// `observe_drift` to consume a retry.
    fn has_drifted(&self, new_actual: PixelRect) -> bool {
        let target = self.target;
        let left = new_actual.x() == target.x();
        let right = new_actual.right() == target.right();
        let top = new_actual.y() == target.y();
        let bottom = new_actual.bottom() == target.bottom();
        !((left || right) && (top || bottom))
    }

    /// Returns the target to re-issue via `set_frame` while retries remain, and
    /// `None` once the budget is exhausted.
    fn observe_drift(&mut self, new_actual: PixelRect) -> Option<PixelRect> {
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

    fn should_retry(&self) -> bool {
        self.retries <= MAX_ENFORCEMENT_RETRIES
    }

    /// Whether we just crossed the retry limit (for one-time logging).
    fn just_gave_up(&self) -> bool {
        self.retries == MAX_ENFORCEMENT_RETRIES + 1
    }

    fn detect_constraint(&self) -> Option<LimitObservation> {
        let (actual, target) = (self.actual, self.target);
        let min_w = (actual.width() > target.width()).then_some(actual.width());
        let min_h = (actual.height() > target.height()).then_some(actual.height());
        let max_w = (actual.width() < target.width()).then_some(actual.width());
        let max_h = (actual.height() < target.height()).then_some(actual.height());
        if min_w.is_none() && min_h.is_none() && max_w.is_none() && max_h.is_none() {
            return None;
        }
        tracing::trace!(
            ?target,
            ?actual,
            ?min_w,
            ?min_h,
            ?max_w,
            ?max_h,
            "window constrained"
        );
        // AX reports the content box, which is the space core stores limits in,
        // so the observation needs no border conversion.
        let observed = |v: Option<Pixels>| {
            v.map_or(LimitUpdate::Unchanged, |v| {
                LimitUpdate::Set(Length::from_pixels(v))
            })
        };
        Some(LimitObservation {
            min_width: observed(min_w),
            min_height: observed(min_h),
            max_width: observed(max_w),
            max_height: observed(max_h),
        })
    }
}

pub(super) fn move_offscreen(
    monitors: &[MonitorInfo],
    actual: &PixelRect,
    ax: &dyn ExternalWindow,
) -> Result<()> {
    let (hidden_x, hidden_y) = hidden_position(monitors);
    // When spaces change or monitors are connected/disconnected, hidden windows
    // may be moved to visible state, so we need to re-hide them
    if actual.x() == hidden_x || actual.y() == hidden_y {
        return Ok(());
    }
    ax.hide_at(Length::from_pixels(hidden_x), Length::from_pixels(hidden_y))
}

/// We pick the monitor whose bottom-right corner is furthest from origin,
/// ensuring hidden windows are placed at a valid screen position that is
/// not visible on any other screen.
pub(super) fn hidden_monitor(monitors: &[MonitorInfo]) -> &MonitorInfo {
    monitors
        .iter()
        .max_by_key(|m| {
            let work_area = m.work_area;
            work_area.right() + work_area.bottom()
        })
        .unwrap()
}

fn hidden_position(monitors: &[MonitorInfo]) -> (Pixels, Pixels) {
    // MacOS doesn't allow completely set windows offscreen, so we need to leave at
    // least one pixel left
    // https://nikitabobko.github.io/AeroSpace/guide#emulation-of-virtual-workspaces
    let work_area = hidden_monitor(monitors).work_area;
    (
        work_area.right() - Pixels::new(1),
        work_area.bottom() - Pixels::new(1),
    )
}

impl Dome {
    #[tracing::instrument(skip_all, fields(window = %new))]
    pub(super) fn add_native_fullscreen_window(&mut self, new: NewWindow) -> Option<WindowId> {
        let window_id = self.hub.insert_window(
            Box::new(new.metadata.clone()),
            PixelRect::new(0, 0, 1, 1),
            WindowRestrictions::ProtectFullscreen,
        )?;
        self.registry
            .insert(new, window_id, WindowState::NativeFullscreen);
        self.pending_created.push(window_id);
        tracing::info!(%window_id, "New native fullscreen window");
        Some(window_id)
    }

    #[tracing::instrument(skip(self), fields(window = tracing::field::Empty))]
    pub(super) fn show_tiling(&mut self, window_id: WindowId, target: PixelRect) {
        debug_assert!(
            !target.is_empty(),
            "caller must guard against an empty content box"
        );
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        tracing::Span::current().record("window", window.to_string());
        if window.is_moving {
            return;
        }
        // User-minimized window being restored via focus_window_by_cg.
        if window.is_minimized {
            window.is_minimized = false;
            if let Err(e) = window.ext.unminimize() {
                tracing::trace!("Failed to unminimize window: {e:#}");
            }
        }
        match &mut window.state {
            WindowState::Positioned(PositionedState::Tiling(p)) => {
                if p.set_target(target)
                    && let Err(e) = window.ext.set_frame(target)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ext);
                }
            }
            // The window just toggled tiling-ward in core, so rebuild as Tiling.
            WindowState::Positioned(PositionedState::Float(_)) => {
                window.state = WindowState::Positioned(PositionedState::Tiling(Placement::new(
                    target, target,
                )));
                if let Err(e) = window.ext.set_frame(target) {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ext);
                }
            }
            WindowState::Positioned(PositionedState::Offscreen(offscreen)) => {
                // Preserve the captured actual position from the offscreen state
                // so drift correction starts from a real coordinate.
                let actual = offscreen.actual;
                window.state = WindowState::Positioned(PositionedState::Tiling(Placement::new(
                    actual, target,
                )));
                if let Err(e) = window.ext.set_frame(target) {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ext);
                }
            }
            WindowState::NativeFullscreen => {
                unreachable!("Native fullscreen windows must be set by `place_fullscreen_window`")
            }
            WindowState::BorderlessFullscreen => {
                unreachable!(
                    "Borderless fullscreen windows must be set by `place_fullscreen_window`"
                )
            }
            WindowState::BorderlessMinimized { .. } => {
                unreachable!("BorderlessMinimized windows must be set by `place_fullscreen_window`")
            }
        }
    }

    #[tracing::instrument(skip(self), fields(window = tracing::field::Empty))]
    pub(super) fn show_float(&mut self, window_id: WindowId, target: PixelRect) {
        debug_assert!(
            !target.is_empty(),
            "caller must guard against an empty content box"
        );
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        tracing::Span::current().record("window", window.to_string());
        if window.is_moving {
            return;
        }
        // User-minimized window being restored via focus_window_by_cg.
        if window.is_minimized {
            window.is_minimized = false;
            if let Err(e) = window.ext.unminimize() {
                tracing::trace!("Failed to unminimize window: {e:#}");
            }
        }
        match &mut window.state {
            WindowState::Positioned(PositionedState::Float(fp)) => {
                if fp.set_target(target)
                    && let Err(e) = window.ext.set_frame(target)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ext);
                }
            }
            WindowState::Positioned(PositionedState::Tiling(_) | PositionedState::Offscreen(_)) => {
                window.state =
                    WindowState::Positioned(PositionedState::Float(FloatPlacement::new(target)));
                if let Err(e) = window.ext.set_frame(target) {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ext);
                }
            }
            WindowState::NativeFullscreen => {
                unreachable!("Native fullscreen windows must be set by `place_fullscreen_window`")
            }
            WindowState::BorderlessFullscreen => {
                unreachable!(
                    "Borderless fullscreen windows must be set by `place_fullscreen_window`"
                )
            }
            WindowState::BorderlessMinimized { .. } => {
                unreachable!("BorderlessMinimized windows must be set by `place_fullscreen_window`")
            }
        }
    }

    #[tracing::instrument(skip(self), fields(window = tracing::field::Empty))]
    pub(super) fn place_fullscreen_window(&mut self, window_id: WindowId, monitor_id: MonitorId) {
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        tracing::Span::current().record("window", window.to_string());
        let monitor = self.monitor_registry.monitor(monitor_id);
        let target = monitor.work_area();
        match &mut window.state {
            WindowState::BorderlessMinimized { .. } => {
                if let Err(err) = window.ext.unminimize() {
                    tracing::trace!("Failed to unminimize window: {err:#}");
                }
                window.state = WindowState::BorderlessFullscreen
            }
            WindowState::Positioned(PositionedState::Offscreen(offscreen)) => {
                let actual = offscreen.actual;
                // Fullscreen is tiling-shaped: always use Tiling placement
                window.state = WindowState::Positioned(PositionedState::Tiling(Placement::new(
                    actual, target,
                )));
                if let Err(err) = window.ext.set_frame(target) {
                    tracing::trace!("Failed to set fullscreen frame: {err:#}");
                }
            }
            WindowState::Positioned(PositionedState::Tiling(p)) => {
                if p.set_target(target)
                    && let Err(err) = window.ext.set_frame(target)
                {
                    tracing::trace!("Failed to set fullscreen frame: {err:#}");
                }
            }
            WindowState::Positioned(PositionedState::Float(fp)) => {
                if fp.set_target(target)
                    && let Err(err) = window.ext.set_frame(target)
                {
                    tracing::trace!("Failed to set fullscreen frame: {err:#}");
                }
            }
            // We can't/don't need to touch native fullscreen windows
            WindowState::NativeFullscreen => {}
            // We shouldn't touch borderless fullscreen windows, sometimes they can be aggressive
            // and cause infinite move/set position loop even though it's the same size
            WindowState::BorderlessFullscreen => {}
        }
    }

    #[tracing::instrument(skip(self), fields(window = tracing::field::Empty))]
    pub(super) fn window_entered_native_fullscreen(&mut self, window_id: WindowId) {
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        tracing::Span::current().record("window", window.to_string());
        let was_minimized = window.is_minimized;
        if was_minimized {
            self.hub.unminimize_window(window_id);
            if let Some(window) = self.registry.by_id_mut(window_id) {
                window.is_minimized = false;
            }
        }
        if let Some(window) = self.registry.by_id_mut(window_id) {
            window.state = WindowState::NativeFullscreen;
        }
        self.hub
            .set_fullscreen(window_id, WindowRestrictions::ProtectFullscreen);
    }

    #[tracing::instrument(skip(self), fields(window = tracing::field::Empty))]
    pub(super) fn window_moved(
        &mut self,
        window_id: WindowId,
        new_placement: PixelRect,
        observed_at: DebounceBurst,
    ) {
        let is_borderless_fullscreen = self.is_borderless_fullscreen_at(new_placement);
        let monitors = self.monitor_registry.all_monitors();
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };

        tracing::Span::current().record("window", window.to_string());

        // User manually brought a minimized window back to screen
        if window.is_minimized {
            self.hub.unminimize_window(window_id);
            window.is_minimized = false;
        }

        match &mut window.state {
            WindowState::Positioned(PositionedState::Offscreen(offscreen)) => {
                if is_borderless_fullscreen {
                    // Window turned fullscreen, but not visible, so we hide it again.
                    self.hub
                        .set_fullscreen(window_id, WindowRestrictions::ProtectFullscreen);
                    window.state = WindowState::BorderlessMinimized { retries: 0 };
                    if let Err(e) = window.ext.minimize() {
                        tracing::trace!("Failed to minimize window: {e:#}");
                    }
                } else if offscreen.record_drift(new_placement, &monitors) {
                    if offscreen.should_retry() {
                        if let Err(e) = move_offscreen(&monitors, &offscreen.actual, &*window.ext) {
                            tracing::trace!("re-hide failed: {e}");
                        }
                    } else if offscreen.just_gave_up() {
                        tracing::debug!("Window {window} exhausted hide retries, giving up");
                    }
                }
            }
            WindowState::Positioned(PositionedState::Tiling(p)) => {
                // A burst that straddles placed_at (observed_at.first < placed_at
                // <= observed_at.last) is kept, since at least one notification fired
                // post-placement.
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
                            && let Err(e) = window.ext.set_frame(target)
                        {
                            tracing::trace!("Window {} set_frame failed: {e}", window);
                        }
                        return;
                    }

                    p.actual = new_placement;
                    let Some(observation) = p.detect_constraint() else {
                        return;
                    };
                    self.hub.set_window_constraint(window_id, observation);
                } else {
                    // This is likely not caused by Dome calling AX's set_frame but by app
                    // resizing itself or user move actions.
                    if let Some(target) = p.observe_drift(new_placement)
                        && let Err(e) = window.ext.set_frame(target)
                    {
                        tracing::trace!("Window {} set_frame failed: {e}", window);
                    }
                }
            }
            WindowState::Positioned(PositionedState::Float(fp)) => {
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

                fp.target = new_placement;
                let monitor_id = self
                    .monitor_registry
                    .find_closest_monitor(new_placement.to_dimension())
                    .map(|m| m.id())
                    .unwrap_or_else(|| self.monitor_registry.primary_monitor_id());
                self.hub
                    .update_float_rect(window_id, new_placement, monitor_id);
            }
            WindowState::BorderlessMinimized { retries } => {
                tracing::trace!("Previously minimized borderless fullscreen window reappeared");
                if is_borderless_fullscreen {
                    *retries = retries.saturating_add(1);
                    if *retries > MAX_ENFORCEMENT_RETRIES {
                        if *retries == MAX_ENFORCEMENT_RETRIES + 1 {
                            tracing::debug!(%window_id, "BorderlessMinimized resurface retries exhausted, giving up");
                        }
                        return;
                    }
                    if let Err(e) = window.ext.minimize() {
                        tracing::trace!("Failed to minimize window: {e:#}");
                    }
                } else {
                    if let Err(e) = window.ext.unminimize() {
                        tracing::debug!("Failed to unminimize window: {e:#}");
                    }
                    let offscreen = OffscreenPlacement::new(new_placement);
                    if let Err(e) = move_offscreen(&monitors, &offscreen.actual, &*window.ext) {
                        tracing::trace!("hide after unminimize failed: {e}");
                    }
                    window.state = WindowState::Positioned(PositionedState::Offscreen(offscreen));
                    self.hub.unset_fullscreen(window_id);
                }
            }
            WindowState::BorderlessFullscreen => {
                // Move to offscreen since the window may belong to a hidden
                // workspace and will be placed back into view by flush_layout
                // if it belongs to the active one.
                if !is_borderless_fullscreen {
                    window.state = WindowState::Positioned(PositionedState::Offscreen(
                        OffscreenPlacement::new(new_placement),
                    ));
                    self.hub.unset_fullscreen(window_id);
                }
            }
            WindowState::NativeFullscreen => {
                if is_borderless_fullscreen {
                    if self.displayed_windows.contains(&window_id) {
                        window.state = WindowState::BorderlessFullscreen;
                    } else {
                        // Window exited native fullscreen on an unfocused workspace.
                        // Hide via BorderlessMinimized so it does not stay visible.
                        window.state = WindowState::BorderlessMinimized { retries: 0 };
                        if let Err(e) = window.ext.minimize() {
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
        }
    }

    #[tracing::instrument(skip(self), fields(window = tracing::field::Empty))]
    pub(super) fn hide_window(&mut self, window_id: WindowId) {
        let monitors = self.monitor_registry.all_monitors();
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        tracing::Span::current().record("window", window.to_string());
        if window.is_minimized {
            return;
        }
        // Minimize borderless fullscreen windows instead of moving offscreen:
        // 1. User-zoomed windows maintain their fullscreen state, so moving them is futile
        // 2. Moving offscreen triggers handle_window_moved which detects fullscreen exit
        let result = match &window.state {
            WindowState::BorderlessFullscreen => {
                window.state = WindowState::BorderlessMinimized { retries: 0 };
                window.ext.minimize()
            }
            WindowState::NativeFullscreen | WindowState::BorderlessMinimized { .. } => Ok(()),
            WindowState::Positioned(positioned_state) => match positioned_state {
                PositionedState::Tiling(placement) => {
                    let offscreen = OffscreenPlacement::new(placement.actual);
                    let result = move_offscreen(&monitors, &offscreen.actual, &*window.ext);
                    window.state = WindowState::Positioned(PositionedState::Offscreen(offscreen));
                    result
                }
                PositionedState::Float(fp) => {
                    let offscreen = OffscreenPlacement::new(fp.target);
                    let result = move_offscreen(&monitors, &offscreen.actual, &*window.ext);
                    window.state = WindowState::Positioned(PositionedState::Offscreen(offscreen));
                    result
                }
                PositionedState::Offscreen(offscreen) => {
                    move_offscreen(&monitors, &offscreen.actual, &*window.ext)
                }
            },
        };
        if let Err(e) = result {
            tracing::trace!("Failed to hide window: {e:#}");
        }
    }

    #[tracing::instrument(skip(self), fields(window = tracing::field::Empty))]
    pub(super) fn move_window_offscreen(&mut self, window_id: WindowId) {
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        tracing::Span::current().record("window", window.to_string());
        let WindowState::Positioned(positioned_state) = window.state else {
            unreachable!("Can only move windows which dome control the positions offscreen");
        };
        let monitors = self.monitor_registry.all_monitors();
        match positioned_state {
            PositionedState::Tiling(placement) => {
                let offscreen = OffscreenPlacement::new(placement.actual);
                if let Err(e) = move_offscreen(&monitors, &offscreen.actual, &*window.ext) {
                    tracing::debug!(%window_id, "Failed to move window offscreen: {e}");
                }
                window.state = WindowState::Positioned(PositionedState::Offscreen(offscreen));
            }
            PositionedState::Float(fp) => {
                let offscreen = OffscreenPlacement::new(fp.target);
                if let Err(e) = move_offscreen(&monitors, &offscreen.actual, &*window.ext) {
                    tracing::debug!(%window_id, "Failed to move window offscreen: {e}");
                }
                window.state = WindowState::Positioned(PositionedState::Offscreen(offscreen));
            }
            PositionedState::Offscreen(offscreen) => {
                if let Err(e) = move_offscreen(&monitors, &offscreen.actual, &*window.ext) {
                    tracing::debug!(%window_id, "Failed to move window offscreen: {e}");
                }
            }
        }
    }

    pub(super) fn rehide_offscreen_windows(&self, monitors: &[MonitorInfo]) {
        for (_, entry) in self.registry.iter() {
            if let WindowState::Positioned(PositionedState::Offscreen(offscreen)) = &entry.state
                && let Err(e) = move_offscreen(monitors, &offscreen.actual, &*entry.ext)
            {
                tracing::trace!("Failed to re-hide window: {e:#}");
            }
        }
    }

    pub(super) fn minimize_window(&mut self, window_id: WindowId) {
        let window = self.registry.by_id_mut(window_id).unwrap();
        self.hub.minimize_window(window_id);
        window.is_minimized = true;
    }

    pub(super) fn is_borderless_fullscreen_at(&self, rect: PixelRect) -> bool {
        self.monitor_registry.is_borderless_fullscreen_at(rect)
    }
}
