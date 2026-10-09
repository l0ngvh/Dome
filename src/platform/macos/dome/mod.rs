mod events;
mod inspect;
mod layout;
mod monitor;
mod recovery;
mod registry;
mod window;

pub(super) use events::{
    ContainerShow, FloatShow, HubEvent, HubMessage, MirrorShow, TilingWindowShow,
};
pub(super) use inspect::{
    ExitNativeFullscreen, ExtRefresh, compute_reconcile_all, compute_reconciliation,
    compute_window_positions,
};
pub(super) use monitor::{MonitorInfo, get_all_monitors};
use window::WindowState;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use calloop::LoopSignal;
use objc2_core_graphics::CGWindowID;

use crate::action::{Actions, MinimizedWindow};
use crate::config::{Config, Keystroke, LuaRuntime, PlatformEffects, PreferredLayouts};
use crate::core::{
    ContainerId, Dimension, Hub, Length, Logical, PixelRect, WindowId, WindowMetadata,
    WindowRestrictions,
};
use crate::core::{TilingConfig, WindowMatcher, pattern_matches};
use crate::platform::keymap::KeymapPublisher;
use crate::platform::macos::accessibility::ExternalWindow;

use monitor::MonitorRegistry;
use recovery::Recovery;
use registry::ManagedWindow;
pub(in crate::platform::macos) use registry::WindowRegistry;

struct MacPlatformEffects<'a> {
    registry: &'a mut WindowRegistry,
    signal: &'a LoopSignal,
    env: &'a HashMap<String, String>,
}

impl PlatformEffects for MacPlatformEffects<'_> {
    fn close(&mut self, id: WindowId) {
        self.registry.close_window(id);
    }

    fn execute(&mut self, command: &str) {
        if let Err(e) = crate::platform::macos::spawn::spawn_disclaimed_sh(command, self.env) {
            tracing::warn!(%command, "Failed to execute: {e}");
        }
    }

    fn exit(&mut self) {
        self.signal.stop();
    }

    fn unminimize(&mut self, id: WindowId) {
        self.registry.unminimize_window(id);
    }
}

pub(in crate::platform::macos) struct NewWindow {
    pub(in crate::platform::macos) ax: Arc<dyn ExternalWindow>,
    pub(in crate::platform::macos) metadata: MacOSMetadata,
}

impl std::fmt::Display for NewWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[pid={}|cg={}] ", self.ax.pid(), self.ax.cg_id())?;
        write!(f, "{}", self.metadata)
    }
}

#[derive(Debug, Clone)]
pub(in crate::platform::macos) struct MacOSMetadata {
    pub title: Option<String>,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

impl std::fmt::Display for MacOSMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.app_name.as_deref().unwrap_or("Unknown"))?;
        if let Some(bid) = &self.bundle_id {
            write!(f, " ({bid})")?;
        }
        if let Some(t) = &self.title {
            write!(f, " - {t}")?;
        }
        Ok(())
    }
}

impl WindowMetadata for MacOSMetadata {
    fn app_name(&self) -> Option<String> {
        self.app_name.clone()
    }
    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    fn bundle_id(&self) -> Option<String> {
        self.bundle_id.clone()
    }
    fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }
    fn clone_box(&self) -> Box<dyn WindowMetadata> {
        Box::new(self.clone())
    }

    fn matches_window_matcher(&self, matcher: &WindowMatcher) -> bool {
        let app = self.app_name.as_deref();
        let bundle_id = self.bundle_id.as_deref();
        let title = self.title.as_deref();

        if let Some(p) = matcher.app.as_deref()
            && !app.is_some_and(|a| pattern_matches(p, a))
        {
            return false;
        }
        if let Some(p) = matcher.bundle_id.as_deref()
            && !bundle_id.is_some_and(|b| pattern_matches(p, b))
        {
            return false;
        }
        if let Some(p) = matcher.title.as_deref()
            && !title.is_some_and(|t| pattern_matches(p, t))
        {
            return false;
        }
        if app.is_none() && bundle_id.is_none() && title.is_none() {
            return false;
        }
        matcher.app.is_some() || matcher.bundle_id.is_some() || matcher.title.is_some()
    }

    fn to_window_matcher(&self) -> WindowMatcher {
        WindowMatcher {
            app: self.app_name.clone(),
            bundle_id: self.bundle_id.clone(),
            title: self.title.clone(),
            ..Default::default()
        }
    }
}

