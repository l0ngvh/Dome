use std::time::Instant;

use anyhow::Result;

use super::monitor::MonitorInfo;
use crate::core::{Dimension, MonitorId, WindowId};
use crate::platform::macos::Dome;
use crate::platform::macos::accessibility::AXWindow;

const MAX_DRIFT_RETRIES: u8 = 5;

impl Dome {
    pub(super) fn add_window(&mut self, ax: AXWindow, dim: RoundedDimension) -> WindowId {
        let monitor = self
            .monitor_registry
            .find_monitor_at(dim.x as f32, dim.y as f32);
        let is_borderless_fullscreen = monitor.is_some_and(|m| {
            let mon = &m.dimension;
            let tolerance = 2;
            (dim.x - mon.x as i32).abs() <= tolerance
                && (dim.y - mon.y as i32).abs() <= tolerance
                && (dim.width - mon.width as i32).abs() <= tolerance
                && (dim.height - mon.height as i32).abs() <= tolerance
        });
        if is_borderless_fullscreen {
            let window_id = self.hub.insert_fullscreen();
            self.registry
                .insert(ax.clone(), window_id, WindowState::BorderlessFullscreen);
            tracing::info!(%window_id, "New borderless fullscreen window");
            window_id
        } else {
            let window_id = self.hub.insert_tiling();
            self.registry.insert(
                ax.clone(),
                window_id,
                WindowState::Positioned(PositionedState::Offscreen { actual: dim }),
            );
            tracing::info!(%window_id, "New tiling window");
            window_id
        }
    }

