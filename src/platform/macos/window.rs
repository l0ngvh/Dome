use std::collections::HashSet;

use anyhow::Result;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace};
use objc2_core_foundation::{
    CFArray, CFDictionary, CFNumber, CFString, CFType, CGPoint, CGRect, CGSize,
};
use objc2_core_graphics::{
    CGWindowID, CGWindowListCopyWindowInfo, CGWindowListOption,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::app::ScreenInfo;
use super::dome::{HubMessage, MessageSender};
use super::mirror::WindowCapture;
use super::objc2_wrapper::kCGWindowNumber;
use crate::config::{Color, Config};
use crate::core::{Dimension, SpawnMode, Window, WindowId};
use crate::platform::macos::accessibility::AXWindow;

pub(super) struct MacWindow {
    ax: AXWindow,
    window_id: WindowId,
    capture: Option<WindowCapture>,
    sender: MessageSender,
    focused: bool,
    physical_placement: Option<(RoundedDimension, u8)>,
    is_ax_hidden: bool,
    monitors: Vec<ScreenInfo>,
}

impl MacWindow {
    pub(super) fn new(
        ax: AXWindow,
        window_id: WindowId,
        hub_window: &Window,
        sender: MessageSender,
        monitors: Vec<ScreenInfo>,
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
        window: &Window,
        monitor: Dimension,
        focused: bool,
        config: &Config,
    ) -> anyhow::Result<()> {
        let content_dim = apply_inset(window.dimension(), config.border_size);
        let scale = self.hidden_monitor().scale;

        if window.is_float() && !focused {
            if let Some(clipped) = clip_to_bounds(content_dim, monitor) {
                let frame = to_ns_rect(self.primary_full_height(), clipped);
                let source_rect = compute_source_rect(content_dim, clipped);

                if let Some(capture) = &mut self.capture {
                    capture.start(
                        self.ax.cg_id(),
                        source_rect,
                        frame.size.width as u32,
                        frame.size.height as u32,
                        scale,
                        self.sender.clone(),
                    );
                }

                if let Err(e) = self.hide_ax() {
                    tracing::trace!("Failed to hide window for float capture: {e:#}");
                }
            } else {
                self.hide()?;
            }
        } else {
            self.try_placement(content_dim, monitor);
        }

        let colors = if window.is_float() && focused {
            [config.focused_color; 4]
        } else if focused {
            spawn_colors(window.spawn_mode(), config)
        } else {
            [config.border_color; 4]
        };
        if let Some((clipped, edges)) =
            compute_border_edges(window.dimension(), monitor, colors, config.border_size)
        {
            self.sender.send(HubMessage::WindowShow {
                cg_id: self.ax.cg_id(),
                frame: to_ns_rect(self.primary_full_height(), clipped),
                is_float: window.is_float(),
                is_focus: focused,
                edges: edges
                    .into_iter()
                    .map(|(r, c)| (to_edge_ns_rect(r, clipped.height), c))
                    .collect(),
                scale,
                border: config.border_size as f64,
            });
        }
        if focused
            && !self.focused
            && let Err(e) = self.ax.focus()
        {
            tracing::trace!("Failed to focus window: {e:#}");
        }
        self.focused = focused;
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
    fn hidden_monitor(&self) -> &ScreenInfo {
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
    fn try_placement(&mut self, placement: Dimension, monitor: Dimension) {
        if is_completely_offscreen(placement, monitor) {
            if self.is_ax_hidden {
                return;
            }
            // TODO: if hide fail to move the window to offscreen position, this window is clearly
            // trying to take focus, so we should pop it to float or something.
            // Exception is full screen window, which, should be handled differently as a first
            // party citizen
            tracing::trace!(
                "Window {} is offscreen (dim={:?}, monitor={monitor:?}), hiding",
                self.ax,
                placement
            );
            if let Err(e) = self.hide_ax() {
                tracing::trace!("Failed to hide window: {e:#}");
            }
            return;
        }

        let mut target = placement;

        // Mac prevents putting windows above menu bar
        if target.y < monitor.y {
            target.height -= monitor.y - target.y;
            target.y = monitor.y;
        }
        // Clip to fit within monitor bounds, as Mac sometimes snaps windows to fit within screen,
        // which might be confused with user setting size manually
        if target.y + target.height > monitor.y + monitor.height {
            target.height = monitor.y + monitor.height - target.y;
        }
        if target.x < monitor.x {
            target.width -= monitor.x - target.x;
            target.x = monitor.x;
        }
        if target.x + target.width > monitor.x + monitor.width {
            target.width = monitor.x + monitor.width - target.x;
        }

        let rounded = round_dim(target);
        let (ax, ay) = self.ax.get_position().unwrap_or((0, 0));
        let (aw, ah) = self.ax.get_size().unwrap_or((0, 0));
        let at_position =
            ax == rounded.x && ay == rounded.y && aw == rounded.width && ah == rounded.height;
        if at_position {
            tracing::trace!(
                "Window {} is already at the correct position {rounded:?}",
                self.ax,
            );
            return;
        }

        if let Some((prev, count)) = &mut self.physical_placement
            && *prev == rounded
            && !self.is_ax_hidden
        {
            if *count >= 5 {
                tracing::debug!(
                    "Window {} can't be moved to the desired position {:?}",
                    self.ax,
                    self.physical_placement
                );
                return;
            }
            *count += 1;
        } else {
            self.physical_placement = Some((rounded, 0));
        }

        tracing::trace!(
            "Window {} placing at {target:?} (was_hidden={})",
            self.ax,
            self.is_ax_hidden
        );
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

        tracing::trace!(
            "Window {} moved from {expected:?} to ({actual_x}, {actual_y}, {actual_width}, {actual_height}), with logical placement at {:?}",
            self.ax,
            window.dimension()
        );

        // At least one edge must match on each axis - user resize moves both edges on one axis
        let left = actual_x == expected.x;
        let right = actual_x + actual_width == expected.x + expected.width;
        let top = actual_y == expected.y;
        let bottom = actual_y + actual_height == expected.y + expected.height;
        if !((left || right) && (top || bottom)) {
            if count < 5 {
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
            Some((min_w, min_h, max_w, max_h))
        } else {
            None
        }
    }

    pub(super) fn update_title(&mut self) {
        self.ax.update_title();
    }

    pub(super) fn is_valid(&self) -> bool {
        self.ax.is_valid()
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

    pub(super) fn on_monitor_change(&mut self, monitors: Vec<ScreenInfo>) {
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

fn is_completely_offscreen(dim: Dimension, screen: Dimension) -> bool {
    dim.x + dim.width <= screen.x
        || dim.x >= screen.x + screen.width
        || dim.y + dim.height <= screen.y
        || dim.y >= screen.y + screen.height
}

/// Constraint on raw window size (min_w, min_h, max_w, max_h).
pub(super) type RawConstraint = (Option<f32>, Option<f32>, Option<f32>, Option<f32>);

pub(super) fn list_cg_window_ids() -> HashSet<CGWindowID> {
    let Some(window_list) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, 0) else {
        tracing::warn!("CGWindowListCopyWindowInfo returned None");
        return HashSet::new();
    };
    let window_list: &CFArray<CFDictionary<CFString, CFType>> =
        unsafe { window_list.cast_unchecked() };

    let mut ids = HashSet::new();
    let key = kCGWindowNumber();
    for dict in window_list {
        // window id is a required attribute
        // https://developer.apple.com/documentation/coregraphics/kcgwindownumber?language=objc
        let id = dict
            .get(&key)
            .unwrap()
            .downcast::<CFNumber>()
            .unwrap()
            .as_i64()
            .unwrap();
        ids.insert(id as CGWindowID);
    }
    ids
}

pub(super) fn running_apps() -> impl Iterator<Item = Retained<NSRunningApplication>> {
    let own_pid = std::process::id() as i32;
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .into_iter()
        .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
        .filter(move |app| app.processIdentifier() != -1 && app.processIdentifier() != own_pid)
}

pub(super) fn get_app_by_pid(pid: i32) -> Option<Retained<NSRunningApplication>> {
    if pid == std::process::id() as i32 {
        return None;
    }
    NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
}

// Quartz uses top-left origin, Cocoa uses bottom-left origin
fn to_ns_rect(primary_full_height: f32, dim: Dimension) -> NSRect {
    NSRect::new(
        NSPoint::new(
            dim.x as f64,
            (primary_full_height - dim.y - dim.height) as f64,
        ),
        NSSize::new(dim.width as f64, dim.height as f64),
    )
}

/// Convert edge rect from Quartz coords to Cocoa coords, relative to the overlay window.
fn to_edge_ns_rect(dim: Dimension, overlay_height: f32) -> NSRect {
    NSRect::new(
        NSPoint::new(dim.x as f64, (overlay_height - dim.y - dim.height) as f64),
        NSSize::new(dim.width as f64, dim.height as f64),
    )
}

/// Clip rect to bounds. Returns None if fully outside.
fn clip_to_bounds(rect: Dimension, bounds: Dimension) -> Option<Dimension> {
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

fn compute_source_rect(original: Dimension, clipped: Dimension) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: (clipped.x - original.x) as f64,
            y: (clipped.y - original.y) as f64,
        },
        size: CGSize {
            width: clipped.width as f64,
            height: clipped.height as f64,
        },
    }
}

fn compute_border_edges(
    frame: Dimension,
    bounds: Dimension,
    colors: [Color; 4],
    b: f32,
) -> Option<(Dimension, Vec<(Dimension, Color)>)> {
    let clipped = clip_to_bounds(frame, bounds)?;

    let offset_x = clipped.x - frame.x;
    let offset_y = clipped.y - frame.y;
    let clip_local = Dimension {
        x: offset_x,
        y: offset_y,
        width: clipped.width,
        height: clipped.height,
    };

    let w = frame.width;
    let h = frame.height;
    let mut edges = Vec::new();

    // top
    let top = Dimension {
        x: 0.0,
        y: 0.0,
        width: w,
        height: b,
    };
    if let Some(r) = clip_to_bounds(top, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[0]));
    }

    // right
    let right = Dimension {
        x: w - b,
        y: b,
        width: b,
        height: h - 2.0 * b,
    };
    if let Some(r) = clip_to_bounds(right, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[1]));
    }

    // bottom
    let bottom = Dimension {
        x: 0.0,
        y: h - b,
        width: w,
        height: b,
    };
    if let Some(r) = clip_to_bounds(bottom, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[2]));
    }

    // left
    let left = Dimension {
        x: 0.0,
        y: b,
        width: b,
        height: h - 2.0 * b,
    };
    if let Some(r) = clip_to_bounds(left, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[3]));
    }

    if edges.is_empty() {
        None
    } else {
        Some((clipped, edges))
    }
}

fn translate_dim(dim: Dimension, dx: f32, dy: f32) -> Dimension {
    Dimension {
        x: dim.x + dx,
        y: dim.y + dy,
        width: dim.width,
        height: dim.height,
    }
}

fn spawn_colors(spawn: SpawnMode, config: &Config) -> [Color; 4] {
    let f = config.focused_color;
    let s = config.spawn_indicator_color;
    [
        if spawn.is_tab() { s } else { f },
        if spawn.is_horizontal() { s } else { f },
        if spawn.is_vertical() { s } else { f },
        f,
    ]
}

fn primary_full_height_from(monitors: &[ScreenInfo]) -> f32 {
    monitors.iter().find(|s| s.is_primary).unwrap().full_height
}

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
