use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_QUIT};

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Color, Config, WindowsOnOpenRule, WindowsWindow};
use crate::core::{Child, Container, Dimension, Hub, MonitorId, SpawnMode, WindowId};

use super::recovery;
use super::window::{Taskbar, WindowHandle, enum_windows, get_size_constraints};
use super::{ScreenInfo, compute_global_bounds};

pub(super) const WM_APP_FRAME: u32 = 0x8000;

/// Hashable key for window lookups
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct WindowKey(isize);

impl From<HWND> for WindowKey {
    fn from(hwnd: HWND) -> Self {
        Self(hwnd.0 as isize)
    }
}

impl From<&WindowHandle> for WindowKey {
    fn from(handle: &WindowHandle) -> Self {
        Self(handle.hwnd().0 as isize)
    }
}

#[expect(
    clippy::large_enum_variant,
    reason = "These messages aren't bottleneck right now"
)]
pub(super) enum HubEvent {
    AppInitialized(AppHandle),
    WindowCreated(WindowHandle),
    WindowDestroyed(WindowHandle),
    WindowFocused(WindowHandle),
    WindowTitleChanged(WindowHandle),
    WindowMovedOrResized(WindowHandle),
    ScreensChanged(Vec<ScreenInfo>),
    Action(Actions),
    ConfigChanged(Config),
    Shutdown,
}

#[derive(Clone, Copy)]
pub(super) struct AppHandle(HWND);

impl AppHandle {
    pub(super) fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }

    pub(super) fn hwnd(self) -> HWND {
        self.0
    }
}

unsafe impl Send for AppHandle {}

#[derive(Clone)]
pub(super) struct OverlayRect {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) color: Color,
}

#[derive(Clone)]
pub(super) struct OverlayLabel {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) text: String,
    pub(super) color: Color,
    pub(super) bold: bool,
}

#[derive(Default)]
pub(super) struct Overlays {
    pub(super) rects: Vec<OverlayRect>,
    pub(super) labels: Vec<OverlayLabel>,
}

struct Registry {
    windows: HashMap<WindowKey, WindowId>,
    reverse: HashMap<WindowId, WindowHandle>,
}

impl Registry {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    fn insert(&mut self, handle: WindowHandle, id: WindowId) {
        self.windows.insert(WindowKey::from(&handle), id);
        self.reverse.insert(id, handle);
    }

    fn remove(&mut self, handle: &WindowHandle) -> Option<WindowId> {
        let key = WindowKey::from(handle);
        if let Some(id) = self.windows.remove(&key) {
            self.reverse.remove(&id);
            return Some(id);
        }
        None
    }

    fn get_id(&self, handle: &WindowHandle) -> Option<WindowId> {
        self.windows.get(&WindowKey::from(handle)).copied()
    }

    fn get_handle(&self, id: WindowId) -> Option<WindowHandle> {
        self.reverse.get(&id).cloned()
    }

    fn get_handle_by_key(&self, key: WindowKey) -> Option<WindowHandle> {
        if let Some(&id) = self.windows.get(&key) {
            return self.reverse.get(&id).cloned();
        }
        None
    }

    fn contains(&self, handle: &WindowHandle) -> bool {
        self.windows.contains_key(&WindowKey::from(handle))
    }

    fn update_title(&mut self, handle: &WindowHandle) {
        let key = WindowKey::from(handle);
        if let Some(&id) = self.windows.get(&key) {
            self.reverse.insert(id, handle.clone());
        }
    }
}

struct MonitorRegistry {
    map: HashMap<isize, MonitorId>,
    reverse: HashMap<MonitorId, isize>,
    primary_handle: isize,
}

impl MonitorRegistry {
    fn new(primary_handle: isize, primary_monitor_id: MonitorId) -> Self {
        let mut map = HashMap::new();
        let mut reverse = HashMap::new();
        map.insert(primary_handle, primary_monitor_id);
        reverse.insert(primary_monitor_id, primary_handle);
        Self {
            map,
            reverse,
            primary_handle,
        }
    }

    fn insert(&mut self, handle: isize, monitor_id: MonitorId) {
        self.map.insert(handle, monitor_id);
        self.reverse.insert(monitor_id, handle);
    }

    fn remove_by_id(&mut self, monitor_id: MonitorId) {
        if let Some(handle) = self.reverse.remove(&monitor_id) {
            self.map.remove(&handle);
        }
    }
}

