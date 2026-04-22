use crate::core::{
    Dimension, FloatWindowPlacement, TilingWindowPlacement, WindowId, WindowRestrictions,
};
use crate::platform::windows::external::{ShowCmd, ZOrder};
use crate::platform::windows::handle::OFFSCREEN_POS;

use super::Dome;

pub(super) const MAX_DRIFT_RETRIES: u8 = 5;

#[derive(Clone, Copy)]
pub(super) struct DriftState {
    pub(super) target: (i32, i32, i32, i32),
    /// The window's last known position — written by `window_moved`
    /// via a dispatched `get_visible_rect` read. When a window goes offscreen,
    /// this preserves its position from before the hide: "actual" means where
    /// the window currently is (or last was), not where we want it.
    pub(super) actual: (i32, i32, i32, i32),
    pub(super) retries: u8,
}

/// Tracks the platform-level visibility and fullscreen status of a managed window.
///
/// The hub tracks logical state (tiling vs float, which workspace). This enum
/// tracks what the platform layer has actually done to the window: is it visible,
/// hidden offscreen, minimized, or in a fullscreen mode?
#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
pub(super) enum PositionedState {
    /// Visible on screen in a tiling layout slot.
    Tiling(DriftState),
    /// Visible on screen as a floating window.
    Float(DriftState),
    /// Hidden offscreen by Dome (e.g. workspace switch, sibling of a
    /// fullscreen window).
    Offscreen {
        retries: u8,
        actual: (i32, i32, i32, i32),
    },
}

impl std::fmt::Display for WindowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Positioned(PositionedState::Tiling(_)) => write!(f, "tiling"),
            Self::Positioned(PositionedState::Float(_)) => write!(f, "float"),
            Self::Positioned(PositionedState::Offscreen { .. }) => write!(f, "offscreen"),
            Self::FullscreenBorderless => write!(f, "fullscreen-borderless"),
            Self::FullscreenExclusive => write!(f, "fullscreen-exclusive"),
            Self::Minimized => write!(f, "minimized"),
        }
    }
}

impl Dome {
    pub(super) fn show_float(
        &mut self,
        id: WindowId,
        wp: &FloatWindowPlacement,
        focus_changed: bool,
        is_focused: bool,
    ) {
        let entry = self.registry.get_mut(id);
        let border = self.config.border_size;
        let content = apply_inset(wp.frame, border);
        let x = content.x.round() as i32;
        let y = content.y.round() as i32;
        let w = content.width.round() as i32;
        let h = content.height.round() as i32;
        let new_target = (x, y, w, h);

        let (needs_topmost, settled, prev_actual) = match entry.state {
            WindowState::FullscreenBorderless | WindowState::FullscreenExclusive => {
                debug_assert!(false, "show_float called on fullscreen window {id}");
                return;
            }
            WindowState::Minimized => {
                entry.ext.show_cmd(ShowCmd::Restore);
                (true, false, new_target)
            }
            WindowState::Positioned(ps) => {
                if entry.ext.is_iconic() {
                    entry.ext.show_cmd(ShowCmd::Restore);
                }
                match ps {
                    PositionedState::Float(d) => {
                        let needs_topmost = focus_changed && is_focused;
                        let settled = d.target == new_target && !needs_topmost;
                        (needs_topmost, settled, d.actual)
                    }
                    PositionedState::Tiling(d) => (true, false, d.actual),
                    PositionedState::Offscreen { actual, .. } => (true, false, actual),
                }
            }
        };

        let z = if needs_topmost {
            ZOrder::Topmost
        } else {
            ZOrder::Unchanged
        };

        if let Some(overlay) = self.float_overlays.get_mut(&id)
            && !settled
        {
            overlay.update(wp, &self.config, z);
            if needs_topmost {
                entry.ext.set_position(ZOrder::Topmost, x, y, w, h);
            } else {
                entry
                    .ext
                    .set_position(ZOrder::After(overlay.id()), x, y, w, h);
            }
        }

        if !settled {
            entry.state = WindowState::Positioned(PositionedState::Float(DriftState {
                target: new_target,
                actual: prev_actual,
                retries: 0,
            }));
        }
    }

    pub(super) fn show_tiling(&mut self, id: WindowId, wp: &TilingWindowPlacement, z: ZOrder) {
        let entry = self.registry.get_mut(id);
        let border = self.config.border_size;
        let content = apply_inset(wp.frame, border);
        let x = content.x.round() as i32;
        let y = content.y.round() as i32;
        let w = content.width.round() as i32;
        let h = content.height.round() as i32;
        let new_target = (x, y, w, h);

        let prev_actual = match entry.state {
            WindowState::FullscreenBorderless | WindowState::FullscreenExclusive => {
                debug_assert!(false, "show_tiling called on fullscreen window {id}");
                return;
            }
            WindowState::Positioned(PositionedState::Tiling(d)) if d.target == new_target => {
                return;
            }
            WindowState::Minimized => {
                entry.ext.show_cmd(ShowCmd::Restore);
                new_target
            }
            WindowState::Positioned(ps) => {
                if entry.ext.is_iconic() {
                    entry.ext.show_cmd(ShowCmd::Restore);
                }
                match ps {
                    PositionedState::Tiling(d) | PositionedState::Float(d) => d.actual,
                    PositionedState::Offscreen { actual, .. } => actual,
                }
            }
        };

        entry.ext.set_position(z, x, y, w, h);
        entry.state = WindowState::Positioned(PositionedState::Tiling(DriftState {
            target: new_target,
            actual: prev_actual,
            retries: 0,
        }));
    }

