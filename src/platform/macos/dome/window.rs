use std::time::Instant;

use anyhow::Result;

use crate::core::Dimension;
use crate::platform::macos::MonitorInfo;
use crate::platform::macos::accessibility::AXWindowApi;

pub(super) const MAX_DRIFT_RETRIES: u8 = 5;

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
    /// untracked and removed from Dome.
    Minimized,
}

#[derive(Clone, Copy)]
pub(super) enum PositionedState {
    /// Window is moved offscreen by Dome. `actual` is the last observed position, may differ from
    /// the current hidden coordinates if monitors changed since the window was hidden.
    Offscreen { actual: RoundedDimension },
    /// Window is tiled or floating with an active placement target.
    InView(Placement),
}

#[derive(Clone, Copy)]
pub(super) struct Placement {
    pub(super) target: RoundedDimension,
    pub(super) actual: RoundedDimension,
    pub(super) retries: u8,
    pub(super) placed_at: Instant,
}

impl Placement {
    pub(super) fn new(actual: RoundedDimension, target: RoundedDimension) -> Self {
        Self {
            target,
            actual,
            retries: 0,
            placed_at: Instant::now(),
        }
    }

    /// Record a new target. Returns true if set_frame is needed.
    pub(super) fn set_target(&mut self, target: RoundedDimension) -> bool {
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
    /// Check edge alignment and track retries. Returns true if this was a
    /// drift (edges not aligned). Caller should check `should_retry()` to
    /// decide whether to issue set_frame.
    pub(super) fn record_drift(&mut self, new_actual: RoundedDimension) -> bool {
        let target = self.target;
        let left = new_actual.x == target.x;
        let right = new_actual.x + new_actual.width == target.x + target.width;
        let top = new_actual.y == target.y;
        let bottom = new_actual.y + new_actual.height == target.y + target.height;

        if (left || right) && (top || bottom) {
            return false;
        }

        self.retries = self.retries.saturating_add(1);
        self.actual = new_actual;
        true
    }

    /// Whether drift retries are not yet exhausted.
    pub(super) fn should_retry(&self) -> bool {
        self.retries <= MAX_DRIFT_RETRIES
    }

    /// Whether we just crossed the retry limit (for one-time logging).
    pub(super) fn just_gave_up(&self) -> bool {
        self.retries == MAX_DRIFT_RETRIES + 1
    }

    /// Compare actual vs target, return constraint if size mismatched.
    pub(super) fn detect_constraint(&self) -> Option<RawConstraint> {
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

pub(super) struct RawConstraint {
    pub(super) min_width: Option<f32>,
    pub(super) min_height: Option<f32>,
    pub(super) max_width: Option<f32>,
    pub(super) max_height: Option<f32>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) struct RoundedDimension {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

pub(super) fn round_dim(dim: Dimension) -> RoundedDimension {
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
    // TODO: if hide fail to move the window to offscreen position, this window is clearly
    // trying to take focus, so we should pop it to float or something. Exception is full
    // screen window, which, should be handled differently as a first party citizen
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
