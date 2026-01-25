use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_QUIT};

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Color, Config, WindowsOnOpenRule, WindowsWindow};
use crate::core::{Child, Container, Dimension, FloatWindowId, Focus, Hub, SpawnMode, WindowId};

use super::recovery;
use super::window::{Taskbar, WindowHandle, enum_windows, get_size_constraints};

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
    tiling: HashMap<WindowKey, WindowId>,
    float: HashMap<WindowKey, FloatWindowId>,
    tiling_rev: HashMap<WindowId, WindowHandle>,
    float_rev: HashMap<FloatWindowId, WindowHandle>,
}

impl Registry {
    fn new() -> Self {
        Self {
            tiling: HashMap::new(),
            float: HashMap::new(),
            tiling_rev: HashMap::new(),
            float_rev: HashMap::new(),
        }
    }

    fn insert_tiling(&mut self, handle: WindowHandle, id: WindowId) {
        self.tiling.insert(WindowKey::from(&handle), id);
        self.tiling_rev.insert(id, handle);
    }

    fn insert_float(&mut self, handle: WindowHandle, id: FloatWindowId) {
        self.float.insert(WindowKey::from(&handle), id);
        self.float_rev.insert(id, handle);
    }

    fn remove(&mut self, handle: &WindowHandle) -> Option<WindowType> {
        let key = WindowKey::from(handle);
        if let Some(id) = self.tiling.remove(&key) {
            self.tiling_rev.remove(&id);
            return Some(WindowType::Tiling(id));
        }
        if let Some(id) = self.float.remove(&key) {
            self.float_rev.remove(&id);
            return Some(WindowType::Float(id));
        }
        None
    }

    fn get_tiling(&self, handle: &WindowHandle) -> Option<WindowId> {
        self.tiling.get(&WindowKey::from(handle)).copied()
    }

    fn get_float(&self, handle: &WindowHandle) -> Option<FloatWindowId> {
        self.float.get(&WindowKey::from(handle)).copied()
    }

    fn get_handle(&self, id: WindowId) -> Option<WindowHandle> {
        self.tiling_rev.get(&id).cloned()
    }

    fn get_float_handle(&self, id: FloatWindowId) -> Option<WindowHandle> {
        self.float_rev.get(&id).cloned()
    }

    fn get_handle_by_key(&self, key: WindowKey) -> Option<WindowHandle> {
        if let Some(&id) = self.tiling.get(&key) {
            return self.tiling_rev.get(&id).cloned();
        }
        if let Some(&id) = self.float.get(&key) {
            return self.float_rev.get(&id).cloned();
        }
        None
    }

    fn contains(&self, handle: &WindowHandle) -> bool {
        let key = WindowKey::from(handle);
        self.tiling.contains_key(&key) || self.float.contains_key(&key)
    }

    fn update_title(&mut self, handle: &WindowHandle) {
        let key = WindowKey::from(handle);
        if let Some(&id) = self.tiling.get(&key) {
            self.tiling_rev.insert(id, handle.clone());
        } else if let Some(&id) = self.float.get(&key) {
            self.float_rev.insert(id, handle.clone());
        }
    }

    fn toggle(&mut self, window_id: WindowId, float_id: FloatWindowId) {
        if let Some(handle) = self.tiling_rev.remove(&window_id) {
            let key = WindowKey::from(&handle);
            self.tiling.remove(&key);
            self.float.insert(key, float_id);
            self.float_rev.insert(float_id, handle);
        } else if let Some(handle) = self.float_rev.remove(&float_id) {
            let key = WindowKey::from(&handle);
            self.float.remove(&key);
            self.tiling.insert(key, window_id);
            self.tiling_rev.insert(window_id, handle);
        }
    }
}

enum WindowType {
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

pub(super) struct Dome {
    hub: Hub,
    registry: Registry,
    taskbar: Taskbar,
    config: Config,
    screen: Dimension,
    app_hwnd: Option<AppHandle>,
    running: bool,
}

impl Dome {
    pub(super) fn new(config: Config, screen: Dimension) -> Self {
        let hub = Hub::new(screen, config.clone().into());
        Self {
            hub,
            registry: Registry::new(),
            taskbar: Taskbar::new().expect("Failed to create taskbar"),
            config,
            screen,
            app_hwnd: None,
            running: true,
        }
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
                if let Some(id) = self.registry.get_tiling(&handle) {
                    self.hub.set_focus(id);
                    tracing::info!("Tiling window focused");
                } else if let Some(id) = self.registry.get_float(&handle) {
                    self.hub.set_float_focus(id);
                    tracing::info!("Float window focused");
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
            HubEvent::Action(actions) => {
                self.execute_actions(&actions);
            }
        }

        self.process_frame(last_focus, previous_displayed);
    }

