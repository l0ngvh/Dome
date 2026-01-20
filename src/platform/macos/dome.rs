use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use objc2::rc::Retained;
use objc2_app_kit::NSRunningApplication;
use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopSource};
use objc2_core_graphics::CGWindowID;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Color, Config, MacosOnOpenRule, MacosWindow};
use crate::core::{
    Child, Container, Dimension, FloatWindowId, Focus, Hub, SpawnMode, Window, WindowId,
};

use super::overlay::{OverlayLabel, OverlayRect, Overlays};
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
    WindowMovedOrResized {
        pid: i32,
    },
    Action(Actions),
    ConfigChanged(Config),
    /// Periodic sync to catch missed AX notifications, as AX notifications are unreliable. Only
    /// syncs window state, not focus, as focus changes should come from user interactions. Beside
    /// we receive plenty of focus events, so missing them isn't a concern.
    Sync,
    Shutdown,
}

pub(super) enum HubMessage {
    Overlays(Overlays),
    RegisterObservers(Vec<Retained<NSRunningApplication>>),
    Shutdown,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum WindowType {
    Tiling(WindowId),
    Float(FloatWindowId),
}

impl std::fmt::Display for WindowType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WindowType::Tiling(id) => write!(f, "Tiling #{id}"),
            WindowType::Float(id) => write!(f, "Float #{id}"),
        }
    }
}

struct WindowEntry {
    window: MacWindow,
    window_type: WindowType,
}

struct Registry {
    windows: HashMap<CGWindowID, WindowEntry>,
    type_to_cg: HashMap<WindowType, CGWindowID>,
    pid_to_cg: HashMap<i32, Vec<CGWindowID>>,
}

impl Registry {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            type_to_cg: HashMap::new(),
            pid_to_cg: HashMap::new(),
        }
    }

    fn insert(&mut self, window: MacWindow, window_type: WindowType) {
        let cg_id = window.cg_id();
        let pid = window.pid();
        self.type_to_cg.insert(window_type, cg_id);
        self.pid_to_cg.entry(pid).or_default().push(cg_id);
        self.windows.insert(
            cg_id,
            WindowEntry {
                window,
                window_type,
            },
        );
    }

    fn remove(&mut self, cg_id: CGWindowID) -> Option<(MacWindow, WindowType)> {
        let entry = self.windows.remove(&cg_id)?;
        self.type_to_cg.remove(&entry.window_type);
        if let Some(ids) = self.pid_to_cg.get_mut(&entry.window.pid()) {
            ids.retain(|&id| id != cg_id);
        }
        Some((entry.window, entry.window_type))
    }

    fn get(&self, cg_id: CGWindowID) -> Option<&MacWindow> {
        self.windows.get(&cg_id).map(|e| &e.window)
    }

    fn get_mut(&mut self, cg_id: CGWindowID) -> Option<&mut MacWindow> {
        self.windows.get_mut(&cg_id).map(|e| &mut e.window)
    }

    fn get_entry(&self, cg_id: CGWindowID) -> Option<&WindowEntry> {
        self.windows.get(&cg_id)
    }

    fn get_cg_id(&self, window_type: WindowType) -> Option<CGWindowID> {
        self.type_to_cg.get(&window_type).copied()
    }

    fn get_window_type(&self, cg_id: CGWindowID) -> Option<WindowType> {
        self.windows.get(&cg_id).map(|e| e.window_type)
    }

    fn contains(&self, cg_id: CGWindowID) -> bool {
        self.windows.contains_key(&cg_id)
    }

    fn cg_ids_for_pid(&self, pid: i32) -> Vec<CGWindowID> {
        self.pid_to_cg.get(&pid).cloned().unwrap_or_default()
    }

    fn remove_by_pid(&mut self, pid: i32) -> Vec<(CGWindowID, MacWindow, WindowType)> {
        let Some(cg_ids) = self.pid_to_cg.remove(&pid) else {
            return Vec::new();
        };
        let mut removed = Vec::new();
        for cg_id in cg_ids {
            if let Some(entry) = self.windows.remove(&cg_id) {
                self.type_to_cg.remove(&entry.window_type);
                removed.push((cg_id, entry.window, entry.window_type));
            }
        }
        removed
    }

    fn is_valid(&self, cg_id: CGWindowID) -> bool {
        self.windows
            .get(&cg_id)
            .is_some_and(|e| e.window.is_valid())
    }

    fn toggle_float(&mut self, window_id: WindowId, float_id: FloatWindowId) {
        let tiling = WindowType::Tiling(window_id);
        let float = WindowType::Float(float_id);
        if let Some(cg_id) = self.type_to_cg.remove(&tiling) {
            self.type_to_cg.insert(float, cg_id);
            if let Some(entry) = self.windows.get_mut(&cg_id) {
                entry.window_type = float;
            }
        } else if let Some(cg_id) = self.type_to_cg.remove(&float) {
            self.type_to_cg.insert(tiling, cg_id);
            if let Some(entry) = self.windows.get_mut(&cg_id) {
                entry.window_type = tiling;
            }
        }
    }

    fn get_title(&self, window_id: WindowId) -> Option<&str> {
        self.type_to_cg
            .get(&WindowType::Tiling(window_id))
            .and_then(|cg_id| self.windows.get(cg_id))
            .and_then(|e| e.window.title())
    }
}

