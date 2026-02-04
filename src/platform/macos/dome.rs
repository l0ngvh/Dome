use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use dispatch2::{DispatchQueue, DispatchQueueAttr, DispatchRetained};
use objc2::rc::Retained;
use objc2_app_kit::{NSFloatingWindowLevel, NSRunningApplication};
use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopSource, CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_io_surface::IOSurface;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Color, Config, MacosOnOpenRule, MacosWindow};
use crate::core::{
    Child, Container, ContainerId, Dimension, Hub, MonitorId, SpawnMode, Window, WindowId,
};

use super::app::{ScreenInfo, compute_global_bounds};
use super::mirror::{WindowCapture, create_captures_async};
use super::overlay::{
    ContainerBorder, FloatBorder, MirrorUpdate, Overlays, TabBarOverlay, TabInfo, TilingBorder,
};
use super::recovery;
use super::window::{MacWindow, get_app_by_pid, get_ax_windows, list_cg_window_ids, running_apps};

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
    Shutdown,
}

struct WindowEntry {
    window: MacWindow,
    window_id: WindowId,
    capture: Option<WindowCapture>,
}

struct Registry {
    windows: HashMap<CGWindowID, WindowEntry>,
    id_to_cg: HashMap<WindowId, CGWindowID>,
    pid_to_cg: HashMap<i32, Vec<CGWindowID>>,
}

impl Registry {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            id_to_cg: HashMap::new(),
            pid_to_cg: HashMap::new(),
        }
    }

    fn insert(&mut self, window: MacWindow, window_id: WindowId) {
        let cg_id = window.cg_id();
        let pid = window.pid();
        if pid as u32 == std::process::id() {
            return;
        }
        self.id_to_cg.insert(window_id, cg_id);
        self.pid_to_cg.entry(pid).or_default().push(cg_id);
        self.windows.insert(
            cg_id,
            WindowEntry {
                window,
                window_id,
                capture: None,
            },
        );
    }

    fn remove(&mut self, cg_id: CGWindowID) -> Option<(MacWindow, WindowId)> {
        let entry = self.windows.remove(&cg_id)?;
        self.id_to_cg.remove(&entry.window_id);
        if let Some(ids) = self.pid_to_cg.get_mut(&entry.window.pid()) {
            ids.retain(|&id| id != cg_id);
        }
        Some((entry.window, entry.window_id))
    }

    fn get(&self, cg_id: CGWindowID) -> Option<&MacWindow> {
        self.windows.get(&cg_id).map(|e| &e.window)
    }

    fn get_mut(&mut self, cg_id: CGWindowID) -> Option<&mut MacWindow> {
        self.windows.get_mut(&cg_id).map(|e| &mut e.window)
    }

    fn get_cg_id(&self, window_id: WindowId) -> Option<CGWindowID> {
        self.id_to_cg.get(&window_id).copied()
    }

    fn get_window_id(&self, cg_id: CGWindowID) -> Option<WindowId> {
        self.windows.get(&cg_id).map(|e| e.window_id)
    }

    fn contains(&self, cg_id: CGWindowID) -> bool {
        self.windows.contains_key(&cg_id)
    }

    fn cg_ids_for_pid(&self, pid: i32) -> Vec<CGWindowID> {
        self.pid_to_cg.get(&pid).cloned().unwrap_or_default()
    }

    fn remove_by_pid(&mut self, pid: i32) -> Vec<(CGWindowID, MacWindow, WindowId)> {
        let Some(cg_ids) = self.pid_to_cg.remove(&pid) else {
            return Vec::new();
        };
        let mut removed = Vec::new();
        for cg_id in cg_ids {
            if let Some(entry) = self.windows.remove(&cg_id) {
                self.id_to_cg.remove(&entry.window_id);
                removed.push((cg_id, entry.window, entry.window_id));
            }
        }
        removed
    }

    fn is_valid(&self, cg_id: CGWindowID) -> bool {
        self.windows
            .get(&cg_id)
            .is_some_and(|e| e.window.is_valid())
    }

    fn get_title(&self, window_id: WindowId) -> Option<&str> {
        self.id_to_cg
            .get(&window_id)
            .and_then(|cg_id| self.windows.get(cg_id))
            .and_then(|e| e.window.title())
    }

    fn set_capture(&mut self, cg_id: CGWindowID, capture: WindowCapture) {
        if let Some(entry) = self.windows.get_mut(&cg_id) {
            entry.capture = Some(capture);
        }
    }

    fn get_capture(&self, cg_id: CGWindowID) -> Option<&WindowCapture> {
        self.windows.get(&cg_id).and_then(|e| e.capture.as_ref())
    }

    fn get_capture_mut(&mut self, cg_id: CGWindowID) -> Option<&mut WindowCapture> {
        self.windows
            .get_mut(&cg_id)
            .and_then(|e| e.capture.as_mut())
    }

    fn cg_ids(&self) -> impl Iterator<Item = CGWindowID> + '_ {
        self.windows.keys().copied()
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

