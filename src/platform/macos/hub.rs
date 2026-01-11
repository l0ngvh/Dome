use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};

use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopSource};
use objc2_core_graphics::CGWindowID;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Color, Config, MacosWindowRule};
use crate::core::{Child, Container, Dimension, FloatWindowId, Focus, Hub, SpawnMode, WindowId};

use super::overlay::{OverlayLabel, OverlayRect, Overlays};

pub(super) struct WindowInfo {
    pub(super) cg_id: CGWindowID,
    pub(super) title: Option<String>,
    pub(super) app_name: String,
    pub(super) bundle_id: Option<String>,
    pub(super) should_tile: bool,
    pub(super) dimension: Dimension,
}

pub(super) enum HubEvent {
    WindowCreated(WindowInfo),
    WindowDestroyed(CGWindowID),
    WindowFocused(CGWindowID),
    TitleChanged { cg_id: CGWindowID, title: String },
    Action(Actions),
    ConfigReloaded(Config),
    Shutdown,
}

pub(super) struct Frame {
    windows: Vec<(CGWindowID, Dimension)>,
    hide: Vec<CGWindowID>,
    overlays: Overlays,
    focus: Option<CGWindowID>,
}

impl Frame {
    pub(super) fn windows(&self) -> &[(CGWindowID, Dimension)] {
        &self.windows
    }

    pub(super) fn hide(&self) -> &[CGWindowID] {
        &self.hide
    }

    pub(super) fn overlays(&self) -> &Overlays {
        &self.overlays
    }

    pub(super) fn focus(&self) -> Option<CGWindowID> {
        self.focus
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowType {
    Tiling(WindowId),
    Float(FloatWindowId),
}

struct WindowEntry {
    title: Option<String>,
    window_type: WindowType,
}

struct HubRegistry {
    windows: HashMap<CGWindowID, WindowEntry>,
    tiling_to_cg: HashMap<WindowId, CGWindowID>,
    float_to_cg: HashMap<FloatWindowId, CGWindowID>,
}

impl HubRegistry {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            tiling_to_cg: HashMap::new(),
            float_to_cg: HashMap::new(),
        }
    }

    fn insert(&mut self, cg_id: CGWindowID, entry: WindowEntry) {
        match entry.window_type {
            WindowType::Tiling(id) => {
                self.tiling_to_cg.insert(id, cg_id);
            }
            WindowType::Float(id) => {
                self.float_to_cg.insert(id, cg_id);
            }
        }
        self.windows.insert(cg_id, entry);
    }

    fn remove(&mut self, cg_id: CGWindowID) -> Option<WindowType> {
        let entry = self.windows.remove(&cg_id)?;
        match entry.window_type {
            WindowType::Tiling(id) => {
                self.tiling_to_cg.remove(&id);
            }
            WindowType::Float(id) => {
                self.float_to_cg.remove(&id);
            }
        }
        Some(entry.window_type)
    }

    fn get(&self, cg_id: CGWindowID) -> Option<WindowType> {
        self.windows.get(&cg_id).map(|e| e.window_type)
    }

    fn get_cg_id(&self, window_id: WindowId) -> Option<CGWindowID> {
        self.tiling_to_cg.get(&window_id).copied()
    }

    fn get_float_cg_id(&self, float_id: FloatWindowId) -> Option<CGWindowID> {
        self.float_to_cg.get(&float_id).copied()
    }

    fn contains(&self, cg_id: CGWindowID) -> bool {
        self.windows.contains_key(&cg_id)
    }

    fn update_title(&mut self, cg_id: CGWindowID, title: String) {
        if let Some(entry) = self.windows.get_mut(&cg_id) {
            entry.title = Some(title);
        }
    }

    fn toggle_float(&mut self, window_id: WindowId, float_id: FloatWindowId) {
        if let Some(cg_id) = self.tiling_to_cg.remove(&window_id) {
            self.float_to_cg.insert(float_id, cg_id);
            if let Some(entry) = self.windows.get_mut(&cg_id) {
                entry.window_type = WindowType::Float(float_id);
            }
        } else if let Some(cg_id) = self.float_to_cg.remove(&float_id) {
            self.tiling_to_cg.insert(window_id, cg_id);
            if let Some(entry) = self.windows.get_mut(&cg_id) {
                entry.window_type = WindowType::Tiling(window_id);
            }
        }
    }

    fn get_title(&self, window_id: WindowId) -> Option<&str> {
        self.tiling_to_cg
            .get(&window_id)
            .and_then(|cg_id| self.windows.get(cg_id))
            .and_then(|e| e.title.as_deref())
    }
}

