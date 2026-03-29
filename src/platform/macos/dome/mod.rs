mod events;
mod inspect;
mod layout;
mod monitor;
mod recovery;
mod registry;
mod window;

pub(super) use events::{HubEvent, HubMessage};
pub(super) use inspect::{compute_reconcile_all, compute_reconciliation, compute_window_positions};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use objc2_app_kit::NSWorkspace;
use objc2_core_graphics::CGWindowID;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Config, MacosOnOpenRule, MacosWindow};
use crate::core::{ContainerId, Dimension, Hub, WindowId};
use crate::platform::macos::MonitorInfo;
use crate::platform::macos::accessibility::AXWindowApi;
use crate::platform::macos::running_application::RunningApp;

use monitor::MonitorRegistry;
use recovery::Recovery;
use registry::{Registry, WindowEntry};
use window::{PositionedState, RoundedDimension, WindowState, move_offscreen};

pub(in crate::platform::macos) struct NewWindow {
    pub(in crate::platform::macos) ax: Arc<dyn AXWindowApi>,
    pub(in crate::platform::macos) app_name: Option<String>,
    pub(in crate::platform::macos) bundle_id: Option<String>,
    pub(in crate::platform::macos) title: Option<String>,
    pub(in crate::platform::macos) x: i32,
    pub(in crate::platform::macos) y: i32,
    pub(in crate::platform::macos) w: i32,
    pub(in crate::platform::macos) h: i32,
    pub(in crate::platform::macos) is_native_fullscreen: bool,
}

pub(in crate::platform::macos) struct WindowMove {
    pub(in crate::platform::macos) window_id: WindowId,
    pub(in crate::platform::macos) x: i32,
    pub(in crate::platform::macos) y: i32,
    pub(in crate::platform::macos) w: i32,
    pub(in crate::platform::macos) h: i32,
    pub(in crate::platform::macos) observed_at: Instant,
    pub(in crate::platform::macos) is_native_fullscreen: bool,
}

pub(in crate::platform::macos) trait FrameSender: Send {
    fn send(&self, msg: HubMessage);
}

pub(in crate::platform::macos) struct Dome {
    hub: Hub,
    registry: Registry,
    monitor_registry: MonitorRegistry,
    config: Config,
    /// Work area of the primary monitor, used for crash recovery positioning.
    primary_screen: Dimension,
    /// Full height of the primary display (including menu bar/dock), used for Quartz→Cocoa
    /// coordinate conversion in overlay rendering.
    primary_full_height: f32,
    observed_pids: HashSet<i32>,
    sender: Box<dyn FrameSender>,
    stopped: bool,
    last_focused: Option<WindowId>,
    recovery: Recovery,
}

impl Dome {
    pub(in crate::platform::macos) fn new(
        screens: &[MonitorInfo],
        config: Config,
        sender: Box<dyn FrameSender>,
    ) -> Self {
        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_registry = MonitorRegistry::new(primary, primary_monitor_id);
        for screen in screens {
            if screen.display_id != primary.display_id {
                let id = hub.add_monitor(screen.name.clone(), screen.dimension);
                monitor_registry.insert(screen, id);
            }
        }
        Self {
            hub,
            registry: Registry::new(),
            monitor_registry,
            config,
            primary_screen: primary.dimension,
            primary_full_height: primary.full_height,
            observed_pids: HashSet::new(),
            sender,
            stopped: false,
            last_focused: None,
            recovery: Recovery::new(),
        }
    }

    pub(in crate::platform::macos) fn reconcile_windows(
        &mut self,
        removed: &[CGWindowID],
        added: Vec<NewWindow>,
    ) -> Vec<WindowId> {
        for &cg_id in removed {
            self.remove_window(cg_id);
        }
        let mut ids = Vec::with_capacity(added.len());
        for new in added {
            let NewWindow {
                ax,
                app_name,
                bundle_id,
                title,
                x,
                y,
                w,
                h,
                is_native_fullscreen,
            } = new;
            if self.registry.contains(ax.cg_id()) {
                continue;
            }
            let window_id = if is_native_fullscreen {
                self.add_native_fullscreen_window(
                    ax.clone(),
                    app_name.clone(),
                    bundle_id.clone(),
                    title.clone(),
                )
            } else {
                self.add_window(
                    ax.clone(),
                    x,
                    y,
                    w,
                    h,
                    app_name.clone(),
                    bundle_id.clone(),
                    title.clone(),
                )
            };
            self.recovery.track(ax, self.primary_screen);
            let actions = {
                let entry = self.registry.by_id(window_id);
                on_open_actions(entry, &self.config.macos.on_open)
            };
            if let Some(actions) = actions {
                self.execute_actions(&actions);
            }
            ids.push(window_id);
        }
        self.flush_layout();
        ids
    }

