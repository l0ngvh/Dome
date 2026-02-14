use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::Retained;
use objc2_app_kit::NSRunningApplication;
use objc2_application_services::AXUIElement;
use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopSource};
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_io_surface::IOSurface;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Color, Config, MacosOnOpenRule, MacosWindow};
use crate::core::{
    Child, Container, ContainerId, Dimension, Hub, MonitorId, SpawnMode, Window, WindowId,
    WorkspaceId,
};
use crate::platform::macos::accessibility::{AXWindow, get_ax_windows};

use super::app::ScreenInfo;
use super::mirror::{WindowCapture, create_captures_async};
use super::overlay::{ContainerBorder, Overlays, TabBarOverlay, TabInfo};
use super::recovery;
use super::window::{MacWindow, get_app_by_pid, list_cg_window_ids, running_apps};

#[expect(
    clippy::large_enum_variant,
    reason = "Config is only sent once every blue moon, so maybe we need to do something here"
)]
pub(super) enum HubEvent {
    /// Sync window state (add/remove windows) for an app. Does NOT update focus.
    SyncApp {
        pid: i32,
    },
    /// Sync focus for an app. Separated from SyncApp because offscreen windows (on other
    /// workspaces) still report as "active", which would hijack focus and prevent switching
    /// to empty workspaces.
    SyncFocus {
        pid: i32,
    },
    AppTerminated {
        pid: i32,
    },
    TitleChanged(CGWindowID),
    /// One or more windows of app with pid got resized or moved.
    /// This can't be on a per CGWindowID basis, as these events are unreliable and are often fired
    /// on the wrong window. For example, Slack doesn't emit this event on the main application
    /// window. This can however create a scenario when one window in the app finishes
    /// moving/resizing and send this notification, but other windows are not finish yet.
    WindowMovedOrResized {
        pid: i32,
    },
    Action(Actions),
    ConfigChanged(Config),
    /// Periodic sync to catch missed AX notifications, as AX notifications are unreliable. Only
    /// syncs window state, not focus, as focus changes should come from user interactions. Beside
    /// we receive plenty of focus events, so missing them isn't a concern.
    Sync,
    ScreensChanged(Vec<ScreenInfo>),
    MirrorClicked(CGWindowID),
    CaptureReady {
        cg_id: CGWindowID,
        capture: WindowCapture,
    },
    Shutdown,
}

pub(super) enum HubMessage {
    Overlays(Overlays),
    RegisterObservers(Vec<Retained<NSRunningApplication>>),
    CaptureFrame {
        cg_id: CGWindowID,
        surface: Retained<IOSurface>,
    },
    CaptureFailed {
        cg_id: CGWindowID,
    },
    WindowShow {
        cg_id: CGWindowID,
        frame: NSRect,
        is_float: bool,
        is_focus: bool,
        edges: Vec<(NSRect, Color)>,
        scale: f64,
        border: f64,
    },
    WindowHide {
        cg_id: CGWindowID,
    },
    WindowCreate {
        cg_id: CGWindowID,
        frame: NSRect,
    },
    WindowDelete {
        cg_id: CGWindowID,
    },
    Shutdown,
}

struct Registry {
    windows: HashMap<CGWindowID, MacWindow>,
    id_to_cg: HashMap<WindowId, CGWindowID>,
    pid_to_cg: HashMap<i32, Vec<CGWindowID>>,
    monitors: Vec<ScreenInfo>,
    sender: MessageSender,
}

impl Registry {
    fn new(monitors: Vec<ScreenInfo>, sender: MessageSender) -> Self {
        Self {
            windows: HashMap::new(),
            id_to_cg: HashMap::new(),
            pid_to_cg: HashMap::new(),
            monitors,
            sender,
        }
    }

    fn insert(&mut self, ax: AXWindow, window_id: WindowId, hub_window: &Window) {
        let cg_id = ax.cg_id();
        let pid = ax.pid();
        if pid as u32 == std::process::id() {
            return;
        }
        self.id_to_cg.insert(window_id, cg_id);
        self.pid_to_cg.entry(pid).or_default().push(cg_id);
        self.windows.insert(
            cg_id,
            MacWindow::new(
                ax,
                window_id,
                hub_window,
                self.sender.clone(),
                self.monitors.clone(),
            ),
        );
    }

