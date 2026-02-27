use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use dispatch2::{DispatchQoS, DispatchQueue, DispatchRetained, GlobalQueueIdentifier};
use objc2::rc::{Retained, autoreleasepool};
use objc2_core_foundation::{
    CFArray, CFDictionary, CFNumber, CFRetained, CFRunLoop, CFRunLoopSource, CFString, CFType,
};
use objc2_core_graphics::{CGWindowID, CGWindowListCopyWindowInfo, CGWindowListOption};
use objc2_foundation::NSRect;
use objc2_io_surface::IOSurface;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Color, Config, MacosOnOpenRule, MacosWindow};
use crate::core::{Dimension, Hub, Window, WindowId};
use crate::platform::macos::accessibility::AXWindow;

use super::mirror::{WindowCapture, create_captures_async};
use super::monitor::{MonitorInfo, MonitorRegistry};
use super::objc2_wrapper::kCGWindowNumber;
use super::overlay::Overlays;
use super::recovery;
use super::running_application::RunningApp;
use super::window::{FullscreenTransition, MacWindow};

pub(super) struct VisibleWindowsReconciled {
    pid: i32,
    is_hidden: bool,
    to_remove: Vec<CGWindowID>,
    to_add: Vec<AXWindow>,
}

#[expect(
    clippy::large_enum_variant,
    reason = "Config is only sent once every blue moon, so maybe we need to do something here"
)]
pub(super) enum HubEvent {
    /// Visible windows changed for an app (window created/destroyed/minimized/shown/hidden).
    VisibleWindowsChanged {
        pid: i32,
    },
    /// Sync focus for an app. Separated from VisibleWindowsChanged because offscreen windows (on
    /// other workspaces) still report as "active", which would hijack focus and prevent switching
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
    ScreensChanged(Vec<MonitorInfo>),
    MirrorClicked(CGWindowID),
    CaptureReady {
        cg_id: CGWindowID,
        capture: WindowCapture,
    },
    AppVisibleWindowsReconciled(VisibleWindowsReconciled),
    AllVisibleWindowsReconciled {
        terminated_pids: Vec<i32>,
        new_apps: Vec<RunningApp>,
        apps: Vec<VisibleWindowsReconciled>,
    },
    Shutdown,
}

pub(super) enum HubMessage {
    Overlays(Overlays),
    RegisterObservers(Vec<RunningApp>),
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

pub(super) struct Registry {
    windows: HashMap<CGWindowID, MacWindow>,
    id_to_cg: HashMap<WindowId, CGWindowID>,
    pid_to_cg: HashMap<i32, Vec<CGWindowID>>,
    monitors: Vec<MonitorInfo>,
    sender: MessageSender,
}

impl Registry {
    fn new(monitors: Vec<MonitorInfo>, sender: MessageSender) -> Self {
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
        let pid = window.pid();
        self.id_to_cg.remove(&window.window_id());
        if let Some(ids) = self.pid_to_cg.get_mut(&pid) {
            ids.retain(|&id| id != cg_id);
            if ids.is_empty() {
                self.pid_to_cg.remove(&pid);
            }
        }
        tracing::info!(%window, window_id = %window.window_id(), "Window removed");
        Some(window.window_id())
    }

    pub(super) fn get_mut(&mut self, cg_id: CGWindowID) -> Option<&mut MacWindow> {
        self.windows.get_mut(&cg_id)
    }

