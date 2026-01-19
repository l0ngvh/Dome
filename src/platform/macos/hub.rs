use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};

use objc2::rc::Retained;
use objc2_app_kit::NSRunningApplication;
use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopSource};
use objc2_core_graphics::CGWindowID;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Color, Config, MacosOnOpenRule, MacosWindow};
use crate::core::{Child, Container, Dimension, FloatWindowId, Focus, Hub, SpawnMode, WindowId};

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

    fn remove(&mut self, cg_id: CGWindowID) -> Option<WindowType> {
        let entry = self.windows.remove(&cg_id)?;
        self.type_to_cg.remove(&entry.window_type);
        if let Some(ids) = self.pid_to_cg.get_mut(&entry.window.pid()) {
            ids.retain(|&id| id != cg_id);
        }
        Some(entry.window_type)
    }

    fn get(&self, cg_id: CGWindowID) -> Option<&MacWindow> {
        self.windows.get(&cg_id).map(|e| &e.window)
    }

    fn get_mut(&mut self, cg_id: CGWindowID) -> Option<&mut MacWindow> {
        self.windows.get_mut(&cg_id).map(|e| &mut e.window)
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

    fn remove_by_pid(&mut self, pid: i32) -> Vec<(CGWindowID, WindowType)> {
        let Some(cg_ids) = self.pid_to_cg.remove(&pid) else {
            return Vec::new();
        };
        let mut removed = Vec::new();
        for cg_id in cg_ids {
            if let Some(entry) = self.windows.remove(&cg_id) {
                self.type_to_cg.remove(&entry.window_type);
                removed.push((cg_id, entry.window_type));
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

struct MessageSender {
    tx: Sender<HubMessage>,
    source: CFRetained<CFRunLoopSource>,
    run_loop: CFRetained<CFRunLoop>,
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

pub(super) struct HubThread {
    handle: JoinHandle<()>,
}

impl HubThread {
    pub(super) fn spawn(
        config: Config,
        screen: Dimension,
        event_rx: Receiver<HubEvent>,
        frame_tx: Sender<HubMessage>,
        source: CFRetained<CFRunLoopSource>,
        main_run_loop: CFRetained<CFRunLoop>,
    ) -> Self {
        let sender = MessageSender {
            tx: frame_tx,
            source,
            run_loop: main_run_loop,
        };
        let handle = thread::spawn(move || {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run(config, screen, event_rx, sender)
            }))
            .is_err()
            {
                recovery::restore_all();
            }
        });
        Self { handle }
    }

    pub(super) fn join(self) {
        self.handle.join().ok();
    }
}

fn run(mut config: Config, screen: Dimension, rx: Receiver<HubEvent>, sender: MessageSender) {
    let mut hub = Hub::new(
        screen,
        config.tab_bar_height,
        config.automatic_tiling,
        config.min_width.resolve(screen.width),
        config.min_height.resolve(screen.height),
    );
    let mut registry = Registry::new();
    let mut observed_pids: HashSet<i32> = HashSet::new();

    // Initial sync of all running apps
    let mut new_apps = Vec::new();
    for app in running_apps() {
        let pid = app.processIdentifier();
        if observed_pids.insert(pid) {
            new_apps.push(app.clone());
        }
        if sync_app_windows(screen, &mut hub, &mut registry, &config, &app).is_break() {
            sender.send(HubMessage::Shutdown);
            return;
        }
    }
    if !new_apps.is_empty() {
        sender.send(HubMessage::RegisterObservers(new_apps));
    }
    let previous_displayed: HashSet<_> = HashSet::new();
    process_frame(&mut hub, &registry, &config, None, previous_displayed);

    let overlays = build_overlays(&hub, &registry, &config);
    sender.send(HubMessage::Overlays(overlays));

    while let Ok(event) = rx.recv() {
        let last_focus = hub.get_workspace(hub.current_workspace()).focused();
        let previous_displayed: HashSet<_> = get_displayed_cg_ids(&hub, &registry);

        match event {
            HubEvent::Shutdown => break,
            HubEvent::ConfigChanged(new_config) => {
                hub.sync_config(
                    new_config.tab_bar_height,
                    new_config.automatic_tiling,
                    new_config.min_width.resolve(screen.width),
                    new_config.min_height.resolve(screen.height),
                );
                config = new_config;
                tracing::info!("Config reloaded");
            }
            HubEvent::SyncApp { pid } => {
                if let Some(app) = get_app_by_pid(pid)
                    && sync_app_windows(screen, &mut hub, &mut registry, &config, &app).is_break()
                {
                    break;
                }
            }
            HubEvent::SyncFocus { pid } => {
                if let Some(app) = get_app_by_pid(pid) {
                    sync_app_focus(&mut hub, &registry, &app);
                }
            }
            HubEvent::AppTerminated { pid } => {
                for (cg_id, wt) in registry.remove_by_pid(pid) {
                    recovery::untrack(cg_id);
                    match wt {
                        WindowType::Tiling(id) => hub.delete_window(id),
                        WindowType::Float(id) => hub.delete_float(id),
                    }
                }
            }
            HubEvent::TitleChanged(cg_id) => {
                if let Some(window) = registry.get_mut(cg_id) {
                    window.update_title();
                }
            }
            HubEvent::Action(actions) => {
                tracing::debug!(%actions, "Executing actions");
                if execute_actions(&mut hub, &mut registry, &actions).is_break() {
                    tracing::debug!("Exiting hub thread");
                    break;
                }
            }
            // AX notifications are unreliable, when new windows are being rapidly created and
            // deleted, macOS may decide skip sending notifications. So we poll periodically to
            // keep the state in sync. https://github.com/nikitabobko/AeroSpace/issues/445
            HubEvent::Sync => {
                let running: Vec<_> = running_apps().collect();
                let running_pids: HashSet<_> =
                    running.iter().map(|app| app.processIdentifier()).collect();

                // Cleanup terminated apps
                let terminated: Vec<_> = observed_pids
                    .iter()
                    .filter(|pid| !running_pids.contains(pid))
                    .copied()
                    .collect();
                for pid in terminated {
                    observed_pids.remove(&pid);
                    for (cg_id, wt) in registry.remove_by_pid(pid) {
                        recovery::untrack(cg_id);
                        match wt {
                            WindowType::Tiling(id) => hub.delete_window(id),
                            WindowType::Float(id) => hub.delete_float(id),
                        }
                    }
                }

                // Sync running apps
                let mut should_exit = false;
                let mut new_apps = Vec::new();
                for app in running {
                    let pid = app.processIdentifier();
                    if observed_pids.insert(pid) {
                        new_apps.push(app.clone());
                    }
                    if sync_app_windows(screen, &mut hub, &mut registry, &config, &app).is_break() {
                        should_exit = true;
                        break;
                    }
                }
                if !new_apps.is_empty() {
                    sender.send(HubMessage::RegisterObservers(new_apps));
                }
                if should_exit {
                    break;
                }
            }
        }

        process_frame(&mut hub, &registry, &config, last_focus, previous_displayed);
        let overlays = build_overlays(&hub, &registry, &config);
        sender.send(HubMessage::Overlays(overlays));
    }

    recovery::restore_all();
    sender.send(HubMessage::Shutdown);
}

/// Sync windows for an app.
fn sync_app_windows(
    screen: Dimension,
    hub: &mut Hub,
    registry: &mut Registry,
    config: &Config,
    app: &NSRunningApplication,
) -> ControlFlow<()> {
    let pid = app.processIdentifier();
    let cg_window_ids = list_cg_window_ids();

    // Remove invalid windows
    let tracked_cg_ids = registry.cg_ids_for_pid(pid);
    if app.isHidden() {
        for cg_id in tracked_cg_ids {
            if let Some(wt) = registry.remove(cg_id) {
                recovery::untrack(cg_id);
                match wt {
                    WindowType::Tiling(id) => hub.delete_window(id),
                    WindowType::Float(id) => hub.delete_float(id),
                }
            }
        }
        return ControlFlow::Continue(());
    }
    for cg_id in tracked_cg_ids {
        if cg_window_ids.contains(&cg_id) && registry.is_valid(cg_id) {
            continue;
        }
        if let Some(wt) = registry.remove(cg_id) {
            recovery::untrack(cg_id);
            match wt {
                WindowType::Tiling(id) => hub.delete_window(id),
                WindowType::Float(id) => hub.delete_float(id),
            }
        }
    }

    // Add new windows
    for (cg_id, ax_element) in get_ax_windows(pid) {
        if registry.contains(cg_id) {
            continue;
        }

        let window = MacWindow::new(ax_element, cg_id, screen, app);
        if !window.is_manageable() {
            continue;
        }
        if should_ignore(&window, &config.macos.ignore) {
            continue;
        }

        let dimension = window.get_dimension();
        let window_type = if window.should_tile() {
            WindowType::Tiling(hub.insert_tiling())
        } else {
            WindowType::Float(hub.insert_float(dimension))
        };

        recovery::track(cg_id, window.clone(), screen);
        registry.insert(window, window_type);
        tracing::info!(cg_id, "Window inserted");

        let window = registry.get(cg_id).unwrap();
        if let Some(actions) = on_open_actions(window, &config.macos.on_open)
            && execute_actions(hub, registry, &actions).is_break()
        {
            return ControlFlow::Break(());
        }
    }

    ControlFlow::Continue(())
}

/// Sync focus for an app. ONLY UPDATES HUB FOCUS IF APP IS FRONTMOST.
fn sync_app_focus(hub: &mut Hub, registry: &Registry, app: &NSRunningApplication) {
    if !app.isActive() {
        return;
    }
    let pid = app.processIdentifier();
    if let Some(cg_id) = get_focused_window_cg_id(pid)
        && let Some(wt) = registry.get_window_type(cg_id)
    {
        match wt {
            WindowType::Tiling(id) => hub.set_focus(id),
            WindowType::Float(id) => hub.set_float_focus(id),
        }
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

#[tracing::instrument(skip(hub, registry))]
fn execute_actions(hub: &mut Hub, registry: &mut Registry, actions: &Actions) -> ControlFlow<()> {
    for action in actions {
        match action {
            Action::Focus { target } => match target {
                FocusTarget::Up => hub.focus_up(),
                FocusTarget::Down => hub.focus_down(),
                FocusTarget::Left => hub.focus_left(),
                FocusTarget::Right => hub.focus_right(),
                FocusTarget::Parent => hub.focus_parent(),
                FocusTarget::NextTab => hub.focus_next_tab(),
                FocusTarget::PrevTab => hub.focus_prev_tab(),
                FocusTarget::Workspace { index } => hub.focus_workspace(*index),
            },
            Action::Move { target } => match target {
                MoveTarget::Up => hub.move_up(),
                MoveTarget::Down => hub.move_down(),
                MoveTarget::Left => hub.move_left(),
                MoveTarget::Right => hub.move_right(),
                MoveTarget::Workspace { index } => hub.move_focused_to_workspace(*index),
            },
            Action::Toggle { target } => match target {
                ToggleTarget::SpawnDirection => hub.toggle_spawn_mode(),
                ToggleTarget::Direction => hub.toggle_direction(),
                ToggleTarget::Layout => hub.toggle_container_layout(),
                ToggleTarget::Float => {
                    if let Some((window_id, float_id)) = hub.toggle_float() {
                        registry.toggle_float(window_id, float_id);
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
            Action::Exit => return ControlFlow::Break(()),
        }
    }
    ControlFlow::Continue(())
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

fn process_frame(
    hub: &mut Hub,
    registry: &Registry,
    config: &Config,
    last_focus: Option<Focus>,
    previous_displayed: HashSet<CGWindowID>,
) {
    let border = config.border_size;
    let screen = hub.screen();

    let current_displayed = get_displayed_cg_ids(hub, registry);

    // Hide windows no longer displayed
    for cg_id in previous_displayed.difference(&current_displayed) {
        if let Some(window) = registry.get(*cg_id)
            && let Err(e) = window.hide()
        {
            tracing::trace!("Failed to hide window: {e:#}");
        }
    }

    // Position tiling windows and discover min sizes. When a window enforces its min size,
    // it can push siblings which may then hit their own min sizes. Loop until stable, with
    // a cap of 64 iterations (typical workspaces have fewer than 10 windows).
    for _ in 0..64 {
        let min_sizes = position_tiling_windows(hub, registry, border, screen);
        if min_sizes.is_empty() {
            break;
        }
        for (id, w, h) in min_sizes {
            hub.set_min_size(id, w, h);
        }
    }

    // Position float windows
    let ws = hub.get_workspace(hub.current_workspace());
    let focused = ws.focused();
    let float_windows: Vec<_> = ws.float_windows().to_vec();
    for float_id in float_windows {
        if let Some(cg_id) = registry.get_cg_id(WindowType::Float(float_id))
            && let Some(window) = registry.get(cg_id)
        {
            let dim = apply_inset(hub.get_float(float_id).dimension(), border);
            if let Err(e) = window.set_dimension(dim) {
                tracing::trace!("Failed to set float dimension: {e:#}");
            }
        }
    }

    // Focus window if changed
    if focused != last_focus {
        let focus_cg_id = match focused {
            Some(Focus::Tiling(Child::Window(id))) => registry.get_cg_id(WindowType::Tiling(id)),
            Some(Focus::Float(id)) => registry.get_cg_id(WindowType::Float(id)),
            _ => None,
        };
        if let Some(cg_id) = focus_cg_id
            && let Some(window) = registry.get(cg_id)
            && let Err(e) = window.focus()
        {
            tracing::trace!("Failed to focus window: {e:#}");
        }
    }
}

/// Position tiling windows, returns discovered min sizes
fn position_tiling_windows(
    hub: &Hub,
    registry: &Registry,
    border: f32,
    screen: Dimension,
) -> Vec<(WindowId, f32, f32)> {
    let mut min_sizes = Vec::new();
    let ws = hub.get_workspace(hub.current_workspace());
    let mut stack: Vec<Child> = ws.root().into_iter().collect();

    while let Some(child) = stack.pop() {
        match child {
            Child::Window(id) => {
                if let Some(cg_id) = registry.get_cg_id(WindowType::Tiling(id))
                    && let Some(window) = registry.get(cg_id)
                {
                    let dim = apply_inset(hub.get_window(id).dimension(), border);
                    if is_completely_offscreen(dim, screen) {
                        if let Err(e) = window.hide() {
                            tracing::trace!("Failed to hide offscreen window: {e:#}");
                        }
                    } else if let Err(e) = window.set_dimension(dim) {
                        tracing::trace!("Failed to set dimension: {e:#}");
                    } else if let Ok((actual_w, actual_h)) = window.get_size() {
                        // Min size discovery: check if window resized itself larger
                        const EPSILON: f32 = 1.0;
                        // Add border back since frame dimensions have border inset applied. If in the
                        // original frame, window width is smaller than sum of borders, then we will
                        // request a size that can accommodate the borders here
                        let discovered_w = if actual_w > dim.width + EPSILON {
                            actual_w + 2.0 * border
                        } else {
                            0.0
                        };
                        let discovered_h = if actual_h > dim.height + EPSILON {
                            actual_h + 2.0 * border
                        } else {
                            0.0
                        };
                        if discovered_w > 0.0 || discovered_h > 0.0 {
                            min_sizes.push((id, discovered_w, discovered_h));
                        }
                    }
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
    min_sizes
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

fn is_completely_offscreen(dim: Dimension, screen: Dimension) -> bool {
    dim.x + dim.width <= screen.x
        || dim.x >= screen.x + screen.width
        || dim.y + dim.height <= screen.y
        || dim.y >= screen.y + screen.height
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