    fn remove(&mut self, cg_id: CGWindowID) -> Option<WindowId> {
        let window = self.windows.remove(&cg_id)?;
        self.id_to_cg.remove(&window.window_id());
        if let Some(ids) = self.pid_to_cg.get_mut(&window.pid()) {
            ids.retain(|&id| id != cg_id);
        }
        tracing::info!(%window, window_id = %window.window_id(), "Window removed");
        Some(window.window_id())
    }

    fn get_mut(&mut self, cg_id: CGWindowID) -> Option<&mut MacWindow> {
        self.windows.get_mut(&cg_id)
    }

    fn get_cg_id(&self, window_id: WindowId) -> Option<CGWindowID> {
        self.id_to_cg.get(&window_id).copied()
    }

    fn get_window_id(&self, cg_id: CGWindowID) -> Option<WindowId> {
        self.windows.get(&cg_id).map(|e| e.window_id())
    }

    fn get_by_window_id(&self, window_id: WindowId) -> Option<&MacWindow> {
        self.id_to_cg
            .get(&window_id)
            .copied()
            .and_then(|cg_id| self.windows.get(&cg_id))
    }

    fn get_mut_by_window_id(&mut self, window_id: WindowId) -> Option<&mut MacWindow> {
        self.id_to_cg
            .get(&window_id)
            .copied()
            .and_then(|cg_id| self.windows.get_mut(&cg_id))
    }

    fn contains(&self, cg_id: CGWindowID) -> bool {
        self.windows.contains_key(&cg_id)
    }

    fn cg_ids_for_pid(&self, pid: i32) -> Vec<CGWindowID> {
        self.pid_to_cg.get(&pid).cloned().unwrap_or_default()
    }

    fn remove_by_pid(&mut self, pid: i32) -> Vec<(CGWindowID, WindowId)> {
        let Some(cg_ids) = self.pid_to_cg.remove(&pid) else {
            return Vec::new();
        };
        let mut removed = Vec::new();
        for cg_id in cg_ids {
            if let Some(window) = self.windows.remove(&cg_id) {
                self.id_to_cg.remove(&window.window_id());
                tracing::info!(%window, window_id = %window.window_id(), "Window removed");
                removed.push((cg_id, window.window_id()));
            }
        }
        removed
    }

    fn is_valid(&self, cg_id: CGWindowID) -> bool {
        self.windows.get(&cg_id).is_some_and(|w| w.is_valid())
    }

    fn get_title(&self, window_id: WindowId) -> Option<&str> {
        self.id_to_cg
            .get(&window_id)
            .and_then(|cg_id| self.windows.get(cg_id))
            .and_then(|w| w.title())
    }

    fn set_capture(&mut self, cg_id: CGWindowID, capture: WindowCapture) {
        if let Some(window) = self.windows.get_mut(&cg_id) {
            window.set_capture(capture);
        }
    }

    fn get(&self, cg_id: CGWindowID) -> Option<&MacWindow> {
        self.windows.get(&cg_id)
    }
}

#[derive(Clone)]
pub(super) struct MessageSender {
    pub(super) tx: Sender<HubMessage>,
    pub(super) source: CFRetained<CFRunLoopSource>,
    pub(super) run_loop: CFRetained<CFRunLoop>,
}

// Safety: CFRunLoopSource and CFRunLoop are thread-safe for signal/wake_up operations
unsafe impl Send for MessageSender {}

impl MessageSender {
    pub(super) fn send(&self, msg: HubMessage) {
        if self.tx.send(msg).is_ok() {
            self.source.signal();
            self.run_loop.wake_up();
        }
    }
}

type DisplayId = u32;

struct MonitorEntry {
    id: MonitorId,
    screen: ScreenInfo,
    displayed_windows: HashSet<CGWindowID>,
}

struct MonitorRegistry {
    map: HashMap<DisplayId, MonitorEntry>,
    reverse: HashMap<MonitorId, DisplayId>,
    primary_display_id: DisplayId,
}