pub(super) struct MessageSender {
    pub(super) tx: Sender<HubMessage>,
    pub(super) source: CFRetained<CFRunLoopSource>,
    pub(super) run_loop: CFRetained<CFRunLoop>,
}

// Safety: CFRunLoopSource and CFRunLoop are thread-safe for signal/wake_up operations
unsafe impl Send for MessageSender {}

impl MessageSender {
    fn send(&self, msg: HubMessage) {
        if self.tx.send(msg).is_ok() {
            self.source.signal();
            self.run_loop.wake_up();
        }
    }
}

pub(super) struct Dome {
    hub: Hub,
    registry: Registry,
    config: Config,
    screen: Dimension,
    observed_pids: HashSet<i32>,
    sender: MessageSender,
    running: bool,
}

impl Dome {
    pub(super) fn new(config: Config, screen: Dimension, sender: MessageSender) -> Self {
        let hub = Hub::new(
            screen,
            config.tab_bar_height,
            config.automatic_tiling,
            config.min_width.resolve(screen.width),
            config.min_height.resolve(screen.height),
            config.max_width.resolve(screen.width),
            config.max_height.resolve(screen.height),
        );
        Self {
            hub,
            registry: Registry::new(),
            config,
            screen,
            observed_pids: HashSet::new(),
            sender,
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
        self.send_overlays();
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
                self.hub.sync_config(
                    new_config.tab_bar_height,
                    new_config.automatic_tiling,
                    new_config.min_width.resolve(self.screen.width),
                    new_config.min_height.resolve(self.screen.height),
                    new_config.max_width.resolve(self.screen.width),
                    new_config.max_height.resolve(self.screen.height),
                );
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
        }

        if !self.running {
            return;
        }
        self.process_frame(last_focus, previous_displayed);
        self.send_overlays();
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

            let window = MacWindow::new(ax_element, cg_id, self.screen, app);
            if !window.is_manageable() {
                continue;
            }
            if should_ignore(&window, &self.config.macos.ignore) {
                continue;
            }

            let dimension = window.get_dimension();
            let window_type = if window.should_tile() {
                WindowType::Tiling(self.hub.insert_tiling())
            } else {
                WindowType::Float(self.hub.insert_float(dimension))
            };

            recovery::track(cg_id, window.clone(), self.screen);
            self.registry.insert(window, window_type);

            let window = self.registry.get(cg_id).unwrap();
            tracing::info!(%window, %window_type, "Window inserted");

            if let Some(actions) = on_open_actions(window, &self.config.macos.on_open) {
                self.execute_actions(&actions);
            }
        }
    }

    #[tracing::instrument(skip_all, fields(pid = app.processIdentifier()))]
    fn sync_app_focus(&mut self, app: &NSRunningApplication) {
        if !app.isActive() {
            return;
        }
        let pid = app.processIdentifier();
        if let Some(cg_id) = get_focused_window_cg_id(pid)
            && let Some(wt) = self.registry.get_window_type(cg_id)
        {
            match wt {
                WindowType::Tiling(id) => self.hub.set_focus(id),
                WindowType::Float(id) => self.hub.set_float_focus(id),
            }
        }
    }

    fn remove_app_windows(&mut self, pid: i32) {
        for (cg_id, window, window_type) in self.registry.remove_by_pid(pid) {
            tracing::info!(%window, %window_type, "Window removed");
            recovery::untrack(cg_id);
            match window_type {
                WindowType::Tiling(id) => self.hub.delete_window(id),
                WindowType::Float(id) => self.hub.delete_float(id),
            }
        }
    }

    fn remove_window(&mut self, cg_id: CGWindowID) {
        if let Some((window, window_type)) = self.registry.remove(cg_id) {
            tracing::info!(%window, %window_type, "Window removed");
            recovery::untrack(cg_id);
            match window_type {
                WindowType::Tiling(id) => self.hub.delete_window(id),
                WindowType::Float(id) => self.hub.delete_float(id),
            }
        }
    }

    fn handle_window_moved(&mut self, pid: i32) {
        let border = self.config.border_size;
        for cg_id in self.registry.cg_ids_for_pid(pid) {
            let Some(entry) = self.registry.get_entry(cg_id) else {
                continue;
            };
            let WindowType::Tiling(id) = entry.window_type else {
                continue;
            };
            let window = self.hub.get_window(id);
            tracing::debug!(
                %pid,
                mac_window = %entry.window,
                ?window,
                "Handling window moved"
            );
            let Some((min_w, min_h, max_w, max_h)) = entry.window.check_placement(window) else {
                continue;
            };
            // Convert actual window size back to frame size by adding border back.
            // Frame dimensions have border inset applied. If in the original frame,
            // window width is smaller than sum of borders, then we will request a size
            // that can accommodate the borders here.
            let remove_inset = |v: f32| v + 2.0 * border;
            self.hub.set_window_constraint(
                id,
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
                    FocusTarget::Workspace { index } => self.hub.focus_workspace(*index),
                },
                Action::Move { target } => match target {
                    MoveTarget::Up => self.hub.move_up(),
                    MoveTarget::Down => self.hub.move_down(),
                    MoveTarget::Left => self.hub.move_left(),
                    MoveTarget::Right => self.hub.move_right(),
                    MoveTarget::Workspace { index } => self.hub.move_focused_to_workspace(*index),
                },
                Action::Toggle { target } => match target {
                    ToggleTarget::SpawnDirection => self.hub.toggle_spawn_mode(),
                    ToggleTarget::Direction => self.hub.toggle_direction(),
                    ToggleTarget::Layout => self.hub.toggle_container_layout(),
                    ToggleTarget::Float => {
                        if let Some((window_id, float_id)) = self.hub.toggle_float() {
                            self.registry.toggle_float(window_id, float_id);
                        }
                    }
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

    fn process_frame(
        &mut self,
        last_focus: Option<Focus>,
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
        let ws = self.hub.get_workspace(self.hub.current_workspace());
        let focused = ws.focused();
        let float_windows: Vec<_> = ws.float_windows().to_vec();
        for float_id in float_windows {
            if let Some(cg_id) = self.registry.get_cg_id(WindowType::Float(float_id))
                && let Some(window) = self.registry.get_mut(cg_id)
            {
                let dim = apply_inset(self.hub.get_float(float_id).dimension(), border);
                if let Err(e) = window.set_dimension(dim) {
                    tracing::trace!("Failed to set float dimension: {e:#}");
                }
            }
        }

        // Focus window if changed
        if focused != last_focus {
            let focus_cg_id = match focused {
                Some(Focus::Tiling(Child::Window(id))) => {
                    self.registry.get_cg_id(WindowType::Tiling(id))
                }
                Some(Focus::Float(id)) => self.registry.get_cg_id(WindowType::Float(id)),
                _ => None,
            };
            if let Some(cg_id) = focus_cg_id
                && let Some(window) = self.registry.get(cg_id)
                && let Err(e) = window.focus()
            {
                tracing::trace!("Failed to focus window: {e:#}");
            }
        }
    }

    fn send_overlays(&self) {
        let overlays = build_overlays(&self.hub, &self.registry, &self.config);
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
    let ws = hub.get_workspace(hub.current_workspace());
    let mut cg_ids = HashSet::new();

    let mut stack: Vec<Child> = ws.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(id) => {
                if let Some(cg_id) = registry.get_cg_id(WindowType::Tiling(id)) {
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
        if let Some(cg_id) = registry.get_cg_id(WindowType::Float(float_id)) {
            cg_ids.insert(cg_id);
        }
    }

    cg_ids
}

/// Position tiling windows, returns discovered size constraints and clipped window ids
fn collect_tiling_windows(hub: &Hub, registry: &Registry) -> Vec<(CGWindowID, WindowId, Window)> {
    let ws = hub.get_workspace(hub.current_workspace());
    let mut stack: Vec<Child> = ws.root().into_iter().collect();
    let mut result = Vec::new();

    while let Some(child) = stack.pop() {
        match child {
            Child::Window(id) => {
                if let Some(cg_id) = registry.get_cg_id(WindowType::Tiling(id)) {
                    result.push((cg_id, id, hub.get_window(id).clone()));
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
    result
}

fn position_tiling_windows(
    windows: Vec<(CGWindowID, WindowId, Window)>,
    registry: &mut Registry,
    border: f32,
) {
    for (cg_id, _id, core_window) in windows {
        let Some(mac_window) = registry.get_mut(cg_id) else {
            continue;
        };
        mac_window.try_placement(&core_window, border);
    }
}

fn build_tab_bar(
    screen: Dimension,
    container: &Container,
    registry: &Registry,
    config: &Config,
    is_focused: bool,
) -> (Vec<OverlayRect>, Vec<OverlayLabel>) {
    let dim = container.dimension();
    let border = config.border_size;
    let height = config.tab_bar_height;
    let tab_color = if is_focused {
        config.focused_color
    } else {
        config.border_color
    };

    let mut rects = vec![OverlayRect {
        x: dim.x,
        y: flip_y(screen, dim.y, height),
        width: dim.width,
        height,
        color: config.tab_bar_background_color,
    }];

    let tab_dim = Dimension {
        x: dim.x,
        y: dim.y,
        width: dim.width,
        height,
    };
    rects.extend(border_rects(screen, tab_dim, border, [tab_color; 4]));

    let children = container.children();
    if children.is_empty() {
        return (rects, Vec::new());
    }

    let tab_width = dim.width / children.len() as f32;
    let active_tab = container.active_tab_index();

    rects.push(OverlayRect {
        x: dim.x + active_tab as f32 * tab_width,
        y: flip_y(screen, dim.y, height),
        width: tab_width,
        height,
        color: config.active_tab_background_color,
    });

    for i in 1..children.len() {
        rects.push(OverlayRect {
            x: dim.x + i as f32 * tab_width - border / 2.0,
            y: flip_y(screen, dim.y, height),
            width: border,
            height,
            color: tab_color,
        });
    }

    let labels = children
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let title = match c {
                Child::Window(wid) => registry.get_title(*wid).unwrap_or("Unknown"),
                Child::Container(_) => "Container",
            };
            let is_active = i == active_tab;
            let text = if is_active {
                format!("[{title}]")
            } else {
                title.to_owned()
            };
            let x = dim.x + i as f32 * tab_width + tab_width / 2.0 - text.len() as f32 * 3.5;
            OverlayLabel {
                x,
                y: flip_y(screen, dim.y + height / 2.0 - 6.0, 12.0),
                text,
                color: Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                bold: is_active,
            }
        })
        .collect();

    (rects, labels)
}

fn build_overlays(hub: &Hub, registry: &Registry, config: &Config) -> Overlays {
    let ws = hub.get_workspace(hub.current_workspace());
    let screen = hub.screen();
    let border = config.border_size;
    let focused = ws.focused();

    let mut rects = Vec::new();
    let mut labels = Vec::new();

    let mut stack: Vec<Child> = ws.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(id) => {
                if registry.get_cg_id(WindowType::Tiling(id)).is_some()
                    && focused != Some(Focus::Tiling(Child::Window(id)))
                {
                    let dim = hub.get_window(id).dimension();
                    rects.extend(border_rects(screen, dim, border, [config.border_color; 4]));
                }
            }
            Child::Container(id) => {
                let container = hub.get_container(id);
                if let Some(active) = container.active_tab() {
                    stack.push(active);
                    let is_focused = focused == Some(Focus::Tiling(Child::Container(id)));
                    let (tab_rects, tab_labels) =
                        build_tab_bar(screen, container, registry, config, is_focused);
                    rects.extend(tab_rects);
                    labels.extend(tab_labels);
                } else {
                    for &c in container.children() {
                        stack.push(c);
                    }
                }
            }
        }
    }

    match focused {
        Some(Focus::Tiling(Child::Window(id))) => {
            let w = hub.get_window(id);
            rects.extend(border_rects(
                screen,
                w.dimension(),
                border,
                spawn_colors(w.spawn_mode(), config),
            ));
        }
        Some(Focus::Tiling(Child::Container(id))) => {
            let c = hub.get_container(id);
            rects.extend(border_rects(
                screen,
                c.dimension(),
                border,
                spawn_colors(c.spawn_mode(), config),
            ));
        }
        _ => {}
    }

    for &float_id in ws.float_windows() {
        if registry.get_cg_id(WindowType::Float(float_id)).is_some() {
            let dim = hub.get_float(float_id).dimension();
            let color = if focused == Some(Focus::Float(float_id)) {
                config.focused_color
            } else {
                config.border_color
            };
            rects.extend(border_rects(screen, dim, border, [color; 4]));
        }
    }

    Overlays { rects, labels }
}

fn spawn_colors(spawn: SpawnMode, config: &Config) -> [Color; 4] {
    let f = config.focused_color;
    let s = config.spawn_indicator_color;
    [
        if spawn.is_tab() { s } else { f },
        if spawn.is_vertical() { s } else { f },
        f,
        if spawn.is_horizontal() { s } else { f },
    ]
}

// macOS uses bottom-left origin, so we flip y here.
// Windows uses top-left origin, so no flip needed there.
fn flip_y(screen: Dimension, y: f32, height: f32) -> f32 {
    screen.y + screen.height - y - height
}

fn apply_inset(dim: Dimension, border: f32) -> Dimension {
    Dimension {
        x: dim.x + border,
        y: dim.y + border,
        width: (dim.width - 2.0 * border).max(0.0),
        height: (dim.height - 2.0 * border).max(0.0),
    }
}

// colors: [top, bottom, left, right]
fn border_rects(
    screen: Dimension,
    dim: Dimension,
    border: f32,
    colors: [Color; 4],
) -> [OverlayRect; 4] {
    [
        OverlayRect {
            x: dim.x,
            y: flip_y(screen, dim.y, border),
            width: dim.width,
            height: border,
            color: colors[0],
        },
        OverlayRect {
            x: dim.x,
            y: flip_y(screen, dim.y + dim.height - border, border),
            width: dim.width,
            height: border,
            color: colors[1],
        },
        OverlayRect {
            x: dim.x,
            y: flip_y(screen, dim.y + border, dim.height - 2.0 * border),
            width: border,
            height: dim.height - 2.0 * border,
            color: colors[2],
        },
        OverlayRect {
            x: dim.x + dim.width - border,
            y: flip_y(screen, dim.y + border, dim.height - 2.0 * border),
            width: border,
            height: dim.height - 2.0 * border,
            color: colors[3],
        },
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
