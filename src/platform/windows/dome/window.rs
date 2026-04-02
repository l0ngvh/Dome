use crate::core::{Dimension, WindowId, WindowPlacement};
use crate::platform::windows::external::{ShowCmd, ZOrder};

use super::Dome;

/// Tracks the platform-level visibility and fullscreen status of a managed window.
///
/// The hub tracks logical state (tiling vs float, which workspace). This enum
/// tracks what the platform layer has actually done to the window: is it visible,
/// hidden offscreen, minimized, or in a fullscreen mode?
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowState {
    /// Window is under Dome's positional control.
    Positioned(PositionedState),
    /// Window covers the entire monitor, initiated by the user (e.g. a game
    /// or media player). Detected by comparing window dimensions to monitor
    /// dimensions in `check_fullscreen_state`.
    FullscreenBorderless,
    /// D3D/Vulkan exclusive fullscreen. Dome must not reposition or minimize
    /// these windows — doing so can crash the application or corrupt the
    /// display. Detected via `is_d3d_exclusive_fullscreen_active` in
    /// `handle_display_change`.
    FullscreenExclusive,
    /// Minimized by Dome to hide a borderless fullscreen window (e.g. on
    /// workspace switch). Not user-initiated — when a user minimizes a
    /// window, Dome removes it from the tree instead.
    Minimized,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PositionedState {
    /// Visible on screen in a tiling layout slot.
    Tiling,
    /// Visible on screen as a floating window.
    Float,
    /// Hidden offscreen by Dome (e.g. workspace switch, sibling of a
    /// fullscreen window).
    Offscreen,
}

impl std::fmt::Display for WindowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Positioned(PositionedState::Tiling) => write!(f, "tiling"),
            Self::Positioned(PositionedState::Float) => write!(f, "float"),
            Self::Positioned(PositionedState::Offscreen) => write!(f, "offscreen"),
            Self::FullscreenBorderless => write!(f, "fullscreen-borderless"),
            Self::FullscreenExclusive => write!(f, "fullscreen-exclusive"),
            Self::Minimized => write!(f, "minimized"),
        }
    }
}

impl Dome {
    pub(super) fn show_window(&mut self, id: WindowId, wp: &WindowPlacement, z: ZOrder) {
        let entry = self.registry.get_mut(id);
        match entry.state {
            WindowState::FullscreenBorderless | WindowState::FullscreenExclusive => return,
            WindowState::Minimized => {
                entry.ext.show_cmd(ShowCmd::Restore);
            }
            WindowState::Positioned(_) => {
                if entry.ext.is_iconic() {
                    entry.ext.show_cmd(ShowCmd::Restore);
                }
            }
        }
        let border = self.config.border_size;
        let content = apply_inset(wp.frame, border);
        let x = content.x.round() as i32;
        let y = content.y.round() as i32;
        let w = content.width.round() as i32;
        let h = content.height.round() as i32;
        match z {
            ZOrder::Topmost => {
                entry.ext.set_position(ZOrder::Topmost, x, y, w, h);
                entry
                    .overlay
                    .update(wp, wp.is_focused, &self.config, ZOrder::Topmost);
            }
            _ => {
                entry.overlay.update(wp, wp.is_focused, &self.config, z);
                entry
                    .ext
                    .set_position(ZOrder::After(entry.overlay.id()), x, y, w, h);
            }
        }
        entry.state = if wp.is_float {
            WindowState::Positioned(PositionedState::Float)
        } else {
            WindowState::Positioned(PositionedState::Tiling)
        };
    }

    pub(super) fn show_fullscreen_window(&mut self, id: WindowId, dimension: Dimension) {
        let entry = self.registry.get_mut(id);
        match entry.state {
            WindowState::FullscreenBorderless | WindowState::FullscreenExclusive => return,
            WindowState::Minimized => {
                entry.ext.show_cmd(ShowCmd::Restore);
                entry.state = WindowState::FullscreenBorderless;
                return;
            }
            WindowState::Positioned(_) => {}
        }
        entry.ext.set_position(
            ZOrder::Unchanged,
            dimension.x.round() as i32,
            dimension.y.round() as i32,
            dimension.width.round() as i32,
            dimension.height.round() as i32,
        );
        entry.overlay.hide();
        entry.state = WindowState::Positioned(PositionedState::Tiling);
    }

    pub(super) fn hide_window(&mut self, id: WindowId) {
        let entry = self.registry.get_mut(id);
        match entry.state {
            WindowState::Positioned(PositionedState::Tiling | PositionedState::Float) => {
                entry.ext.move_offscreen();
                entry.overlay.hide();
                entry.state = WindowState::Positioned(PositionedState::Offscreen);
            }
            WindowState::FullscreenBorderless => {
                entry.ext.show_cmd(ShowCmd::Minimize);
                entry.overlay.hide();
                entry.state = WindowState::Minimized;
            }
            WindowState::Positioned(PositionedState::Offscreen)
            | WindowState::FullscreenExclusive
            | WindowState::Minimized => {}
        }
    }

    pub(super) fn enter_fullscreen_borderless(&mut self, id: WindowId) {
        self.registry.get_mut(id).state = WindowState::FullscreenBorderless;
        self.hub.set_fullscreen(id);
    }

    pub(super) fn exit_fullscreen_borderless(&mut self, id: WindowId) {
        let entry = self.registry.get_mut(id);
        if entry.state == WindowState::FullscreenBorderless {
            entry.state = WindowState::Positioned(PositionedState::Tiling);
        }
        self.hub.unset_fullscreen(id);
    }

    pub(super) fn enter_fullscreen_exclusive(&mut self, id: WindowId) {
        self.registry.get_mut(id).state = WindowState::FullscreenExclusive;
        if !self.hub.get_window(id).is_fullscreen() {
            self.hub.set_fullscreen(id);
        }
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
