use std::sync::Arc;
use std::time::Instant;

use super::Dome;
use super::display_from_process;
use crate::config::{WindowMatcher, pattern_matches};
use crate::core::{
    Dimension, FloatWindowPlacement, Length, MonitorId, Physical, TilingWindowPlacement, WindowId,
    WindowRestrictions,
};
use crate::platform::windows::external::{ManageExternalWindow, ShowCmd, ZOrder};
use crate::platform::windows::handle::OFFSCREEN_POS;

/// Per-window metadata gathered by the inspection worker that travels
/// together through `add_window` and the per-mode `insert_*_window`
/// helpers. Exists to keep those signatures from accumulating ~five
/// always-co-occurring scalars apiece.
pub(in crate::platform::windows) struct NewWindow {
    pub(in crate::platform::windows) ext: Arc<dyn ManageExternalWindow>,
    pub(in crate::platform::windows) metadata: WindowsMetadata,
    pub(in crate::platform::windows) constraints: (f32, f32, f32, f32),
}

impl std::fmt::Display for NewWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[pid={}|hwnd={}] ", self.ext.pid(), self.ext.id())?;
        write!(f, "{}", self.metadata)
    }
}

#[derive(Debug, Clone)]
pub(in crate::platform::windows) struct WindowsMetadata {
    pub title: Option<String>,
    pub process: String,
    pub class: Option<String>,
    pub aumid: Option<String>,
    pub app_name: Option<String>,
}

impl std::fmt::Display for WindowsMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.app_name.as_deref() {
            Some(name) => write!(f, "{name} ({})", self.process)?,
            None => write!(f, "{}", self.process)?,
        }
        if let Some(title) = &self.title {
            write!(f, " - {title}")?;
        }
        if let Some(class) = &self.class {
            write!(f, " class={class}")?;
        }
        if let Some(aumid) = &self.aumid {
            write!(f, " aumid={aumid}")?;
        }
        Ok(())
    }
}

impl crate::core::WindowMetadata for WindowsMetadata {
    fn icon_key(&self) -> Option<String> {
        Some(self.process.clone())
    }
    fn app_name(&self) -> Option<String> {
        self.app_name
            .clone()
            .or_else(|| Some(display_from_process(&self.process)))
    }
    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }
    fn clone_box(&self) -> Box<dyn crate::core::WindowMetadata> {
        Box::new(self.clone())
    }

    fn matches_window_matcher(&self, matcher: &WindowMatcher) -> bool {
        let title = self.title.as_deref();
        let class = self.class.as_deref();
        let aumid = self.aumid.as_deref();

        if let Some(p) = matcher.process.as_deref()
            && !pattern_matches(p, &self.process)
        {
            return false;
        }
        if let Some(p) = matcher.title.as_deref()
            && !title.is_some_and(|t| pattern_matches(p, t))
        {
            return false;
        }
        if let Some(p) = matcher.class.as_deref()
            && !class.is_some_and(|c| pattern_matches(p, c))
        {
            return false;
        }
        if let Some(p) = matcher.aumid.as_deref()
            && !aumid.is_some_and(|a| pattern_matches(p, a))
        {
            return false;
        }
        matcher.process.is_some()
            || matcher.title.is_some()
            || matcher.class.is_some()
            || matcher.aumid.is_some()
    }

    fn to_window_matcher(&self) -> WindowMatcher {
        WindowMatcher {
            process: Some(self.process.clone()),
            title: self.title.clone(),
            class: self.class.clone(),
            aumid: self.aumid.clone(),
            ..Default::default()
        }
    }
}

pub(crate) const MAX_DRIFT_RETRIES: u8 = 5;

#[derive(Clone, Copy)]
pub(super) struct DriftState {
    /// Target state of the window, controlled by the tiling strategy.
    pub(super) target: Dimension<Physical>,
    /// The window's last known position reported by the OS.
    pub(super) actual: Dimension<Physical>,
    pub(super) retries: u8,
    /// Monitor this window was last placed on.
    pub(super) monitor: MonitorId,
    /// Anchor for the most recent outbound `set_position` for this state.
    /// Observations stamped before this instant are pre-placement and dropped.
    pub(super) placed_at: Instant,
}

impl DriftState {
    pub(super) fn new(
        target: Dimension<Physical>,
        actual: Dimension<Physical>,
        monitor: MonitorId,
    ) -> Self {
        Self {
            target,
            actual,
            retries: 0,
            monitor,
            placed_at: Instant::now(),
        }
    }
}