    pub(super) fn get_cg_id(&self, window_id: WindowId) -> Option<CGWindowID> {
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

    pub(super) fn get_mut_by_window_id(&mut self, window_id: WindowId) -> Option<&mut MacWindow> {
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

    pub(super) fn get_title(&self, window_id: WindowId) -> Option<&str> {
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

    pub(super) fn get(&self, cg_id: CGWindowID) -> Option<&MacWindow> {
        self.windows.get(&cg_id)
    }

    fn ax_windows_for_pid(&self, pid: i32) -> HashMap<CGWindowID, AXWindow> {
        self.cg_ids_for_pid(pid)
            .into_iter()
            .filter_map(|cg_id| Some((cg_id, self.windows.get(&cg_id)?.ax().clone())))
            .collect()
    }

    fn all_ax_windows(&self) -> HashMap<CGWindowID, AXWindow> {
        self.windows
            .iter()
            .map(|(&cg_id, w)| (cg_id, w.ax().clone()))
            .collect()
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
        screens: Vec<MonitorInfo>,
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
        dispatch_reconcile_all_windows(
            self.observed_pids.clone(),
            self.registry.all_ax_windows(),
            self.config.macos.ignore.clone(),
            self.hub_tx.clone(),
        );
        while self.running {
            let Ok(event) = rx.recv() else { break };
            self.handle_event(event);
        }
    }

    fn handle_event(&mut self, event: HubEvent) {
        autoreleasepool(|_| {
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
                HubEvent::VisibleWindowsChanged { pid } => {
                    dispatch_refresh_app_windows(
                        pid,
                        self.registry.ax_windows_for_pid(pid),
                        self.config.macos.ignore.clone(),
                        self.hub_tx.clone(),
                    );
                }
                HubEvent::SyncFocus { pid } => {
                    if let Some(app) = RunningApp::new(pid) {
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
                HubEvent::Sync => {
                    dispatch_reconcile_all_windows(
                        self.observed_pids.clone(),
                        self.registry.all_ax_windows(),
                        self.config.macos.ignore.clone(),
                        self.hub_tx.clone(),
                    );
                }
                HubEvent::ScreensChanged(screens) => {
                    tracing::info!(count = screens.len(), "Screens changed");
                    self.update_screens(screens);
                }
                HubEvent::MirrorClicked(cg_id) => {
                    if let Some(window) = self.registry.get(cg_id) {
                        window.focus();
                    }
                    if let Some(window_id) = self.registry.get_window_id(cg_id) {
                        self.hub.set_focus(window_id);
                    }
                }
                HubEvent::CaptureReady { cg_id, capture } => {
                    self.registry.set_capture(cg_id, capture);
                }
                HubEvent::AppVisibleWindowsReconciled(result) => {
                    self.apply_visible_windows_change(result);
                }
                HubEvent::AllVisibleWindowsReconciled {
                    terminated_pids,
                    new_apps,
                    apps,
                } => {
                    for pid in terminated_pids {
                        self.observed_pids.remove(&pid);
                        self.remove_app_windows(pid);
                    }
                    for result in apps {
                        self.apply_visible_windows_change(result);
                    }
                    if !new_apps.is_empty() {
                        for app in &new_apps {
                            self.observed_pids.insert(app.pid());
                        }
                        self.sender.send(HubMessage::RegisterObservers(new_apps));
                    }
                }
            }

            if !self.running {
                return;
            }
            let overlays = self.apply_layout();

            self.sender.send(HubMessage::Overlays(overlays));
        });
    }

    #[tracing::instrument(skip_all, fields(pid = app.pid()))]
    fn sync_app_focus(&mut self, app: &RunningApp) {
        if !app.is_active() {
            return;
        }
        if let Some(cg_id) = app.focused_window_cg_id()
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

            let Some(window) = self.registry.get(cg_id) else {
                continue;
            };
            let Ok((x, y)) = window.ax().get_position() else {
                continue;
            };
            let Some(monitor) = self.monitor_registry.find_monitor_at(x as f32, y as f32) else {
                continue;
            };
            let monitor_dim = monitor.dimension;
            let Some(mac_window) = self.registry.get_mut(cg_id) else {
                continue;
            };
            match mac_window.sync_fullscreen(&monitor_dim) {
                FullscreenTransition::Entered(_) => {
                    self.hub.set_fullscreen(window_id);
                    continue;
                }
                FullscreenTransition::Exited => {
                    self.hub.unset_fullscreen(window_id);
                    continue;
                }
                FullscreenTransition::Unchanged => {}
            }

            if self.hub.get_window(window_id).is_float()
                || self.hub.get_window(window_id).is_fullscreen()
            {
                continue;
            }
            let hub_window = self.hub.get_window(window_id);
            let Some(mac_window) = self.registry.get_mut(cg_id) else {
                continue;
            };
            let Some((min_w, min_h, max_w, max_h)) = mac_window.check_placement(hub_window) else {
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

    fn update_screens(&mut self, screens: Vec<MonitorInfo>) {
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
                    self.stop();
                }
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn apply_layout(&mut self) -> Overlays {
        let mut overlays = Overlays {
            container_borders: vec![],
            tab_bars: vec![],
        };
        for mp in self.hub.get_visible_placements() {
            let entry = self.monitor_registry.get_entry_mut(mp.monitor_id).unwrap();
            let o = entry.apply_placements(
                &mp,
                &self.hub,
                &mut self.registry,
                &self.config,
                self.primary_full_height,
            );
            overlays.container_borders.extend(o.container_borders);
            overlays.tab_bars.extend(o.tab_bars);
        }
        overlays
    }

    fn apply_visible_windows_change(&mut self, result: VisibleWindowsReconciled) {
        if result.is_hidden {
            for cg_id in self.registry.cg_ids_for_pid(result.pid) {
                self.remove_window(cg_id);
            }
            return;
        }

        for cg_id in result.to_remove {
            self.remove_window(cg_id);
        }

        let mut new_cg_ids = Vec::new();
        for ax in result.to_add {
            if let Some(cg_id) = self.add_window(ax) {
                new_cg_ids.push(cg_id);
            }
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

    fn add_window(&mut self, ax: AXWindow) -> Option<CGWindowID> {
        if self.registry.contains(ax.cg_id()) {
            return None;
        }

        let (x, y) = ax.get_position().ok()?;
        let (width, height) = ax.get_size().ok()?;

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

        // Hide immediately - window may spawn outside viewport due to scrolling
        if let Some(window) = self.registry.get_mut(cg_id) {
            window.hide().ok();
        }

        let window = self.registry.get_by_window_id(window_id).unwrap();
        tracing::info!(%window, %window_id, "Window inserted");

        if let Some(actions) = on_open_actions(window, &self.config.macos.on_open) {
            self.execute_actions(&actions);
        }

        Some(cg_id)
    }
}

impl Drop for Dome {
    fn drop(&mut self) {
        recovery::restore_all();
        self.sender.send(HubMessage::Shutdown);
    }
}

fn dispatch_refresh_app_windows(
    pid: i32,
    tracked: HashMap<CGWindowID, AXWindow>,
    ignore_rules: Vec<MacosWindow>,
    hub_tx: Sender<HubEvent>,
) {
    let queue = DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
        DispatchQoS::UserInitiated,
    ));
    queue.exec_async(move || {
        autoreleasepool(|_| {
            let Some(app) = RunningApp::new(pid) else {
                return;
            };
            let result = compute_app_visible_windows(&app, &tracked, &ignore_rules);
            hub_tx
                .send(HubEvent::AppVisibleWindowsReconciled(result))
                .ok();
        });
    });
}

fn dispatch_reconcile_all_windows(
    observed_pids: HashSet<i32>,
    tracked: HashMap<CGWindowID, AXWindow>,
    ignore_rules: Vec<MacosWindow>,
    hub_tx: Sender<HubEvent>,
) {
    let queue = DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
        DispatchQoS::UserInitiated,
    ));
    queue.exec_async(move || {
        autoreleasepool(|_| {
            let running: Vec<_> = RunningApp::all().collect();
            let running_pids: HashSet<_> = running.iter().map(|app| app.pid()).collect();

            let terminated_pids: Vec<_> = observed_pids
                .iter()
                .filter(|pid| !running_pids.contains(pid))
                .copied()
                .collect();

            let new_apps: Vec<_> = running
                .iter()
                .filter(|app| !observed_pids.contains(&app.pid()))
                .cloned()
                .collect();

            let apps: Vec<_> = running
                .iter()
                .map(|app| compute_app_visible_windows(app, &tracked, &ignore_rules))
                .collect();

            hub_tx
                .send(HubEvent::AllVisibleWindowsReconciled {
                    terminated_pids,
                    new_apps,
                    apps,
                })
                .ok();
        });
    });
}

fn compute_app_visible_windows(
    app: &RunningApp,
    tracked: &HashMap<CGWindowID, AXWindow>,
    ignore_rules: &[MacosWindow],
) -> VisibleWindowsReconciled {
    let pid = app.pid();
    let is_hidden = app.is_hidden();

    if is_hidden {
        return VisibleWindowsReconciled {
            pid,
            is_hidden,
            to_remove: Vec::new(),
            to_add: Vec::new(),
        };
    }

    let cg_window_ids = list_cg_window_ids();

    let mut to_remove = Vec::new();
    for (&cg_id, ax) in tracked.iter().filter(|(_, ax)| ax.pid() == pid) {
        if !cg_window_ids.contains(&cg_id) || !ax.is_valid() {
            to_remove.push(cg_id);
        }
    }

    let mut to_add = Vec::new();
    for ax in app.ax_windows() {
        if tracked.contains_key(&ax.cg_id()) {
            continue;
        }
        if !ax.is_manageable() {
            continue;
        }
        if should_ignore(&ax, ignore_rules) {
            continue;
        }
        to_add.push(ax);
    }

    VisibleWindowsReconciled {
        pid,
        is_hidden,
        to_remove,
        to_add,
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

fn list_cg_window_ids() -> HashSet<CGWindowID> {
    let Some(window_list) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, 0) else {
        tracing::warn!("CGWindowListCopyWindowInfo returned None");
        return HashSet::new();
    };
    let window_list: &CFArray<CFDictionary<CFString, CFType>> =
        unsafe { window_list.cast_unchecked() };

    let mut ids = HashSet::new();
    let key = kCGWindowNumber();
    for dict in window_list {
        // window id is a required attribute
        // https://developer.apple.com/documentation/coregraphics/kcgwindownumber?language=objc
        let id = dict
            .get(&key)
            .unwrap()
            .downcast::<CFNumber>()
            .unwrap()
            .as_i64()
            .unwrap();
        ids.insert(id as CGWindowID);
    }
    ids
}
