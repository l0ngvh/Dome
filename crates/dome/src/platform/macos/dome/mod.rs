use super::menu_bar::MenuBar;
use crate::platform::macos::event_loop::RunLoop;

pub(super) mod events;
pub(super) mod ipc;
mod layout;
mod monitor;
mod recovery;
mod registry;
mod sources;
#[cfg(test)]
mod tests;
mod window;

use super::ui::Overlays;
use super::window_server::WindowServer;
pub(super) use events::HubEvent;
pub(super) use monitor::MonitorInfo;
use window::WindowState;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use crate::action::{Action, MinimizedWindow, WorkspaceInfo};
use crate::config::{Config, Keystroke, LuaRuntime, PlatformEffects, PreferredLayouts};
use crate::platform::keymap::KeymapPublisher;
use dome_core::{
    ContainerId, Dimension, FractionalLength, Hub, Logical, PixelRect, WindowId, WindowMetadata,
    WindowRestrictions,
};
use dome_core::{TilingConfig, WindowMatcher, pattern_matches};

use monitor::MonitorRegistry;
use recovery::Recovery;
pub(in crate::platform::macos) use registry::ManagedWindow;
pub(in crate::platform::macos) use registry::WindowRegistry;

pub(in crate::platform::macos) struct WindowMove {
    pub(in crate::platform::macos) window_id: WindowId,
    pub(in crate::platform::macos) rect: PixelRect,
    pub(in crate::platform::macos) observed_at: DebounceBurst,
}

/// Native changes applied as one reconciliation batch, before the layout is flushed.
pub(in crate::platform::macos) enum WindowChange {
    Refreshed {
        window_id: WindowId,
        corner_radius: FractionalLength<Logical>,
    },
    Removed(WindowId),
    Minimized(WindowId),
    Added {
        window_id: WindowId,
        pid: i32,
        metadata: MacOSMetadata,
        rect: PixelRect,
        corner_radius: FractionalLength<Logical>,
    },
    AddedNativeFullscreen {
        window_id: WindowId,
        pid: i32,
        metadata: MacOSMetadata,
    },
    EnteredNativeFullscreen(WindowId),
    ExitedNativeFullscreen {
        window_id: WindowId,
        rect: PixelRect,
        corner_radius: FractionalLength<Logical>,
    },
}

pub(in crate::platform::macos) struct Dome {
    hub: Hub,
    registry: WindowRegistry,
    monitor_registry: MonitorRegistry,
    displayed_windows: HashSet<WindowId>,
    /// Full height of the primary display (including menu bar/dock), used for Quartz→Cocoa
    /// coordinate conversion in overlay rendering.
    primary_full_height: f64,
    pub(super) appearance: crate::config::Appearance,
    overlays: Box<dyn Overlays>,
    menu_bar: Rc<dyn MenuBar>,
    pub(in crate::platform::macos) window_server: Rc<dyn WindowServer>,
    logger: Box<dyn crate::logging::Logger>,
    bundle_path: Option<String>,
    last_focused_monitor: Option<dome_core::MonitorId>,
    last_focused: Option<WindowId>,
    recovery: Recovery,
    /// The last monitor enumeration, exactly as the OS reported it.
    monitors: Vec<MonitorInfo>,
    /// Suppresses constraint and drift detection while a monitor change settles.
    monitor_settling: bool,
    keymap_publisher: KeymapPublisher,
    env: HashMap<String, String>,
}

