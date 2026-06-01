use crate::core::{
    Dimension, FloatWindowPlacement, Length, MonitorId, Physical, TilingWindowPlacement, WindowId,
    WindowRestrictions,
};
use crate::platform::windows::external::{ShowCmd, ZOrder};
use crate::platform::windows::handle::OFFSCREEN_POS;

use super::Dome;

pub(super) const MAX_DRIFT_RETRIES: u8 = 5;

#[derive(Clone, Copy)]
pub(super) struct DriftState {
    pub(super) target: Dimension<Physical>,
    /// The window's last known position -- written by `window_moved`
    /// via a dispatched `get_visible_rect` read. When a window goes offscreen,
    /// this preserves its position from before the hide: "actual" means where
    /// the window currently is (or last was), not where we want it.
    pub(super) actual: Dimension<Physical>,
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
    pub(super) target: Dimension<Physical>,
    /// Tracks OS ownership via `MonitorFromWindow`, reported atomically with
    /// the rect observation in `ObservedPosition::Visible`. Updated by
    /// `window_drifted` whenever a drift observation arrives.
    pub(super) monitor: MonitorId,
}

/// Tracks the platform-level visibility and fullscreen status of a managed window.
///
/// The hub tracks logical state (tiling vs float, which workspace). This enum
/// tracks what the platform layer has actually done to the window: is it
/// visible, hidden offscreen, in a fullscreen mode, or hidden via a Dome-
/// driven minimize. User-initiated minimize is captured by the orthogonal
/// `is_minimized` flag on `ManagedWindow`, which preserves the prior state
/// across the minimize round trip.
#[derive(Clone, Copy)]
pub(super) enum WindowState {
    /// Window is under Dome's positional control.
    Positioned(PositionedState),
    /// Window covers the entire monitor, initiated by the user (e.g. a game
    /// or media player). Detected by comparing window dimensions to monitor
    /// dimensions in `check_fullscreen_state`.
    BorderlessFullscreen,
    /// Borderless-fullscreen window currently OS-minimized by Dome because
    /// its workspace is inactive. Hub-side fullscreen is preserved;
    /// transitioning back to `BorderlessFullscreen` (and a `ShowCmd::Restore`)
    /// brings it back. Mutually exclusive with the user-initiated
    /// `is_minimized` flag on `ManagedWindow`: the user can't minimize a
    /// window that's already hidden by Dome on an inactive workspace.
    BorderlessMinimized,
    /// D3D/Vulkan exclusive fullscreen. Dome must not reposition or minimize
    /// these windows — doing so can crash the application or corrupt the
    /// display. Detected via `is_d3d_exclusive_fullscreen_active` in
    /// `handle_display_change`.
    ExclusiveFullscreen,
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
        actual: Dimension<Physical>,
    },
}

impl std::fmt::Display for WindowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Positioned(PositionedState::Tiling(_)) => write!(f, "tiling"),
            Self::Positioned(PositionedState::Float(_)) => write!(f, "float"),
            Self::Positioned(PositionedState::Offscreen { .. }) => write!(f, "offscreen"),
            Self::BorderlessFullscreen => write!(f, "borderless-fullscreen"),
            Self::BorderlessMinimized => write!(f, "borderless-minimized"),
            Self::ExclusiveFullscreen => write!(f, "exclusive-fullscreen"),
        }
    }
}