    pub(super) fn show_fullscreen_window(&mut self, id: WindowId, dimension: Dimension) {
        let entry = self.registry.get_mut(id);
        match entry.state {
            WindowState::FullscreenBorderless | WindowState::FullscreenExclusive => {}
            WindowState::Minimized => {
                entry.ext.show_cmd(ShowCmd::Restore);
                entry.state = WindowState::FullscreenBorderless;
            }
            WindowState::Positioned(ps) => {
                let x = dimension.x.round() as i32;
                let y = dimension.y.round() as i32;
                let w = dimension.width.round() as i32;
                let h = dimension.height.round() as i32;
                let new_target = (x, y, w, h);
                if matches!(ps, PositionedState::Tiling(d) if d.target == new_target) {
                    return;
                }
                entry.ext.set_position(ZOrder::Unchanged, x, y, w, h);
                self.float_overlays.remove(&id);
                let prev_actual = match ps {
                    PositionedState::Tiling(d) | PositionedState::Float(d) => d.actual,
                    PositionedState::Offscreen { actual, .. } => actual,
                };
                entry.state = WindowState::Positioned(PositionedState::Tiling(DriftState {
                    target: new_target,
                    actual: prev_actual,
                    retries: 0,
                }));
            }
        }
    }

    pub(super) fn hide_window(&mut self, id: WindowId) {
        let entry = self.registry.get_mut(id);
        match entry.state {
            WindowState::Positioned(PositionedState::Tiling(d) | PositionedState::Float(d)) => {
                entry.ext.move_offscreen();
                if let Some(overlay) = self.float_overlays.get_mut(&id) {
                    overlay.hide();
                }
                entry.state = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: d.actual,
                });
            }
            WindowState::FullscreenBorderless => {
                entry.ext.show_cmd(ShowCmd::Minimize);
                entry.state = WindowState::Minimized;
            }
            WindowState::Positioned(PositionedState::Offscreen { actual, .. }) => {
                if actual.0 > OFFSCREEN_POS as i32 && actual.1 > OFFSCREEN_POS as i32 {
                    entry.ext.move_offscreen();
                }
            }
            WindowState::FullscreenExclusive | WindowState::Minimized => {}
        }
    }

    pub(super) fn window_entered_borderless_fullscreen(&mut self, id: WindowId) {
        let window = self.registry.get_mut(id);
        match window.state {
            WindowState::Positioned(_) => {
                window.state = WindowState::FullscreenBorderless;
                self.hub
                    .set_fullscreen(id, WindowRestrictions::ProtectFullscreen);
            }
            WindowState::FullscreenExclusive | WindowState::FullscreenBorderless => {}
            WindowState::Minimized => window.ext.show_cmd(ShowCmd::Restore),
        }
    }

    pub(super) fn window_drifted(&mut self, id: WindowId, x: i32, y: i32, w: i32, h: i32) {
        let visible_rect = (x, y, w, h);
        let entry = self.registry.get_mut(id);
        match &mut entry.state {
            WindowState::FullscreenExclusive => {}
            WindowState::FullscreenBorderless | WindowState::Minimized => {
                if matches!(entry.state, WindowState::Minimized) {
                    entry.ext.show_cmd(ShowCmd::Restore);
                }
                entry.state = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: visible_rect,
                });
                self.hub.unset_fullscreen(id);
            }
            WindowState::Positioned(
                PositionedState::Tiling(drift) | PositionedState::Float(drift),
            ) => {
                drift.actual = visible_rect;
                if drift.actual != drift.target {
                    drift.retries = drift.retries.saturating_add(1);
                    if drift.retries > MAX_DRIFT_RETRIES {
                        tracing::debug!("Drift retries exhausted, giving up");
                    } else {
                        let (x, y, w, h) = drift.target;
                        entry.ext.set_position(ZOrder::Unchanged, x, y, w, h);
                    }
                }
            }
            WindowState::Positioned(PositionedState::Offscreen { retries, actual }) => {
                *actual = visible_rect;
                if actual.0 > OFFSCREEN_POS as i32 && actual.1 > OFFSCREEN_POS as i32 {
                    *retries = retries.saturating_add(1);
                    if *retries >= MAX_DRIFT_RETRIES {
                        tracing::debug!("Offscreen re-hide retries exhausted");
                    } else {
                        entry.ext.move_offscreen();
                    }
                }
            }
        }
    }

    pub(super) fn enter_fullscreen_exclusive(&mut self, id: WindowId) {
        self.registry.get_mut(id).state = WindowState::FullscreenExclusive;
        self.hub.set_fullscreen(id, WindowRestrictions::BlockAll);
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