impl MonitorRegistry {
    fn new(primary: &ScreenInfo, primary_monitor_id: MonitorId) -> Self {
        let mut map = HashMap::new();
        let mut reverse = HashMap::new();
        map.insert(
            primary.display_id,
            MonitorEntry {
                id: primary_monitor_id,
                screen: primary.clone(),
                displayed_windows: HashSet::new(),
            },
        );
        reverse.insert(primary_monitor_id, primary.display_id);
        Self {
            map,
            reverse,
            primary_display_id: primary.display_id,
        }
    }

    fn contains(&self, display_id: DisplayId) -> bool {
        self.map.contains_key(&display_id)
    }

    fn get(&self, display_id: DisplayId) -> Option<MonitorId> {
        self.map.get(&display_id).map(|e| e.id)
    }

    fn get_entry_mut(&mut self, monitor_id: MonitorId) -> Option<&mut MonitorEntry> {
        self.reverse
            .get(&monitor_id)
            .and_then(|d| self.map.get_mut(d))
    }

    fn primary_monitor_id(&self) -> MonitorId {
        self.get(self.primary_display_id).unwrap()
    }

    fn insert(&mut self, screen: &ScreenInfo, monitor_id: MonitorId) {
        self.map.insert(
            screen.display_id,
            MonitorEntry {
                id: monitor_id,
                screen: screen.clone(),
                displayed_windows: HashSet::new(),
            },
        );
        self.reverse.insert(monitor_id, screen.display_id);
    }

    fn remove_by_id(&mut self, monitor_id: MonitorId) {
        if let Some(display_id) = self.reverse.remove(&monitor_id) {
            self.map.remove(&display_id);
        }
    }

    fn replace(&mut self, old_display_id: DisplayId, new_display_id: DisplayId) {
        if let Some(mut entry) = self.map.remove(&old_display_id) {
            entry.screen.display_id = new_display_id;
            self.map.insert(new_display_id, entry);
        }
    }

    fn remove_stale(&mut self, current: &HashSet<DisplayId>) -> Vec<MonitorId> {
        let stale: Vec<_> = self
            .map
            .iter()
            .filter(|(key, _)| !current.contains(key))
            .map(|(_, e)| e.id)
            .collect();
        for &id in &stale {
            self.remove_by_id(id);
        }
        stale
    }

    fn all_screens(&self) -> Vec<ScreenInfo> {
        self.map.values().map(|e| e.screen.clone()).collect()
    }
}

pub(super) struct Dome {
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
    sender: MessageSender,
    hub_tx: Sender<HubEvent>,
    capture_queue: DispatchRetained<DispatchQueue>,
    running: bool,
}

impl Dome {
    pub(super) fn new(
        config: Config,
        screens: Vec<ScreenInfo>,
        hub_tx: Sender<HubEvent>,
        sender: MessageSender,
    ) -> Self {
        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_registry = MonitorRegistry::new(primary, primary_monitor_id);
        tracing::info!(%primary, "Primary monitor");

        for screen in &screens {
            if screen.display_id != primary.display_id {
                let id = hub.add_monitor(screen.name.clone(), screen.dimension);
                monitor_registry.insert(screen, id);
                tracing::info!(%screen, "Monitor");
            }
        }

        Self {
            hub,
            registry: Registry::new(monitor_registry.all_screens(), sender.clone()),
            monitor_registry,
            config,
            primary_screen: primary.dimension,
            primary_full_height: primary.full_height,
            observed_pids: HashSet::new(),
            sender,
            hub_tx,
            capture_queue: DispatchQueue::main().into(),
            running: true,
        }
    }

    fn stop(&mut self) {
        self.running = false;
    }

    pub(super) fn run(mut self, rx: Receiver<HubEvent>) {
        self.initial_sync();
        while self.running {
            let Ok(event) = rx.recv() else { break };
            self.handle_event(event);
        }
    }