    pub(super) fn add_native_fullscreen_window(&mut self, ax: AXWindow) -> WindowId {
        let window_id = self.hub.insert_fullscreen();
        self.registry
            .insert(ax, window_id, WindowState::NativeFullscreen);
        tracing::info!(%window_id, "New native fullscreen window");
        window_id
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn place_window(&mut self, window_id: WindowId, dim: Dimension) {
        let window = self.registry.by_id_mut(window_id);
        if self.placement_tracker.is_moving(window.ax.pid()) {
            return;
        }
        let WindowState::Positioned(positioned_state) = window.state else {
            debug_assert!(
                false,
                "We can only position windows in Positioned state, it seems core's state and platform's state differ"
            );
            return;
        };

        let target = round_dim(dim);
        match positioned_state {
            PositionedState::InView(mut p) => {
                if p.set_target(target)
                    && let Err(e) =
                        window
                            .ax
                            .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ax);
                };
            }
            PositionedState::Offscreen { actual } => {
                window.state = WindowState::Positioned(PositionedState::InView(Placement::new(
                    actual, target,
                )));
                if let Err(e) = window
                    .ax
                    .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ax);
                }
            }
        };
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn place_fullscreen_window(&mut self, window_id: WindowId, monitor_id: MonitorId) {
        let window = self.registry.by_id_mut(window_id);
        let monitor = self.monitor_registry.get_entry_mut(monitor_id);
        let screen_dim = monitor.screen.dimension;
        match window.state {
            WindowState::Minimized => {
                if let Err(err) = window.ax.unminimize() {
                    tracing::trace!("Failed to unminimize window: {err:#}");
                }
                window.state = WindowState::BorderlessFullscreen
            }
            WindowState::Positioned(PositionedState::Offscreen { actual }) => {
                let target = round_dim(screen_dim);
                window.state = WindowState::Positioned(PositionedState::InView(Placement::new(
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
            WindowState::Positioned(PositionedState::InView(mut p)) => {
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
            // We don't touch OS managed fullscreen windows
            _ => {}
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn window_entered_native_fullscreen(&mut self, window_id: WindowId) {
        let window = self.registry.by_id_mut(window_id);

        window.state = WindowState::NativeFullscreen;
        self.hub.set_fullscreen(window.window_id);
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn window_moved(
        &mut self,
        window_id: WindowId,
        new_placement: RoundedDimension,
        observed_at: std::time::Instant,
    ) {
        let monitors = self.monitor_registry.all_screens();
        let window = self.registry.by_id_mut(window_id);
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

        match &mut window.state {
            WindowState::Positioned(PositionedState::Offscreen { actual }) => {
                if is_borderless_fullscreen {
                    // Window turned fullscreen, but not visible, so we hide them again
                    self.hub.set_fullscreen(window_id);
                    window.state = WindowState::Minimized;
                    if let Err(e) = window.ax.minimize() {
                        tracing::trace!("Failed to minimize window: {e:#}");
                    }
                } else {
                    *actual = new_placement;
                    if let Err(e) = move_offscreen(&monitors, actual, &window.ax) {
                        tracing::trace!("re-hide failed: {e}");
                    }
                }
            }
            WindowState::Positioned(PositionedState::InView(p)) => {
                if p.placed_at > observed_at {
                    tracing::trace!(?new_placement, "stale observation, ignoring");
                    return;
                }

                if new_placement == p.target {
                    p.actual = new_placement;
                    return;
                }

                if is_borderless_fullscreen {
                    window.state = WindowState::BorderlessFullscreen;
                    self.hub.set_fullscreen(window_id);
                    return;
                }
                let hub_window = self.hub.get_window(window_id);
                // Float can only be moved when focused (otherwise it's the mirror), and focused
                // floats are always inside viewport
                if hub_window.is_float() {
                    p.actual = new_placement;
                    // TODO: update float dimension
                    return;
                }

                if p.record_drift(new_placement) {
                    let target = p.target;
                    let should_retry = p.should_retry();
                    let just_gave_up = p.just_gave_up();
                    if should_retry {
                        tracing::trace!(?target, ?new_placement, "window drifted, correcting");
                        if let Err(e) =
                            window
                                .ax
                                .set_frame(target.x, target.y, target.width, target.height)
                        {
                            tracing::trace!("Window {} set_frame failed: {e}", window);
                        }
                    } else if just_gave_up {
                        tracing::debug!(
                            "Window {} can't be moved to {:?} (actual: {:?})",
                            window,
                            target,
                            new_placement
                        );
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
            }
            WindowState::Minimized => {
                // Window somehow got brought back to screen, maybe through window focused but the
                // notification was not fired
                tracing::trace!(
                    "Previously minimized borderless fullscreen window reappeared at {new_placement:?}"
                );
                if is_borderless_fullscreen && let Err(e) = window.ax.minimize() {
                    tracing::trace!("Failed to minimize window: {e:#}");
                }
                // No longer fullscreen borderless, so bring them back and put in offscreen
                else {
                    if let Err(e) = window.ax.unminimize() {
                        tracing::debug!("Failed to unminimize window: {e:#}");
                    }
                    if let Err(e) = move_offscreen(&monitors, &new_placement, &window.ax) {
                        tracing::trace!("hide after unminimize failed: {e}");
                    }
                    window.state = WindowState::Positioned(PositionedState::Offscreen {
                        actual: new_placement,
                    });
                    self.hub.unset_fullscreen(window_id);
                }
            }
            WindowState::BorderlessFullscreen => {
                // No longer border borderless fullscreen. Move to offscreen position as these
                // windows might now be inserted offscreen, which will be put back into view later
                // if it's in view
                if !is_borderless_fullscreen {
                    window.state = WindowState::Positioned(PositionedState::Offscreen {
                        actual: new_placement,
                    });
                    self.hub.unset_fullscreen(window_id);
                }
            }
            WindowState::NativeFullscreen => {
                // No longer native fullscreen
                if is_borderless_fullscreen {
                    window.state = WindowState::BorderlessFullscreen;
                } else {
                    window.state = WindowState::Positioned(PositionedState::Offscreen {
                        actual: new_placement,
                    });
                    self.hub.unset_fullscreen(window_id);
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn hide_window(&mut self, window_id: WindowId) {
        let monitors = self.monitor_registry.all_screens();
        let window = self.registry.by_id_mut(window_id);
        if let Some(capture) = self.captures.get_mut(&window_id) {
            capture.stop();
        }
        // Minimize borderless fullscreen windows instead of moving offscreen:
        // 1. User-zoomed windows maintain their fullscreen state, so moving them is futile
        // 2. Moving offscreen triggers handle_window_moved which detects fullscreen exit
        // Native fullscreen windows are on a separate Space and don't need hiding.
        let result = match &window.state {
            WindowState::BorderlessFullscreen => {
                window.state = WindowState::Minimized;
                window.ax.minimize()
            }
            WindowState::NativeFullscreen | WindowState::Minimized => Ok(()),
            WindowState::Positioned(positioned_state) => match positioned_state {
                PositionedState::InView(placement) => {
                    let actual = placement.actual;
                    window.state = WindowState::Positioned(PositionedState::Offscreen { actual });
                    move_offscreen(&monitors, &actual, &window.ax)
                }
                PositionedState::Offscreen { actual } => {
                    move_offscreen(&monitors, actual, &window.ax)
                }
            },
        };
        if let Err(e) = result {
            tracing::trace!("Failed to hide window: {e:#}");
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn move_offscreen(&mut self, window_id: WindowId) {
        let window = self.registry.by_id_mut(window_id);
        let WindowState::Positioned(positioned_state) = window.state else {
            debug_assert!(
                false,
                "Can only move windows which dome control the positions offscreen"
            );
            return;
        };
        let monitors = self.monitor_registry.all_screens();
        match positioned_state {
            PositionedState::InView(placement) => {
                let actual = placement.actual;
                move_offscreen(&monitors, &actual, &window.ax);
                window.state = WindowState::Positioned(PositionedState::Offscreen { actual })
            }
            PositionedState::Offscreen { actual } => {
                move_offscreen(&monitors, &actual, &window.ax);
            }
        }
    }
}

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
    ax: &AXWindow,
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
