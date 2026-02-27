use anyhow::Result;

use super::dome::{HubMessage, MessageSender};
use super::mirror::WindowCapture;
use super::monitor::{MonitorInfo, primary_full_height_from};
use super::rendering::{clip_to_bounds, compute_window_border, to_ns_rect};
use crate::config::Config;
use crate::core::WindowPlacement;
use crate::core::{Dimension, Window, WindowId};
use crate::platform::macos::accessibility::AXWindow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FullscreenState {
    None,
    Native,
    Mock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FullscreenTransition {
    Entered(FullscreenState),
    Exited,
    Unchanged,
}

pub(super) struct MacWindow {
    ax: AXWindow,
    window_id: WindowId,
    capture: Option<WindowCapture>,
    sender: MessageSender,
    focused: bool,
    physical_placement: Option<(RoundedDimension, u8)>,
    is_ax_hidden: bool,
    monitors: Vec<MonitorInfo>,
    fullscreen: FullscreenState,
}

impl MacWindow {
    pub(super) fn new(
        ax: AXWindow,
        window_id: WindowId,
        hub_window: &Window,
        sender: MessageSender,
        monitors: Vec<MonitorInfo>,
    ) -> Self {
        let primary_full_height = primary_full_height_from(&monitors);
        let frame = to_ns_rect(primary_full_height, hub_window.dimension());
        sender.send(HubMessage::WindowCreate {
            cg_id: ax.cg_id(),
            frame,
        });
        Self {
            ax,
            window_id,
            capture: None,
            sender,
            focused: false,
            physical_placement: None,
            is_ax_hidden: false,
            monitors,
            fullscreen: FullscreenState::None,
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

    pub(super) fn show(
        &mut self,
        wp: &WindowPlacement,
        config: &Config,
    ) -> anyhow::Result<()> {
        self.fullscreen = FullscreenState::None;
        // content_dim from the full unclipped frame, NOT visible_frame.
        // apply_inset insets all 4 sides. When we later clip against visible_frame:
        // - Non-clipped edges: content stays inset by border_size (border exists)
        // - Clipped edges: clip overrides the inset (no gap, border is gone on that side)
        let content_dim = apply_inset(wp.frame, config.border_size);
        let scale = self.hidden_monitor().scale;
        let primary_full_height = self.primary_full_height();

        if let Some(border) = compute_window_border(
            wp.frame,
            wp.visible_frame,
            wp.spawn_mode,
            wp.is_float,
            wp.is_focused,
            config,
            primary_full_height,
        ) {
            self.sender.send(HubMessage::WindowShow {
                cg_id: self.ax.cg_id(),
                frame: border.frame,
                is_float: wp.is_float,
                is_focus: wp.is_focused,
                edges: border.edges,
                scale,
                border: config.border_size as f64,
            });
        } else {
            return self.hide();
        }

        if wp.is_float && !wp.is_focused {
            if let Some(capture) = &mut self.capture {
                capture.start(
                    self.ax.cg_id(),
                    content_dim,
                    wp.visible_frame,
                    scale,
                    primary_full_height,
                    self.sender.clone(),
                );
            }
            self.hide_ax()?;
        } else {
            // try_placement clips to visible_frame bounds — macOS doesn't reliably allow
            // placing windows partially off-screen (especially above menu bar)
            self.try_placement(content_dim, wp.visible_frame);
            if let Some(capture) = &mut self.capture {
                capture.stop();
            }
        }

        if wp.is_focused
            && !self.focused
            && let Err(e) = self.ax.focus()
        {
            tracing::trace!("Failed to focus window: {e:#}");
        }
        self.focused = wp.is_focused;
        Ok(())
    }

    pub(super) fn hide(&mut self) -> anyhow::Result<()> {
        if let Some(capture) = &mut self.capture {
            capture.stop();
        }
        self.sender.send(HubMessage::WindowHide {
            cg_id: self.ax.cg_id(),
        });
        self.focused = false;
        self.hide_ax()
    }

    fn primary_full_height(&self) -> f32 {
        primary_full_height_from(&self.monitors)
    }

    /// Returns the monitor used for hiding windows offscreen.
    /// We pick the monitor whose bottom-right corner is furthest from origin,
    /// ensuring hidden windows are placed at a valid screen position that is
    /// not visible on any other screen.
    fn hidden_monitor(&self) -> &MonitorInfo {
        self.monitors
            .iter()
            .max_by_key(|m| {
                (m.dimension.x + m.dimension.width) as i32
                    + (m.dimension.y + m.dimension.height) as i32
            })
            .unwrap()
    }

    fn hidden_position(&self) -> (i32, i32) {
        // MacOS doesn't allow completely set windows offscreen, so we need to leave at
        // least one pixel left
        // https://nikitabobko.github.io/AeroSpace/guide#emulation-of-virtual-workspaces
        let d = &self.hidden_monitor().dimension;
        ((d.x + d.width - 1.0) as i32, (d.y + d.height - 1.0) as i32)
    }

    fn hide_ax(&mut self) -> Result<()> {
        let (x, y) = self.hidden_position();
        self.is_ax_hidden = true;
        self.ax.hide_at(x, y)
    }

    /// Try to place this physical window on the logical placement. Mac has restrictions on how
    /// windows can be placed, so if we try to put a window above menu bar, or stretch a window
    /// taller than screen height, Mac will instead come up with an alternative placement. For our
    /// use case, the alternative placements are acceptable, albeit they will mess a little with
    /// our border rendering
    fn try_placement(&mut self, placement: Dimension, bounds: Dimension) {
        let Some(target) = clip_to_bounds(placement, bounds) else {
            if self.is_ax_hidden {
                return;
            }
            // TODO: if hide fail to move the window to offscreen position, this window is clearly
            // trying to take focus, so we should pop it to float or something.
            // Exception is full screen window, which, should be handled differently as a first
            // party citizen
            tracing::trace!(
                "Window {} is offscreen (dim={:?}, bounds={bounds:?}), hiding",
                self.ax,
                placement
            );
            // only hide the physical window, the border can be partially visible
            if let Err(e) = self.hide_ax() {
                tracing::trace!("Failed to hide window: {e:#}");
            }
            return;
        };

        let rounded = round_dim(target);
        let (ax, ay) = self.ax.get_position().unwrap_or((0, 0));
        let (aw, ah) = self.ax.get_size().unwrap_or((0, 0));
        let at_position =
            ax == rounded.x && ay == rounded.y && aw == rounded.width && ah == rounded.height;
        if at_position {
            return;
        }

        if let Some((prev, count)) = &mut self.physical_placement
            && *prev == rounded
            && !self.is_ax_hidden
        {
            if *count < 5 {
                *count += 1;
            } else if *count == 5 {
                *count += 1;
                tracing::debug!(
                    "Window {} can't be moved to the desired position {:?}",
                    self.ax,
                    self.physical_placement
                );
            } else {
                return;
            }
        } else {
            self.physical_placement = Some((rounded, 0));
            tracing::trace!(
                "Placing window {self} at {rounded:?}, with its logical placement at {placement:?}"
            );
        }

        self.is_ax_hidden = false;
        if let Err(e) = self.ax.set_frame(
            rounded.x as i32,
            rounded.y as i32,
            rounded.width as i32,
            rounded.height as i32,
        ) {
            tracing::trace!("Window {} set_frame failed: {e}", self.ax);
        }
    }

    /// Check if window settled at expected position and detect constraints.
    /// If position doesn't align, attempts to move window back.
    /// Had to make sure to only call this function once this window has finished moving/resizing
    pub(super) fn check_placement(&mut self, window: &Window) -> Option<RawConstraint> {
        if self.is_ax_hidden {
            // When spaces change or monitors are connected/disconnected, hidden windows
            // may be moved to visible state, so we need to re-hide them
            let (x, y) = self.ax.get_position().unwrap_or((0, 0));
            let (hidden_x, hidden_y) = self.hidden_position();
            if (x != hidden_x || y != hidden_y)
                && let Err(e) = self.hide_ax()
            {
                tracing::trace!("Window {} hide failed: {e}", self.ax);
            }
            return None;
        }
        let (expected, count) = self.physical_placement?;
        let (actual_x, actual_y) = self.ax.get_position().unwrap_or((0, 0));
        let (actual_width, actual_height) = self.ax.get_size().unwrap_or((0, 0));
        let logical = window.dimension();

        // At least one edge must match on each axis - user resize moves both edges on one axis
        let left = actual_x == expected.x;
        let right = actual_x + actual_width == expected.x + expected.width;
        let top = actual_y == expected.y;
        let bottom = actual_y + actual_height == expected.y + expected.height;

        if !((left || right) && (top || bottom)) {
            if count < 5 {
                tracing::trace!(
                    window = %self.ax,
                    ?expected,
                    actual = ?(actual_x, actual_y, actual_width, actual_height),
                    ?logical,
                    attempt = count + 1,
                    "window drifted, correcting"
                );
                self.physical_placement = Some((expected, count + 1));
                if let Err(e) =
                    self.ax
                        .set_frame(expected.x, expected.y, expected.width, expected.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", self.ax);
                }
            }
            return None;
        }

        let min_w = (actual_width > expected.width).then_some(actual_width as f32);
        let min_h = (actual_height > expected.height).then_some(actual_height as f32);
        let max_w = (actual_width < expected.width).then_some(actual_width as f32);
        let max_h = (actual_height < expected.height).then_some(actual_height as f32);
        if min_w.is_some() || min_h.is_some() || max_w.is_some() || max_h.is_some() {
            tracing::trace!(
                window = %self.ax,
                ?expected,
                actual = ?(actual_x, actual_y, actual_width, actual_height),
                ?logical,
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

    pub(super) fn sync_fullscreen(&mut self, monitor: &Dimension) -> FullscreenTransition {
        let new_state = if self.ax.is_native_fullscreen() {
            FullscreenState::Native
        } else if self.ax.is_mock_fullscreen(monitor) {
            FullscreenState::Mock
        } else {
            FullscreenState::None
        };
        let old = self.fullscreen;
        self.fullscreen = new_state;
        match (old, new_state) {
            (FullscreenState::None, FullscreenState::None) => FullscreenTransition::Unchanged,
            (FullscreenState::None, entered) => FullscreenTransition::Entered(entered),
            (_, FullscreenState::None) => FullscreenTransition::Exited,
            _ => FullscreenTransition::Unchanged,
        }
    }

    pub(super) fn fullscreen(&self) -> FullscreenState {
        self.fullscreen
    }

    pub(super) fn set_fullscreen(&mut self, dim: Dimension) {
        self.fullscreen = FullscreenState::Mock;
        self.sender.send(HubMessage::WindowHide {
            cg_id: self.ax.cg_id(),
        });
        if let Err(e) = self.ax.set_frame(
            dim.x as i32,
            dim.y as i32,
            dim.width as i32,
            dim.height as i32,
        ) {
            tracing::trace!("Failed to set fullscreen frame: {e:#}");
        }
    }

    pub(super) fn ax(&self) -> &AXWindow {
        &self.ax
    }

    pub(super) fn on_monitor_change(&mut self, monitors: Vec<MonitorInfo>) {
        self.monitors = monitors;
        if self.is_ax_hidden
            && let Err(e) = self.hide_ax()
        {
            tracing::trace!("Failed to re-hide window: {e:#}");
        }
    }
}

impl std::fmt::Display for MacWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ax)
    }
}

impl Drop for MacWindow {
    fn drop(&mut self) {
        self.sender.send(HubMessage::WindowDelete {
            cg_id: self.ax.cg_id(),
        });
    }
}

fn apply_inset(dim: Dimension, border: f32) -> Dimension {
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
struct RoundedDimension {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn round_dim(dim: Dimension) -> RoundedDimension {
    RoundedDimension {
        x: dim.x.round() as i32,
        y: dim.y.round() as i32,
        width: dim.width.round() as i32,
        height: dim.height.round() as i32,
    }
}