    fn initial_sync(&mut self) {
        let mut new_apps = Vec::new();
        for app in running_apps() {
            if !self.running {
                return;
            }
            let pid = app.processIdentifier();
            if self.observed_pids.insert(pid) {
                new_apps.push(app.clone());
            }
            self.sync_app_windows(&app);
        }
        if !new_apps.is_empty() {
            self.sender.send(HubMessage::RegisterObservers(new_apps));
        }
        let overlays = self.process_frame();

        self.sender.send(HubMessage::Overlays(overlays));
    }

    fn handle_event(&mut self, event: HubEvent) {
        match event {
            HubEvent::Shutdown => {
                tracing::info!("Shutdown requested");
                self.stop();
            }
            HubEvent::ConfigChanged(new_config) => {
                self.hub.sync_config(new_config.clone().into());
                self.config = new_config;
                tracing::info!("Config reloaded");
            }
            HubEvent::SyncApp { pid } => {
                if let Some(app) = get_app_by_pid(pid) {
                    self.sync_app_windows(&app);
                }
            }
            HubEvent::SyncFocus { pid } => {
                if let Some(app) = get_app_by_pid(pid) {
                    self.sync_app_focus(&app);
                }
            }
            HubEvent::AppTerminated { pid } => {
                tracing::debug!(pid, "App terminated");
                self.remove_app_windows(pid);
            }
            HubEvent::TitleChanged(cg_id) => {
                if let Some(window) = self.registry.get_mut(cg_id) {
                    window.update_title();
                    tracing::trace!(%window, "Title changed");
                }
            }
            HubEvent::WindowMovedOrResized { pid } => {
                self.handle_window_moved(pid);
            }
            HubEvent::Action(actions) => {
                tracing::debug!(%actions, "Executing actions");
                self.execute_actions(&actions);
            }
            // AX notifications are unreliable, when new windows are being rapidly created and
            // deleted, macOS may decide skip sending notifications. So we poll periodically to
            // keep the state in sync. https://github.com/nikitabobko/AeroSpace/issues/445
            HubEvent::Sync => {
                self.periodic_sync();
            }
            HubEvent::ScreensChanged(screens) => {
                tracing::info!(count = screens.len(), "Screens changed");
                self.update_screens(screens);
            }
            HubEvent::MirrorClicked(cg_id) => {
                if let Some(window) = self.registry.get(cg_id) {
                    window.focus().ok();
                }
                if let Some(window_id) = self.registry.get_window_id(cg_id) {
                    self.hub.set_focus(window_id);
                }
            }
            HubEvent::CaptureReady { cg_id, capture } => {
                self.registry.set_capture(cg_id, capture);
            }
        }

        if !self.running {
            return;
        }
        let overlays = self.process_frame();

        self.sender.send(HubMessage::Overlays(overlays));
    }

    #[tracing::instrument(skip_all, fields(pid = app.processIdentifier()))]
    fn sync_app_windows(&mut self, app: &NSRunningApplication) {
        let pid = app.processIdentifier();
        let cg_window_ids = list_cg_window_ids();

        // Remove invalid windows
        let tracked_cg_ids = self.registry.cg_ids_for_pid(pid);
        if app.isHidden() {
            for cg_id in tracked_cg_ids {
                self.remove_window(cg_id);
            }
            return;
        }
        for cg_id in tracked_cg_ids {
            if cg_window_ids.contains(&cg_id) && self.registry.is_valid(cg_id) {
                continue;
            }
            self.remove_window(cg_id);
        }

        // Add new windows
        let mut new_cg_ids = Vec::new();
        for ax in get_ax_windows(app) {
            if !self.running {
                return;
            }
            if self.registry.contains(ax.cg_id()) {
                continue;
            }

            if !ax.is_manageable() {
                continue;
            }
            if should_ignore(&ax, &self.config.macos.ignore) {
                continue;
            }

            let Ok((x, y)) = ax.get_position() else {
                continue;
            };
            let Ok((width, height)) = ax.get_size() else {
                continue;
            };
            let window_id = if ax.should_tile() {
                self.hub.insert_tiling()
            } else {
                self.hub.insert_float(Dimension {
                    x: x as f32,
                    y: y as f32,
                    width: width as f32,
                    height: height as f32,
                })
            };

            recovery::track(ax.clone(), self.primary_screen);
            let hub_window = self.hub.get_window(window_id);
            let cg_id = ax.cg_id();
            self.registry.insert(ax, window_id, hub_window);

            let window = self.registry.get_by_window_id(window_id).unwrap();
            tracing::info!(%window, %window_id, "Window inserted");

            if let Some(actions) = on_open_actions(window, &self.config.macos.on_open) {
                self.execute_actions(&actions);
            }

            new_cg_ids.push(cg_id);
        }

        if !new_cg_ids.is_empty() {
            create_captures_async(
                new_cg_ids,
                self.hub_tx.clone(),
                self.sender.clone(),
                self.capture_queue.clone(),
            );
        }
    }