struct MonitorRegistry {
    map: HashMap<DisplayId, (MonitorId, ScreenInfo)>,
    reverse: HashMap<MonitorId, DisplayId>,
    primary_display_id: DisplayId,
}

impl MonitorRegistry {
    fn new(primary: &ScreenInfo, primary_monitor_id: MonitorId) -> Self {
        let mut map = HashMap::new();
        let mut reverse = HashMap::new();
        map.insert(primary.display_id, (primary_monitor_id, primary.clone()));
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
        self.map.get(&display_id).map(|(id, _)| *id)
    }

    fn get_screen(&self, display_id: DisplayId) -> Option<&ScreenInfo> {
        self.map.get(&display_id).map(|(_, info)| info)
    }

    fn get_screen_by_monitor(&self, monitor_id: MonitorId) -> Option<&ScreenInfo> {
        self.reverse
            .get(&monitor_id)
            .and_then(|d| self.get_screen(*d))
    }

    fn primary_monitor_id(&self) -> MonitorId {
        self.get(self.primary_display_id).unwrap()
    }

    fn insert(&mut self, screen: &ScreenInfo, monitor_id: MonitorId) {
        self.map
            .insert(screen.display_id, (monitor_id, screen.clone()));
        self.reverse.insert(monitor_id, screen.display_id);
    }

    fn remove_by_id(&mut self, monitor_id: MonitorId) {
        if let Some(display_id) = self.reverse.remove(&monitor_id) {
            self.map.remove(&display_id);
        }
    }

    fn replace(&mut self, old_display_id: DisplayId, new_display_id: DisplayId) {
        if let Some((monitor_id, mut info)) = self.map.remove(&old_display_id) {
            info.display_id = new_display_id;
            self.map.insert(new_display_id, (monitor_id, info));
            self.reverse.insert(monitor_id, new_display_id);
        }
    }

