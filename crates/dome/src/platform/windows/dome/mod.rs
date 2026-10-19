use super::tray::Tray;
use crate::platform::windows::event_loop::RunLoop;
use crate::platform::windows::external::ShowCmd;

pub(super) mod events;
pub(super) mod ipc;
mod layout;
pub(super) mod monitor;
mod registry;
mod sources;
pub(super) mod window;

use super::display::MonitorInfo;
use super::window_server::WindowObservation;
pub(super) use events::HubEvent;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use crate::action::Action;
use crate::action::{MinimizedWindow, WorkspaceInfo};
use crate::config::{Appearance, Config, Keystroke, LuaRuntime, PlatformEffects, PreferredLayouts};
use crate::platform::keymap::KeymapPublisher;
use crate::platform::windows::window_server::{WindowOrder, WindowServer};
use dome_core::TilingConfig;
use dome_core::{
    ContainerId, Coordinate, Hub, LimitObservation, MonitorId, Physical, PixelRect, WindowId,
    WindowRestrictions,
};

use self::monitor::MonitorRegistry;
use self::registry::ManagedWindow;
use self::registry::WindowRegistry;
use self::window::{PositionedState, WindowState};
use super::ui::Overlays;

pub(crate) struct Dome {
    hub: Hub,
    registry: WindowRegistry,
    monitors: MonitorRegistry,
    displayed_windows: HashSet<WindowId>,
    appearance: Appearance,
    overlays: Box<dyn Overlays>,
    tray: Rc<dyn Tray>,
    pub(super) window_server: Rc<dyn WindowServer>,
    logger: Box<dyn crate::logging::Logger>,
    last_focused: Option<WindowId>,
    last_focused_monitor: Option<MonitorId>,
    moving_windows: HashSet<WindowId>,
    keymap_publisher: KeymapPublisher,
    env: HashMap<String, String>,
}

impl Drop for Dome {
    fn drop(&mut self) {
        for window_id in self.registry.iter() {
            let entry = self.registry.get(window_id).expect("registered window");
            if let WindowState::Positioned(state) = entry.state {
                let actual = match state {
                    PositionedState::Tiling(drift) => drift.actual,
                    PositionedState::Float(placement) => placement.actual,
                    PositionedState::Offscreen { actual, .. } => actual,
                };
                self.window_server.set_position(
                    window_id,
                    WindowOrder::Unchanged,
                    PixelRect::from_pixels(
                        Coordinate::new(100),
                        Coordinate::new(100),
                        actual.width(),
                        actual.height(),
                    ),
                );
            }
            if matches!(entry.state, WindowState::BorderlessMinimized { .. })
                && !entry.is_minimized
                && let Err(error) = self.window_server.show_cmd(window_id, ShowCmd::Restore)
            {
                tracing::warn!(%error, "Window restore on shutdown failed");
            }
            self.window_server.add_tab(window_id);
        }
    }
}