    #[tracing::instrument(skip_all, fields(pid = app.processIdentifier()))]
    fn sync_app_focus(&mut self, app: &NSRunningApplication) {
        if !app.isActive() {
            return;
        }
        let pid = app.processIdentifier();
        if let Some(cg_id) = get_focused_window_cg_id(pid)
            && let Some(window_id) = self.registry.get_window_id(cg_id)
        {
            self.hub.set_focus(window_id);
        }
    }

    fn remove_app_windows(&mut self, pid: i32) {
        for (cg_id, window_id) in self.registry.remove_by_pid(pid) {
            recovery::untrack(cg_id);
            self.hub.delete_window(window_id);
        }
    }

    fn remove_window(&mut self, cg_id: CGWindowID) {
        if let Some(window_id) = self.registry.remove(cg_id) {
            recovery::untrack(cg_id);
            self.hub.delete_window(window_id);
        }
    }

    #[tracing::instrument(skip(self))]
    fn handle_window_moved(&mut self, pid: i32) {
        let border = self.config.border_size;
        for cg_id in self.registry.cg_ids_for_pid(pid) {
            let _span = tracing::trace_span!("check_placement", %cg_id).entered();
            let Some(window_id) = self.registry.get_window_id(cg_id) else {
                continue;
            };
            if self.hub.get_window(window_id).is_float() {
                continue;
            }
            let window = self.hub.get_window(window_id);
            let Some(mac_window) = self.registry.get_mut(cg_id) else {
                continue;
            };
            let Some((min_w, min_h, max_w, max_h)) = mac_window.check_placement(window) else {
                continue;
            };
            // Convert actual window size back to frame size by adding border back.
            // Frame dimensions have border inset applied. If in the original frame,
            // window width is smaller than sum of borders, then we will request a size
            // that can accommodate the borders here.
            let remove_inset = |v: f32| v + 2.0 * border;
            self.hub.set_window_constraint(
                window_id,
                min_w.map(remove_inset),
                min_h.map(remove_inset),
                max_w.map(remove_inset),
                max_h.map(remove_inset),
            );
        }
    }

    // TODO: this is not hiding unmaximized windows, like zoom
    #[tracing::instrument(skip_all)]
    fn periodic_sync(&mut self) {
        let running: Vec<_> = running_apps().collect();
        let running_pids: HashSet<_> = running.iter().map(|app| app.processIdentifier()).collect();

        // Cleanup terminated apps
        let terminated: Vec<_> = self
            .observed_pids
            .iter()
            .filter(|pid| !running_pids.contains(pid))
            .copied()
            .collect();
        for pid in terminated {
            self.observed_pids.remove(&pid);
            self.remove_app_windows(pid);
        }

        // Sync running apps
        let mut new_apps = Vec::new();
        for app in running {
            if !self.running {
                return;
            }
            let pid = app.processIdentifier();
            if self.observed_pids.insert(pid) {
                new_apps.push(app.clone());
            }
            self.sync_app_windows(&app);
        }
        if !new_apps.is_empty() {
            self.sender.send(HubMessage::RegisterObservers(new_apps));
        }
    }

