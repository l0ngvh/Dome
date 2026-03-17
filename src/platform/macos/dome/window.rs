use anyhow::Result;

use super::mirror::WindowCapture;
use super::monitor::MonitorInfo;
use crate::core::WindowPlacement;
use crate::core::{Dimension, Window, WindowId};
use crate::platform::macos::accessibility::AXWindow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FullscreenState {
    None,
    Native,
    Borderless,
}

pub(super) struct MacWindow {
    ax: AXWindow,
    window_id: WindowId,
    capture: Option<WindowCapture>,
    monitors: Vec<MonitorInfo>,
    state: WindowState,
}

impl MacWindow {
    pub(super) fn new(
        ax: AXWindow,
        window_id: WindowId,
        monitors: Vec<MonitorInfo>,
        current_placement: RoundedDimension,
    ) -> Self {
        Self {
            ax,
            window_id,
            capture: None,
            monitors,
            state: WindowState::Placed(Placement::new(current_placement)),
        }
    }

    pub(super) fn new_native_fullscreen(
        ax: AXWindow,
        window_id: WindowId,
        monitors: Vec<MonitorInfo>,
    ) -> Self {
        Self {
            ax,
            window_id,
            capture: None,
            monitors,
            state: WindowState::NativeFullscreen,
        }
    }

    pub(super) fn pid(&self) -> i32 {
        self.ax.pid()
    }

    pub(super) fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub(super) fn set_capture(&mut self, capture: WindowCapture) {
        self.capture = Some(capture);
    }

    pub(super) fn set_borderless_fullscreen(&mut self, dim: Dimension) {
        // macOS zoom button aggressively maintains fullscreen position — repeated set_frame
        // calls cause a fight loop. Only set frame on the initial transition.
        if matches!(self.state, WindowState::BorderlessFullscreen) {
            return;
        }
        self.state = WindowState::BorderlessFullscreen;

        if let Err(e) = self.ax.unminimize() {
            tracing::debug!("Failed to unminimize window: {e:#}");
        }
        if let Err(e) = self.ax.set_frame(
            dim.x as i32,
            dim.y as i32,
            dim.width as i32,
            dim.height as i32,
        ) {
            tracing::trace!("Failed to set fullscreen frame: {e:#}");
        }
    }

    pub(super) fn unset_fullscreen(&mut self, actual: RoundedDimension) {
        match self.state {
            WindowState::BorderlessFullscreen | WindowState::NativeFullscreen => {
                self.state = WindowState::Placed(Placement::new(actual));
            }
            WindowState::Placed(_) => {}
        }
    }