pub(super) struct Dome {
    hub: Hub,
    registry: Registry,
    monitor_registry: MonitorRegistry,
    taskbar: Taskbar,
    config: Config,
    /// Primary screen dimension for crash recovery
    primary_screen: Dimension,
    /// Bounding box of all monitors for hiding windows offscreen
    global_bounds: Dimension,
    app_hwnd: Option<AppHandle>,
    running: bool,
}

impl Dome {
    pub(super) fn new(config: Config, screens: Vec<ScreenInfo>, global_bounds: Dimension) -> Self {
        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_registry = MonitorRegistry::new(primary.handle, primary_monitor_id);
        tracing::info!(
            name = %primary.name,
            handle = ?primary.handle,
            dimension = ?primary.dimension,
            "Primary monitor"
        );

        for screen in &screens {
            if screen.handle != primary.handle {
                let id = hub.add_monitor(screen.name.clone(), screen.dimension);
                monitor_registry.insert(screen.handle, id);
                tracing::info!(
                    name = %screen.name,
                    handle = ?screen.handle,
                    dimension = ?screen.dimension,
                    "Monitor"
                );
            }
        }

        Self {
            hub,
            registry: Registry::new(),
            monitor_registry,
            taskbar: Taskbar::new().expect("Failed to create taskbar"),
            config,
            primary_screen: primary.dimension,
            global_bounds,
            app_hwnd: None,
            running: true,
        }
    }

    fn update_screens(&mut self, screens: Vec<ScreenInfo>) {
        if screens.is_empty() {
            tracing::warn!("Empty screen list, skipping update");
            return;
        }
        if let Some(primary) = screens.iter().find(|s| s.is_primary) {
            self.primary_screen = primary.dimension;
        }
        self.global_bounds = compute_global_bounds(&screens);
        reconcile_monitors(&mut self.hub, &mut self.monitor_registry, screens);
    }

    pub(super) fn run(mut self, rx: Receiver<HubEvent>) {
        self.enumerate_windows();
        while self.running {
            if let Ok(event) = rx.recv() {
                self.handle_event(event);
            }
        }
    }

    fn enumerate_windows(&mut self) {
        if let Err(e) = enum_windows(|hwnd| {
            let handle = WindowHandle::new(hwnd);
            if handle.is_manageable() && !should_ignore(&handle, &self.config.windows.ignore) {
                self.insert_window(&handle);
            }
        }) {
            tracing::warn!("Failed to enumerate windows: {e}");
        }
    }

    fn handle_event(&mut self, event: HubEvent) {
        let last_focus = self
            .hub
            .get_workspace(self.hub.current_workspace())
            .focused();
        let previous_displayed: HashSet<_> = get_displayed_windows(&self.hub, &self.registry)
            .into_iter()
            .map(|(handle, _)| WindowKey::from(&handle))
            .collect();

        match event {
            HubEvent::AppInitialized(hwnd) => {
                self.app_hwnd = Some(hwnd);
                let overlays = build_overlays(&self.hub, &self.registry, &self.config);
                send_overlays(overlays, hwnd);
            }
            HubEvent::Shutdown => self.running = false,
            HubEvent::ConfigChanged(new_config) => {
                self.hub.sync_config(new_config.clone().into());
                self.config = new_config;
                tracing::info!("Config reloaded");
            }
            HubEvent::WindowCreated(handle) => {
                let _span = tracing::info_span!("window_created", %handle).entered();
                if self.registry.contains(&handle) {
                    return;
                }
                if should_ignore(&handle, &self.config.windows.ignore) {
                    return;
                }
                self.insert_window(&handle);
                if let Some(actions) = on_open_actions(&handle, &self.config.windows.on_open) {
                    self.execute_actions(&actions);
                }
            }
            HubEvent::WindowDestroyed(handle) => {
                let _span = tracing::info_span!("window_destroyed", %handle).entered();
                self.remove_window(&handle);
            }
            HubEvent::WindowFocused(handle) => {
                let _span = tracing::info_span!("window_focused", %handle).entered();
                if let Some(id) = self.registry.get_id(&handle) {
                    self.hub.set_focus(id);
                    tracing::info!("Window focused");
                }
            }
            // TODO: update float window position in hub instead of re-rendering
            HubEvent::WindowMovedOrResized(_) => {}
            HubEvent::WindowTitleChanged(handle) => {
                let _span = tracing::info_span!("window_title_changed", %handle).entered();
                if self.registry.contains(&handle) {
                    self.registry.update_title(&handle);
                    return;
                }
                // Some apps have a brief moment where their title is empty
                if should_ignore(&handle, &self.config.windows.ignore) {
                    return;
                }
                self.insert_window(&handle);
                if let Some(actions) = on_open_actions(&handle, &self.config.windows.on_open) {
                    self.execute_actions(&actions);
                }
            }
            HubEvent::ScreensChanged(screens) => {
                tracing::info!(count = screens.len(), "Screen parameters changed");
                self.update_screens(screens);
            }
            HubEvent::Action(actions) => {
                self.execute_actions(&actions);
            }
        }

        self.process_frame(last_focus, previous_displayed);
    }