    fn update_screens(&mut self, screens: Vec<ScreenInfo>) {
        if screens.is_empty() {
            tracing::warn!("Empty screen list, skipping reconciliation");
            return;
        }

        if let Some(primary) = screens.iter().find(|s| s.is_primary) {
            self.primary_screen = primary.dimension;
            self.primary_full_height = primary.full_height;
        }
        self.registry.monitors = screens.clone();

        for window in self.registry.windows.values_mut() {
            window.on_monitor_change(screens.clone());
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
                    self.stop();
                }
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn process_frame(&mut self) -> Overlays {
        let current_ws_id = self.hub.current_workspace();
        let b = self.config.border_size;
        let mut container_borders = Vec::new();
        let mut tab_bars = Vec::new();

        // Hide windows no longer displayed and update displayed state per monitor
        for ws_id in self.hub.visible_workspaces() {
            let ws = self.hub.get_workspace(ws_id);
            let monitor_id = ws.monitor();
            let current_windows = get_displayed_for_workspace(&self.hub, ws_id, &self.registry);

            let entry = self.monitor_registry.get_entry_mut(monitor_id).unwrap();
            for cg_id in entry.displayed_windows.difference(&current_windows) {
                if let Some(window_entry) = self.registry.get_mut(*cg_id)
                    && let Err(e) = window_entry.hide()
                {
                    tracing::trace!("Failed to hide window: {e:#}");
                }
            }
            entry.displayed_windows = current_windows;
        }

        for ws_id in self.hub.visible_workspaces() {
            let ws = self.hub.get_workspace(ws_id);
            let monitor_id = ws.monitor();
            let monitor_dim = self.hub.get_monitor(monitor_id).dimension();
            let mut stack: Vec<Child> = ws.root().into_iter().collect();
            let focused = if ws_id == current_ws_id {
                ws.focused()
            } else {
                None
            };

            while let Some(child) = stack.pop() {
                match child {
                    Child::Window(id) => {
                        let focused = focused == Some(Child::Window(id));
                        let window = self.registry.get_mut_by_window_id(id).unwrap();
                        if let Err(e) =
                            window.show(self.hub.get_window(id), monitor_dim, focused, &self.config)
                        {
                            tracing::trace!("Failed to set position for window: {e:#}");
                        };
                    }
                    Child::Container(id) => {
                        let container = self.hub.get_container(id);
                        if let Some(active) = container.active_tab() {
                            stack.push(active);
                            if let Some(tab_bar) = build_tab_bar(
                                self.primary_full_height,
                                monitor_dim,
                                id,
                                container,
                                &self.registry,
                                &self.config,
                            ) {
                                tab_bars.push(tab_bar);
                            }
                        } else {
                            for &c in container.children() {
                                stack.push(c);
                            }
                        }
                    }
                }
            }

            if let Some(Child::Container(id)) = focused {
                let c = self.hub.get_container(id);
                let colors = spawn_colors(c.spawn_mode(), &self.config);
                if let Some((clipped, edges)) =
                    compute_border_edges(c.dimension(), monitor_dim, colors, b)
                {
                    container_borders.push(ContainerBorder {
                        key: id,
                        frame: to_ns_rect(self.primary_full_height, clipped),
                        edges: edges
                            .into_iter()
                            .map(|(r, c)| (to_edge_ns_rect(r, clipped.height), c))
                            .collect(),
                    });
                }
            }

            for &float_id in ws.float_windows() {
                let float_focused = focused == Some(Child::Window(float_id));
                let window = self.registry.get_mut_by_window_id(float_id).unwrap();
                if let Err(e) = window.show(
                    self.hub.get_window(float_id),
                    monitor_dim,
                    float_focused,
                    &self.config,
                ) {
                    tracing::trace!("Failed to set float dimension: {e:#}");
                }
            }
        }

        Overlays {
            container_borders,
            tab_bars,
        }
    }
}

impl Drop for Dome {
    fn drop(&mut self) {
        recovery::restore_all();
        self.sender.send(HubMessage::Shutdown);
    }
}

fn get_focused_window_cg_id(pid: i32) -> Option<CGWindowID> {
    use super::objc2_wrapper::{get_attribute, get_cg_window_id, kAXFocusedWindowAttribute};
    let ax_app = unsafe { AXUIElement::new_application(pid) };
    let focused = get_attribute::<AXUIElement>(&ax_app, &kAXFocusedWindowAttribute()).ok()?;
    get_cg_window_id(&focused)
}

fn get_displayed_for_workspace(
    hub: &Hub,
    ws_id: WorkspaceId,
    registry: &Registry,
) -> HashSet<CGWindowID> {
    let mut windows = HashSet::new();
    let ws = hub.get_workspace(ws_id);

    let mut stack: Vec<Child> = ws.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(id) => {
                if let Some(cg_id) = registry.get_cg_id(id) {
                    windows.insert(cg_id);
                }
            }
            Child::Container(id) => {
                let container = hub.get_container(id);
                if let Some(active) = container.active_tab() {
                    stack.push(active);
                } else {
                    for &c in container.children() {
                        stack.push(c);
                    }
                }
            }
        }
    }

    for &float_id in ws.float_windows() {
        if let Some(cg_id) = registry.get_cg_id(float_id) {
            windows.insert(cg_id);
        }
    }

    windows
}

