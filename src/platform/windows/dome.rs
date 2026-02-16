use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_QUIT};

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Color, Config, WindowsOnOpenRule, WindowsWindow};
use crate::core::{Child, ContainerId, Dimension, Hub, MonitorId, SpawnMode, WindowId, WorkspaceId};

use super::recovery;
use super::window::{Taskbar, WindowHandle, enum_windows, get_size_constraints};
use super::{ScreenInfo, compute_global_bounds};

pub(super) const WM_APP_OVERLAY: u32 = 0x8001;

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

pub(super) struct OverlayFrame {
    pub(super) creates: Vec<OverlayCreate>,
    pub(super) deletes: Vec<WindowId>,
    pub(super) focused: Option<WindowId>,
    pub(super) windows: Vec<WindowOverlay>,
    pub(super) containers: Vec<ContainerOverlay>,
}

pub(super) struct OverlayCreate {
    pub(super) window_id: WindowId,
    pub(super) hwnd: HWND,
    pub(super) is_float: bool,
}

pub(super) struct WindowOverlay {
    pub(super) window_id: WindowId,
    pub(super) frame: Dimension,
    pub(super) edges: Vec<(Dimension, Color)>,
    pub(super) is_float: bool,
}

pub(super) struct ContainerOverlay {
    pub(super) container_id: ContainerId,
    pub(super) frame: Dimension,
    pub(super) edges: Vec<(Dimension, Color)>,
    pub(super) tab_bar: Option<TabBarInfo>,
}

pub(super) struct TabBarInfo {
    pub(super) tabs: Vec<TabInfo>,
    pub(super) height: f32,
    pub(super) background_color: Color,
    pub(super) active_background_color: Color,
    pub(super) border_color: Color,
    pub(super) border: f32,
}

pub(super) struct TabInfo {
    pub(super) title: String,
    pub(super) x: f32,
    pub(super) width: f32,
    pub(super) is_active: bool,
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

struct MonitorEntry {
    id: MonitorId,
    displayed_windows: HashSet<WindowKey>,
}

struct MonitorRegistry {
    map: HashMap<isize, MonitorEntry>,
    reverse: HashMap<MonitorId, isize>,
    primary_handle: isize,
}

impl MonitorRegistry {
    fn new(primary_handle: isize, primary_monitor_id: MonitorId) -> Self {
        let mut map = HashMap::new();
        let mut reverse = HashMap::new();
        map.insert(
            primary_handle,
            MonitorEntry {
                id: primary_monitor_id,
                displayed_windows: HashSet::new(),
            },
        );
        reverse.insert(primary_monitor_id, primary_handle);
        Self {
            map,
            reverse,
            primary_handle,
        }
    }

    fn insert(&mut self, handle: isize, monitor_id: MonitorId) {
        self.map.insert(
            handle,
            MonitorEntry {
                id: monitor_id,
                displayed_windows: HashSet::new(),
            },
        );
        self.reverse.insert(monitor_id, handle);
    }

    fn remove_by_id(&mut self, monitor_id: MonitorId) {
        if let Some(handle) = self.reverse.remove(&monitor_id) {
            self.map.remove(&handle);
        }
    }

    fn get_entry_mut(&mut self, monitor_id: MonitorId) -> Option<&mut MonitorEntry> {
        self.reverse
            .get(&monitor_id)
            .and_then(|h| self.map.get_mut(h))
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
        while self.running {
            if let Ok(event) = rx.recv() {
                let last_focus = self.hub.get_workspace(self.hub.current_workspace()).focused();
                let (creates, deletes) = self.handle_event(event);
                self.process_frame(creates, deletes, last_focus);
            }
        }
    }

    fn enumerate_windows(&mut self) -> Vec<OverlayCreate> {
        let mut creates = Vec::new();
        if let Err(e) = enum_windows(|hwnd| {
            let handle = WindowHandle::new(hwnd);
            if handle.is_manageable() && !should_ignore(&handle, &self.config.windows.ignore) {
                let id = self.insert_window(&handle);
                let is_float = self.hub.get_window(id).is_float();
                creates.push(OverlayCreate { window_id: id, hwnd, is_float });
            }
        }) {
            tracing::warn!("Failed to enumerate windows: {e}");
        }
        creates
    }