/// Placement state for floating windows. The `actual` field tracks the
/// last known OS-reported geometry, while `target` tracks what Dome last
/// issued via `set_position`. They diverge when a placement is silently
/// dropped by the window; the drift retry timer catches that gap.
#[derive(Clone, Copy)]
pub(super) struct FloatPlacement {
    pub(super) target: Dimension<Physical>,
    pub(super) actual: Dimension<Physical>,
    pub(super) retries: u8,
    pub(super) monitor: MonitorId,
    pub(super) placed_at: Instant,
}

impl FloatPlacement {
    pub(super) fn new(
        target: Dimension<Physical>,
        actual: Dimension<Physical>,
        monitor: MonitorId,
    ) -> Self {
        Self {
            target,
            actual,
            retries: 0,
            monitor,
            placed_at: Instant::now(),
        }
    }
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
    BorderlessMinimized { retries: u8 },
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
            Self::BorderlessMinimized { .. } => write!(f, "borderless-minimized"),
            Self::ExclusiveFullscreen => write!(f, "exclusive-fullscreen"),
        }
    }
}

impl Dome {
    #[tracing::instrument(
        level = "trace",
        skip(self, wp),
        fields(window_id = %id),
    )]
    pub(super) fn show_float(
        &mut self,
        id: WindowId,
        wp: &FloatWindowPlacement,
        focus_changed: bool,
        is_focused: bool,
        monitor: MonitorId,
    ) {
        let scale = self.monitors.monitor(monitor).scale();
        let border = self
            .monitors
            .physical_border(monitor, self.config.border_size);
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        let content = apply_inset(wp.frame, border);
        let new_target = content.round();

        let (needs_topmost, settled) = match entry.state {
            WindowState::BorderlessFullscreen
            | WindowState::BorderlessMinimized { .. }
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
                     windows are detached from their workspace"
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
                // Window is already Topmost, so we shouldn't set topmost again to avoid bringing it
                // up the z-order stack
                entry.ext.set_position(ZOrder::Unchanged, new_target);
                overlay.update(wp, &self.config, ZOrder::After(entry.ext.id()), scale);
            } else if focus_changed {
                overlay.update(wp, &self.config, ZOrder::Unchanged, scale);
            }
        }

        if !settled {
            let prev_actual = match &entry.state {
                WindowState::Positioned(PositionedState::Float(fp)) => fp.actual,
                WindowState::Positioned(PositionedState::Tiling(d)) => d.actual,
                WindowState::Positioned(PositionedState::Offscreen { actual, .. }) => *actual,
                _ => new_target,
            };
            entry.state = WindowState::Positioned(PositionedState::Float(FloatPlacement::new(
                new_target,
                prev_actual,
                monitor,
            )));
        }
    }

    #[tracing::instrument(
        level = "trace",
        skip(self, wp),
        fields(window_id = %id),
    )]
    pub(super) fn show_tiling(
        &mut self,
        id: WindowId,
        wp: &TilingWindowPlacement,
        monitor: MonitorId,
    ) {
        // Hub delivers frames in physical pixels on Windows.
        let border = self
            .monitors
            .physical_border(monitor, self.config.border_size);

        let overlay = self
            .tiling_overlays
            .get_mut(&monitor)
            .expect("tiling overlay exists for monitor");
        let above = overlay.window_above();

        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        let content = apply_inset(wp.frame, border);
        let new_target = content.round();

        let tiling_state = |actual: Dimension<Physical>| {
            WindowState::Positioned(PositionedState::Tiling(DriftState::new(
                new_target, actual, monitor,
            )))
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
                        entry.state = tiling_state(fp.actual);
                    }
                    None => {
                        // NotTopmost above already wrote geometry; just park
                        // the overlay below self.
                        let id = entry.ext.id();
                        entry.state = tiling_state(fp.actual);
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
            | WindowState::BorderlessMinimized { .. }
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
        fields(window_id = %id),
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
        // Borderless-fullscreen window hidden by Dome because its workspace
        // was inactive. The workspace is now visible again, so transition
        // back and drive the OS-side restore.
        if matches!(entry.state, WindowState::BorderlessMinimized { .. }) {
            entry.ext.show_cmd(ShowCmd::Restore);
            entry.state = WindowState::BorderlessFullscreen;
            return;
        }
        match entry.state {
            WindowState::BorderlessFullscreen
            | WindowState::BorderlessMinimized { .. }
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
                    PositionedState::Float(fp) => fp.actual,
                    PositionedState::Offscreen { actual, .. } => actual,
                };
                entry.state = WindowState::Positioned(PositionedState::Tiling(DriftState::new(
                    new_target,
                    prev_actual,
                    monitor,
                )));
            }
        }
    }

    pub(super) fn hide_window(&mut self, id: WindowId) {
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        if entry.is_minimized || matches!(entry.state, WindowState::BorderlessMinimized { .. }) {
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
                entry.state = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: fp.actual,
                });
            }
            WindowState::BorderlessFullscreen => {
                entry.ext.show_cmd(ShowCmd::Minimize);
                entry.state = WindowState::BorderlessMinimized { retries: 0 };
            }
            WindowState::Positioned(PositionedState::Offscreen { actual, .. }) => {
                if actual.x > OFFSCREEN_POS && actual.y > OFFSCREEN_POS {
                    entry.ext.move_offscreen();
                }
            }
            WindowState::BorderlessMinimized { .. } => {
                unreachable!("handled by early return above")
            }
            WindowState::ExclusiveFullscreen => {}
        }
    }

    /// Apply a fresh visible-rect observation from the OS.
    pub(in crate::platform::windows) fn window_moved(
        &mut self,
        id: WindowId,
        new_placement: Dimension<Physical>,
        monitor_handle: isize,
        observed_at: Instant,
    ) {
        let is_fullscreen = self
            .monitors
            .is_borderless_fullscreen_at(new_placement, monitor_handle);
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };

        if entry.is_minimized {
            self.hub.unminimize_window(id);
            entry.is_minimized = false;
        }

        match (&mut entry.state, is_fullscreen) {
            (WindowState::ExclusiveFullscreen, _) => {}

            (WindowState::BorderlessFullscreen, true) => {
                // Already in BorderlessFullscreen and still fullscreen-shaped:
                // either a Dome-issued placement echo or a benign re-observation.
            }
            (WindowState::BorderlessFullscreen, false) => {
                // Rect no longer covers the work area: user resized or moved
                // the window off the monitor, or unknown-monitor fall-through.
                entry.state = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: new_placement,
                });
                self.hub.unset_fullscreen(id);
            }

            (WindowState::BorderlessMinimized { retries }, true) => {
                *retries = retries.saturating_add(1);
                if *retries > MAX_DRIFT_RETRIES {
                    // Uses `>` (5 retries before give-up) to match the macOS
                    // `Placement::just_gave_up` pattern, keeping cross-platform
                    // symmetry. The neighbouring Offscreen arm uses `>=` (4 retries)
                    // because it inherited the older convention.
                    if *retries == MAX_DRIFT_RETRIES + 1 {
                        tracing::debug!(%id, "BorderlessMinimized resurface retries exhausted, giving up");
                    }
                    return;
                }
                entry.ext.show_cmd(ShowCmd::Minimize);
            }
            (WindowState::BorderlessMinimized { .. }, false) => {
                // Resurfaced but not fullscreen-shaped: user dragged or shrunk
                // it. Demote to Offscreen.
                tracing::trace!(%id, "Previously-minimized borderless-fullscreen window reappeared");
                entry.ext.show_cmd(ShowCmd::Restore);
                entry.state = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: new_placement,
                });
                self.hub.unset_fullscreen(id);
            }

            (WindowState::Positioned(PositionedState::Tiling(drift)), true) => {
                // Strict-<: an observation timestamped at the same Instant as placed_at
                // is fresh. A constraint enforcement echo arriving exactly at placement
                // time is the new target, not stale.
                if observed_at < drift.placed_at {
                    tracing::trace!(
                        %id, ?observed_at, placed_at = ?drift.placed_at,
                        "stale tiling observation, ignoring",
                    );
                    return;
                }
                if drift.target == new_placement {
                    tracing::trace!(%id, "ignoring fullscreen observation: new_placement matches Dome-issued target");
                    return;
                }
                entry.state = WindowState::BorderlessFullscreen;
                self.hub
                    .set_fullscreen(id, WindowRestrictions::ProtectFullscreen);
            }
            (WindowState::Positioned(PositionedState::Tiling(drift)), false) => {
                if observed_at < drift.placed_at {
                    tracing::trace!(
                        %id, ?observed_at, placed_at = ?drift.placed_at,
                        "stale tiling observation, ignoring",
                    );
                    return;
                }
                drift.actual = new_placement;
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

            (WindowState::Positioned(PositionedState::Float(fp)), true) => {
                if observed_at < fp.placed_at {
                    tracing::trace!(
                        %id, ?observed_at, placed_at = ?fp.placed_at,
                        "stale float observation, ignoring",
                    );
                    return;
                }
                // Float turned borderless fullscreen
                entry.state = WindowState::BorderlessFullscreen;
                self.hub
                    .set_fullscreen(id, WindowRestrictions::ProtectFullscreen);
            }
            (WindowState::Positioned(PositionedState::Float(fp)), false) => {
                if observed_at < fp.placed_at {
                    tracing::trace!(
                        %id, ?observed_at, placed_at = ?fp.placed_at,
                        "stale float observation, ignoring",
                    );
                    return;
                }
                let resolved = match self.monitors.id_for_handle(monitor_handle) {
                    Some(id) => id,
                    None => {
                        tracing::debug!(
                            handle = monitor_handle,
                            %id,
                            "MonitorFromWindow returned an HMONITOR not in monitor_handles; \
                             skipping float-drift observation"
                        );
                        return;
                    }
                };
                fp.monitor = resolved;
                fp.actual = new_placement;
                fp.target = new_placement;
                let border = self
                    .monitors
                    .physical_border(resolved, self.config.border_size);
                let scale = self.monitors.monitor(resolved).scale();
                let outer_dim = reverse_inset(new_placement, border);
                self.hub.update_float_dimension(id, outer_dim, resolved);

                // Reposition the float overlay to follow the drag.
                let hwnd = entry.ext.id();
                let wp = FloatWindowPlacement {
                    id,
                    frame: outer_dim,
                    visible_frame: outer_dim,
                    is_highlighted: self.last_focused == Some(id),
                };
                if let Some(overlay) = self.float_overlays.get_mut(&id) {
                    overlay.update(&wp, &self.config, ZOrder::After(hwnd), scale);
                }
            }

            (
                WindowState::Positioned(PositionedState::Offscreen {
                    retries: _,
                    actual: _,
                }),
                true,
            ) => {
                // Window turned fullscreen, but not visible, so we hide it again.
                self.hub
                    .set_fullscreen(id, WindowRestrictions::ProtectFullscreen);
                entry.state = WindowState::BorderlessMinimized { retries: 0 };
                entry.ext.show_cmd(ShowCmd::Minimize);
            }

            (WindowState::Positioned(PositionedState::Offscreen { retries, actual }), false) => {
                *actual = new_placement;
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

    /// Called periodically by the drift retry timer.
    /// Re-issues the last placement if the window has not yet
    /// acknowledged it, up to `MAX_DRIFT_RETRIES` attempts.
    pub(super) fn retry_drift(&mut self, id: WindowId) {
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        match &mut entry.state {
            WindowState::Positioned(PositionedState::Tiling(drift)) => {
                if drift.actual == drift.target || drift.retries > MAX_DRIFT_RETRIES {
                    return;
                }
                drift.retries = drift.retries.saturating_add(1);
                drift.placed_at = Instant::now();
                entry.ext.set_position(ZOrder::Unchanged, drift.target);
            }
            WindowState::Positioned(PositionedState::Float(fp)) => {
                if fp.actual == fp.target || fp.retries > MAX_DRIFT_RETRIES {
                    return;
                }
                fp.retries = fp.retries.saturating_add(1);
                fp.placed_at = Instant::now();
                entry.ext.set_position(ZOrder::Unchanged, fp.target);
            }
            WindowState::Positioned(PositionedState::Offscreen { retries, actual }) => {
                if actual.x <= OFFSCREEN_POS || actual.y <= OFFSCREEN_POS {
                    return;
                }
                if *retries >= MAX_DRIFT_RETRIES {
                    return;
                }
                *retries = retries.saturating_add(1);
                entry.ext.move_offscreen();
            }
            _ => {}
        }
    }

    pub(super) fn enter_fullscreen_exclusive(&mut self, id: WindowId) {
        let was_minimized = self
            .registry
            .get(id)
            .map(|entry| entry.is_minimized)
            .unwrap_or(false);
        if was_minimized {
            self.hub.unminimize_window(id);
            if let Some(entry) = self.registry.get_mut(id) {
                entry.is_minimized = false;
            }
        }
        if let Some(entry) = self.registry.get_mut(id) {
            entry.state = WindowState::ExclusiveFullscreen;
        }
        self.hub.set_fullscreen(id, WindowRestrictions::BlockAll);
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