    fn remove_stale(&mut self, current: &HashSet<DisplayId>) -> Vec<MonitorId> {
        let stale: Vec<_> = self
            .map
            .iter()
            .filter(|(key, _)| !current.contains(key))
            .map(|(_, (id, _))| *id)
            .collect();
        for &id in &stale {
            self.remove_by_id(id);
        }
        stale
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
    /// Bounding box of all monitors, used for hiding windows offscreen.
    global_bounds: Dimension,
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
        global_bounds: Dimension,
        hub_tx: Sender<HubEvent>,
        sender: MessageSender,
    ) -> Self {
        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_registry = MonitorRegistry::new(primary, primary_monitor_id);
        tracing::info!(
            name = %primary.name,
            display_id = primary.display_id,
            dimension = ?primary.dimension,
            "Primary monitor"
        );

        for screen in &screens {
            if screen.display_id != primary.display_id {
                let id = hub.add_monitor(screen.name.clone(), screen.dimension);
                monitor_registry.insert(screen, id);
                tracing::info!(
                    name = %screen.name,
                    display_id = screen.display_id,
                    dimension = ?screen.dimension,
                    "Monitor"
                );
            }
        }

        Self {
            hub,
            registry: Registry::new(),
            monitor_registry,
            config,
            primary_screen: primary.dimension,
            primary_full_height: primary.full_height,
            global_bounds,
            observed_pids: HashSet::new(),
            sender,
            hub_tx,
            capture_queue: DispatchQueue::new("dome.capture", DispatchQueueAttr::SERIAL),
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
        self.process_frame(None, HashSet::new());
        self.send_overlays(&HashSet::new());
    }

    fn handle_event(&mut self, event: HubEvent) {
        let last_focus = self
            .hub
            .get_workspace(self.hub.current_workspace())
            .focused();
        let previous_displayed = get_displayed_cg_ids(&self.hub, &self.registry);

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
        self.process_frame(last_focus, previous_displayed.clone());
        self.send_overlays(&previous_displayed);
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
        for (cg_id, ax_element) in get_ax_windows(pid) {
            if !self.running {
                return;
            }
            if self.registry.contains(cg_id) {
                continue;
            }

            let window = MacWindow::new(ax_element, cg_id, self.global_bounds, app);
            if !window.is_manageable() {
                continue;
            }
            if should_ignore(&window, &self.config.macos.ignore) {
                continue;
            }

            let dimension = window.get_dimension();
            let window_id = if window.should_tile() {
                self.hub.insert_tiling()
            } else {
                self.hub.insert_float(dimension)
            };

            recovery::track(cg_id, window.clone(), self.primary_screen);
            self.registry.insert(window, window_id);

            let window = self.registry.get(cg_id).unwrap();
            tracing::info!(%window, %window_id, "Window inserted");

            if let Some(actions) = on_open_actions(window, &self.config.macos.on_open) {
                self.execute_actions(&actions);
            }
        }

        // Create captures for windows without one
        let need_capture: Vec<_> = self
            .registry
            .cg_ids_for_pid(pid)
            .into_iter()
            .filter(|id| self.registry.get_capture(*id).is_none())
            .collect();
        if !need_capture.is_empty() {
            create_captures_async(
                need_capture,
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
        for (cg_id, window, window_id) in self.registry.remove_by_pid(pid) {
            tracing::info!(%window, %window_id, "Window removed");
            recovery::untrack(cg_id);
            self.hub.delete_window(window_id);
        }
    }

    fn remove_window(&mut self, cg_id: CGWindowID) {
        if let Some((window, window_id)) = self.registry.remove(cg_id) {
            tracing::info!(%window, %window_id, "Window removed");
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
        self.global_bounds = compute_global_bounds(&screens);

        for entry in self.registry.windows.values_mut() {
            entry.window.set_global_bounds(self.global_bounds);
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
    fn process_frame(
        &mut self,
        last_focus: Option<Child>,
        previous_displayed: HashSet<CGWindowID>,
    ) {
        let border = self.config.border_size;
        let current_displayed = get_displayed_cg_ids(&self.hub, &self.registry);

        // Hide windows no longer displayed
        for cg_id in previous_displayed.difference(&current_displayed) {
            if let Some(window) = self.registry.get_mut(*cg_id)
                && let Err(e) = window.hide()
            {
                tracing::trace!("Failed to hide window: {e:#}");
            }
        }

        // Position tiling windows
        let windows = collect_tiling_windows(&self.hub, &self.registry);
        position_tiling_windows(windows, &mut self.registry, border);

        // Position float windows
        for ws_id in self.hub.visible_workspaces() {
            let ws = self.hub.get_workspace(ws_id);
            for &float_id in ws.float_windows() {
                if let Some(cg_id) = self.registry.get_cg_id(float_id)
                    && let Some(window) = self.registry.get_mut(cg_id)
                {
                    let dim = apply_inset(self.hub.get_window(float_id).dimension(), border);
                    if let Err(e) = window.set_dimension(dim) {
                        tracing::trace!("Failed to set float dimension: {e:#}");
                    }
                }
            }
        }

        // Focus window if changed
        let ws = self.hub.get_workspace(self.hub.current_workspace());
        let focused = ws.focused();
        if focused != last_focus {
            let focus_cg_id = match focused {
                Some(Child::Window(id)) => self.registry.get_cg_id(id),
                Some(Child::Container(_)) => None,
                None => None,
            };
            if let Some(cg_id) = focus_cg_id
                && let Some(window) = self.registry.get(cg_id)
                && let Err(e) = window.focus()
            {
                tracing::trace!("Failed to focus window: {e:#}");
            }
        }
    }

    fn build_overlays(&mut self, previous_displayed: &HashSet<CGWindowID>) -> Overlays {
        let current_ws_id = self.hub.current_workspace();
        let mut tiling_borders = Vec::new();
        let mut float_borders = Vec::new();
        let mut container_borders = Vec::new();
        let mut tab_bars = Vec::new();
        let mut mirrors = Vec::new();
        let mut curr_mirrors = HashSet::new();
        let b = self.config.border_size;

        for ws_id in self.hub.visible_workspaces() {
            let ws = self.hub.get_workspace(ws_id);
            let monitor_dim = self.hub.get_monitor(ws.monitor()).dimension();
            let focused = if ws_id == current_ws_id {
                ws.focused()
            } else {
                None
            };

            let mut stack: Vec<Child> = ws.root().into_iter().collect();
            while let Some(child) = stack.pop() {
                match child {
                    Child::Window(id) => {
                        if self.registry.get_cg_id(id).is_some() {
                            let w = self.hub.get_window(id);
                            let colors = if focused == Some(Child::Window(id)) {
                                spawn_colors(w.spawn_mode(), &self.config)
                            } else {
                                [self.config.border_color; 4]
                            };
                            if let Some((clipped, edges)) =
                                compute_border_edges(w.dimension(), monitor_dim, colors, b)
                            {
                                tiling_borders.push(TilingBorder {
                                    key: id,
                                    frame: to_ns_rect(self.primary_full_height, clipped),
                                    edges: edges
                                        .into_iter()
                                        .map(|(r, c)| (to_edge_ns_rect(r, clipped.height), c))
                                        .collect(),
                                });
                            }
                        }
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
                let Some(cg_id) = self.registry.get_cg_id(float_id) else {
                    continue;
                };

                let dim = self.hub.get_window(float_id).dimension();
                let color = if focused == Some(Child::Window(float_id)) {
                    self.config.focused_color
                } else {
                    self.config.border_color
                };
                if let Some((clipped, edges)) =
                    compute_border_edges(dim, monitor_dim, [color; 4], b)
                {
                    float_borders.push(FloatBorder {
                        key: float_id,
                        frame: to_ns_rect(self.primary_full_height, clipped),
                        edges: edges
                            .into_iter()
                            .map(|(r, c)| (to_edge_ns_rect(r, clipped.height), c))
                            .collect(),
                    });
                }

                // Mirror unfocused floats
                if focused == Some(Child::Window(float_id)) {
                    continue;
                }

                let content_dim = apply_inset(dim, b);
                if let Some(clipped) = clip_to_bounds(content_dim, monitor_dim) {
                    let frame = to_ns_rect(self.primary_full_height, clipped);
                    let source_rect = compute_source_rect(content_dim, clipped);
                    let scale = self
                        .monitor_registry
                        .get_screen_by_monitor(ws.monitor())
                        .unwrap()
                        .scale;

                    if let Some(capture) = self.registry.get_capture_mut(cg_id) {
                        capture.start(
                            cg_id,
                            source_rect,
                            frame.size.width as u32,
                            frame.size.height as u32,
                            scale,
                            self.sender.clone(),
                        );
                    }

                    if let Some(window) = self.registry.get_mut(cg_id) {
                        window.hide().ok();
                    }

                    mirrors.push(MirrorUpdate {
                        cg_id,
                        frame,
                        level: NSFloatingWindowLevel as isize + 1,
                        scale,
                    });
                    curr_mirrors.insert(cg_id);
                }
            }
        }

        // Stop captures no longer mirrored
        for cg_id in previous_displayed.difference(&curr_mirrors) {
            if let Some(capture) = self.registry.get_capture_mut(*cg_id) {
                capture.stop();
            }
        }

        Overlays {
            tiling_borders,
            float_borders,
            container_borders,
            tab_bars,
            mirrors,
        }
    }

    fn send_overlays(&mut self, previous_mirrored: &HashSet<CGWindowID>) {
        let overlays = self.build_overlays(previous_mirrored);
        self.sender.send(HubMessage::Overlays(overlays));
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
    let ax_app = unsafe { objc2_application_services::AXUIElement::new_application(pid) };
    let focused = get_attribute::<objc2_application_services::AXUIElement>(
        &ax_app,
        &kAXFocusedWindowAttribute(),
    )
    .ok()?;
    get_cg_window_id(&focused)
}

fn get_displayed_cg_ids(hub: &Hub, registry: &Registry) -> HashSet<CGWindowID> {
    let mut cg_ids = HashSet::new();

    for ws_id in hub.visible_workspaces() {
        let ws = hub.get_workspace(ws_id);
        let mut stack: Vec<Child> = ws.root().into_iter().collect();
        while let Some(child) = stack.pop() {
            match child {
                Child::Window(id) => {
                    if let Some(cg_id) = registry.get_cg_id(id) {
                        cg_ids.insert(cg_id);
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
                cg_ids.insert(cg_id);
            }
        }
    }

    cg_ids
}

/// Position tiling windows, returns discovered size constraints and clipped window ids
fn collect_tiling_windows(
    hub: &Hub,
    registry: &Registry,
) -> Vec<(CGWindowID, WindowId, Window, Dimension)> {
    let mut result = Vec::new();

    for ws_id in hub.visible_workspaces() {
        let ws = hub.get_workspace(ws_id);
        let monitor_dim = hub.get_monitor(ws.monitor()).dimension();
        let mut stack: Vec<Child> = ws.root().into_iter().collect();

        while let Some(child) = stack.pop() {
            match child {
                Child::Window(id) => {
                    if let Some(cg_id) = registry.get_cg_id(id) {
                        result.push((cg_id, id, hub.get_window(id).clone(), monitor_dim));
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
    }
    result
}

fn position_tiling_windows(
    windows: Vec<(CGWindowID, WindowId, Window, Dimension)>,
    registry: &mut Registry,
    border: f32,
) {
    for (cg_id, _id, core_window, monitor_dim) in windows {
        let Some(mac_window) = registry.get_mut(cg_id) else {
            continue;
        };
        mac_window.try_placement(&core_window, border, monitor_dim);
    }
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

fn compute_source_rect(original: Dimension, clipped: Dimension) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: (clipped.x - original.x) as f64,
            y: (clipped.y - original.y) as f64,
        },
        size: CGSize {
            width: clipped.width as f64,
            height: clipped.height as f64,
        },
    }
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

fn apply_inset(dim: Dimension, border: f32) -> Dimension {
    Dimension {
        x: dim.x + border,
        y: dim.y + border,
        width: (dim.width - 2.0 * border).max(0.0),
        height: (dim.height - 2.0 * border).max(0.0),
    }
}

fn on_open_actions(window: &MacWindow, rules: &[MacosOnOpenRule]) -> Option<Actions> {
    let rule = rules.iter().find(|r| {
        r.window
            .matches(window.app_name(), window.bundle_id(), window.title())
    })?;
    tracing::debug!(%window, actions = %rule.run, "Running on_open actions");
    Some(rule.run.clone())
}

fn should_ignore(window: &MacWindow, rules: &[MacosWindow]) -> bool {
    let matched = rules
        .iter()
        .find(|r| r.matches(window.app_name(), window.bundle_id(), window.title()));
    if let Some(rule) = matched {
        tracing::debug!(%window, ?rule, "Window ignored by rule");
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
            tracing::info!(
                name = %screen.name,
                display_id = screen.display_id,
                dimension = ?screen.dimension,
                "Monitor added"
            );
        }
    }

    // Remove monitors that no longer exist
    for monitor_id in registry.remove_stale(&current_keys) {
        hub.remove_monitor(monitor_id, registry.primary_monitor_id());
        tracing::info!(%monitor_id, fallback = %registry.primary_monitor_id(), "Monitor removed");
    }

    // Update dimensions
    for screen in screens {
        if let Some(monitor_id) = registry.get(screen.display_id) {
            let old_dim = hub.get_monitor(monitor_id).dimension();
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
