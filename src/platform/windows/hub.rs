use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_QUIT};

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Color, Config, WindowsWindowRule};
use crate::core::{Child, Dimension, FloatWindowId, Focus, Hub, SpawnMode, WindowId};

use super::window::{get_process_name, get_window_title};

pub(super) const WM_APP_FRAME: u32 = 0x8000;

/// Hashable key for window lookups
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct WindowKey(isize);

impl From<HWND> for WindowKey {
    fn from(hwnd: HWND) -> Self {
        Self(hwnd.0 as isize)
    }
}

// HWND is safe to send across threads, but doesn't implement Send
// https://users.rust-lang.org/t/moving-window-hwnd-or-handle-from-one-thread-to-a-new-one/126341/2
#[derive(Clone)]
pub(super) struct WindowHandle {
    hwnd: HWND,
    title: Option<String>,
    process: String,
}

impl WindowHandle {
    pub(super) fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            title: get_window_title(hwnd),
            process: get_process_name(hwnd).unwrap_or_default(),
        }
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(super) fn process(&self) -> &str {
        &self.process
    }

    fn key(&self) -> WindowKey {
        WindowKey::from(self.hwnd)
    }
}

unsafe impl Send for WindowHandle {}

impl PartialEq for WindowHandle {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for WindowHandle {}

impl std::hash::Hash for WindowHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl std::fmt::Display for WindowHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let title = self.title().unwrap_or("<no title>");
        write!(f, "'{title}' from '{}'", self.process)
    }
}

pub(super) enum HubEvent {
    WindowCreated(WindowHandle),
    WindowDestroyed(WindowHandle),
    WindowFocused(WindowHandle),
    WindowTitleChanged(WindowHandle),
    WindowMovedOrResized(WindowHandle),
    Action(Actions),
    ConfigReloaded(Config),
    Shutdown,
}

pub(super) struct Frame {
    pub(super) windows: Vec<(WindowHandle, Dimension)>,
    pub(super) floats: Vec<(WindowHandle, Dimension)>,
    pub(super) overlays: Vec<OverlayRect>,
    pub(super) focus: Option<WindowHandle>,
}

#[derive(Clone)]
pub(super) struct OverlayRect {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) color: Color,
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
        self.tiling.insert(handle.key(), id);
        self.tiling_rev.insert(id, handle);
    }

    fn remove(&mut self, handle: &WindowHandle) -> Option<WindowType> {
        let key = handle.key();
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
        self.tiling.get(&handle.key()).copied()
    }

    fn get_float(&self, handle: &WindowHandle) -> Option<FloatWindowId> {
        self.float.get(&handle.key()).copied()
    }

    fn get_handle(&self, id: WindowId) -> Option<WindowHandle> {
        self.tiling_rev.get(&id).cloned()
    }

    fn get_float_handle(&self, id: FloatWindowId) -> Option<WindowHandle> {
        self.float_rev.get(&id).cloned()
    }

    fn contains(&self, handle: &WindowHandle) -> bool {
        let key = handle.key();
        self.tiling.contains_key(&key) || self.float.contains_key(&key)
    }

    fn update_title(&mut self, handle: &WindowHandle) {
        let key = handle.key();
        if let Some(&id) = self.tiling.get(&key) {
            self.tiling_rev.insert(id, handle.clone());
        } else if let Some(&id) = self.float.get(&key) {
            self.float_rev.insert(id, handle.clone());
        }
    }

    fn toggle(&mut self, window_id: WindowId, float_id: FloatWindowId) {
        if let Some(handle) = self.tiling_rev.remove(&window_id) {
            self.tiling.remove(&handle.key());
            self.float.insert(handle.key(), float_id);
            self.float_rev.insert(float_id, handle);
        } else if let Some(handle) = self.float_rev.remove(&float_id) {
            self.float.remove(&handle.key());
            self.tiling.insert(handle.key(), window_id);
            self.tiling_rev.insert(window_id, handle);
        }
    }
}

enum WindowType {
    Tiling(WindowId),
    Float(FloatWindowId),
}

pub(super) struct HubThread {
    sender: Sender<HubEvent>,
    handle: JoinHandle<()>,
}

impl HubThread {
    pub(super) fn spawn(config: Config, screen: Dimension, main_hwnd: WindowHandle) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || run(config, screen, rx, main_hwnd));
        Self { sender: tx, handle }
    }

    pub(super) fn sender(&self) -> Sender<HubEvent> {
        self.sender.clone()
    }

    pub(super) fn shutdown(self) {
        self.sender.send(HubEvent::Shutdown).ok();
        self.handle.join().ok();
    }
}