    fn insert_window(&mut self, handle: &WindowHandle) {
        recovery::track(handle);
        let id = if handle.should_tile() {
            self.hub.insert_tiling()
        } else {
            self.hub.insert_float(handle.dimension())
        };
        self.registry.insert(handle.clone(), id);
        tracing::info!("Window inserted");
    }

    fn remove_window(&mut self, handle: &WindowHandle) {
        if let Some(id) = self.registry.remove(handle) {
            recovery::untrack(handle);
            self.hub.delete_window(id);
            tracing::info!("Window deleted");
        }
    }

    #[tracing::instrument(skip(self), fields(actions = %actions))]
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
                    if let Err(e) = std::process::Command::new("cmd")
                        .args(["/C", command])
                        .spawn()
                    {
                        tracing::warn!(%command, "Failed to exec: {e}");
                    }
                }
                Action::Exit => {
                    if let Some(hwnd) = self.app_hwnd {
                        unsafe {
                            PostMessageW(Some(hwnd.hwnd()), WM_QUIT, WPARAM(0), LPARAM(0)).ok()
                        };
                    }
                }
            }
        }
    }

    fn process_frame(&mut self, last_focus: Option<Child>, previous_displayed: HashSet<WindowKey>) {
        let border = self.config.border_size;

        let tiling_windows: Vec<_> = get_tiling_windows(&self.hub, &self.registry)
            .into_iter()
            .map(|(handle, dim)| (handle, apply_inset(dim, border)))
            .collect();

        let float_windows: Vec<_> = get_float_windows(&self.hub, &self.registry)
            .into_iter()
            .map(|(handle, dim)| (handle, apply_inset(dim, border)))
            .collect();

        let current: HashSet<_> = tiling_windows
            .iter()
            .chain(float_windows.iter())
            .map(|(h, _)| WindowKey::from(h))
            .collect();

        for key in &previous_displayed {
            if !current.contains(key)
                && let Some(handle) = self.registry.get_handle_by_key(*key)
            {
                handle.hide();
                self.taskbar.delete_tab(handle.hwnd()).ok();
            }
        }

        for (handle, dim) in &tiling_windows {
            self.check_and_set_constraints(handle, dim);
            handle.set_position(dim);
            self.taskbar.add_tab(handle.hwnd()).ok();
        }

        for (handle, dim) in &float_windows {
            self.check_and_set_constraints(handle, dim);
            handle.set_position(dim);
            handle.set_topmost();
            self.taskbar.add_tab(handle.hwnd()).ok();
        }

        let ws = self.hub.get_workspace(self.hub.current_workspace());
        if ws.focused() != last_focus
            && let Some(Child::Window(id)) = ws.focused()
            && let Some(handle) = self.registry.get_handle(id)
        {
            handle.focus();
        }

        if let Some(app_hwnd) = self.app_hwnd {
            let overlays = build_overlays(&self.hub, &self.registry, &self.config);
            send_overlays(overlays, app_hwnd);
        }
    }

    fn check_and_set_constraints(&mut self, handle: &WindowHandle, dim: &Dimension) {
        let (min_w, min_h, max_w, max_h) = get_size_constraints(handle.hwnd());
        let min_w = if min_w > dim.width { min_w } else { 0.0 };
        let min_h = if min_h > dim.height { min_h } else { 0.0 };
        let max_w = if max_w > 0.0 && max_w < dim.width {
            max_w
        } else {
            0.0
        };
        let max_h = if max_h > 0.0 && max_h < dim.height {
            max_h
        } else {
            0.0
        };
        if min_w > 0.0 || min_h > 0.0 || max_w > 0.0 || max_h > 0.0 {
            let border = self.config.border_size;
            let to_opt = |v: f32| {
                if v > 0.0 {
                    Some(v + 2.0 * border)
                } else {
                    None
                }
            };
            if let Some(id) = self.registry.get_id(handle) {
                self.hub.set_window_constraint(
                    id,
                    to_opt(min_w),
                    to_opt(min_h),
                    to_opt(max_w),
                    to_opt(max_h),
                );
            }
        }
    }
}