    fn handle_event(&mut self, event: HubEvent) -> (Vec<OverlayCreate>, Vec<WindowId>) {
        let mut creates = Vec::new();
        let mut deletes = Vec::new();

        match event {
            HubEvent::AppInitialized(hwnd) => {
                self.app_hwnd = Some(hwnd);
                creates = self.enumerate_windows();
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
                    return (creates, deletes);
                }
                if should_ignore(&handle, &self.config.windows.ignore) {
                    return (creates, deletes);
                }
                let hwnd = handle.hwnd();
                let id = self.insert_window(&handle);
                let is_float = self.hub.get_window(id).is_float();
                creates.push(OverlayCreate { window_id: id, hwnd, is_float });
                if let Some(actions) = on_open_actions(&handle, &self.config.windows.on_open) {
                    self.execute_actions(&actions);
                }
            }
            HubEvent::WindowDestroyed(handle) => {
                let _span = tracing::info_span!("window_destroyed", %handle).entered();
                if let Some(id) = self.remove_window(&handle) {
                    deletes.push(id);
                }
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
                    return (creates, deletes);
                }
                // Some apps have a brief moment where their title is empty
                if should_ignore(&handle, &self.config.windows.ignore) {
                    return (creates, deletes);
                }
                let hwnd = handle.hwnd();
                let id = self.insert_window(&handle);
                let is_float = self.hub.get_window(id).is_float();
                creates.push(OverlayCreate { window_id: id, hwnd, is_float });
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

        (creates, deletes)
    }

    fn insert_window(&mut self, handle: &WindowHandle) -> WindowId {
        recovery::track(handle);
        let id = if handle.should_tile() {
            self.hub.insert_tiling()
        } else {
            self.hub.insert_float(handle.dimension())
        };
        self.registry.insert(handle.clone(), id);
        tracing::info!("Window inserted");
        id
    }

