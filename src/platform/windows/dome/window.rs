use crate::core::{
    Dimension, FloatWindowPlacement, MonitorId, TilingWindowPlacement, WindowId, WindowRestrictions,
};
use crate::platform::windows::external::{ShowCmd, ZOrder};
use crate::platform::windows::handle::OFFSCREEN_POS;

use super::Dome;

pub(super) const MAX_DRIFT_RETRIES: u8 = 5;

#[derive(Clone, Copy)]
pub(super) struct DriftState {
    pub(super) target: (i32, i32, i32, i32),
    /// The window's last known position -- written by `window_moved`
    /// via a dispatched `get_visible_rect` read. When a window goes offscreen,
    /// this preserves its position from before the hide: "actual" means where
    /// the window currently is (or last was), not where we want it.
    pub(super) actual: (i32, i32, i32, i32),
    pub(super) retries: u8,
    /// Monitor this window was last placed on. `show_tiling` compares against
    /// the incoming monitor to detect cross-monitor moves.
    pub(super) monitor: MonitorId,
}

/// Lightweight placement state for floating windows. Floats accept the
/// OS-reported geometry as ground truth, so there is no `actual` field
/// (target IS actual after each observation) and no retry/drift fields.
#[derive(Clone, Copy)]
pub(super) struct FloatPlacement {
    /// Last rect reconciled with the OS. `show_float` compares
    /// `target == new_target` to skip redundant `set_position` calls;
    /// `window_drifted` writes the observed rect back here on user drag.
    pub(super) target: (i32, i32, i32, i32),
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
    /// workspace switch). Not user-initiated.
    Minimized,
    /// Window was minimized by the user (minimize button, taskbar, etc.).
    /// Tracked in hub.minimized_windows for the picker.
    UserMinimized,
}