impl Drop for Dome {
    fn drop(&mut self) {
        recovery::restore_all();
    }
}

fn send_overlays(overlays: Overlays, app_hwnd: AppHandle) {
    let cmd = Box::new(overlays);
    let ptr = Box::into_raw(cmd) as usize;
    unsafe { PostMessageW(Some(app_hwnd.hwnd()), WM_APP_FRAME, WPARAM(ptr), LPARAM(0)).ok() };
}

fn get_tiling_windows(hub: &Hub, registry: &Registry) -> Vec<(WindowHandle, Dimension)> {
    let mut windows = Vec::new();

    for ws_id in hub.visible_workspaces() {
        let ws = hub.get_workspace(ws_id);
        let mut stack: Vec<Child> = ws.root().into_iter().collect();
        while let Some(child) = stack.pop() {
            match child {
                Child::Window(id) => {
                    if let Some(handle) = registry.get_handle(id) {
                        windows.push((handle, hub.get_window(id).dimension()));
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

    windows
}

fn get_float_windows(hub: &Hub, registry: &Registry) -> Vec<(WindowHandle, Dimension)> {
    let mut windows = Vec::new();
    for ws_id in hub.visible_workspaces() {
        let ws = hub.get_workspace(ws_id);
        for &id in ws.float_windows() {
            if let Some(handle) = registry.get_handle(id) {
                windows.push((handle, hub.get_window(id).dimension()));
            }
        }
    }
    windows
}

fn get_displayed_windows(hub: &Hub, registry: &Registry) -> Vec<(WindowHandle, Dimension)> {
    let mut windows = get_tiling_windows(hub, registry);
    windows.extend(get_float_windows(hub, registry));
    windows
}

fn build_overlays(hub: &Hub, registry: &Registry, config: &Config) -> Overlays {
    let ws = hub.get_workspace(hub.current_workspace());
    let border = config.border_size;
    let focused = ws.focused();

    let mut rects = Vec::new();
    let mut labels = Vec::new();

    let mut stack: Vec<Child> = ws.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(id) => {
                if registry.get_handle(id).is_some() && focused != Some(Child::Window(id)) {
                    let dim = hub.get_window(id).dimension();
                    rects.extend(border_rects(dim, border, [config.border_color; 4]));
                }
            }
            Child::Container(id) => {
                let container = hub.get_container(id);
                if let Some(active) = container.active_tab() {
                    stack.push(active);
                    let is_focused = focused == Some(Child::Container(id));
                    let (tab_rects, tab_labels) =
                        build_tab_bar(container, registry, config, is_focused);
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
        Some(Child::Window(id)) => {
            let w = hub.get_window(id);
            if w.is_float() {
                rects.extend(border_rects(
                    w.dimension(),
                    border,
                    [config.focused_color; 4],
                ));
            } else {
                rects.extend(border_rects(
                    w.dimension(),
                    border,
                    spawn_colors(w.spawn_mode(), config),
                ));
            }
        }
        Some(Child::Container(id)) => {
            let c = hub.get_container(id);
            rects.extend(border_rects(
                c.dimension(),
                border,
                spawn_colors(c.spawn_mode(), config),
            ));
        }
        None => {}
    }

    for &float_id in ws.float_windows() {
        if registry.get_handle(float_id).is_some() && focused != Some(Child::Window(float_id)) {
            let dim = hub.get_window(float_id).dimension();
            rects.extend(border_rects(dim, border, [config.border_color; 4]));
        }
    }

    Overlays { rects, labels }
}

fn build_tab_bar(
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
        y: dim.y,
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
    rects.extend(border_rects(tab_dim, border, [tab_color; 4]));

    let children = container.children();
    if children.is_empty() {
        return (rects, Vec::new());
    }

    let tab_width = dim.width / children.len() as f32;
    let active_tab = container.active_tab_index();

    rects.push(OverlayRect {
        x: dim.x + active_tab as f32 * tab_width,
        y: dim.y,
        width: tab_width,
        height,
        color: config.active_tab_background_color,
    });

    for i in 1..children.len() {
        rects.push(OverlayRect {
            x: dim.x + i as f32 * tab_width - border / 2.0,
            y: dim.y,
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
                Child::Window(wid) => registry
                    .get_handle(*wid)
                    .and_then(|h| h.title().map(|s| s.to_owned()))
                    .unwrap_or_else(|| "Unknown".to_owned()),
                Child::Container(_) => "Container".to_owned(),
            };
            let is_active = i == active_tab;
            let text = if is_active {
                format!("[{title}]")
            } else {
                title
            };
            let x = dim.x + i as f32 * tab_width + tab_width / 2.0 - text.len() as f32 * 3.5;
            OverlayLabel {
                x,
                y: dim.y + height / 2.0 - 6.0,
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

fn apply_inset(dim: Dimension, border: f32) -> Dimension {
    Dimension {
        x: dim.x + border,
        y: dim.y + border,
        width: (dim.width - 2.0 * border).max(0.0),
        height: (dim.height - 2.0 * border).max(0.0),
    }
}

fn border_rects(dim: Dimension, border: f32, colors: [Color; 4]) -> [OverlayRect; 4] {
    [
        OverlayRect {
            x: dim.x,
            y: dim.y,
            width: dim.width,
            height: border,
            color: colors[0],
        },
        OverlayRect {
            x: dim.x,
            y: dim.y + dim.height - border,
            width: dim.width,
            height: border,
            color: colors[1],
        },
        OverlayRect {
            x: dim.x,
            y: dim.y + border,
            width: border,
            height: dim.height - 2.0 * border,
            color: colors[2],
        },
        OverlayRect {
            x: dim.x + dim.width - border,
            y: dim.y + border,
            width: border,
            height: dim.height - 2.0 * border,
            color: colors[3],
        },
    ]
}

fn on_open_actions(handle: &WindowHandle, rules: &[WindowsOnOpenRule]) -> Option<Actions> {
    let rule = rules
        .iter()
        .find(|r| r.window.matches(handle.process(), handle.title()))?;
    tracing::debug!(%handle, actions = %rule.run, "Running on_open actions");
    Some(rule.run.clone())
}

fn should_ignore(handle: &WindowHandle, rules: &[WindowsWindow]) -> bool {
    if !handle.is_manageable() {
        tracing::debug!(%handle, "Window ignored: not manageable");
        return true;
    }
    let matched = rules
        .iter()
        .find(|r| r.matches(handle.process(), handle.title()));
    if let Some(rule) = matched {
        tracing::debug!(%handle, ?rule, "Window ignored by rule");
        return true;
    }
    false
}

fn reconcile_monitors(hub: &mut Hub, registry: &mut MonitorRegistry, screens: Vec<ScreenInfo>) {
    if screens.is_empty() {
        return;
    }

    if let Some(new_primary) = screens.iter().find(|s| s.is_primary) {
        registry.primary_handle = new_primary.handle;
    }

    let current_keys: HashSet<_> = screens.iter().map(|s| s.handle).collect();

    // Add new monitors first
    for screen in &screens {
        if !registry.map.contains_key(&screen.handle) {
            let id = hub.add_monitor(screen.name.clone(), screen.dimension);
            registry.insert(screen.handle, id);
            tracing::info!(
                name = %screen.name,
                handle = ?screen.handle,
                dimension = ?screen.dimension,
                "Monitor added"
            );
        }
    }

    // Remove monitors that no longer exist
    let to_remove: Vec<_> = registry
        .map
        .iter()
        .filter(|(key, _)| !current_keys.contains(key))
        .map(|(_, &id)| id)
        .collect();

    let fallback_id = registry.map.get(&registry.primary_handle).copied();
    for monitor_id in to_remove {
        if let Some(fallback) = fallback_id
            && fallback != monitor_id
        {
            hub.remove_monitor(monitor_id, fallback);
            registry.remove_by_id(monitor_id);
            tracing::info!(%monitor_id, fallback = %fallback, "Monitor removed");
        }
    }

    // Update dimensions for existing monitors
    for screen in &screens {
        if let Some(&monitor_id) = registry.map.get(&screen.handle) {
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