    pub(in crate::platform::macos) fn windows_moved(&mut self, moves: Vec<WindowMove>) {
        for m in moves {
            if m.is_native_fullscreen {
                self.window_entered_native_fullscreen(m.window_id);
            } else {
                self.window_moved(m.window_id, m.x, m.y, m.w, m.h, m.observed_at);
            }
        }
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn app_terminated(&mut self, pid: i32) {
        self.remove_app_windows(pid);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn run_actions(&mut self, actions: &Actions) {
        self.execute_actions(actions);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn window_id_for_cg(
        &self,
        cg_id: CGWindowID,
    ) -> Option<WindowId> {
        self.registry.get(cg_id).map(|e| e.window_id)
    }

    pub(in crate::platform::macos) fn stop(&mut self) {
        self.stopped = true;
    }

    pub(in crate::platform::macos) fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub(in crate::platform::macos) fn config_changed(&mut self, new_config: Config) {
        self.hub.sync_config(new_config.clone().into());
        self.sender
            .send(HubMessage::ConfigChanged(new_config.clone()));
        self.config = new_config;
        tracing::info!("Config reloaded");
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn sync_focus(&mut self, pid: i32) {
        if let Some(app) = RunningApp::new(pid) {
            self.sync_app_focus(&app);
        }
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn title_changed(&mut self, cg_id: CGWindowID) {
        if let Some(entry) = self.registry.get_mut(cg_id) {
            entry.title = entry.ax.read_title();
            tracing::trace!(%entry, "Title changed");
        }
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn screens_changed(&mut self, screens: Vec<MonitorInfo>) {
        self.update_screens(screens);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn mirror_clicked(&mut self, window_id: WindowId) {
        let entry = self.registry.by_id(window_id);
        if let Err(e) = entry.ax.focus() {
            tracing::debug!("Failed to focus window: {e:#}");
        }
        self.hub.set_focus(window_id);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn tab_clicked(
        &mut self,
        container_id: ContainerId,
        tab_idx: usize,
    ) {
        self.hub.focus_tab_index(container_id, tab_idx);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn space_changed(&mut self) {
        self.handle_space_changed();
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn tracked_for_pid(
        &self,
        pid: i32,
    ) -> HashMap<CGWindowID, WindowEntry> {
        self.registry
            .for_pid(pid)
            .map(|(id, e)| (id, e.clone()))
            .collect()
    }

    pub(in crate::platform::macos) fn all_tracked(&self) -> HashMap<CGWindowID, WindowEntry> {
        self.registry
            .iter()
            .map(|(id, e)| (id, e.clone()))
            .collect()
    }

    pub(in crate::platform::macos) fn ignore_rules(&self) -> Vec<MacosWindow> {
        self.config.macos.ignore.clone()
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

    pub(in crate::platform::macos) fn unmark_pid_observed(&mut self, pid: i32) {
        self.observed_pids.remove(&pid);
    }

    pub(in crate::platform::macos) fn remove_untracked_app(&mut self, pid: i32) {
        self.remove_app_windows(pid);
    }

    pub(in crate::platform::macos) fn register_observers(&mut self, apps: Vec<RunningApp>) {
        self.sender.send(HubMessage::RegisterObservers(apps));
    }

    #[tracing::instrument(skip_all, fields(pid = app.pid()))]
    fn sync_app_focus(&mut self, app: &RunningApp) {
        if !app.is_active() {
            return;
        }
        if let Some(ax) = app.focused_window()
            && let Some(entry) = self.registry.get(ax.cg_id())
        {
            self.hub.set_focus(entry.window_id);
        }
    }

    fn handle_space_changed(&mut self) {
        let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
            return;
        };
        let app = RunningApp::from(app);
        // All AX APIs should be synchronous here, as we should pause everything until we know
        // whether we are dealing with native fullscreen or not.
        let Some(ax) = app.focused_window() else {
            return;
        };
        let cg_id = ax.cg_id();
        let is_native_fs = ax.is_native_fullscreen();

        if let Some(entry) = self.registry.get_mut(cg_id) {
            let _span = tracing::debug_span!("space_changed",).entered();
            let window_id = entry.window_id;
            if is_native_fs {
                entry.state = WindowState::NativeFullscreen;
                self.hub.set_fullscreen(window_id);
            } else if !is_native_fs && matches!(entry.state, WindowState::NativeFullscreen) {
                let Ok(pos) = ax.get_position() else {
                    return;
                };
                let Ok(size) = ax.get_size() else {
                    return;
                };
                self.window_moved(window_id, pos.0, pos.1, size.0, size.1, Instant::now());
            }
        } else if is_native_fs {
            let window_id = self.hub.insert_fullscreen();
            self.registry.insert(
                Arc::new(ax.clone()),
                window_id,
                WindowState::NativeFullscreen,
                ax.app_name().map(str::to_owned),
                ax.bundle_id().map(str::to_owned),
                ax.title().map(str::to_owned),
            );
            tracing::info!(%ax, %window_id, "New native fullscreen window");
        }
    }

    fn remove_app_windows(&mut self, pid: i32) {
        for (cg_id, window_id) in self.registry.remove_by_pid(pid) {
            self.recovery.untrack(cg_id);
            self.hub.delete_window(window_id);
        }
    }

    fn remove_window(&mut self, cg_id: CGWindowID) {
        if let Some(window_id) = self.registry.remove(cg_id) {
            self.recovery.untrack(cg_id);
            self.hub.delete_window(window_id);
        }
    }

    fn update_screens(&mut self, screens: Vec<MonitorInfo>) {
        if screens.is_empty() {
            tracing::warn!("Empty screen list, skipping reconciliation");
            return;
        }

        if let Some(primary) = screens.iter().find(|s| s.is_primary) {
            self.primary_screen = primary.dimension;
            self.primary_full_height = primary.full_height;
        }

        // Re-hide windows that are offscreen with updated monitor positions
        for (_, entry) in self.registry.iter() {
            if let WindowState::Positioned(PositionedState::Offscreen { actual }) = &entry.state
                && let Err(e) = move_offscreen(&screens, actual, &*entry.ax)
            {
                tracing::trace!("Failed to re-hide window: {e:#}");
            }
        }

        reconcile_monitors(&mut self.hub, &mut self.monitor_registry, &screens);
    }

    #[tracing::instrument(skip(self))]
    fn execute_actions(&mut self, actions: &Actions) {
        for action in actions {
            match action {
                Action::Focus { target } => match target {
                    FocusTarget::Up => self.hub.focus_up(),
                    FocusTarget::Down => self.hub.focus_down(),
                    FocusTarget::Left => self.hub.focus_left(),
                    FocusTarget::Right => self.hub.focus_right(),
                    FocusTarget::Parent => self.hub.focus_parent(),
                    FocusTarget::NextTab => self.hub.focus_next_tab(),
                    FocusTarget::PrevTab => self.hub.focus_prev_tab(),
                    FocusTarget::Workspace { name } => self.hub.focus_workspace(name),
                    FocusTarget::Monitor { target } => self.hub.focus_monitor(target),
                },
                Action::Move { target } => match target {
                    MoveTarget::Up => self.hub.move_up(),
                    MoveTarget::Down => self.hub.move_down(),
                    MoveTarget::Left => self.hub.move_left(),
                    MoveTarget::Right => self.hub.move_right(),
                    MoveTarget::Workspace { name } => self.hub.move_focused_to_workspace(name),
                    MoveTarget::Monitor { target } => self.hub.move_focused_to_monitor(target),
                },
                Action::Toggle { target } => match target {
                    ToggleTarget::SpawnDirection => self.hub.toggle_spawn_mode(),
                    ToggleTarget::Direction => self.hub.toggle_direction(),
                    ToggleTarget::Layout => self.hub.toggle_container_layout(),
                    ToggleTarget::Float => self.hub.toggle_float(),
                    ToggleTarget::Fullscreen => self.hub.toggle_fullscreen(),
                },
                Action::Exec { command } => {
                    if let Err(e) = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(command)
                        .spawn()
                    {
                        tracing::warn!(%command, "Failed to exec: {e}");
                    }
                }
                Action::Exit => {
                    tracing::debug!("Exiting hub thread");
                    self.stopped = true;
                }
            }
        }
    }

    fn add_window(
        &mut self,
        ax: Arc<dyn AXWindowApi>,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        app_name: Option<String>,
        bundle_id: Option<String>,
        title: Option<String>,
    ) -> WindowId {
        let dim = RoundedDimension {
            x,
            y,
            width: w,
            height: h,
        };
        let monitor = self
            .monitor_registry
            .find_monitor_at(dim.x as f32, dim.y as f32);
        let is_borderless_fullscreen = monitor.is_some_and(|m| {
            let mon = &m.dimension;
            let tolerance = 2;
            (dim.x - mon.x as i32).abs() <= tolerance
                && (dim.y - mon.y as i32).abs() <= tolerance
                && (dim.width - mon.width as i32).abs() <= tolerance
                && (dim.height - mon.height as i32).abs() <= tolerance
        });
        if is_borderless_fullscreen {
            let window_id = self.hub.insert_fullscreen();
            self.registry.insert(
                ax.clone(),
                window_id,
                WindowState::BorderlessFullscreen,
                app_name.clone(),
                bundle_id.clone(),
                title.clone(),
            );
            tracing::info!(%window_id, "New borderless fullscreen window");
            window_id
        } else {
            let window_id = self.hub.insert_tiling();
            self.registry.insert(
                ax.clone(),
                window_id,
                WindowState::Positioned(PositionedState::Offscreen { actual: dim }),
                app_name,
                bundle_id,
                title,
            );
            tracing::info!(%window_id, "New tiling window");
            window_id
        }
    }

    fn add_native_fullscreen_window(
        &mut self,
        ax: Arc<dyn AXWindowApi>,
        app_name: Option<String>,
        bundle_id: Option<String>,
        title: Option<String>,
    ) -> WindowId {
        let window_id = self.hub.insert_fullscreen();
        self.registry.insert(
            ax,
            window_id,
            WindowState::NativeFullscreen,
            app_name,
            bundle_id,
            title,
        );
        tracing::info!(%window_id, "New native fullscreen window");
        window_id
    }
}

impl Drop for Dome {
    fn drop(&mut self) {
        self.recovery.restore_all();
        self.sender.send(HubMessage::Shutdown);
    }
}

fn reconcile_monitors(hub: &mut Hub, registry: &mut MonitorRegistry, screens: &[MonitorInfo]) {
    let current_keys: HashSet<_> = screens.iter().map(|s| s.display_id).collect();

    // Special handling for when the primary monitor got replaced, i.e. due to mirroring to prevent
    // disruption due to removal and addition of workspaces.
    if let Some(new_primary) = screens.iter().find(|s| s.is_primary) {
        if !registry.contains(new_primary.display_id) {
            registry.replace_primary(new_primary);
            hub.update_monitor_dimension(registry.primary_monitor_id(), new_primary.dimension);
        } else {
            registry.set_primary_display_id(new_primary.display_id);
        }
    }

    // Add new monitors first to prevent exhausting all monitors
    for screen in screens {
        if !registry.contains(screen.display_id) {
            let id = hub.add_monitor(screen.name.clone(), screen.dimension);
            registry.insert(screen, id);
            tracing::info!(%screen, "Monitor added");
        }
    }

    // Remove monitors that no longer exist
    for monitor_id in registry.remove_stale(&current_keys) {
        hub.remove_monitor(monitor_id, registry.primary_monitor_id());
        tracing::info!(%monitor_id, fallback = %registry.primary_monitor_id(), "Monitor removed");
    }

    // Update screen info (dimension, scale, etc.)
    for screen in screens {
        if let Some((monitor_id, old_dim)) = registry.update_screen(screen) {
            if old_dim != screen.dimension {
                tracing::info!(
                    name = %screen.name,
                    ?old_dim,
                    new_dim = ?screen.dimension,
                    "Monitor dimension changed"
                );
            }
            hub.update_monitor_dimension(monitor_id, screen.dimension);
        }
    }
}

fn on_open_actions(entry: &WindowEntry, rules: &[MacosOnOpenRule]) -> Option<Actions> {
    let rule = rules.iter().find(|r| {
        r.window.matches(
            entry.app_name.as_deref(),
            entry.bundle_id.as_deref(),
            entry.title.as_deref(),
        )
    })?;
    tracing::debug!(%entry, actions = %rule.run, "Running on_open actions");
    Some(rule.run.clone())
}