pub(in crate::platform::macos) enum PendingAdd {
    Positioned {
        new: NewWindow,
        rect: PixelRect,
        corner_radius: Length<Logical>,
    },
    /// Native fullscreen windows lives on their own space and thus has no dimension
    NativeFullscreen { new: NewWindow },
}

/// Timestamps of the first and last AX move/resize notifications in a
/// coalesced debounce burst (equal when only one fired). The first is
/// compared against the post-placement debounce window (was this burst
/// caused by our placement?), and the last against the latest placement
/// time (is this burst stale?).
#[derive(Debug, Copy, Clone)]
pub(in crate::platform::macos) struct DebounceBurst {
    pub(in crate::platform::macos) first: Instant,
    pub(in crate::platform::macos) last: Instant,
}

pub(in crate::platform::macos) struct WindowMove {
    pub(in crate::platform::macos) cg_id: CGWindowID,
    pub(in crate::platform::macos) rect: PixelRect,
    pub(in crate::platform::macos) observed_at: DebounceBurst,
}

pub(in crate::platform::macos) trait SceneSender: Send {
    fn send(&self, msg: HubMessage);
}

/// Platform-specific state machine that bridges macOS accessibility events with the
/// core tree model. Event-loop–facing methods accept `CGWindowID` rather than `WindowId`
/// because callers dispatch work to background threads that capture registry snapshots —
/// by the time results arrive the window may have been removed, so resolution to
/// `WindowId` happens here where the registry can be checked.
pub(in crate::platform::macos) struct Dome {
    hub: Hub,
    registry: WindowRegistry,
    monitor_registry: MonitorRegistry,
    /// The windows Dome currently has on screen. Owned here rather than per monitor entry
    /// so it survives a monitor removal.
    displayed_windows: HashSet<WindowId>,
    /// Full height of the primary display (including menu bar/dock), used for Quartz→Cocoa
    /// coordinate conversion in overlay rendering.
    primary_full_height: f32,
    observed_pids: HashSet<i32>,
    sender: Box<dyn SceneSender>,
    last_focused: Option<WindowId>,
    recovery: Recovery,
    pending_created: Vec<WindowId>,
    pending_deleted: Vec<WindowId>,
    /// The last monitor enumeration, exactly as the OS reported it.
    monitors: Vec<MonitorInfo>,
    /// Detection is suppressed while the display settles after a monitor change.
    monitor_settling: bool,
    keymap_publisher: KeymapPublisher,
    env: HashMap<String, String>,
}

impl Dome {
    pub(in crate::platform::macos) fn new(
        monitors: &[MonitorInfo],
        tiling: TilingConfig,
        workspace_overrides: PreferredLayouts,
        sender: Box<dyn SceneSender>,
        runtime: LuaRuntime,
        keymap_publisher: KeymapPublisher,
        env: HashMap<String, String>,
    ) -> Self {
        let primary = monitors
            .iter()
            .find(|s| s.is_primary)
            .unwrap_or(&monitors[0]);
        let mut hub = Hub::new(primary.into(), tiling, workspace_overrides.clone(), runtime);
        let primary_monitor_id = hub.primary_monitor();
        let mut monitor_registry = MonitorRegistry::new(primary, primary_monitor_id);
        for monitor in monitors {
            if monitor.display_id != primary.display_id {
                let id = hub.add_monitor(monitor.into());
                monitor_registry.insert(monitor, id);
            }
        }
        Self {
            hub,
            registry: WindowRegistry::new(),
            monitor_registry,
            primary_full_height: primary.full_height,
            observed_pids: HashSet::new(),
            sender,
            last_focused: None,
            recovery: Recovery::new(),
            pending_created: Vec::new(),
            pending_deleted: Vec::new(),
            displayed_windows: HashSet::new(),
            monitors: monitors.to_vec(),
            monitor_settling: false,
            keymap_publisher,
            env,
        }
    }

    pub(in crate::platform::macos) fn refresh_ext_cache(&mut self, refresh: &[ExtRefresh]) {
        for r in refresh {
            if self
                .registry
                .replace_ext(r.cg_id, r.ext.clone(), r.corner_radius)
            {
                tracing::trace!(cg_id = %r.cg_id, pid = r.ext.pid(), "Replaced stale ext handle");
            }
        }
    }