fn run(mut config: Config, screen: Dimension, rx: Receiver<HubEvent>, main_hwnd: WindowHandle) {
    let mut hub = Hub::new(screen, config.tab_bar_height, config.automatic_tiling);
    let mut registry = Registry::new();
    let mut last_focus: Option<Focus> = None;

    let frame = build_frame(&hub, &registry, &config, last_focus);
    last_focus = hub.get_workspace(hub.current_workspace()).focused();
    send_frame(frame, &main_hwnd);

    while let Ok(event) = rx.recv() {
        match event {
            HubEvent::Shutdown => break,
            HubEvent::ConfigReloaded(new_config) => {
                hub.sync_config(new_config.tab_bar_height, new_config.automatic_tiling);
                config = new_config;
                tracing::info!("Config reloaded");
            }
            HubEvent::WindowCreated(handle) => {
                let _span = tracing::info_span!("window_created", %handle).entered();
                if registry.contains(&handle) {
                    continue;
                }
                if !should_manage(
                    handle.process(),
                    handle.title(),
                    &config.windows.window_rules,
                ) {
                    continue;
                }
                let id = hub.insert_tiling();
                registry.insert_tiling(handle.clone(), id);
                tracing::info!("Window inserted");
                if let Some(rule) = match_rule(
                    handle.process(),
                    handle.title(),
                    &config.windows.window_rules,
                ) {
                    execute_actions(&mut hub, &mut registry, &rule.run, &main_hwnd);
                }
            }
            HubEvent::WindowDestroyed(handle) => {
                let _span = tracing::info_span!("window_destroyed", %handle).entered();
                if let Some(wt) = registry.remove(&handle) {
                    match wt {
                        WindowType::Tiling(id) => hub.delete_window(id),
                        WindowType::Float(id) => hub.delete_float(id),
                    }
                    tracing::info!("Window deleted");
                }
            }
            HubEvent::WindowFocused(handle) => {
                let _span = tracing::info_span!("window_focused", %handle).entered();
                if let Some(id) = registry.get_tiling(&handle) {
                    hub.set_focus(id);
                    tracing::info!("Tiling window focused");
                } else if let Some(id) = registry.get_float(&handle) {
                    hub.set_float_focus(id);
                    tracing::info!("Float window focused");
                }
                last_focus = hub.get_workspace(hub.current_workspace()).focused();
            }
            // TODO: update float window position in hub instead of re-rendering
            HubEvent::WindowMovedOrResized(_) => {}
            HubEvent::WindowTitleChanged(handle) => {
                let _span = tracing::info_span!("window_title_changed", %handle).entered();
                if registry.contains(&handle) {
                    registry.update_title(&handle);
                    continue;
                }
                // Some apps have a brief moment where their title is empty
                if !should_manage(
                    handle.process(),
                    handle.title(),
                    &config.windows.window_rules,
                ) {
                    continue;
                }
                let id = hub.insert_tiling();
                registry.insert_tiling(handle.clone(), id);
                tracing::info!("Window inserted after title change");
                if let Some(rule) = match_rule(
                    handle.process(),
                    handle.title(),
                    &config.windows.window_rules,
                ) {
                    execute_actions(&mut hub, &mut registry, &rule.run, &main_hwnd);
                }
            }
            HubEvent::Action(actions) => {
                execute_actions(&mut hub, &mut registry, &actions, &main_hwnd);
            }
        }
        let frame = build_frame(&hub, &registry, &config, last_focus);
        last_focus = hub.get_workspace(hub.current_workspace()).focused();
        send_frame(frame, &main_hwnd);
    }
}

fn send_frame(frame: Frame, main_hwnd: &WindowHandle) {
    let cmd = Box::new(frame);
    let ptr = Box::into_raw(cmd) as usize;
    unsafe { PostMessageW(Some(main_hwnd.hwnd()), WM_APP_FRAME, WPARAM(ptr), LPARAM(0)).ok() };
}

#[tracing::instrument(skip(hub, registry, main_hwnd), fields(actions = %actions))]
fn execute_actions(
    hub: &mut Hub,
    registry: &mut Registry,
    actions: &Actions,
    main_hwnd: &WindowHandle,
) {
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
                        registry.toggle(window_id, float_id);
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
                unsafe { PostMessageW(Some(main_hwnd.hwnd()), WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
            }
        }
    }
}