impl Dome {
    pub(in crate::platform::windows) fn update_tray(&self) {
        self.tray.update_workspaces(&self.query_workspaces());
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "constructor wiring the platform collaborators, the runtime, and the keymap publisher"
    )]
    pub(super) fn new(
        monitors: Vec<MonitorInfo>,
        tiling: TilingConfig,
        workspace_overrides: PreferredLayouts,
        window_server: Rc<dyn WindowServer>,
        appearance: Appearance,
        runtime: LuaRuntime,
        keymap_publisher: KeymapPublisher,
        env: HashMap<String, String>,
        overlays: Box<dyn Overlays>,
        tray: Rc<dyn Tray>,
        logger: Box<dyn crate::logging::Logger>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(!monitors.is_empty(), "No monitors detected");
        let primary = monitors
            .iter()
            .find(|s| s.is_primary)
            .unwrap_or(&monitors[0]);
        let mut hub = Hub::new(primary.into(), tiling, workspace_overrides.clone(), runtime);
        let primary_monitor_id = hub.primary_monitor();
        let mut monitors_reg = MonitorRegistry::new();
        monitors_reg.insert(primary_monitor_id, primary);
        tracing::info!(
            name = %primary.name,
            handle = ?primary.handle,
            work_area = ?primary.work_area,
            "Primary monitor"
        );

        for monitor in &monitors {
            if monitor.handle != primary.handle {
                let id = hub.add_monitor(monitor.into());
                monitors_reg.insert(id, monitor);
                tracing::info!(
                    name = %monitor.name,
                    handle = ?monitor.handle,
                    work_area = ?monitor.work_area,
                    "Monitor"
                );
            }
        }

        Ok(Self {
            hub,
            registry: WindowRegistry::new(),
            monitors: monitors_reg,
            appearance,
            overlays,
            tray,
            window_server,
            logger,
            last_focused: None,
            last_focused_monitor: None,
            moving_windows: HashSet::new(),
            displayed_windows: HashSet::new(),
            keymap_publisher,
            env,
        })
    }

    pub(super) fn is_managed(&self, id: WindowId) -> bool {
        self.registry.contains(id)
    }

    pub(super) fn query_workspaces(&self) -> Vec<WorkspaceInfo> {
        self.hub.query_workspaces()
    }

    pub(crate) fn query_monitors_value(&self) -> serde_json::Value {
        serde_json::to_value(self.hub.query_monitors())
            .expect("MonitorDetails is infallibly serializable")
    }

    pub(crate) fn query_workspaces_value(&self) -> serde_json::Value {
        serde_json::to_value(self.hub.query_workspaces())
            .expect("WorkspaceInfo is infallibly serializable")
    }

    pub(in crate::platform::windows) fn tab_clicked(
        &mut self,
        container_id: ContainerId,
        tab_idx: usize,
    ) {
        self.hub.focus_tab_index(container_id, tab_idx);
        self.apply_layout();
    }

    pub(super) fn monitors_changed(&mut self, monitors: Vec<MonitorInfo>) -> Vec<WindowId> {
        tracing::info!(count = monitors.len(), "Monitor parameters changed");
        self.update_monitors(monitors)
    }

    pub(crate) fn config_changed(&mut self, config: Box<Config>) {
        self.hub.config_changed(&config);
        self.env = config.env.clone();
        self.apply_appearance(&config.appearance);
        tracing::info!("Config reloaded");
        self.apply_layout();
        self.logger.set_level(config.log_level);
        super::login_item::sync_login_item(config.start_at_login);
    }

    pub(crate) fn update_titles(&mut self, titles: Vec<(WindowId, Option<String>)>) {
        for (window_id, title) in &titles {
            if let Some(title) = title
                && self.registry.contains(*window_id)
                && self.hub.set_window_title(*window_id, title.clone())
            {
                tracing::trace!(window_id = %window_id, title = %title, "Title changed");
            }
        }
        self.apply_layout();
    }

    pub(super) fn run_binding(&mut self, keymap: &str, keystroke: &Keystroke) {
        {
            let mut effects = WinPlatformEffects {
                registry: &mut self.registry,
                window_server: &*self.window_server,
                env: &self.env,
                error: None,
            };
            self.hub
                .run_binding(keymap, keystroke, &mut effects, &mut self.keymap_publisher);
        }
        self.apply_layout();
    }

    pub(super) fn apply_display_change(
        &mut self,
        monitors: Vec<MonitorInfo>,
        exclusive: Option<WindowId>,
    ) -> Vec<WindowId> {
        let to_refresh = self.monitors_changed(monitors);
        if let Some(id) = exclusive.filter(|window| self.registry.contains(*window)) {
            tracing::info!(%id, "D3D exclusive fullscreen entered");
            self.enter_fullscreen_exclusive(id);
        }
        self.apply_layout();
        to_refresh
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn handle_action(&mut self, action: &Action) -> anyhow::Result<()> {
        let mut effects = WinPlatformEffects {
            registry: &mut self.registry,
            window_server: &*self.window_server,
            env: &self.env,
            error: None,
        };
        self.hub
            .handle_action(action, &mut effects, &mut self.keymap_publisher);
        let error = effects.error;
        self.apply_layout();
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(crate) fn query_minimized_windows_value(&self) -> serde_json::Value {
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

    pub(in crate::platform::windows) fn thumbnail_clicked(&mut self, id: WindowId) {
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        // A click can arrive for a window that was minimized after its thumbnail showed, so
        // such a window is restored instead of focused.
        if entry.is_minimized {
            if let Err(error) = self.registry.unminimize_window(id, &*self.window_server) {
                tracing::warn!(%error, "Unminimize failed");
            }
            return;
        }
        self.hub.set_focus(id);
        self.apply_layout();
    }

    #[tracing::instrument(
        skip(self, id),
        fields(window_id = %id),
    )]
    fn window_destroyed(&mut self, id: WindowId) {
        self.moving_windows.remove(&id);
        self.window_server.delete_tab(id);
        if self.registry.remove(id).is_some() {
            tracing::info!(%id, "Window removed");
            self.displayed_windows.remove(&id);
            self.hub.delete_window(id);
            self.apply_layout();
        }
    }

    #[tracing::instrument(
        skip(self, id),
        fields(window_id = %id),
    )]
    fn window_minimized(&mut self, id: WindowId) {
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        // Dome-initiated minimize
        if matches!(entry.state, WindowState::BorderlessMinimized { .. }) {
            return;
        }
        self.hub.minimize_window(id);
        if let Some(entry) = self.registry.get_mut(id) {
            entry.is_minimized = true;
        }
        self.apply_layout();
    }

    #[tracing::instrument(skip_all, fields(pid, window_id = %window_id, metadata = %metadata))]
    fn add_window(
        &mut self,
        WindowObservation {
            id: window_id,
            pid,
            metadata,
            constraints,
            ..
        }: WindowObservation,
        rect: PixelRect<Physical>,
        monitor: isize,
    ) {
        if self.registry.contains(window_id) {
            return;
        }
        let borderless_fs = self.monitors.is_borderless_fullscreen_at(rect, monitor);
        let restrictions = if borderless_fs {
            WindowRestrictions::ProtectFullscreen
        } else {
            WindowRestrictions::None
        };
        let Some(id) =
            self.hub
                .insert_window(window_id, Box::new(metadata.clone()), rect, restrictions)
        else {
            tracing::trace!(window_id = %window_id, pid = pid, "ignored by rule");
            return;
        };
        tracing::info!(%id, "New window");
        let state = if borderless_fs {
            WindowState::BorderlessFullscreen
        } else {
            WindowState::Positioned(PositionedState::Offscreen {
                retries: 0,
                actual: rect,
            })
        };
        self.hub.set_window_constraint(id, constraints);
        self.registry.insert(
            id,
            ManagedWindow {
                state,
                is_minimized: false,
                monitor,
            },
        );
        let placements = self.hub.get_visible_placements();
        if self.registry.contains(id) && !placements.contains_window(id) {
            self.hide_window(id);
        }
        self.apply_visible_placements(&placements);
    }

    fn set_constraints_for(&mut self, window_id: WindowId, constraints: LimitObservation) {
        let id = window_id;
        if !self.registry.contains(id) {
            return;
        }
        self.hub.set_window_constraint(id, constraints);
    }

    #[tracing::instrument(
        skip(self, id),
        fields(window_id = %id),
    )]
    fn handle_focus(&mut self, id: WindowId) {
        if !self.registry.contains(id) {
            return;
        }
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
        self.hub.set_focus(id);
        tracing::info!("Window focused");
        self.apply_layout();
    }

    /// Returns `true` when the window settled on a different monitor than
    /// before, the signal to re-read its size constraints.
    fn handle_window_moved(
        &mut self,
        id: WindowId,
        new_placement: PixelRect<Physical>,
        monitor_handle: isize,
        observed_at: Instant,
    ) -> bool {
        if !self.registry.contains(id) {
            return false;
        }
        let monitor_changed = self.registry.get_mut(id).is_some_and(|entry| {
            let changed = entry.monitor != monitor_handle;
            entry.monitor = monitor_handle;
            changed
        });
        self.window_moved(id, new_placement, monitor_handle, observed_at);
        monitor_changed
    }

    fn retry_drifted_windows(&mut self) {
        let window_ids: Vec<WindowId> = self.registry.iter().collect();
        for window_id in window_ids {
            self.retry_drift(window_id);
        }
    }

    fn update_monitors(&mut self, monitors: Vec<MonitorInfo>) -> Vec<WindowId> {
        if monitors.is_empty() {
            tracing::warn!("Empty monitor list, skipping update");
            return Vec::new();
        }
        self.monitors.reconcile(&mut self.hub, &monitors);

        self.registry
            .iter()
            .filter(|id| {
                self.registry
                    .get(*id)
                    .is_none_or(|e| !matches!(e.state, WindowState::ExclusiveFullscreen))
            })
            .collect()
    }
}

pub(super) fn display_from_process(process: &str) -> String {
    process.strip_suffix(".exe").unwrap_or(process).to_string()
}

struct WinPlatformEffects<'a> {
    registry: &'a mut WindowRegistry,
    window_server: &'a dyn WindowServer,
    env: &'a HashMap<String, String>,
    error: Option<anyhow::Error>,
}

impl PlatformEffects for WinPlatformEffects<'_> {
    fn close(&mut self, id: WindowId) {
        if let Err(error) = self.registry.close_window(id, self.window_server) {
            tracing::warn!(%error, "Close failed");
            self.error.get_or_insert(error);
        }
    }

    fn execute(&mut self, command: &str) {
        if let Err(e) = crate::platform::windows::spawn::spawn(command, self.env) {
            tracing::warn!(%command, "Failed to execute: {e:#}");
        }
    }

    fn exit(&mut self) {
        RunLoop::terminate();
    }

    fn unminimize(&mut self, id: WindowId) {
        if let Err(error) = self.registry.unminimize_window(id, self.window_server) {
            tracing::warn!(%error, "Unminimize failed");
            self.error.get_or_insert(error);
        }
    }
}

#[cfg(test)]
mod tests;