struct SendableSource(CFRetained<CFRunLoopSource>);
struct SendableRunLoop(CFRetained<CFRunLoop>);

// Safety: CFRunLoopSource and CFRunLoop are thread-safe for signal/wake_up operations
unsafe impl Send for SendableSource {}
unsafe impl Send for SendableRunLoop {}

pub(super) struct HubThread {
    handle: JoinHandle<()>,
}

impl HubThread {
    pub(super) fn spawn(
        config: Config,
        screen: Dimension,
        event_rx: Receiver<HubEvent>,
        frame_tx: Sender<Frame>,
        source: CFRetained<CFRunLoopSource>,
        main_run_loop: CFRetained<CFRunLoop>,
    ) -> Self {
        let source = SendableSource(source);
        let main_run_loop = SendableRunLoop(main_run_loop);
        let handle =
            thread::spawn(move || run(config, screen, event_rx, frame_tx, source, main_run_loop));
        Self { handle }
    }

    pub(super) fn join(self) {
        self.handle.join().ok();
    }
}

fn run(
    mut config: Config,
    screen: Dimension,
    rx: Receiver<HubEvent>,
    frame_tx: Sender<Frame>,
    source: SendableSource,
    main_run_loop: SendableRunLoop,
) {
    let mut hub = Hub::new(screen, config.tab_bar_height, config.automatic_tiling);
    let mut registry = HubRegistry::new();

    let send_frame = |frame: Frame| {
        if frame_tx.send(frame).is_ok() {
            source.0.signal();
            main_run_loop.0.wake_up();
        }
    };

    let frame = build_frame(&hub, &registry, &config, None, HashSet::new());
    send_frame(frame);

    while let Ok(event) = rx.recv() {
        let last_focus = hub.get_workspace(hub.current_workspace()).focused();
        let previous_displayed: HashSet<_> = get_displayed_windows(&hub, &registry)
            .into_iter()
            .map(|(id, _)| id)
            .collect();

        match event {
            HubEvent::Shutdown => break,
            HubEvent::ConfigReloaded(new_config) => {
                hub.sync_config(new_config.tab_bar_height, new_config.automatic_tiling);
                config = new_config;
                tracing::info!("Config reloaded");
            }
            HubEvent::WindowCreated(info) => {
                let _span =
                    tracing::info_span!("window_created", cg_id = info.cg_id, app = %info.app_name)
                        .entered();
                if registry.contains(info.cg_id) {
                    continue;
                }
                if !should_manage(&info, &config.macos.window_rules) {
                    continue;
                }
                let window_type = if info.should_tile {
                    WindowType::Tiling(hub.insert_tiling())
                } else {
                    WindowType::Float(hub.insert_float(info.dimension))
                };
                registry.insert(
                    info.cg_id,
                    WindowEntry {
                        title: info.title.clone(),
                        window_type,
                    },
                );
                tracing::info!("Window inserted");

                if let Some(rule) = match_rule(&info, &config.macos.window_rules)
                    && execute_actions(&mut hub, &mut registry, &rule.run)
                {
                    break;
                }
            }
            HubEvent::WindowDestroyed(cg_id) => {
                let _span = tracing::info_span!("window_destroyed", cg_id).entered();
                if let Some(wt) = registry.remove(cg_id) {
                    match wt {
                        WindowType::Tiling(id) => hub.delete_window(id),
                        WindowType::Float(id) => hub.delete_float(id),
                    }
                    tracing::info!("Window deleted");
                }
            }
            HubEvent::WindowFocused(cg_id) => {
                let _span = tracing::info_span!("window_focused", cg_id).entered();
                if let Some(wt) = registry.get(cg_id) {
                    match wt {
                        WindowType::Tiling(id) => hub.set_focus(id),
                        WindowType::Float(id) => hub.set_float_focus(id),
                    }
                }
            }
            HubEvent::TitleChanged { cg_id, title } => {
                registry.update_title(cg_id, title);
            }
            HubEvent::Action(actions) => {
                if execute_actions(&mut hub, &mut registry, &actions) {
                    break;
                }
            }
        }

        let frame = build_frame(&hub, &registry, &config, last_focus, previous_displayed);
        send_frame(frame);
    }
}

fn execute_actions(hub: &mut Hub, registry: &mut HubRegistry, actions: &Actions) -> bool {
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
            Action::Exit => return true,
        }
    }
    false
}