fn build_frame(
    hub: &Hub,
    registry: &Registry,
    config: &Config,
    last_focus: Option<Focus>,
) -> Frame {
    let ws = hub.get_workspace(hub.current_workspace());
    let border = config.border_size;

    let mut windows = Vec::new();
    let mut floats = Vec::new();
    let mut overlays = Vec::new();
    let focused = ws.focused();

    let mut stack: Vec<Child> = ws.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(id) => {
                if let Some(handle) = registry.get_handle(id) {
                    let dim = hub.get_window(id).dimension();
                    windows.push((handle, dim));
                    if focused != Some(Focus::Tiling(Child::Window(id))) {
                        overlays.extend(border_rects(dim, border, [config.border_color; 4]));
                    }
                }
            }
            Child::Container(id) => {
                let container = hub.get_container(id);
                if container.is_tabbed() {
                    if let Some(active) = container.active_tab() {
                        stack.push(active);
                    }
                    let dim = container.dimension();
                    let is_focused = focused == Some(Focus::Tiling(Child::Container(id)));
                    let tab_color = if is_focused {
                        config.focused_color
                    } else {
                        config.border_color
                    };

                    overlays.push(OverlayRect {
                        x: dim.x,
                        y: dim.y,
                        width: dim.width,
                        height: config.tab_bar_height,
                        color: config.tab_bar_background_color,
                    });
                    let tab_dim = Dimension {
                        x: dim.x,
                        y: dim.y,
                        width: dim.width,
                        height: config.tab_bar_height,
                    };
                    overlays.extend(border_rects(tab_dim, border, [tab_color; 4]));

                    let children = container.children();

                    // SAFETY: children can never be empty
                    let tab_width = dim.width / children.len() as f32;
                    overlays.push(OverlayRect {
                        x: dim.x + container.active_tab_index() as f32 * tab_width,
                        y: dim.y,
                        width: tab_width,
                        height: config.tab_bar_height,
                        color: config.active_tab_background_color,
                    });
                    for i in 1..children.len() {
                        overlays.push(OverlayRect {
                            x: dim.x + i as f32 * tab_width - border / 2.0,
                            y: dim.y,
                            width: border,
                            height: config.tab_bar_height,
                            color: tab_color,
                        });
                    }
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
            overlays.extend(border_rects(
                w.dimension(),
                border,
                spawn_colors(w.spawn_mode(), config),
            ));
        }
        Some(Focus::Tiling(Child::Container(id))) => {
            let c = hub.get_container(id);
            overlays.extend(border_rects(
                c.dimension(),
                border,
                spawn_colors(c.spawn_mode(), config),
            ));
        }
        _ => {}
    }

    for &id in ws.float_windows() {
        if let Some(handle) = registry.get_float_handle(id) {
            let dim = hub.get_float(id).dimension();
            floats.push((handle, dim));
            let color = if focused == Some(Focus::Float(id)) {
                config.focused_color
            } else {
                config.border_color
            };
            overlays.extend(border_rects(dim, border, [color; 4]));
        }
    }

    let focus = if focused != last_focus {
        match focused {
            Some(Focus::Tiling(Child::Window(id))) => registry.get_handle(id),
            Some(Focus::Float(id)) => registry.get_float_handle(id),
            _ => None,
        }
    } else {
        None
    };

    Frame {
        windows,
        floats,
        overlays,
        focus,
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

fn match_rule<'a>(
    process: &str,
    title: Option<&str>,
    rules: &'a [WindowsWindowRule],
) -> Option<&'a WindowsWindowRule> {
    for rule in rules {
        if let Some(pattern) = &rule.process
            && !pattern_matches(pattern, process)
        {
            continue;
        }
        if let Some(pattern) = &rule.title
            && !title.is_some_and(|t| pattern_matches(pattern, t))
        {
            continue;
        }
        if rule.process.is_some() || rule.title.is_some() {
            return Some(rule);
        }
    }
    None
}

fn should_manage(process: &str, title: Option<&str>, rules: &[WindowsWindowRule]) -> bool {
    match_rule(process, title, rules).is_none_or(|r| r.manage)
}

fn pattern_matches(pattern: &str, text: &str) -> bool {
    if let Some(regex) = pattern.strip_prefix('/').and_then(|p| p.strip_suffix('/')) {
        regex::Regex::new(regex)
            .map(|r| r.is_match(text))
            .unwrap_or(false)
    } else {
        pattern == text
    }
}