impl Dome {
    pub(in crate::platform::macos) fn update_menu_bar(&self) {
        self.menu_bar.update_workspaces(&self.query_workspaces());
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "all startup dependencies must exist before installation"
    )]
    pub(in crate::platform::macos) fn new(
        monitors: &[MonitorInfo],
        tiling: TilingConfig,
        workspace_overrides: PreferredLayouts,
        runtime: LuaRuntime,
        keymap_publisher: KeymapPublisher,
        env: HashMap<String, String>,
        window_server: Rc<dyn WindowServer>,
        overlays: Box<dyn Overlays>,
        menu_bar: Rc<dyn MenuBar>,
        appearance: crate::config::Appearance,
        logger: Box<dyn crate::logging::Logger>,
        bundle_path: Option<String>,
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
            appearance,
            overlays,
            menu_bar,
            window_server,
            logger,
            bundle_path,
            last_focused_monitor: None,
            last_focused: None,
            recovery: Recovery::new(),
            displayed_windows: HashSet::new(),
            monitors: monitors.to_vec(),
            monitor_settling: false,
            keymap_publisher,
            env,
        }
    }

    pub(super) fn query_workspaces(&self) -> Vec<WorkspaceInfo> {
        self.hub.query_workspaces()
    }

    pub(in crate::platform::macos) fn finish_monitor_settle(&mut self) {
        self.monitor_settling = false;
    }

    pub(in crate::platform::macos) fn app_terminated(&mut self, pid: i32) {
        self.remove_app_windows(pid);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn query_monitors_value(&self) -> serde_json::Value {
        serde_json::to_value(self.hub.query_monitors())
            .expect("MonitorDetails is infallibly serializable")
    }

    pub(in crate::platform::macos) fn query_workspaces_value(&self) -> serde_json::Value {
        serde_json::to_value(self.hub.query_workspaces())
            .expect("WorkspaceInfo is infallibly serializable")
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

    pub(in crate::platform::macos) fn monitors_changed(&mut self, monitors: Vec<MonitorInfo>) {
        self.rehide_offscreen_windows(&monitors);
        self.monitors = monitors;
        self.reconcile_monitors();
        self.monitor_settling = true;
        self.flush_layout();
    }

    /// Takes effect at the next layout application.
    pub(in crate::platform::macos) fn update_corner_radius(
        &mut self,
        window_id: WindowId,
        corner_radius: FractionalLength<Logical>,
    ) {
        if let Some(entry) = self.registry.get_mut(window_id) {
            entry.corner_radius = corner_radius;
        }
    }

    #[tracing::instrument(skip_all)]
    pub(in crate::platform::macos) fn windows_moved(&mut self, moves: Vec<WindowMove>) {
        for m in moves {
            if self.registry.contains(m.window_id) {
                self.window_moved(m.window_id, m.rect, m.observed_at);
            }
        }
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn run_binding(&mut self, keymap: &str, keystroke: &Keystroke) {
        let mut effects = MacPlatformEffects {
            registry: &mut self.registry,
            server: &*self.window_server,
            env: &self.env,
            error: None,
        };
        self.hub
            .run_binding(keymap, keystroke, &mut effects, &mut self.keymap_publisher);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn config_changed(&mut self, config: Box<Config>) {
        self.hub.config_changed(&config);
        self.env = config.env.clone();
        self.apply_appearance(&config.appearance);
        tracing::info!("Config reloaded");
        self.flush_layout();
        self.logger.set_level(config.log_level);
    }

    #[tracing::instrument(skip(self, title), fields(window_id = %window_id))]
    pub(in crate::platform::macos) fn update_title(
        &mut self,
        window_id: WindowId,
        title: Option<String>,
    ) {
        if let Some(entry) = self.registry.get_mut(window_id)
            && let Some(title) = title
        {
            if self.hub.set_window_title(entry.window_id, title.clone()) {
                tracing::trace!(title = %title, "Title changed");
            }
            self.flush_layout();
        }
    }

    #[tracing::instrument(skip(self, pid, metadata), fields(window_id = %window_id))]
    pub(in crate::platform::macos) fn enter_native_fullscreen(
        &mut self,
        window_id: WindowId,
        pid: i32,
        metadata: MacOSMetadata,
    ) {
        let change = if self.registry.contains(window_id) {
            WindowChange::EnteredNativeFullscreen(window_id)
        } else {
            WindowChange::AddedNativeFullscreen {
                window_id,
                pid,
                metadata,
            }
        };
        self.reconcile_windows([change]);
    }

    pub(in crate::platform::macos) fn handle_action(
        &mut self,
        action: &Action,
    ) -> anyhow::Result<()> {
        let mut effects = MacPlatformEffects {
            registry: &mut self.registry,
            server: &*self.window_server,
            env: &self.env,
            error: None,
        };
        self.hub
            .handle_action(action, &mut effects, &mut self.keymap_publisher);
        let error = effects.error;
        self.flush_layout();
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(in crate::platform::macos) fn query_minimized_windows_value(&self) -> serde_json::Value {
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
        serde_json::to_value(entries).expect("MinimizedWindow is infallibly serializable")
    }

    #[tracing::instrument(skip(self), fields(window_id = %window_id))]
    pub(in crate::platform::macos) fn mirror_clicked(&mut self, window_id: WindowId) {
        let Some(entry) = self.registry.get(window_id) else {
            return;
        };
        let window_id = entry.window_id;
        if entry.is_minimized {
            if let Err(error) = self
                .registry
                .unminimize_window(window_id, &*self.window_server)
            {
                tracing::warn!(%error, "Unminimize failed");
            }
            return;
        }
        if let Err(e) = self.window_server.focus(window_id) {
            tracing::debug!("Failed to focus window: {e:#}");
        }
        self.hub.set_focus(window_id);
        self.flush_layout();
    }

    #[tracing::instrument(skip(self), fields(window_id = %window_id))]
    pub(in crate::platform::macos) fn focus_window(&mut self, window_id: WindowId) {
        let Some(entry) = self.registry.get(window_id) else {
            return;
        };
        let window_id = entry.window_id;
        // A minimized window holds no workspace, so the hub cannot focus it
        // until the deminiaturize notification reattaches it.
        if entry.is_minimized {
            if let Err(error) = self
                .registry
                .unminimize_window(window_id, &*self.window_server)
            {
                tracing::warn!(%error, "Unminimize failed");
            }
            return;
        }
        self.hub.set_focus(window_id);
        self.flush_layout();
    }

    /// Handles the frontmost window exiting native fullscreen after a space change.
    #[tracing::instrument(skip(self, pos, size), fields(window_id = %window_id))]
    pub(in crate::platform::macos) fn exit_native_fullscreen(
        &mut self,
        window_id: WindowId,
        pos: (
            dome_core::FractionalCoordinate<Logical>,
            dome_core::FractionalCoordinate<Logical>,
        ),
        size: (FractionalLength<Logical>, FractionalLength<Logical>),
    ) {
        if let Some(entry) = self.registry.get(window_id)
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

    #[tracing::instrument(skip_all)]
    fn reconcile_windows(&mut self, changes: impl IntoIterator<Item = WindowChange>) {
        let mut created = Vec::new();
        for change in changes {
            match change {
                WindowChange::Refreshed {
                    window_id,
                    corner_radius,
                } => {
                    if self.registry.contains(window_id) {
                        self.update_corner_radius(window_id, corner_radius);
                    }
                }
                WindowChange::Removed(window_id) => self.remove_window(window_id),
                WindowChange::Minimized(window_id) => {
                    if self.registry.contains(window_id) {
                        self.minimize_window(window_id);
                    }
                }
                WindowChange::AddedNativeFullscreen {
                    window_id,
                    pid,
                    metadata,
                } => {
                    if !self.registry.contains(window_id)
                        && let Some(id) =
                            self.add_native_fullscreen_window(window_id, pid, metadata)
                    {
                        created.push(id);
                    }
                }
                WindowChange::Added {
                    window_id,
                    pid,
                    metadata,
                    rect,
                    corner_radius,
                } => {
                    if pid as u32 == std::process::id() {
                        continue;
                    }
                    if self.registry.contains(window_id) {
                        continue;
                    }
                    let id_for_recovery = window_id;
                    let borderless_fs = self.is_borderless_fullscreen_at(rect);
                    let restrictions = if borderless_fs {
                        WindowRestrictions::ProtectFullscreen
                    } else {
                        WindowRestrictions::None
                    };
                    let Some(id) = self.hub.insert_window(
                        window_id,
                        Box::new(metadata.clone()),
                        rect,
                        restrictions,
                    ) else {
                        crate::trace_once!(
                            key: (window_id, pid),
                            %window_id, %pid,
                            new = %format_args!("[pid={pid}|id={window_id}] {metadata}"),
                            "Window ignored"
                        );
                        continue;
                    };
                    tracing::info!(
                        %id,
                        new = %format_args!("[pid={pid}|id={window_id}] {metadata}"),
                        "New window"
                    );
                    let state = if borderless_fs {
                        WindowState::BorderlessFullscreen
                    } else {
                        WindowState::Positioned(window::PositionedState::Offscreen(
                            window::OffscreenPlacement::new(rect),
                        ))
                    };
                    self.registry.insert(window_id, pid, state, corner_radius);
                    created.push(id);
                    self.recovery.track(
                        id_for_recovery,
                        rect.width(),
                        rect.height(),
                        self.monitor_registry.primary_monitor().work_area,
                    );
                }
                WindowChange::EnteredNativeFullscreen(window_id) => {
                    if self.registry.contains(window_id) {
                        self.window_entered_native_fullscreen(window_id);
                    }
                }
                WindowChange::ExitedNativeFullscreen {
                    window_id,
                    rect,
                    corner_radius,
                } => {
                    if let Some(entry) = self.registry.get_mut(window_id)
                        && matches!(entry.state, WindowState::NativeFullscreen)
                    {
                        entry.corner_radius = corner_radius;
                        let window_id = entry.window_id;
                        let now = Instant::now();
                        // Native fullscreen does not emit move notifications.
                        self.window_moved(
                            window_id,
                            rect,
                            DebounceBurst {
                                first: now,
                                last: now,
                            },
                        );
                    }
                }
            }
        }
        let placements = self.hub.get_visible_placements();
        for id in created {
            if self.registry.contains(id) && !placements.contains_window(id) {
                self.hide_window(id);
            }
        }
        self.apply_visible_placements(&placements);
    }

    fn tracked_for_pid(&self, pid: i32) -> HashMap<WindowId, ManagedWindow> {
        self.registry
            .for_pid(pid)
            .map(|(id, e)| (id, e.clone()))
            .collect()
    }

    fn all_tracked(&self) -> HashMap<WindowId, ManagedWindow> {
        self.registry
            .iter()
            .map(|(id, e)| (id, e.clone()))
            .collect()
    }

    fn set_pid_moving(&mut self, pid: i32, moving: bool) {
        self.registry.set_pid_moving(pid, moving);
    }

    fn remove_untracked_app(&mut self, pid: i32) {
        self.remove_app_windows(pid);
    }

    fn remove_window(&mut self, window_id: WindowId) {
        if !self.registry.contains(window_id) {
            return;
        }
        if self.displayed_windows.contains(&window_id) {
            self.hide_window(window_id);
        }
        self.displayed_windows.remove(&window_id);
        self.hub.delete_window(window_id);
        self.recovery.untrack(window_id);
        self.registry.remove(window_id);
    }

    fn remove_app_windows(&mut self, pid: i32) {
        let window_ids: Vec<WindowId> = self
            .registry
            .for_pid(pid)
            .map(|(_, entry)| entry.window_id)
            .collect();
        for window_id in window_ids {
            self.remove_window(window_id);
        }
    }
}

struct MacPlatformEffects<'a> {
    registry: &'a mut WindowRegistry,
    server: &'a dyn WindowServer,
    env: &'a HashMap<String, String>,
    error: Option<anyhow::Error>,
}

impl PlatformEffects for MacPlatformEffects<'_> {
    fn close(&mut self, id: WindowId) {
        if let Err(error) = self.registry.close_window(id, self.server) {
            tracing::warn!(%error, "Close failed");
            self.error.get_or_insert(error);
        }
    }

    fn execute(&mut self, command: &str) {
        if let Err(e) = crate::platform::macos::spawn::spawn_disclaimed_sh(command, self.env) {
            tracing::warn!(%command, "Failed to execute: {e}");
        }
    }

    fn exit(&mut self) {
        RunLoop::terminate();
    }

    fn unminimize(&mut self, id: WindowId) {
        if let Err(error) = self.registry.unminimize_window(id, self.server) {
            tracing::warn!(%error, "Unminimize failed");
            self.error.get_or_insert(error);
        }
    }
}

pub(in crate::platform::macos) use super::window_server::{
    DebounceBurst, WindowMetadata as MacOSMetadata,
};

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

impl Drop for Dome {
    fn drop(&mut self) {
        self.recovery.restore_all(&*self.window_server);
    }
}