fn get_displayed_windows(hub: &Hub, registry: &HubRegistry) -> Vec<(CGWindowID, Dimension)> {
    let ws = hub.get_workspace(hub.current_workspace());
    let mut windows = Vec::new();

    let mut stack: Vec<Child> = ws.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(id) => {
                if let Some(cg_id) = registry.get_cg_id(id) {
                    windows.push((cg_id, hub.get_window(id).dimension()));
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
        if let Some(cg_id) = registry.get_float_cg_id(float_id) {
            windows.push((cg_id, hub.get_float(float_id).dimension()));
        }
    }

    windows
}

fn build_tab_bar(
    screen: Dimension,
    container: &Container,
    registry: &HubRegistry,
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

fn build_overlays(hub: &Hub, registry: &HubRegistry, config: &Config) -> Overlays {
    let ws = hub.get_workspace(hub.current_workspace());
    let screen = hub.screen();
    let border = config.border_size;
    let focused = ws.focused();

    let mut tiling_rects = Vec::new();
    let mut tiling_labels = Vec::new();
    let mut float_rects = Vec::new();

    let mut stack: Vec<Child> = ws.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(id) => {
                if registry.get_cg_id(id).is_some()
                    && focused != Some(Focus::Tiling(Child::Window(id)))
                {
                    let dim = hub.get_window(id).dimension();
                    tiling_rects.extend(border_rects(
                        screen,
                        dim,
                        border,
                        [config.border_color; 4],
                    ));
                }
            }
            Child::Container(id) => {
                let container = hub.get_container(id);
                if let Some(active) = container.active_tab() {
                    stack.push(active);
                    let is_focused = focused == Some(Focus::Tiling(Child::Container(id)));
                    let (rects, labels) =
                        build_tab_bar(screen, container, registry, config, is_focused);
                    tiling_rects.extend(rects);
                    tiling_labels.extend(labels);
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
            tiling_rects.extend(border_rects(
                screen,
                w.dimension(),
                border,
                spawn_colors(w.spawn_mode(), config),
            ));
        }
        Some(Focus::Tiling(Child::Container(id))) => {
            let c = hub.get_container(id);
            tiling_rects.extend(border_rects(
                screen,
                c.dimension(),
                border,
                spawn_colors(c.spawn_mode(), config),
            ));
        }
        _ => {}
    }

    for &float_id in ws.float_windows() {
        if registry.get_float_cg_id(float_id).is_some() {
            let dim = hub.get_float(float_id).dimension();
            let color = if focused == Some(Focus::Float(float_id)) {
                config.focused_color
            } else {
                config.border_color
            };
            float_rects.extend(border_rects(screen, dim, border, [color; 4]));
        }
    }

    Overlays {
        tiling_rects,
        tiling_labels,
        float_rects,
    }
}

fn build_frame(
    hub: &Hub,
    registry: &HubRegistry,
    config: &Config,
    last_focus: Option<Focus>,
    previous_displayed: HashSet<CGWindowID>,
) -> Frame {
    let ws = hub.get_workspace(hub.current_workspace());
    let focused = ws.focused();

    let windows = get_displayed_windows(hub, registry);
    let overlays = build_overlays(hub, registry, config);

    let focus = if focused != last_focus {
        match focused {
            Some(Focus::Tiling(Child::Window(id))) => registry.get_cg_id(id),
            Some(Focus::Float(id)) => registry.get_float_cg_id(id),
            _ => None,
        }
    } else {
        None
    };

    let current: HashSet<_> = windows.iter().map(|(id, _)| *id).collect();
    let hide = previous_displayed
        .into_iter()
        .filter(|id| !current.contains(id))
        .collect();

    Frame {
        windows,
        hide,
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

// macOS uses bottom-left origin, so we flip y here.
// Windows uses top-left origin, so no flip needed there.
fn flip_y(screen: Dimension, y: f32, height: f32) -> f32 {
    screen.y + screen.height - y - height
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

fn match_rule<'a>(info: &WindowInfo, rules: &'a [MacosWindowRule]) -> Option<&'a MacosWindowRule> {
    for rule in rules {
        if let Some(app) = &rule.app
            && !pattern_matches(app, &info.app_name)
        {
            continue;
        }
        if let Some(b) = &rule.bundle_id
            && info.bundle_id.as_ref() != Some(b)
        {
            continue;
        }
        if let Some(t) = &rule.title
            && !info
                .title
                .as_ref()
                .is_some_and(|title| pattern_matches(t, title))
        {
            continue;
        }
        if rule.app.is_some() || rule.bundle_id.is_some() || rule.title.is_some() {
            return Some(rule);
        }
    }
    None
}

fn should_manage(info: &WindowInfo, rules: &[MacosWindowRule]) -> bool {
    match_rule(info, rules).is_none_or(|r| r.manage)
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