    fn insert_window(&mut self, handle: &WindowHandle) {
        recovery::track(handle);
        if handle.should_tile() {
            let id = self.hub.insert_tiling();
            self.registry.insert_tiling(handle.clone(), id);
            tracing::info!("Tiling window inserted");
        } else {
            let id = self.hub.insert_float(handle.dimension());
            self.registry.insert_float(handle.clone(), id);
            tracing::info!("Float window inserted");
        }
    }

    fn remove_window(&mut self, handle: &WindowHandle) {
        if let Some(wt) = self.registry.remove(handle) {
            recovery::untrack(handle);
            match wt {
                WindowType::Tiling(id) => self.hub.delete_window(id),
                WindowType::Float(id) => self.hub.delete_float(id),
            }
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
                    MoveTarget::Monitor { target } => self.hub.move_to_monitor(target),
                },
                Action::Toggle { target } => match target {
                    ToggleTarget::SpawnDirection => self.hub.toggle_spawn_mode(),
                    ToggleTarget::Direction => self.hub.toggle_direction(),
                    ToggleTarget::Layout => self.hub.toggle_container_layout(),
                    ToggleTarget::Float => {
                        if let Some((window_id, float_id)) = self.hub.toggle_float() {
                            self.registry.toggle(window_id, float_id);
                        }
                    }
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

    fn process_frame(&mut self, last_focus: Option<Focus>, previous_displayed: HashSet<WindowKey>) {
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
            handle.set_position(dim);
            handle.set_topmost();
            self.taskbar.add_tab(handle.hwnd()).ok();
        }

        let ws = self.hub.get_workspace(self.hub.current_workspace());
        if ws.focused() != last_focus {
            match ws.focused() {
                Some(Focus::Tiling(Child::Window(id))) => {
                    if let Some(handle) = self.registry.get_handle(id) {
                        handle.focus();
                    }
                }
                Some(Focus::Float(id)) => {
                    if let Some(handle) = self.registry.get_float_handle(id) {
                        handle.focus();
                    }
                }
                _ => {}
            }
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
            if let Some(id) = self.registry.get_tiling(handle) {
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
    let ws = hub.get_workspace(hub.current_workspace());
    let mut windows = Vec::new();

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

    windows
}

fn get_float_windows(hub: &Hub, registry: &Registry) -> Vec<(WindowHandle, Dimension)> {
    let ws = hub.get_workspace(hub.current_workspace());
    ws.float_windows()
        .iter()
        .filter_map(|&id| {
            registry
                .get_float_handle(id)
                .map(|h| (h, hub.get_float(id).dimension()))
        })
        .collect()
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
                if registry.get_handle(id).is_some()
                    && focused != Some(Focus::Tiling(Child::Window(id)))
                {
                    let dim = hub.get_window(id).dimension();
                    rects.extend(border_rects(dim, border, [config.border_color; 4]));
                }
            }
            Child::Container(id) => {
                let container = hub.get_container(id);
                if let Some(active) = container.active_tab() {
                    stack.push(active);
                    let is_focused = focused == Some(Focus::Tiling(Child::Container(id)));
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
        Some(Focus::Tiling(Child::Window(id))) => {
            let w = hub.get_window(id);
            rects.extend(border_rects(
                w.dimension(),
                border,
                spawn_colors(w.spawn_mode(), config),
            ));
        }
        Some(Focus::Tiling(Child::Container(id))) => {
            let c = hub.get_container(id);
            rects.extend(border_rects(
                c.dimension(),
                border,
                spawn_colors(c.spawn_mode(), config),
            ));
        }
        _ => {}
    }

    for &float_id in ws.float_windows() {
        if registry.get_float_handle(float_id).is_some() {
            let dim = hub.get_float(float_id).dimension();
            let color = if focused == Some(Focus::Float(float_id)) {
                config.focused_color
            } else {
                config.border_color
            };
            rects.extend(border_rects(dim, border, [color; 4]));
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