#[derive(Clone, Copy)]
pub(super) enum PositionedState {
    /// Visible on screen in a tiling layout slot.
    Tiling(DriftState),
    /// Visible on screen as a floating window.
    Float(FloatPlacement),
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
            Self::UserMinimized => write!(f, "user_minimized"),
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
        // FloatPlacement doesn't track monitor (YAGNI for cross-monitor float reconciliation)
        _monitor: MonitorId,
    ) {
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        let border = self.config.border_size;
        let content = apply_inset(wp.frame, border);
        let x = content.x.round() as i32;
        let y = content.y.round() as i32;
        let w = content.width.round() as i32;
        let h = content.height.round() as i32;
        let new_target = (x, y, w, h);

        let (needs_topmost, settled) = match entry.state {
            WindowState::FullscreenBorderless | WindowState::FullscreenExclusive => {
                debug_assert!(false, "show_float called on fullscreen window {id}");
                return;
            }
            WindowState::Minimized | WindowState::UserMinimized => {
                entry.ext.show_cmd(ShowCmd::Restore);
                (true, false)
            }
            WindowState::Positioned(ps) => {
                if entry.ext.is_iconic() {
                    entry.ext.show_cmd(ShowCmd::Restore);
                }
                match ps {
                    PositionedState::Float(fp) => {
                        let needs_topmost = focus_changed && is_focused;
                        let settled = fp.target == new_target && !needs_topmost;
                        (needs_topmost, settled)
                    }
                    PositionedState::Tiling(_) | PositionedState::Offscreen { .. } => (true, false),
                }
            }
        };

        if let Some(overlay) = self.float_overlays.get_mut(&id) {
            if needs_topmost {
                entry.ext.set_position(ZOrder::Topmost, x, y, w, h);
                overlay.update(wp, &self.config, ZOrder::After(entry.ext.id()));
            } else if !settled {
                // Unchanged is safe: this branch only fires for Float-to-Float
                // position changes where the window is already visible from a
                // prior Topmost placement.
                entry.ext.set_position(ZOrder::Unchanged, x, y, w, h);
                overlay.update(wp, &self.config, ZOrder::After(entry.ext.id()));
            } else if focus_changed {
                // Full overlay update is acceptable here: typically 1-3 floats, each a single GL draw.
                // Matches macOS which unconditionally re-renders every float overlay every frame.
                overlay.update(wp, &self.config, ZOrder::Unchanged);
            }
        }

        if !settled {
            entry.state = WindowState::Positioned(PositionedState::Float(FloatPlacement {
                target: new_target,
            }));
        }
    }

    pub(super) fn show_tiling(
        &mut self,
        id: WindowId,
        wp: &TilingWindowPlacement,
        monitor: MonitorId,
    ) {
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        let border = self.config.border_size;
        let content = apply_inset(wp.frame, border);
        let x = content.x.round() as i32;
        let y = content.y.round() as i32;
        let w = content.width.round() as i32;
        let h = content.height.round() as i32;
        let new_target = (x, y, w, h);

        let (z, prev_actual) = match entry.state {
            WindowState::FullscreenBorderless | WindowState::FullscreenExclusive => {
                debug_assert!(false, "show_tiling called on fullscreen window {id}");
                return;
            }
            // Stable: same monitor, same target. No set_position needed.
            WindowState::Positioned(PositionedState::Tiling(d))
                if d.monitor == monitor && d.target == new_target =>
            {
                return;
            }
            // Same monitor, target drifted: reposition without raising.
            WindowState::Positioned(PositionedState::Tiling(d)) if d.monitor == monitor => {
                (ZOrder::Unchanged, d.actual)
            }
            // New window, restored from Minimized/Offscreen, Float->Tiling, or
            // cross-monitor Tiling: raise to Top above the overlay.
            WindowState::Minimized | WindowState::UserMinimized => {
                entry.ext.show_cmd(ShowCmd::Restore);
                (ZOrder::Top, new_target)
            }
            WindowState::Positioned(ps) => {
                if entry.ext.is_iconic() {
                    entry.ext.show_cmd(ShowCmd::Restore);
                }
                let prev = match ps {
                    PositionedState::Tiling(d) => d.actual,
                    // Post-sync: fp.target is the last observed rect
                    PositionedState::Float(fp) => fp.target,
                    PositionedState::Offscreen { actual, .. } => actual,
                };
                (ZOrder::Top, prev)
            }
        };

        entry.ext.set_position(z, x, y, w, h);
        entry.state = WindowState::Positioned(PositionedState::Tiling(DriftState {
            target: new_target,
            actual: prev_actual,
            retries: 0,
            monitor,
        }));
    }

    pub(super) fn show_fullscreen_window(
        &mut self,
        id: WindowId,
        dimension: Dimension,
        monitor: MonitorId,
    ) {
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        match entry.state {
            WindowState::FullscreenBorderless | WindowState::FullscreenExclusive => {}
            WindowState::Minimized | WindowState::UserMinimized => {
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
                    PositionedState::Tiling(d) => d.actual,
                    // Post-sync: fp.target is the last observed rect
                    PositionedState::Float(fp) => fp.target,
                    PositionedState::Offscreen { actual, .. } => actual,
                };
                entry.state = WindowState::Positioned(PositionedState::Tiling(DriftState {
                    target: new_target,
                    actual: prev_actual,
                    retries: 0,
                    monitor,
                }));
            }
        }
    }

    pub(super) fn hide_window(&mut self, id: WindowId) {
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        match entry.state {
            WindowState::Positioned(PositionedState::Tiling(d)) => {
                entry.ext.move_offscreen();
                if let Some(overlay) = self.float_overlays.get_mut(&id) {
                    overlay.hide();
                }
                entry.state = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: d.actual,
                });
            }
            WindowState::Positioned(PositionedState::Float(fp)) => {
                entry.ext.move_offscreen();
                if let Some(overlay) = self.float_overlays.get_mut(&id) {
                    overlay.hide();
                }
                // Post-sync: fp.target is the last observed rect
                entry.state = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: fp.target,
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
            WindowState::FullscreenExclusive
            | WindowState::Minimized
            | WindowState::UserMinimized => {}
        }
    }

    pub(super) fn window_entered_borderless_fullscreen(&mut self, id: WindowId) {
        let Some(window) = self.registry.get_mut(id) else {
            return;
        };
        match window.state {
            WindowState::Positioned(_) => {
                window.state = WindowState::FullscreenBorderless;
                self.hub
                    .set_fullscreen(id, WindowRestrictions::ProtectFullscreen);
            }
            WindowState::FullscreenExclusive | WindowState::FullscreenBorderless => {}
            WindowState::Minimized | WindowState::UserMinimized => {
                window.ext.show_cmd(ShowCmd::Restore)
            }
        }
    }

    pub(super) fn window_drifted(&mut self, id: WindowId, x: i32, y: i32, w: i32, h: i32) {
        let visible_rect = (x, y, w, h);
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        match &mut entry.state {
            WindowState::FullscreenExclusive | WindowState::UserMinimized => {}
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
            WindowState::Positioned(PositionedState::Tiling(drift)) => {
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
            // Float windows accept the OS-reported position: the user dragged/resized
            // them, so we sync core and mark the position as settled.
            WindowState::Positioned(PositionedState::Float(fp)) => {
                fp.target = visible_rect;
                let outer_dim = reverse_inset(visible_rect, self.config.border_size);
                self.hub.update_float_dimension(id, outer_dim);
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
        if let Some(entry) = self.registry.get_mut(id) {
            entry.state = WindowState::FullscreenExclusive;
        }
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

/// Inverse of `apply_inset`: converts an observed content rect (post-inset, i32)
/// back to the outer frame stored in core's `float_windows`.
// TODO: revisit if config.border_size is ever non-integer -- round-trip can drift by +/-1 px per edge
fn reverse_inset(visible: (i32, i32, i32, i32), border: f32) -> Dimension {
    let (x, y, w, h) = visible;
    Dimension {
        x: x as f32 - border,
        y: y as f32 - border,
        width: w as f32 + 2.0 * border,
        height: h as f32 + 2.0 * border,
    }
}