    pub(super) fn position(&mut self, wp: WindowPlacement, border_size: f32) -> anyhow::Result<()> {
        let WindowState::Placed(p) = &mut self.state else {
            debug_assert!(false, "position() called on fullscreen window {}", self.ax);
            return Ok(());
        };
        let actual = p.actual;
        let content_dim = apply_inset(wp.frame, border_size);

        if wp.is_float && !wp.is_focused {
            let scale = hidden_monitor(&self.monitors).scale;
            if let Some(capture) = &mut self.capture {
                let visible_content = clip_to_bounds(content_dim, wp.visible_frame);
                if let Some(visible_content) = visible_content {
                    capture.start(self.window_id, content_dim, visible_content, scale);
                } else {
                    capture.stop();
                }
            }
            p.target = None;
            move_offscreen(&self.monitors, &p.actual, &self.ax)?;
        } else {
            // Clip to visible_frame bounds — macOS doesn't reliably allow
            // placing windows partially off-screen (especially above menu bar)
            let Some(target) = clip_to_bounds(content_dim, wp.visible_frame) else {
                tracing::trace!(
                    "Window {} is offscreen (dim={:?}, bounds={:?}), hiding",
                    self.ax,
                    content_dim,
                    wp.visible_frame,
                );
                p.target = None;
                move_offscreen(&self.monitors, &p.actual, &self.ax)?;
                return Ok(());
            };

            let target = round_dim(target);
            p.target = Some(target);
            if actual != target {
                if let Err(e) = self
                    .ax
                    .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", self);
                }
            }

            if let Some(capture) = &mut self.capture {
                capture.stop();
            }
        }
        Ok(())
    }

    pub(super) fn set_native_fullscreen(&mut self) {
        self.state = WindowState::NativeFullscreen;
    }

    pub(super) fn hide(&mut self) -> anyhow::Result<()> {
        if let Some(capture) = &mut self.capture {
            capture.stop();
        }
        // Minimize borderless fullscreen windows instead of moving offscreen:
        // 1. User-zoomed windows maintain their fullscreen state, so moving them is futile
        // 2. Moving offscreen triggers handle_window_moved which detects fullscreen exit
        // Native fullscreen windows are on a separate Space and don't need hiding.
        match &mut self.state {
            WindowState::BorderlessFullscreen => self.ax.minimize(),
            WindowState::NativeFullscreen => Ok(()),
            WindowState::Placed(p) => {
                p.target = None;
                move_offscreen(&self.monitors, &p.actual, &self.ax)
            }
        }
    }

    /// Check if window settled at expected position and detect constraints.
    /// If position doesn't align, attempts to move window back.
    /// Had to make sure to only call this function once this window has finished moving/resizing
    pub(super) fn check_placement(
        &mut self,
        window: &Window,
        new_placement: RoundedDimension,
    ) -> Option<RawConstraint> {
        let p = match &mut self.state {
            WindowState::Placed(p) => p,
            _ => return None,
        };

        let (hidden_x, hidden_y) = hidden_position(&self.monitors);
        if new_placement.x == hidden_x || new_placement.y == hidden_y {
            p.actual = new_placement;
            return None;
        }

        let target = match p.target {
            Some(t) => t,
            None => {
                p.actual = new_placement;
                if let Err(e) = move_offscreen(&self.monitors, &p.actual, &self.ax) {
                    tracing::trace!("Window {} hide failed: {e}", self.ax);
                }
                return None;
            }
        };

        // Float can only be moved when focused (otherwise it's the mirror), and focused
        // floats are always inside viewport
        if window.is_float() {
            p.actual = new_placement;
            return None;
        }

        // FIXME: Change this to if new placement encompass the old placement
        // At least one edge must match on each axis - user resize moves both edges on one axis
        let left = new_placement.x == target.x;
        let right = new_placement.x + new_placement.width == target.x + target.width;
        let top = new_placement.y == target.y;
        let bottom = new_placement.y + new_placement.height == target.y + target.height;

        if !((left || right) && (top || bottom)) {
            if p.target == Some(target) && p.actual == new_placement {
                p.retries = p.retries.saturating_add(1);
            } else {
                p.retries = 0;
            }
            let retries = p.retries;
            p.target = Some(target);
            p.actual = new_placement;
            if retries <= 5 {
                tracing::trace!(
                    window = %self.ax,
                    ?target,
                    ?new_placement,
                    "window drifted, correcting"
                );
                if let Err(e) = self
                    .ax
                    .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", self.ax);
                }
            } else if retries == 6 {
                tracing::debug!(
                    "Window {} can't be moved to {:?} (actual: {:?})",
                    self,
                    target,
                    new_placement
                );
            }
            return None;
        }

        p.actual = new_placement;

        let min_w = (new_placement.width > target.width).then_some(new_placement.width as f32);
        let min_h = (new_placement.height > target.height).then_some(new_placement.height as f32);
        let max_w = (new_placement.width < target.width).then_some(new_placement.width as f32);
        let max_h = (new_placement.height < target.height).then_some(new_placement.height as f32);
        if min_w.is_some() || min_h.is_some() || max_w.is_some() || max_h.is_some() {
            tracing::trace!(
                window = %self,
                ?target,
                ?new_placement,
                ?min_w, ?min_h, ?max_w, ?max_h,
                "window constrained"
            );
            Some((min_w, min_h, max_w, max_h))
        } else {
            None
        }
    }

    pub(super) fn update_title(&mut self) {
        self.ax.update_title();
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.ax.title()
    }

    pub(super) fn app_name(&self) -> Option<&str> {
        self.ax.app_name()
    }

    pub(super) fn bundle_id(&self) -> Option<&str> {
        self.ax.bundle_id()
    }

    pub(super) fn focus(&self) -> Result<()> {
        self.ax.focus()
    }

    pub(super) fn fullscreen(&self) -> FullscreenState {
        match &self.state {
            WindowState::NativeFullscreen => FullscreenState::Native,
            WindowState::BorderlessFullscreen => FullscreenState::Borderless,
            WindowState::Placed(_) => FullscreenState::None,
        }
    }

    pub(super) fn ax(&self) -> &AXWindow {
        &self.ax
    }

    pub(super) fn mirror_source_scale(&self) -> f64 {
        hidden_monitor(&self.monitors).scale
    }

    pub(super) fn on_monitor_change(&mut self, monitors: Vec<MonitorInfo>) {
        self.monitors = monitors;
        if let WindowState::Placed(p) = &self.state {
            if p.target.is_none() {
                if let Err(e) = move_offscreen(&self.monitors, &p.actual, &self.ax) {
                    tracing::trace!("Failed to re-hide window: {e:#}");
                }
            }
        }
    }
}

impl std::fmt::Display for MacWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}|{}:{}] {}",
            self.window_id,
            self.ax.pid(),
            self.ax.cg_id(),
            self.ax.app_name().unwrap_or("Unknown")
        )?;
        if let Some(bundle_id) = self.ax.bundle_id() {
            write!(f, " ({bundle_id})")?;
        }
        if let Some(title) = self.ax.title() {
            write!(f, " - {title}")?;
        }
        Ok(())
    }
}

struct Placement {
    /// The desired placement. None if it should be hidden
    target: Option<RoundedDimension>,
    actual: RoundedDimension,
    retries: u8,
}

impl Placement {
    fn new(actual: RoundedDimension) -> Self {
        Self {
            target: None,
            actual,
            retries: 0,
        }
    }
}

enum WindowState {
    Placed(Placement),
    BorderlessFullscreen,
    NativeFullscreen,
}

pub(super) fn apply_inset(dim: Dimension, border: f32) -> Dimension {
    Dimension {
        x: dim.x + border,
        y: dim.y + border,
        width: (dim.width - 2.0 * border).max(0.0),
        height: (dim.height - 2.0 * border).max(0.0),
    }
}

/// Constraint on raw window size (min_w, min_h, max_w, max_h).
pub(super) type RawConstraint = (Option<f32>, Option<f32>, Option<f32>, Option<f32>);

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

fn move_offscreen(
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
fn hidden_monitor(monitors: &[MonitorInfo]) -> &MonitorInfo {
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