fn build_tab_bar(
    primary_full_height: f32,
    monitor_dim: Dimension,
    id: ContainerId,
    container: &Container,
    registry: &Registry,
    config: &Config,
) -> Option<TabBarOverlay> {
    let dim = container.dimension();
    let tab_bar_dim = Dimension {
        x: dim.x,
        y: dim.y,
        width: dim.width,
        height: config.tab_bar_height,
    };

    let clipped = clip_to_bounds(tab_bar_dim, monitor_dim)?;

    let children = container.children();
    let active_tab = container.active_tab_index();
    let tab_width = if children.is_empty() {
        0.0
    } else {
        dim.width / children.len() as f32
    };

    let tabs = children
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let title = match c {
                Child::Window(wid) => registry.get_title(*wid).unwrap_or("Unknown").to_owned(),
                Child::Container(_) => "Container".to_owned(),
            };
            TabInfo {
                title,
                x: i as f32 * tab_width,
                width: tab_width,
                is_active: i == active_tab,
            }
        })
        .collect();

    Some(TabBarOverlay {
        key: id,
        frame: to_ns_rect(primary_full_height, clipped),
        tabs,
        background_color: config.tab_bar_background_color,
        active_background_color: config.active_tab_background_color,
    })
}

// Quartz uses top-left origin, Cocoa uses bottom-left origin
fn to_ns_rect(primary_full_height: f32, dim: Dimension) -> NSRect {
    NSRect::new(
        NSPoint::new(
            dim.x as f64,
            (primary_full_height - dim.y - dim.height) as f64,
        ),
        NSSize::new(dim.width as f64, dim.height as f64),
    )
}

/// Convert edge rect from Quartz coords to Cocoa coords, relative to the overlay window.
/// Used for positioning border edges within their parent overlay NSWindow/NSView.
fn to_edge_ns_rect(dim: Dimension, overlay_height: f32) -> NSRect {
    NSRect::new(
        NSPoint::new(dim.x as f64, (overlay_height - dim.y - dim.height) as f64),
        NSSize::new(dim.width as f64, dim.height as f64),
    )
}