    #[tracing::instrument(skip_all)]
    pub(in crate::platform::macos) fn reconcile_windows(
        &mut self,
        refresh: &[ExtRefresh],
        removed: &[CGWindowID],
        minimized: &[CGWindowID],
        added: Vec<PendingAdd>,
        to_enter_native_fullscreen: &[CGWindowID],
        to_exit_native_fullscreen: &[ExitNativeFullscreen],
    ) {
        self.refresh_ext_cache(refresh);
        for &cg_id in removed {
            if let Some(entry) = self.registry.get(cg_id) {
                self.remove_window(entry.window_id);
            }
        }
        for &cg_id in minimized {
            if let Some(entry) = self.registry.get(cg_id) {
                self.minimize_window(entry.window_id);
            }
        }
        for pending in added {
            let new_ref = match &pending {
                PendingAdd::Positioned { new, .. } | PendingAdd::NativeFullscreen { new } => new,
            };
            if self.registry.contains(new_ref.ax.cg_id()) {
                continue;
            }

            match pending {
                PendingAdd::NativeFullscreen { new } => {
                    self.add_native_fullscreen_window(new);
                }
                PendingAdd::Positioned {
                    new,
                    rect,
                    corner_radius,
                } => {
                    let ax_for_recovery = new.ax.clone();
                    let borderless_fs = self.is_borderless_fullscreen_at(rect);
                    let restrictions = if borderless_fs {
                        WindowRestrictions::ProtectFullscreen
                    } else {
                        WindowRestrictions::None
                    };
                    let Some(id) =
                        self.hub
                            .insert_window(Box::new(new.metadata.clone()), rect, restrictions)
                    else {
                        let cg_id = new.ax.cg_id();
                        let pid = new.ax.pid();
                        crate::trace_once!(
                            key: (cg_id, pid),
                            %cg_id, %pid, %new, "Window ignored"
                        );
                        continue;
                    };
                    tracing::info!(%id, %new, "New window");
                    let state = if borderless_fs {
                        WindowState::BorderlessFullscreen
                    } else {
                        WindowState::Positioned(window::PositionedState::Offscreen(
                            window::OffscreenPlacement::new(rect),
                        ))
                    };
                    self.registry.insert(new, id, state, corner_radius);
                    self.pending_created.push(id);
                    self.recovery.track(
                        ax_for_recovery,
                        rect.width(),
                        rect.height(),
                        self.monitor_registry.primary_monitor().work_area,
                    );
                }
            }
        }
        for &cg_id in to_enter_native_fullscreen {
            if let Some(entry) = self.registry.get(cg_id) {
                let window_id = entry.window_id;
                self.window_entered_native_fullscreen(window_id);
            }
        }
        for e in to_exit_native_fullscreen {
            if let Some(entry) = self.registry.get_mut(e.cg_id)
                && matches!(entry.state, WindowState::NativeFullscreen)
            {
                entry.corner_radius = e.corner_radius;
                let window_id = entry.window_id;
                let now = Instant::now();
                // NativeFullscreen doesn't emit any move/resize event, so we need to simulate one
                self.window_moved(
                    window_id,
                    e.rect,
                    DebounceBurst {
                        first: now,
                        last: now,
                    },
                );
            }
        }
        self.flush_layout();
    }