impl Dome {
    #[tracing::instrument(
        level = "trace",
        skip(self, wp),
        fields(window_id = %id, window = tracing::field::Empty),
    )]
    pub(super) fn show_float(
        &mut self,
        id: WindowId,
        wp: &FloatWindowPlacement,
        focus_changed: bool,
        is_focused: bool,
        // monitor is caller-supplied (not part of FloatWindowPlacement) for DPI scale lookup.
        monitor: MonitorId,
    ) {
        // Hub delivers frames in the OS-native unit (physical pixels on Windows
        // under PMv2).
        let scale = self.monitors[&monitor].scale;
        let border = self.physical_border(monitor);
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        tracing::Span::current().record("window", entry.to_string());
        let content = apply_inset(wp.frame, border);
        let new_target = content.round();

        let (needs_topmost, settled) = match entry.state {
            WindowState::BorderlessFullscreen
            | WindowState::BorderlessMinimized
            | WindowState::ExclusiveFullscreen => {
                debug_assert!(
                    false,
                    "show_float called on fullscreen/borderless-minimized window {id}"
                );
                return;
            }
            WindowState::Positioned(ps) => {
                debug_assert!(
                    !entry.is_minimized,
                    "show_float reached with user-minimized window {id}: minimized \
                     windows are detached from their workspace by the hub"
                );
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
                entry.ext.set_position(ZOrder::Topmost, new_target);
                overlay.update(wp, &self.config, ZOrder::After(entry.ext.id()), scale);
            } else if !settled {
                // Unchanged is safe: this branch only fires for Float-to-Float
                // position changes where the window is already visible from a
                // prior Topmost placement.
                entry.ext.set_position(ZOrder::Unchanged, new_target);
                overlay.update(wp, &self.config, ZOrder::After(entry.ext.id()), scale);
            } else if focus_changed {
                // Full overlay update is acceptable here: typically 1-3 floats, each a single GL draw.
                // Matches macOS which unconditionally re-renders every float overlay every frame.
                overlay.update(wp, &self.config, ZOrder::Unchanged, scale);
            }
        }

        if !settled {
            entry.state = WindowState::Positioned(PositionedState::Float(FloatPlacement {
                target: new_target,
                monitor,
            }));
        }
    }

    #[tracing::instrument(
        level = "trace",
        skip(self, wp),
        fields(window_id = %id, window = tracing::field::Empty),
    )]
    pub(super) fn show_tiling(
        &mut self,
        id: WindowId,
        wp: &TilingWindowPlacement,
        monitor: MonitorId,
    ) {
        // Hub delivers frames in physical pixels on Windows.
        let border = self.physical_border(monitor);

        let overlay = self
            .tiling_overlays
            .get_mut(&monitor)
            .expect("tiling overlay exists for monitor");
        let above = overlay.window_above();

        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        tracing::Span::current().record("window", entry.to_string());
        let content = apply_inset(wp.frame, border);
        let new_target = content.round();

        let tiling_state = |actual: Dimension<Physical>| {
            WindowState::Positioned(PositionedState::Tiling(DriftState {
                target: new_target,
                actual,
                retries: 0,
                monitor,
            }))
        };

        // Fullscreen windows should never reach show_tiling. The hub routes
        // fullscreen windows through show_fullscreen_window instead.
        if matches!(
            entry.state,
            WindowState::BorderlessFullscreen | WindowState::ExclusiveFullscreen
        ) {
            debug_assert!(false, "show_tiling called on fullscreen window {id}");
            return;
        }

        debug_assert!(
            !entry.is_minimized,
            "show_tiling reached with user-minimized window {id}: minimized windows \
             are detached from their workspace by the hub"
        );

        match entry.state {
            WindowState::Positioned(PositionedState::Tiling(d)) => {
                if d.monitor != monitor {
                    // Cross-monitor: window is re-entering a different overlay's
                    // monitor.
                    match above {
                        Some(prev) => {
                            entry.ext.set_position(ZOrder::After(prev), new_target);
                            entry.state = tiling_state(d.actual);
                        }
                        None => {
                            entry.ext.set_position(ZOrder::Unchanged, new_target);
                            let id = entry.ext.id();
                            entry.state = tiling_state(d.actual);
                            overlay.demote_below(id);
                        }
                    }
                } else if d.target != new_target {
                    // Same-monitor drift: reposition without touching z-order.
                    entry.ext.set_position(ZOrder::Unchanged, new_target);
                    entry.state = tiling_state(d.actual);
                }
                // else: stable on the same monitor at the same target, no-op.
            }
            WindowState::Positioned(PositionedState::Float(fp)) => {
                // Two-step exit from the topmost band. Placing self below a
                // non-topmost reference does not, by itself, clear WS_EX_TOPMOST;
                // only HWND_NOTOPMOST and HWND_BOTTOM are documented to drop the
                // flag. NotTopmost first to escape the band, then a second call
                // to position above the overlay reference.
                entry.ext.set_position(ZOrder::NotTopmost, new_target);
                match above {
                    Some(prev) => {
                        entry.ext.set_position(ZOrder::After(prev), new_target);
                        entry.state = tiling_state(fp.target);
                    }
                    None => {
                        // NotTopmost above already wrote geometry; just park
                        // the overlay below self.
                        let id = entry.ext.id();
                        entry.state = tiling_state(fp.target);
                        overlay.demote_below(id);
                    }
                }
            }
            WindowState::Positioned(PositionedState::Offscreen { actual, .. }) => match above {
                Some(prev) => {
                    entry.ext.set_position(ZOrder::After(prev), new_target);
                    entry.state = tiling_state(actual);
                }
                None => {
                    entry.ext.set_position(ZOrder::Unchanged, new_target);
                    let id = entry.ext.id();
                    entry.state = tiling_state(actual);
                    overlay.demote_below(id);
                }
            },
            // Fullscreen and borderless-minimized variants are early-returned
            // above; reaching here means the guard was bypassed or removed.
            WindowState::BorderlessFullscreen
            | WindowState::BorderlessMinimized
            | WindowState::ExclusiveFullscreen => {
                unreachable!(
                    "fullscreen / borderless-minimized variants are handled by the \
                     early-return guard above"
                )
            }
        }
    }

    #[tracing::instrument(
        level = "trace",
        skip(self),
        fields(window_id = %id, window = tracing::field::Empty),
    )]
    pub(super) fn show_fullscreen_window(
        &mut self,
        id: WindowId,
        dimension: Dimension,
        monitor: MonitorId,
    ) {
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        tracing::Span::current().record("window", entry.to_string());
        // Borderless-fullscreen window hidden by Dome because its workspace
        // was inactive. The workspace is now visible again, so transition
        // back and drive the OS-side restore.
        if matches!(entry.state, WindowState::BorderlessMinimized) {
            entry.ext.show_cmd(ShowCmd::Restore);
            entry.state = WindowState::BorderlessFullscreen;
            return;
        }
        match entry.state {
            WindowState::BorderlessFullscreen
            | WindowState::BorderlessMinimized
            | WindowState::ExclusiveFullscreen => {}
            WindowState::Positioned(ps) => {
                let new_target = dimension.round();
                if matches!(ps, PositionedState::Tiling(d) if d.target == new_target) {
                    return;
                }
                entry.ext.set_position(ZOrder::Unchanged, new_target);
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
        if entry.is_minimized || matches!(entry.state, WindowState::BorderlessMinimized) {
            // Already hidden via minimize (by user or by Dome); nothing to do.
            return;
        }
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
            WindowState::BorderlessFullscreen => {
                entry.ext.show_cmd(ShowCmd::Minimize);
                entry.state = WindowState::BorderlessMinimized;
            }
            WindowState::Positioned(PositionedState::Offscreen { actual, .. }) => {
                if actual.x > OFFSCREEN_POS && actual.y > OFFSCREEN_POS {
                    entry.ext.move_offscreen();
                }
            }
            WindowState::BorderlessMinimized => unreachable!("handled by early return above"),
            WindowState::ExclusiveFullscreen => {}
        }
    }

    pub(super) fn window_entered_borderless_fullscreen(&mut self, id: WindowId) {
        let Some(window) = self.registry.get_mut(id) else {
            return;
        };
        if window.is_minimized {
            window.ext.show_cmd(ShowCmd::Restore);
            window.is_minimized = false;
        }
        match window.state {
            WindowState::Positioned(_) | WindowState::BorderlessMinimized => {
                window.state = WindowState::BorderlessFullscreen;
                self.hub
                    .set_fullscreen(id, WindowRestrictions::ProtectFullscreen);
            }
            WindowState::ExclusiveFullscreen | WindowState::BorderlessFullscreen => {}
        }
    }

    pub(super) fn window_drifted(
        &mut self,
        id: WindowId,
        visible_rect: Dimension<Physical>,
        monitor_handle: isize,
    ) {
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        if entry.is_minimized {
            // The user is interacting with the picker; the next apply_layout
            // will reposition.
            return;
        }
        if matches!(entry.state, WindowState::BorderlessMinimized) {
            // The OS handed us a real geometry observation for a window Dome
            // had hidden via minimize. The fullscreen has been forcibly
            // resurfaced (Dock click, etc.); demote to Offscreen and unset
            // hub-side fullscreen.
            tracing::trace!(%id, "Previously-minimized borderless-fullscreen window reappeared");
            entry.ext.show_cmd(ShowCmd::Restore);
            entry.state = WindowState::Positioned(PositionedState::Offscreen {
                retries: 0,
                actual: visible_rect,
            });
            self.hub.unset_fullscreen(id);
            return;
        }
        match &mut entry.state {
            WindowState::ExclusiveFullscreen => {}
            WindowState::BorderlessFullscreen => {
                entry.state = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: visible_rect,
                });
                self.hub.unset_fullscreen(id);
            }
            WindowState::BorderlessMinimized => unreachable!("handled by early return above"),
            WindowState::Positioned(PositionedState::Tiling(drift)) => {
                drift.actual = visible_rect;
                if drift.actual != drift.target {
                    drift.retries = drift.retries.saturating_add(1);
                    if drift.retries > MAX_DRIFT_RETRIES {
                        tracing::debug!("Drift retries exhausted, giving up");
                    } else {
                        tracing::trace!(%id, target = ?drift.target, actual = ?drift.actual, retries = drift.retries, "window drifted, correcting");
                        entry.ext.set_position(ZOrder::Unchanged, drift.target);
                    }
                }
            }
            // Float windows accept the OS-reported position: the user dragged/resized
            // them, so we sync core and mark the position as settled.
            // The observation is physical pixels (DWM extended frame bounds); stored
            // directly since core is physical-native on Windows.
            WindowState::Positioned(PositionedState::Float(fp)) => {
                // Look up OS-reported monitor. On miss (display-topology race with
                // reconcile_monitors), skip the entire observation -- the next drift
                // event after reconcile will converge.
                let resolved = match self.monitor_handles.get(&monitor_handle) {
                    Some(&id) => id,
                    None => {
                        tracing::debug!(
                            handle = monitor_handle,
                            %id,
                            "MonitorFromWindow returned an HMONITOR not in monitor_handles; \
                             skipping float-drift observation (display-topology race with \
                             reconcile_monitors)"
                        );
                        return;
                    }
                };
                fp.monitor = resolved;
                fp.target = visible_rect;
                // Inlined: can't call self.physical_border() here because the
                // mutable borrow on self.registry (via entry/fp) conflicts with
                // the shared &self the method needs. Same expression as physical_border().
                let scale = self.monitors[&resolved].scale;
                let border = Length::<Physical>::new(self.config.border_size * scale);
                let outer_dim = reverse_inset(visible_rect, border);
                self.hub.update_float_dimension(id, outer_dim);

                // Reposition the float overlay to follow the drag. This lives in
                // window_drifted (not show_float) because show_float's `settled`
                // gate short-circuits after window_drifted writes both fp.target
                // and hub dimension in sync. Moving through show_float's !settled
                // branch would call entry.ext.set_position on the user-dragged
                // HWND, violating the drag invariant.
                let hwnd = entry.ext.id();
                // visible_frame == outer_dim here intentionally skips clip(frame, screen).
                // Windows constrains drag targets to on-screen and the OS clips HWND
                // rendering at monitor edges, so the unclipped rect is fine for the
                // overlay update.
                let wp = FloatWindowPlacement {
                    id,
                    frame: outer_dim,
                    visible_frame: outer_dim,
                    is_highlighted: self.last_focused == Some(id),
                };
                // ZOrder::After(hwnd) because click-to-drag foregrounds the managed
                // HWND, which can push it above the overlay. After(hwnd) re-establishes
                // "overlay immediately above its window". Unchanged would leave the
                // overlay covered.
                if let Some(overlay) = self.float_overlays.get_mut(&id) {
                    overlay.update(&wp, &self.config, ZOrder::After(hwnd), scale);
                }
            }
            WindowState::Positioned(PositionedState::Offscreen { retries, actual }) => {
                *actual = visible_rect;
                if actual.x > OFFSCREEN_POS && actual.y > OFFSCREEN_POS {
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
            entry.state = WindowState::ExclusiveFullscreen;
        }
        self.hub.set_fullscreen(id, WindowRestrictions::BlockAll);
    }

    /// Physical-pixel border width for `monitor`. `config.border_size` is
    /// a logical, config-denominated value; scaling it by the monitor's DPI
    /// scale at this boundary is the shell's contract with `apply_inset` /
    /// `reverse_inset`, both of which operate in physical pixels on Windows.
    pub(super) fn physical_border(&self, monitor: MonitorId) -> Length<Physical> {
        Length::new(self.config.border_size * self.monitors[&monitor].scale)
    }
}

fn apply_inset(dim: Dimension<Physical>, border: Length<Physical>) -> Dimension<Physical> {
    Dimension::new(
        dim.x + border,
        dim.y + border,
        (dim.width - 2.0 * border).max(Length::ZERO),
        (dim.height - 2.0 * border).max(Length::ZERO),
    )
}

/// Inverse of `apply_inset`: converts a content rect back to the outer frame
/// stored in core's `float_windows`. Both input and output are in physical
/// pixels on Windows.
fn reverse_inset(visible: Dimension<Physical>, border: Length<Physical>) -> Dimension<Physical> {
    Dimension::new(
        visible.x - border,
        visible.y - border,
        visible.width + 2.0 * border,
        visible.height + 2.0 * border,
    )
}