/// Clip rect to bounds. Returns None if fully outside.
fn clip_to_bounds(rect: Dimension, bounds: Dimension) -> Option<Dimension> {
    if rect.x >= bounds.x + bounds.width
        || rect.y >= bounds.y + bounds.height
        || rect.x + rect.width <= bounds.x
        || rect.y + rect.height <= bounds.y
    {
        return None;
    }
    let x = rect.x.max(bounds.x);
    let y = rect.y.max(bounds.y);
    let right = (rect.x + rect.width).min(bounds.x + bounds.width);
    let bottom = (rect.y + rect.height).min(bounds.y + bounds.height);
    Some(Dimension {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

/// Compute border edges for a window, clipped to monitor bounds.
/// Returns (clipped_frame, edges) or None if fully outside bounds.
/// All coordinates in Quartz (top-left origin).
///
/// # Arguments
/// * `colors` - Edge colors in order: [top, right, bottom, left]
///
/// # Returns
/// Edges in same order as colors: [top, right, bottom, left] (if visible after clipping)
fn compute_border_edges(
    frame: Dimension,
    bounds: Dimension,
    colors: [Color; 4],
    b: f32,
) -> Option<(Dimension, Vec<(Dimension, Color)>)> {
    let clipped = clip_to_bounds(frame, bounds)?;

    let offset_x = clipped.x - frame.x;
    let offset_y = clipped.y - frame.y;
    let clip_local = Dimension {
        x: offset_x,
        y: offset_y,
        width: clipped.width,
        height: clipped.height,
    };

    let w = frame.width;
    let h = frame.height;
    let mut edges = Vec::new();

    // top (y = 0 in Quartz)
    let top = Dimension {
        x: 0.0,
        y: 0.0,
        width: w,
        height: b,
    };
    if let Some(r) = clip_to_bounds(top, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[0]));
    }

    // right (exclude corners)
    let right = Dimension {
        x: w - b,
        y: b,
        width: b,
        height: h - 2.0 * b,
    };
    if let Some(r) = clip_to_bounds(right, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[1]));
    }

    // bottom (y = h - b in Quartz)
    let bottom = Dimension {
        x: 0.0,
        y: h - b,
        width: w,
        height: b,
    };
    if let Some(r) = clip_to_bounds(bottom, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[2]));
    }

    // left (exclude corners)
    let left = Dimension {
        x: 0.0,
        y: b,
        width: b,
        height: h - 2.0 * b,
    };
    if let Some(r) = clip_to_bounds(left, clip_local) {
        edges.push((translate_dim(r, -offset_x, -offset_y), colors[3]));
    }

    if edges.is_empty() {
        None
    } else {
        Some((clipped, edges))
    }
}

fn translate_dim(dim: Dimension, dx: f32, dy: f32) -> Dimension {
    Dimension {
        x: dim.x + dx,
        y: dim.y + dy,
        width: dim.width,
        height: dim.height,
    }
}

fn spawn_colors(spawn: SpawnMode, config: &Config) -> [Color; 4] {
    let f = config.focused_color;
    let s = config.spawn_indicator_color;
    // [top, right, bottom, left] to match BorderView draw order
    [
        if spawn.is_tab() { s } else { f },
        if spawn.is_horizontal() { s } else { f },
        if spawn.is_vertical() { s } else { f },
        f,
    ]
}

fn on_open_actions(window: &MacWindow, rules: &[MacosOnOpenRule]) -> Option<Actions> {
    let rule = rules.iter().find(|r| {
        r.window
            .matches(window.app_name(), window.bundle_id(), window.title())
    })?;
    tracing::debug!(%window, actions = %rule.run, "Running on_open actions");
    Some(rule.run.clone())
}

fn should_ignore(ax_window: &AXWindow, rules: &[MacosWindow]) -> bool {
    let matched = rules
        .iter()
        .find(|r| r.matches(ax_window.title(), ax_window.bundle_id(), ax_window.title()));
    if let Some(rule) = matched {
        tracing::debug!(
            %ax_window,
            ?rule,
            "Window ignored by rule"
        );
        return true;
    }
    false
}

fn reconcile_monitors(hub: &mut Hub, registry: &mut MonitorRegistry, screens: &[ScreenInfo]) {
    let current_keys: HashSet<_> = screens.iter().map(|s| s.display_id).collect();

    // Special handling for when the primary monitor got replaced, i.e. due to mirroring to prevent
    // disruption due to removal and addition of workspaces.
    if let Some(new_primary) = screens.iter().find(|s| s.is_primary) {
        if !registry.contains(new_primary.display_id) {
            let old_display_id = registry.primary_display_id;
            registry.replace(old_display_id, new_primary.display_id);
            registry.primary_display_id = new_primary.display_id;
            hub.update_monitor_dimension(registry.primary_monitor_id(), new_primary.dimension);
        } else {
            registry.primary_display_id = new_primary.display_id;
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
        if let Some(entry) = registry.map.get_mut(&screen.display_id) {
            let old_dim = hub.get_monitor(entry.id).dimension();
            if old_dim != screen.dimension {
                tracing::info!(
                    name = %screen.name,
                    ?old_dim,
                    new_dim = ?screen.dimension,
                    "Monitor dimension changed"
                );
            }
            hub.update_monitor_dimension(entry.id, screen.dimension);
            entry.screen = screen.clone();
        }
    }
}