    #[tracing::instrument(skip_all)]
    pub(in crate::platform::macos) fn windows_moved(&mut self, moves: Vec<WindowMove>) {
        for m in moves {
            let Some(entry) = self.registry.get(m.cg_id) else {
                continue;
            };
            let window_id = entry.window_id;
            self.window_moved(window_id, m.rect, m.observed_at);
        }
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn app_terminated(&mut self, pid: i32) {
        self.remove_app_windows(pid);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn apply_layout_file(&mut self, path: &str) {
        let layouts = match PreferredLayouts::load(path) {
            Ok(layouts) => layouts,
            Err(e) => {
                tracing::error!("Apply layout failed: {e:#}");
                return;
            }
        };
        self.hub.apply_preferred_layouts(layouts);
        tracing::info!(%path, "Layout applied");
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn tracked_window(
        &self,
        cg_id: CGWindowID,
    ) -> Option<ManagedWindow> {
        self.registry.get(cg_id).cloned()
    }

    #[tracing::instrument(skip(self), fields(cg_id = %cg_id))]
    pub(in crate::platform::macos) fn focus_window_by_cg(&mut self, cg_id: CGWindowID) {
        let Some(entry) = self.registry.get(cg_id) else {
            return;
        };
        let window_id = entry.window_id;
        // A minimized window holds no workspace, so the hub cannot focus it
        // until the deminiaturize notification reattaches it.
        if entry.is_minimized {
            self.registry.unminimize_window(window_id);
            return;
        }
        self.hub.set_focus(window_id);
        self.flush_layout();
    }

    #[tracing::instrument(skip(self, title), fields(cg_id = %cg_id))]
    pub(in crate::platform::macos) fn update_title(
        &mut self,
        cg_id: CGWindowID,
        title: Option<String>,
    ) {
        if let Some(entry) = self.registry.get_mut(cg_id)
            && let Some(title) = title
        {
            if self.hub.set_window_title(entry.window_id, title.clone()) {
                tracing::trace!(title = %title, "Title changed");
            }
            self.flush_layout();
        }
    }

    /// Takes effect at the next `flush_layout`.
    pub(in crate::platform::macos) fn update_corner_radius(
        &mut self,
        cg_id: CGWindowID,
        corner_radius: Length<Logical>,
    ) {
        if let Some(entry) = self.registry.get_mut(cg_id) {
            entry.corner_radius = corner_radius;
        }
    }

    pub(in crate::platform::macos) fn monitors_changed(&mut self, monitors: Vec<MonitorInfo>) {
        self.rehide_offscreen_windows(&monitors);
        self.monitors = monitors;
        self.reconcile_monitors();
        self.monitor_settling = true;
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn finish_monitor_settle(&mut self) {
        self.monitor_settling = false;
    }

    #[tracing::instrument(skip(self), fields(cg_id = %cg_id))]
    pub(in crate::platform::macos) fn mirror_clicked(&mut self, cg_id: CGWindowID) {
        let Some(entry) = self.registry.get(cg_id) else {
            return;
        };
        let window_id = entry.window_id;
        let ext = entry.ext.clone();
        if entry.is_minimized {
            self.registry.unminimize_window(window_id);
            return;
        }
        if let Err(e) = ext.focus() {
            tracing::debug!("Failed to focus window: {e:#}");
        }
        self.hub.set_focus(window_id);
        self.flush_layout();
    }

    #[tracing::instrument(skip(self), fields(container_id = %container_id, tab_idx))]
    pub(in crate::platform::macos) fn tab_clicked(
        &mut self,
        container_id: ContainerId,
        tab_idx: usize,
    ) {
        self.hub.focus_tab_index(container_id, tab_idx);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn save_layout_file(&mut self, path: &std::path::Path) {
        if let Err(e) = self.hub.save_layout(path) {
            tracing::error!("Save layout failed: {e:#}");
        }
    }

    /// Handles the frontmost window entering native fullscreen after a space
    /// change.
    #[tracing::instrument(skip(self, new), fields(cg_id = %cg_id))]
    pub(in crate::platform::macos) fn enter_native_fullscreen(
        &mut self,
        cg_id: CGWindowID,
        new: NewWindow,
    ) {
        if let Some(entry) = self.registry.get(cg_id) {
            let window_id = entry.window_id;
            self.window_entered_native_fullscreen(window_id);
        } else {
            self.add_native_fullscreen_window(new);
        }
        self.flush_layout();
    }

    /// Handles the frontmost window exiting native fullscreen after a space
    /// change. Routes through `window_moved` so the window re-enters tiling via
    /// the same path as reconcile-detected exits.
    #[tracing::instrument(skip(self, pos, size), fields(cg_id = %cg_id))]
    pub(in crate::platform::macos) fn exit_native_fullscreen(
        &mut self,
        cg_id: CGWindowID,
        pos: (Length<Logical>, Length<Logical>),
        size: (Length<Logical>, Length<Logical>),
    ) {
        if let Some(entry) = self.registry.get(cg_id)
            && matches!(entry.state, WindowState::NativeFullscreen)
        {
            let window_id = entry.window_id;
            let now = Instant::now();
            self.window_moved(
                window_id,
                PixelRect::from_dimension(Dimension::new(pos.0, pos.1, size.0, size.1)),
                DebounceBurst {
                    first: now,
                    last: now,
                },
            );
            self.flush_layout();
        }
    }

    pub(in crate::platform::macos) fn tracked_for_pid(
        &self,
        pid: i32,
    ) -> HashMap<CGWindowID, ManagedWindow> {
        self.registry
            .for_pid(pid)
            .map(|(id, e)| (id, e.clone()))
            .collect()
    }

    pub(in crate::platform::macos) fn all_tracked(&self) -> HashMap<CGWindowID, ManagedWindow> {
        self.registry
            .iter()
            .map(|(id, e)| (id, e.clone()))
            .collect()
    }

    pub(in crate::platform::macos) fn observed_pids(&self) -> HashSet<i32> {
        self.observed_pids.clone()
    }

    pub(in crate::platform::macos) fn set_pid_moving(&mut self, pid: i32, moving: bool) {
        self.registry.set_pid_moving(pid, moving);
    }

    pub(in crate::platform::macos) fn mark_pid_observed(&mut self, pid: i32) {
        self.observed_pids.insert(pid);
    }

    pub(in crate::platform::macos) fn set_observed_pids(&mut self, pids: HashSet<i32>) {
        self.observed_pids = pids;
    }

    pub(in crate::platform::macos) fn remove_untracked_app(&mut self, pid: i32) {
        self.remove_app_windows(pid);
    }

    pub(in crate::platform::macos) fn refresh_observers(&self) {
        self.sender.send(HubMessage::RefreshObservers);
    }

    fn remove_window(&mut self, wid: WindowId) {
        self.hub.delete_window(wid);
        self.pending_deleted.push(wid);
    }

    fn remove_app_windows(&mut self, pid: i32) {
        let window_ids: Vec<WindowId> = self
            .registry
            .for_pid(pid)
            .map(|(_, entry)| entry.window_id)
            .collect();
        for wid in window_ids {
            self.remove_window(wid);
        }
    }

    pub(in crate::platform::macos) fn query_workspaces_json(&self) -> String {
        serde_json::to_string(&self.hub.query_workspaces())
            .expect("WorkspaceInfo is infallibly serializable")
    }

    pub(in crate::platform::macos) fn query_monitors_json(&self) -> String {
        serde_json::to_string(&self.hub.query_monitors())
            .expect("MonitorDetails is infallibly serializable")
    }

    pub(in crate::platform::macos) fn query_minimized_windows_json(&self) -> String {
        let entries: Vec<MinimizedWindow> = self
            .hub
            .minimized_window_entries()
            .into_iter()
            .map(|e| MinimizedWindow {
                id: e.id,
                title: e.title,
                app_name: e.app_name,
                bundle_id: e.bundle_id,
                executable_path: e.executable_path,
            })
            .collect();
        serde_json::to_string(&entries).expect("MinimizedWindow is infallibly serializable")
    }

    pub(in crate::platform::macos) fn run_binding(
        &mut self,
        keymap: &str,
        keystroke: &Keystroke,
        signal: &LoopSignal,
    ) {
        let mut effects = MacPlatformEffects {
            registry: &mut self.registry,
            signal,
            env: &self.env,
        };
        self.hub
            .run_binding(keymap, keystroke, &mut effects, &mut self.keymap_publisher);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn handle_actions(
        &mut self,
        actions: &Actions,
        signal: &LoopSignal,
    ) {
        let mut effects = MacPlatformEffects {
            registry: &mut self.registry,
            signal,
            env: &self.env,
        };
        self.hub
            .handle_actions(actions, &mut effects, &mut self.keymap_publisher);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn reload(&mut self) -> Option<Box<Config>> {
        let config = self.hub.reload_config(&mut self.keymap_publisher)?;
        self.env = config.env.clone();
        self.sender
            .send(HubMessage::AppearanceChanged(config.appearance.clone()));
        tracing::info!("Config reloaded");
        self.flush_layout();
        Some(config)
    }
}

impl Drop for Dome {
    fn drop(&mut self) {
        self.recovery.restore_all();
        self.sender.send(HubMessage::Shutdown);
    }
}