    fn remove_window(&mut self, handle: &WindowHandle) -> Option<WindowId> {
        if let Some(id) = self.registry.remove(handle) {
            recovery::untrack(handle);
            self.hub.delete_window(id);
            tracing::info!("Window deleted");
            return Some(id);
        }
        None
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

    fn process_frame(
        &mut self,
        creates: Vec<OverlayCreate>,
        deletes: Vec<WindowId>,
        last_focus: Option<Child>,
    ) {
        let border = self.config.border_size;
        let ws = self.hub.get_workspace(self.hub.current_workspace());
        let focused = ws.focused();

        let mut frame = OverlayFrame {
            creates,
            deletes,
            focused: match focused {
                Some(Child::Window(id)) => Some(id),
                _ => None,
            },
            windows: Vec::new(),
            containers: Vec::new(),
        };

        // Hide windows no longer displayed per monitor
        for ws_id in self.hub.visible_workspaces() {
            let monitor_id = self.hub.get_workspace(ws_id).monitor();
            let current_windows =
                get_displayed_for_workspace(&self.hub, ws_id, &self.registry);

            if let Some(entry) = self.monitor_registry.get_entry_mut(monitor_id) {
                for key in entry.displayed_windows.difference(&current_windows) {
                    if let Some(handle) = self.registry.get_handle_by_key(*key) {
                        handle.hide();
                        self.taskbar.delete_tab(handle.hwnd()).ok();
                    }
                }
                entry.displayed_windows = current_windows;
            }
        }

        // Tiling windows
        for (handle, dim) in get_tiling_windows(&self.hub, &self.registry) {
            let content_dim = apply_inset(dim, border);
            self.check_and_set_constraints(&handle, &content_dim);
            handle.set_position(&content_dim);
            self.taskbar.add_tab(handle.hwnd()).ok();

            if let Some(id) = self.registry.get_id(&handle) {
                let colors = if focused == Some(Child::Window(id)) {
                    spawn_colors(self.hub.get_window(id).spawn_mode(), &self.config)
                } else {
                    [self.config.border_color; 4]
                };
                frame.windows.push(WindowOverlay {
                    window_id: id,
                    frame: dim,
                    edges: border_edges(dim, border, colors),
                    is_float: false,
                });
            }
        }

        // Float windows
        for (handle, dim) in get_float_windows(&self.hub, &self.registry) {
            let content_dim = apply_inset(dim, border);
            self.check_and_set_constraints(&handle, &content_dim);
            handle.set_position(&content_dim);
            handle.set_topmost();
            self.taskbar.add_tab(handle.hwnd()).ok();

            if let Some(id) = self.registry.get_id(&handle) {
                let colors = if focused == Some(Child::Window(id)) {
                    [self.config.focused_color; 4]
                } else {
                    [self.config.border_color; 4]
                };
                frame.windows.push(WindowOverlay {
                    window_id: id,
                    frame: dim,
                    edges: border_edges(dim, border, colors),
                    is_float: true,
                });
            }
        }

        // Focus window if focus changed
        if focused != last_focus
            && let Some(Child::Window(id)) = focused
            && let Some(handle) = self.registry.get_handle(id)
        {
            handle.focus();
        }

        // Container borders and tab bars
        for (container_id, dim, is_tabbed) in get_containers(&self.hub) {
            let is_focused = focused == Some(Child::Container(container_id));
            if is_tabbed {
                let edges = if is_focused {
                    let colors = spawn_colors(self.hub.get_container(container_id).spawn_mode(), &self.config);
                    border_edges(dim, border, colors)
                } else {
                    vec![]
                };
                frame.containers.push(ContainerOverlay {
                    container_id,
                    frame: dim,
                    edges,
                    tab_bar: Some(build_tab_info(&self.hub, &self.registry, container_id, &self.config, is_focused)),
                });
            } else if is_focused {
                let colors = spawn_colors(self.hub.get_container(container_id).spawn_mode(), &self.config);
                frame.containers.push(ContainerOverlay {
                    container_id,
                    frame: dim,
                    edges: border_edges(dim, border, colors),
                    tab_bar: None,
                });
            }
        }

        if let Some(app_hwnd) = self.app_hwnd {
            send_overlay_frame(frame, app_hwnd);
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

fn send_overlay_frame(frame: OverlayFrame, app_hwnd: AppHandle) {
    let boxed = Box::new(frame);
    let ptr = Box::into_raw(boxed) as usize;
    unsafe { PostMessageW(Some(app_hwnd.hwnd()), WM_APP_OVERLAY, WPARAM(ptr), LPARAM(0)).ok() };
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

fn get_displayed_for_workspace(
    hub: &Hub,
    ws_id: WorkspaceId,
    registry: &Registry,
) -> HashSet<WindowKey> {
    let mut windows = HashSet::new();
    let ws = hub.get_workspace(ws_id);

    let mut stack: Vec<Child> = ws.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(id) => {
                if let Some(handle) = registry.get_handle(id) {
                    windows.insert(WindowKey::from(&handle));
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
        if let Some(handle) = registry.get_handle(float_id) {
            windows.insert(WindowKey::from(&handle));
        }
    }

    windows
}

fn get_containers(hub: &Hub) -> Vec<(ContainerId, Dimension, bool)> {
    let mut containers = Vec::new();
    for ws_id in hub.visible_workspaces() {
        let ws = hub.get_workspace(ws_id);
        let mut stack: Vec<Child> = ws.root().into_iter().collect();
        while let Some(child) = stack.pop() {
            if let Child::Container(id) = child {
                let container = hub.get_container(id);
                let is_tabbed = container.active_tab().is_some();
                containers.push((id, container.dimension(), is_tabbed));
                if is_tabbed {
                    if let Some(active) = container.active_tab() {
                        stack.push(active);
                    }
                } else {
                    for &c in container.children() {
                        stack.push(c);
                    }
                }
            }
        }
    }
    containers
}

fn build_tab_info(
    hub: &Hub,
    registry: &Registry,
    container_id: ContainerId,
    config: &Config,
    is_focused: bool,
) -> TabBarInfo {
    let container = hub.get_container(container_id);
    let dim = container.dimension();
    let children = container.children();
    let tab_width = if children.is_empty() {
        dim.width
    } else {
        dim.width / children.len() as f32
    };
    let active_tab = container.active_tab_index();

    let tabs = children
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
            TabInfo {
                title,
                x: dim.x + i as f32 * tab_width,
                width: tab_width,
                is_active: i == active_tab,
            }
        })
        .collect();

    TabBarInfo {
        tabs,
        height: config.tab_bar_height,
        background_color: config.tab_bar_background_color,
        active_background_color: config.active_tab_background_color,
        border_color: if is_focused { config.focused_color } else { config.border_color },
        border: config.border_size,
    }
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

fn border_edges(dim: Dimension, border: f32, colors: [Color; 4]) -> Vec<(Dimension, Color)> {
    vec![
        // Top
        (Dimension { x: dim.x, y: dim.y, width: dim.width, height: border }, colors[0]),
        // Bottom
        (Dimension { x: dim.x, y: dim.y + dim.height - border, width: dim.width, height: border }, colors[1]),
        // Left
        (Dimension { x: dim.x, y: dim.y + border, width: border, height: dim.height - 2.0 * border }, colors[2]),
        // Right
        (Dimension { x: dim.x + dim.width - border, y: dim.y + border, width: border, height: dim.height - 2.0 * border }, colors[3]),
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
        .map(|(_, entry)| entry.id)
        .collect();

    let fallback_id = registry.map.get(&registry.primary_handle).map(|e| e.id);
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
        if let Some(monitor_id) = registry.map.get(&screen.handle).map(|e| e.id) {
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
