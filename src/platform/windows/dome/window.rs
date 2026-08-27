use std::sync::Arc;
use std::time::Instant;

use super::Dome;
use super::display_from_process;
use super::events::FloatOverlayAction;
use crate::core::{
    FloatWindowPlacement, LimitObservation, MonitorId, Physical, PixelRect, Pixels,
    TilingWindowPlacement, WindowId, WindowRestrictions,
};
use crate::core::{WindowMatcher, pattern_matches};
use crate::platform::windows::external::{ManageExternalWindow, ShowCmd, ZOrder};
use crate::platform::windows::handle::OFFSCREEN_POS;

/// Per-window metadata gathered by the inspection worker that travels
/// together through `add_window` and the per-mode `insert_*_window`
/// helpers. Exists to keep those signatures from accumulating ~five
/// always-co-occurring scalars apiece.
pub(in crate::platform::windows) struct NewWindow {
    pub(in crate::platform::windows) ext: Arc<dyn ManageExternalWindow>,
    pub(in crate::platform::windows) metadata: WindowsMetadata,
    pub(in crate::platform::windows) constraints: LimitObservation,
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
    pub process_path: Option<String>,
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
    fn app_name(&self) -> Option<String> {
        self.app_name
            .clone()
            .or_else(|| Some(display_from_process(&self.process)))
    }
    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    fn executable_path(&self) -> Option<String> {
        self.process_path.clone()
    }
    fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }
    fn clone_box(&self) -> Box<dyn crate::core::WindowMetadata> {
        Box::new(self.clone())
    }

    fn matches_window_matcher(&self, matcher: &WindowMatcher) -> bool {
        let app = self.app_name.as_deref();
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
        if let Some(p) = matcher.app.as_deref()
            && !app.is_some_and(|a| pattern_matches(p, a))
        {
            return false;
        }
        matcher.app.is_some()
            || matcher.process.is_some()
            || matcher.title.is_some()
            || matcher.class.is_some()
            || matcher.aumid.is_some()
    }

    fn to_window_matcher(&self) -> WindowMatcher {
        WindowMatcher {
            app: self.app_name.clone(),
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
    pub(super) target: PixelRect<Physical>,
    /// The window's last known position reported by the OS.
    pub(super) actual: PixelRect<Physical>,
    pub(super) retries: u8,
    /// Monitor this window was last placed on.
    pub(super) monitor: MonitorId,
    /// Anchor for the most recent outbound `set_position` for this state.
    /// Observations stamped before this instant are pre-placement and dropped.
    pub(super) placed_at: Instant,
}

impl DriftState {
    pub(super) fn new(
        target: PixelRect<Physical>,
        actual: PixelRect<Physical>,
        monitor: MonitorId,
    ) -> Self {
        debug_assert!(
            !target.is_empty(),
            "an empty content box is filtered out before it reaches a placement"
        );
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
/// dropped by the window. The drift retry timer catches that gap.
#[derive(Clone, Copy)]
pub(super) struct FloatPlacement {
    pub(super) target: PixelRect<Physical>,
    pub(super) actual: PixelRect<Physical>,
    pub(super) retries: u8,
    pub(super) monitor: MonitorId,
    pub(super) placed_at: Instant,
}

impl FloatPlacement {
    pub(super) fn new(
        target: PixelRect<Physical>,
        actual: PixelRect<Physical>,
        monitor: MonitorId,
    ) -> Self {
        debug_assert!(
            !target.is_empty(),
            "an empty content box is filtered out before it reaches a placement"
        );
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
    /// dimensions.
    BorderlessFullscreen,
    /// Borderless-fullscreen window currently OS-minimized by Dome because
    /// its workspace is inactive. Hub-side fullscreen is preserved.
    /// Transitioning back to `BorderlessFullscreen` (and a `ShowCmd::Restore`)
    /// brings it back. Mutually exclusive with the user-initiated
    /// `is_minimized` flag on `ManagedWindow`: the user can't minimize a
    /// window that's already hidden by Dome on an inactive workspace.
    BorderlessMinimized { retries: u8 },
    /// D3D/Vulkan exclusive fullscreen. Dome must not reposition or minimize
    /// these windows, doing so can crash the application or corrupt the
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
        actual: PixelRect<Physical>,
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
    #[must_use]
    pub(super) fn show_float(
        &mut self,
        id: WindowId,
        wp: &FloatWindowPlacement,
        focus_changed: bool,
        is_focused: bool,
        monitor: MonitorId,
        border_thickness: Pixels<Physical>,
    ) -> Option<FloatOverlayAction> {
        let scale = self.monitors.monitor(monitor).scale();
        let entry = self.registry.get_mut(id)?;
        let new_target = wp.content_box;
        debug_assert!(
            !entry.is_minimized,
            "show_float reached with user-minimized window {id}: minimized \
             windows are detached from their workspace"
        );

        let (needs_topmost, settled) = match entry.state {
            WindowState::BorderlessFullscreen
            | WindowState::BorderlessMinimized { .. }
            | WindowState::ExclusiveFullscreen => {
                unreachable!(
                    "fullscreen / borderless-minimized windows are routed through \
                     show_fullscreen_window by the hub"
                )
            }
            WindowState::Positioned(ps) => match ps {
                PositionedState::Float(fp) => {
                    let needs_topmost = focus_changed && is_focused;
                    let settled = fp.target == new_target && !needs_topmost;
                    (needs_topmost, settled)
                }
                PositionedState::Tiling(_) | PositionedState::Offscreen { .. } => (true, false),
            },
        };

        let hwnd_id = entry.ext.id();
        let ext = entry.ext.clone();
        let z_order = if needs_topmost {
            ext.set_position(ZOrder::Topmost, new_target);
            ZOrder::After(hwnd_id)
        } else if !settled {
            // Already Topmost, so leave the z-order unchanged rather than re-set topmost
            // and raise it up the stack.
            ext.set_position(ZOrder::Unchanged, new_target);
            ZOrder::After(hwnd_id)
        } else {
            ZOrder::Unchanged
        };
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

        Some(match self.float_overlays.get(&id) {
            Some(overlay) => {
                if !matches!(z_order, ZOrder::Unchanged) {
                    overlay.set_z_order(z_order);
                }
                FloatOverlayAction::Update {
                    window_id: id,
                    placement: *wp,
                    scale,
                    border_thickness,
                }
            }
            None => FloatOverlayAction::Create {
                window_id: id,
                placement: *wp,
                z_order,
                scale,
                border_thickness,
            },
        })
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
        z: ZOrder,
    ) {
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        let new_target = wp.content_box;

        let tiling_state = |actual: PixelRect<Physical>| {
            WindowState::Positioned(PositionedState::Tiling(DriftState::new(
                new_target, actual, monitor,
            )))
        };

        debug_assert!(
            !entry.is_minimized,
            "show_tiling reached with user-minimized window {id}: minimized windows \
             are detached from their workspace by the hub"
        );

        let ext = entry.ext.clone();
        let mut rect_changed = true;
        let mut escape_topmost = false;
        match entry.state {
            WindowState::Positioned(PositionedState::Tiling(d)) => {
                if d.monitor == monitor && d.target == new_target {
                    rect_changed = false;
                } else {
                    entry.state = tiling_state(d.actual);
                }
            }
            WindowState::Positioned(PositionedState::Float(fp)) => {
                escape_topmost = true;
                entry.state = tiling_state(fp.actual);
            }
            WindowState::Positioned(PositionedState::Offscreen { actual, .. }) => {
                entry.state = tiling_state(actual);
            }
            WindowState::BorderlessFullscreen
            | WindowState::BorderlessMinimized { .. }
            | WindowState::ExclusiveFullscreen => {
                unreachable!(
                    "fullscreen / borderless-minimized windows are routed through \
                     show_fullscreen_window by the hub"
                )
            }
        }

        if escape_topmost {
            ext.set_z_order(ZOrder::NotTopmost);
        }
        if rect_changed {
            ext.set_position(z, new_target);
        } else if !matches!(z, ZOrder::Unchanged) {
            // SWP_NOMOVE | SWP_NOSIZE, so a settled window reaches its slot without
            // dragging its owned children through a move.
            ext.set_z_order(z);
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
        work_area: PixelRect,
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
                if matches!(ps, PositionedState::Tiling(d) if d.target == work_area) {
                    return;
                }
                entry.ext.set_position(ZOrder::Unchanged, work_area);
                let prev_actual = match ps {
                    PositionedState::Tiling(d) => d.actual,
                    PositionedState::Float(fp) => fp.actual,
                    PositionedState::Offscreen { actual, .. } => actual,
                };
                entry.state = WindowState::Positioned(PositionedState::Tiling(DriftState::new(
                    work_area,
                    prev_actual,
                    monitor,
                )));
            }
        }
    }

    #[tracing::instrument(level = "trace", skip(self))]
    #[must_use]
    pub(super) fn hide_window(&mut self, id: WindowId) -> Option<FloatOverlayAction> {
        let entry = self.registry.get_mut(id)?;
        if entry.is_minimized {
            return None;
        }
        match entry.state {
            WindowState::Positioned(PositionedState::Tiling(d)) => {
                entry.ext.move_offscreen();
                entry.state = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: d.actual,
                });
                Some(FloatOverlayAction::Hide(id))
            }
            WindowState::Positioned(PositionedState::Float(fp)) => {
                entry.ext.move_offscreen();
                entry.state = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: fp.actual,
                });
                Some(FloatOverlayAction::Hide(id))
            }
            WindowState::BorderlessFullscreen => {
                entry.ext.show_cmd(ShowCmd::Minimize);
                entry.state = WindowState::BorderlessMinimized { retries: 0 };
                None
            }
            WindowState::Positioned(PositionedState::Offscreen { actual, .. }) => {
                if actual.x() > OFFSCREEN_POS && actual.y() > OFFSCREEN_POS {
                    entry.ext.move_offscreen();
                }
                None
            }
            WindowState::BorderlessMinimized { .. } => None,
            WindowState::ExclusiveFullscreen => None,
        }
    }

    /// Apply a fresh visible-rect observation from the OS.
    #[tracing::instrument(level = "trace", skip(self))]
    pub(in crate::platform::windows) fn window_moved(
        &mut self,
        id: WindowId,
        new_placement: PixelRect<Physical>,
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
                        let target = drift.target;
                        entry.ext.set_position(ZOrder::Unchanged, target);
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
                self.hub.update_float_rect(id, new_placement, resolved);
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
                if actual.x() > OFFSCREEN_POS && actual.y() > OFFSCREEN_POS {
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

    /// Re-issues the last placement when the window has not acknowledged it, up to
    /// `MAX_DRIFT_RETRIES` attempts.
    #[tracing::instrument(level = "trace", skip(self))]
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
                let target = drift.target;
                entry.ext.set_position(ZOrder::Unchanged, target);
            }
            WindowState::Positioned(PositionedState::Float(fp)) => {
                if fp.actual == fp.target || fp.retries > MAX_DRIFT_RETRIES {
                    return;
                }
                fp.retries = fp.retries.saturating_add(1);
                fp.placed_at = Instant::now();
                let target = fp.target;
                entry.ext.set_position(ZOrder::Unchanged, target);
            }
            WindowState::Positioned(PositionedState::Offscreen { retries, actual }) => {
                if actual.x() <= OFFSCREEN_POS || actual.y() <= OFFSCREEN_POS {
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

    #[tracing::instrument(level = "trace", skip(self))]
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

#[cfg(test)]
mod tests {
    use super::WindowsMetadata;
    use crate::core::{WindowMatcher, WindowMetadata};

    fn notepad() -> WindowsMetadata {
        WindowsMetadata {
            title: Some(String::from("Untitled - Notepad")),
            process: String::from("notepad.exe"),
            process_path: None,
            class: Some(String::from("Notepad")),
            aumid: None,
            app_name: Some(String::from("Notepad")),
        }
    }

    #[test]
    fn an_app_only_matcher_matches() {
        let matcher = WindowMatcher {
            app: Some(String::from("Notepad")),
            ..WindowMatcher::default()
        };
        assert!(notepad().matches_window_matcher(&matcher));
    }

    #[test]
    fn an_app_only_matcher_rejects_another_app() {
        let matcher = WindowMatcher {
            app: Some(String::from("Calculator")),
            ..WindowMatcher::default()
        };
        assert!(!notepad().matches_window_matcher(&matcher));
    }

    #[test]
    fn an_empty_matcher_matches_nothing() {
        assert!(!notepad().matches_window_matcher(&WindowMatcher::default()));
    }
}
