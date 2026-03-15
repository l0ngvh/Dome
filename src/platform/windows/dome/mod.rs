mod inspect;
mod throttle;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use calloop::channel::{Channel, Event as ChannelEvent};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle, LoopSignal};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromWindow};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, PostThreadMessageW, WM_QUIT};

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Config, WindowsOnOpenRule, WindowsWindow};
use crate::core::{
    Child, ContainerId, ContainerPlacement, Dimension, Hub, MonitorId, MonitorLayout, SpawnMode,
    WindowId,
};

use self::inspect::{
    enum_windows, get_process_name, get_size_constraints, get_window_title, initial_window_mode,
    is_fullscreen, is_manageable,
};
use self::throttle::{Throttle, ThrottleResult};
use super::ScreenInfo;
use super::handle::{ManagedHwnd, WindowMode, get_dimension};

pub(super) const WM_APP_LAYOUT: u32 = 0x8001;
pub(super) const WM_APP_CONFIG: u32 = 0x8002;
pub(super) const WM_APP_TITLE: u32 = 0x8003;

const FOCUS_THROTTLE: Duration = Duration::from_millis(500);
const RESIZE_THROTTLE: Duration = Duration::from_millis(16);

#[expect(
    clippy::large_enum_variant,
    reason = "These messages aren't bottleneck right now"
)]
pub(super) enum HubEvent {
    AppInitialized(AppHandle),
    WindowCreated(ManagedHwnd),
    WindowDestroyed(ManagedHwnd),
    WindowMinimized(ManagedHwnd),
    WindowFocused(ManagedHwnd),
    WindowTitleChanged(ManagedHwnd),
    WindowMovedOrResized(ManagedHwnd),
    ScreensChanged(Vec<ScreenInfo>),
    Action(Actions),
    ConfigChanged(Config),
    TabClicked(ContainerId, usize),
    SetFullscreen(WindowId),
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

/// A frame of work for Wm to execute.
///
/// `to_show` contains every window that Wm should position this frame.
/// Windows not in `to_show` are ignored — this includes borderless
/// fullscreen and exclusive fullscreen windows. Wm must not touch them.
///
/// `to_hide` contains windows that were visible last frame but are no longer.
/// Wm hides the managed window, its overlay, and removes the taskbar tab.
/// Exclusive fullscreen windows are skipped entirely.
///
/// `tabs_to_add` contains windows that became visible this frame and need
/// a taskbar tab. Unlike `to_show`, this includes borderless and exclusive
/// fullscreen windows — all windows on the current workspace get tabs.
pub(super) struct LayoutFrame {
    pub(super) to_show: Vec<WindowShow>,
    pub(super) to_hide: Vec<WindowId>,
    pub(super) containers_to_show: Vec<ContainerRender>,
    pub(super) containers_to_hide: Vec<ContainerId>,
    pub(super) created_windows: Vec<WindowCreate>,
    pub(super) deleted_windows: Vec<WindowId>,
    pub(super) created_containers: Vec<ContainerId>,
    pub(super) deleted_containers: Vec<ContainerId>,
    pub(super) tabs_to_add: Vec<WindowId>,
    pub(super) focused: Option<WindowId>,
}

// Windows-specific — only the fields Wm actually consumes.
// Does NOT wrap core::WindowPlacement; Dome translates at the boundary.
// No is_focused — Wm derives it from LayoutFrame::focused.
pub(super) struct WindowShow {
    pub(super) id: WindowId,
    pub(super) frame: Dimension,
    pub(super) visible_frame: Dimension,
    pub(super) is_float: bool,
    pub(super) spawn_mode: SpawnMode,
    // Some(monitor_dim) → call set_fullscreen; None → call show with border inset
    pub(super) fullscreen_dim: Option<Dimension>,
}

// Per-monitor displayed state, tracked by Dome across frames
struct DisplayedMonitor {
    window_ids: Vec<WindowId>,
    container_ids: Vec<ContainerId>,
}

pub(super) struct WindowCreate {
    pub(super) hwnd: HWND,
    pub(super) id: WindowId,
    pub(super) mode: WindowMode,
    pub(super) title: Option<String>,
    pub(super) process: String,
}

#[derive(Clone)]
pub(super) struct ContainerRender {
    pub(super) placement: ContainerPlacement,
    pub(super) children: Vec<Child>,
}

pub(super) struct TitleUpdate {
    pub(super) titles: Vec<(ManagedHwnd, Option<String>)>,
    pub(super) container_renders: Vec<ContainerRender>,
}

#[derive(Clone, Copy)]
enum ThrottleKind {
    Focus,
    Resize,
}

/// Per-window state maintained by Dome. Mode is synced from hub placements
/// at the start of each `apply_layout` call, before building the frame.
struct TrackedWindow {
    hwnd: HWND,
    mode: WindowMode,
    title: Option<String>,
    process: String,
}

pub(super) struct Dome {
    hub: Hub,
    window_map: HashMap<ManagedHwnd, WindowId>,
    tracked_windows: HashMap<WindowId, TrackedWindow>,
    monitor_handles: HashMap<isize, MonitorId>,
    monitor_dimensions: HashMap<MonitorId, Dimension>,
    displayed: HashMap<MonitorId, DisplayedMonitor>,
    config: Config,
    app_hwnd: Option<AppHandle>,
    main_thread_id: u32,
    signal: LoopSignal,
    handle: LoopHandle<'static, Self>,
    focus_throttle: Throttle<ManagedHwnd>,
    resize_throttle: Throttle<ManagedHwnd>,
}

impl Dome {
    pub(super) fn new(
        config: Config,
        screens: Vec<ScreenInfo>,
        handle: LoopHandle<'static, Self>,
        signal: LoopSignal,
        main_thread_id: u32,
    ) -> Self {
        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_handles = HashMap::new();
        let mut monitor_dimensions = HashMap::new();
        monitor_handles.insert(primary.handle, primary_monitor_id);
        monitor_dimensions.insert(primary_monitor_id, primary.dimension);
        tracing::info!(
            name = %primary.name,
            handle = ?primary.handle,
            dimension = ?primary.dimension,
            "Primary monitor"
        );

        for screen in &screens {
            if screen.handle != primary.handle {
                let id = hub.add_monitor(screen.name.clone(), screen.dimension);
                monitor_handles.insert(screen.handle, id);
                monitor_dimensions.insert(id, screen.dimension);
                tracing::info!(
                    name = %screen.name,
                    handle = ?screen.handle,
                    dimension = ?screen.dimension,
                    "Monitor"
                );
            }
        }

        // Drain initial allocations from Hub::new() and add_monitor()
        hub.drain_changes();

        Self {
            hub,
            window_map: HashMap::new(),
            tracked_windows: HashMap::new(),
            monitor_handles,
            monitor_dimensions,
            displayed: HashMap::new(),
            config,
            app_hwnd: None,
            main_thread_id,
            signal,
            handle,
            focus_throttle: Throttle::new(FOCUS_THROTTLE),
            resize_throttle: Throttle::new(RESIZE_THROTTLE),
        }
    }

    pub(super) fn run(
        mut self,
        channel: Channel<HubEvent>,
        mut event_loop: EventLoop<'static, Self>,
    ) {
        event_loop
            .handle()
            .insert_source(channel, |event, _, dome| match event {
                ChannelEvent::Msg(hub_event) => {
                    if dome.handle_event(hub_event) {
                        dome.apply_layout();
                    }
                }
                ChannelEvent::Closed => dome.signal.stop(),
            })
            .expect("Failed to insert channel source");

        event_loop
            .run(None, &mut self, |_| {})
            .expect("Event loop failed");
    }

    fn handle_event(&mut self, event: HubEvent) -> bool {
        match event {
            HubEvent::AppInitialized(hwnd) => {
                self.app_hwnd = Some(hwnd);
                if let Err(e) = enum_windows(|hwnd| {
                    self.try_manage_window(hwnd);
                }) {
                    tracing::warn!("Failed to enumerate windows: {e}");
                }
            }
            HubEvent::Shutdown => {
                self.signal.stop();
                return false;
            }
            HubEvent::ConfigChanged(new_config) => {
                self.hub.sync_config(new_config.clone().into());
                if let Some(app_hwnd) = self.app_hwnd {
                    send_config(new_config.clone(), app_hwnd);
                }
                self.config = new_config;
                tracing::info!("Config reloaded");
            }
            HubEvent::WindowCreated(h) => {
                if self.window_map.contains_key(&h) {
                    return true;
                }
                self.try_manage_window(h.hwnd());
            }
            HubEvent::WindowDestroyed(h) => {
                let _span = tracing::info_span!("window_destroyed").entered();
                self.remove_window(h);
            }
            HubEvent::WindowMinimized(h) => {
                let _span = tracing::info_span!("window_minimized").entered();
                let is_fullscreen = self
                    .window_map
                    .get(&h)
                    .map(|&id| self.hub.get_window(id).is_fullscreen())
                    .unwrap_or(false);
                if !is_fullscreen {
                    self.remove_window(h);
                }
            }
            HubEvent::WindowFocused(h) => {
                self.submit_focus(h);
                return true;
            }
            HubEvent::WindowMovedOrResized(h) => {
                self.submit_resize(h);
                return true;
            }
            HubEvent::WindowTitleChanged(h) => {
                if self.window_map.contains_key(&h) {
                    let new_title = get_window_title(h.hwnd());
                    self.send_title_update(vec![(h, new_title)]);
                    return true;
                }
                // Some apps have a brief moment where their title is empty
                self.try_manage_window(h.hwnd());
            }
            HubEvent::ScreensChanged(screens) => {
                tracing::info!(count = screens.len(), "Screen parameters changed");
                self.update_screens(screens);
            }
            HubEvent::Action(actions) => {
                self.execute_actions(&actions);
            }
            HubEvent::TabClicked(container_id, tab_idx) => {
                self.hub.focus_tab_index(container_id, tab_idx);
            }
            HubEvent::SetFullscreen(id) => {
                if let Some(info) = self.tracked_windows.get_mut(&id) {
                    info.mode = WindowMode::FullscreenExclusive;
                    if !self.hub.get_window(id).is_fullscreen() {
                        self.hub.set_fullscreen(id);
                    }
                }
            }
        }

        true
    }

    fn try_manage_window(&mut self, hwnd: HWND) {
        if !is_manageable(hwnd) {
            return;
        }
        let title = get_window_title(hwnd);
        let process = get_process_name(hwnd).unwrap_or_default();
        if should_ignore(&process, title.as_deref(), &self.config.windows.ignore) {
            return;
        }
        let actions = on_open_actions(&process, title.as_deref(), &self.config.windows.on_open);
        self.insert_window(hwnd, title, process);
        if let Some(actions) = actions {
            self.execute_actions(&actions);
        }
    }

    fn insert_window(&mut self, hwnd: HWND, title: Option<String>, process: String) {
        let managed = ManagedHwnd::new(hwnd);
        let dim = get_dimension(hwnd);
        let monitor = self.find_monitor_dimension(hwnd);

        let mode = initial_window_mode(hwnd, monitor.as_ref());
        let id = match mode {
            WindowMode::FullscreenBorderless
            | WindowMode::ManagedFullscreen
            | WindowMode::FullscreenExclusive => self.hub.insert_fullscreen(),
            WindowMode::Float => self.hub.insert_float(dim),
            WindowMode::Tiling => self.hub.insert_tiling(),
        };
        self.set_constraints(id, hwnd);

        self.window_map.insert(managed, id);
        self.tracked_windows.insert(
            id,
            TrackedWindow {
                hwnd,
                mode,
                title,
                process,
            },
        );
    }

    fn remove_window(&mut self, h: ManagedHwnd) {
        if let Some(id) = self.window_map.remove(&h) {
            self.tracked_windows.remove(&id);
            self.hub.delete_window(id);
        }
    }

    fn set_constraints(&mut self, id: WindowId, hwnd: HWND) {
        let border = self.config.border_size;
        let (min_w, min_h, max_w, max_h) = get_size_constraints(hwnd);
        if min_w > 0.0 || min_h > 0.0 || max_w > 0.0 || max_h > 0.0 {
            let to_frame = |v: f32| {
                if v > 0.0 {
                    Some(v + 2.0 * border)
                } else {
                    None
                }
            };
            let (new_min_w, new_min_h, new_max_w, new_max_h) = (
                to_frame(min_w),
                to_frame(min_h),
                to_frame(max_w),
                to_frame(max_h),
            );
            let (cur_min_w, cur_min_h) = self.hub.get_window(id).min_size();
            let (cur_max_w, cur_max_h) = self.hub.get_window(id).max_size();
            if new_min_w.unwrap_or(cur_min_w) == cur_min_w
                && new_min_h.unwrap_or(cur_min_h) == cur_min_h
                && new_max_w.unwrap_or(cur_max_w) == cur_max_w
                && new_max_h.unwrap_or(cur_max_h) == cur_max_h
            {
                return;
            }
            self.hub
                .set_window_constraint(id, new_min_w, new_min_h, new_max_w, new_max_h);
        }
    }

    fn find_monitor_dimension(&self, hwnd: HWND) -> Option<Dimension> {
        let hmonitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) };
        let id = self.monitor_handles.get(&(hmonitor.0 as isize))?;
        self.monitor_dimensions.get(id).copied()
    }

    fn submit_focus(&mut self, h: ManagedHwnd) {
        match self.focus_throttle.submit(h) {
            ThrottleResult::Send(h) => self.handle_focus(h),
            ThrottleResult::Pending => {}
            ThrottleResult::ScheduleFlush(delay) => {
                self.schedule_throttle_timer(delay, ThrottleKind::Focus);
            }
        }
    }

    fn submit_resize(&mut self, h: ManagedHwnd) {
        match self.resize_throttle.submit(h) {
            ThrottleResult::Send(h) => self.handle_resize(h),
            ThrottleResult::Pending => {}
            ThrottleResult::ScheduleFlush(delay) => {
                self.schedule_throttle_timer(delay, ThrottleKind::Resize);
            }
        }
    }

    fn schedule_throttle_timer(&mut self, delay: Duration, kind: ThrottleKind) {
        let timer = Timer::from_duration(delay);
        // Token is intentionally discarded: at most one timer exists at a time
        // (guarded by has_pending_timer), and it always self-removes via TimeoutAction::Drop.
        self.handle
            .insert_source(timer, move |_, _, dome| {
                match kind {
                    ThrottleKind::Focus => {
                        if let Some(h) = dome.focus_throttle.flush() {
                            dome.handle_focus(h);
                        }
                    }
                    ThrottleKind::Resize => {
                        if let Some(h) = dome.resize_throttle.flush() {
                            dome.handle_resize(h);
                        }
                    }
                }
                TimeoutAction::Drop
            })
            .expect("Failed to insert timer");

        match kind {
            ThrottleKind::Focus => self.focus_throttle.mark_timer_scheduled(),
            ThrottleKind::Resize => self.resize_throttle.mark_timer_scheduled(),
        }
    }

    fn handle_focus(&mut self, h: ManagedHwnd) {
        if let Some(&id) = self.window_map.get(&h) {
            self.hub.set_focus(id);
            tracing::info!(hwnd = ?h.hwnd(), "Window focused");
            self.apply_layout();
        }
    }

    fn handle_resize(&mut self, h: ManagedHwnd) {
        let Some(&id) = self.window_map.get(&h) else {
            return;
        };
        if self
            .tracked_windows
            .get(&id)
            .is_some_and(|i| i.mode == WindowMode::FullscreenExclusive)
        {
            return;
        }
        self.set_constraints(id, h.hwnd());
        self.check_fullscreen_state(h);
        self.apply_layout();
    }

    fn check_fullscreen_state(&mut self, h: ManagedHwnd) {
        let Some(&id) = self.window_map.get(&h) else {
            return;
        };
        let Some(monitor_dim) = self.find_monitor_dimension(h.hwnd()) else {
            return;
        };

        let was_fs = self.hub.get_window(id).is_fullscreen();
        let window_dim = get_dimension(h.hwnd());
        let is_fs = is_fullscreen(&window_dim, &monitor_dim);
        if was_fs != is_fs {
            tracing::debug!(hwnd = ?h.hwnd(), ?window_dim, ?monitor_dim, was_fs, is_fs, "Fullscreen state changed");
        }

        match (was_fs, is_fs) {
            (false, true) => {
                self.hub.set_fullscreen(id);
            }
            (true, false) => {
                self.hub.unset_fullscreen(id);
            }
            _ => {}
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
                    ToggleTarget::Fullscreen => self.hub.toggle_fullscreen(),
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
                    unsafe {
                        PostThreadMessageW(self.main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok()
                    };
                    self.signal.stop();
                }
            }
        }
    }

    fn apply_layout(&mut self) {
        let changes = self.hub.drain_changes();

        let created_windows: Vec<WindowCreate> = changes
            .created_windows
            .iter()
            .filter_map(|&id| {
                let info = self.tracked_windows.get(&id)?;
                Some(WindowCreate {
                    hwnd: info.hwnd,
                    id,
                    mode: info.mode,
                    title: info.title.clone(),
                    process: info.process.clone(),
                })
            })
            .collect();

        let focused = match self
            .hub
            .get_workspace(self.hub.current_workspace())
            .focused()
        {
            Some(Child::Window(id)) => Some(id),
            _ => None,
        };

        let placements = self.hub.get_visible_placements();

        let mut to_show = Vec::new();
        let mut containers_to_show = Vec::new();
        let mut new_displayed: HashMap<MonitorId, DisplayedMonitor> = HashMap::new();

        for mp in placements {
            let dimension = self
                .monitor_dimensions
                .get(&mp.monitor_id)
                .copied()
                .unwrap_or_default();

            let mut window_ids = Vec::new();
            let mut container_ids = Vec::new();

            match mp.layout {
                MonitorLayout::Fullscreen(id) => {
                    window_ids.push(id);
                    if let Some(info) = self.tracked_windows.get_mut(&id) {
                        match info.mode {
                            WindowMode::FullscreenExclusive | WindowMode::FullscreenBorderless => {}
                            _ => {
                                info.mode = WindowMode::ManagedFullscreen;
                                to_show.push(WindowShow {
                                    id,
                                    frame: dimension,
                                    visible_frame: dimension,
                                    is_float: false,
                                    spawn_mode: self.hub.get_window(id).spawn_mode(),
                                    fullscreen_dim: Some(dimension),
                                });
                            }
                        }
                    }
                }
                MonitorLayout::Normal {
                    windows,
                    containers,
                } => {
                    for wp in windows {
                        window_ids.push(wp.id);
                        if let Some(info) = self.tracked_windows.get_mut(&wp.id) {
                            info.mode = if wp.is_float {
                                WindowMode::Float
                            } else {
                                WindowMode::Tiling
                            };
                        }
                        to_show.push(WindowShow {
                            id: wp.id,
                            frame: wp.frame,
                            visible_frame: wp.visible_frame,
                            is_float: wp.is_float,
                            spawn_mode: wp.spawn_mode,
                            fullscreen_dim: None,
                        });
                    }
                    for cp in &containers {
                        if !cp.is_tabbed && !cp.is_focused {
                            continue;
                        }
                        container_ids.push(cp.id);
                        let children = if cp.is_tabbed {
                            self.hub.get_container(cp.id).children().to_vec()
                        } else {
                            vec![]
                        };
                        containers_to_show.push(ContainerRender {
                            placement: *cp,
                            children,
                        });
                    }
                }
            }

            new_displayed.insert(
                mp.monitor_id,
                DisplayedMonitor {
                    window_ids,
                    container_ids,
                },
            );
        }

        // Global diff (not per-monitor) avoids hiding windows that moved between monitors,
        // since hide() uses SWP_ASYNCWINDOWPOS and could race with the show() on the new monitor.
        let old_window_ids: HashSet<WindowId> = self
            .displayed
            .values()
            .flat_map(|m| &m.window_ids)
            .copied()
            .collect();
        let new_window_ids: HashSet<WindowId> = new_displayed
            .values()
            .flat_map(|m| &m.window_ids)
            .copied()
            .collect();
        let to_hide: Vec<WindowId> = old_window_ids
            .difference(&new_window_ids)
            .copied()
            .collect();
        let tabs_to_add: Vec<WindowId> = new_window_ids
            .difference(&old_window_ids)
            .copied()
            .collect();

        let old_container_ids: HashSet<ContainerId> = self
            .displayed
            .values()
            .flat_map(|m| &m.container_ids)
            .copied()
            .collect();
        let new_container_ids: HashSet<ContainerId> = new_displayed
            .values()
            .flat_map(|m| &m.container_ids)
            .copied()
            .collect();
        let containers_to_hide: Vec<ContainerId> = old_container_ids
            .difference(&new_container_ids)
            .copied()
            .collect();

        self.displayed = new_displayed;

        let frame = LayoutFrame {
            to_show,
            to_hide,
            containers_to_show,
            containers_to_hide,
            created_windows,
            deleted_windows: changes.deleted_windows,
            created_containers: changes.created_containers,
            deleted_containers: changes.deleted_containers,
            tabs_to_add,
            focused,
        };

        if let Some(app_hwnd) = self.app_hwnd {
            send_layout_frame(frame, app_hwnd);
        }
    }

    fn send_title_update(&self, titles: Vec<(ManagedHwnd, Option<String>)>) {
        let Some(app_hwnd) = self.app_hwnd else {
            return;
        };

        let affected_ids: HashSet<WindowId> = titles
            .iter()
            .filter_map(|(h, _)| self.window_map.get(h).copied())
            .collect();
        let container_renders = self.build_container_renders_for(&affected_ids);

        let update = TitleUpdate {
            titles,
            container_renders,
        };
        let boxed = Box::new(update);
        let ptr = Box::into_raw(boxed) as usize;
        unsafe { PostMessageW(Some(app_hwnd.hwnd()), WM_APP_TITLE, WPARAM(ptr), LPARAM(0)).ok() };
    }

    fn build_container_renders_for(
        &self,
        affected_ids: &HashSet<WindowId>,
    ) -> Vec<ContainerRender> {
        let mut renders = Vec::new();
        for mp in self.hub.get_visible_placements() {
            if let MonitorLayout::Normal { containers, .. } = &mp.layout {
                for cp in containers {
                    if !cp.is_tabbed {
                        continue;
                    }
                    let container = self.hub.get_container(cp.id);
                    let has_affected = container
                        .children()
                        .iter()
                        .any(|c| matches!(c, Child::Window(wid) if affected_ids.contains(wid)));
                    if has_affected {
                        let children = container.children().to_vec();
                        renders.push(ContainerRender {
                            placement: *cp,
                            children,
                        });
                    }
                }
            }
        }
        renders
    }

    fn update_screens(&mut self, screens: Vec<ScreenInfo>) {
        if screens.is_empty() {
            tracing::warn!("Empty screen list, skipping update");
            return;
        }
        self.reconcile_monitors(screens);

        let windows: Vec<_> = self.window_map.iter().map(|(&h, &id)| (h, id)).collect();
        for (managed, id) in windows {
            if self
                .tracked_windows
                .get(&id)
                .is_some_and(|i| i.mode == WindowMode::FullscreenExclusive)
            {
                continue;
            }
            self.set_constraints(id, managed.hwnd());
        }
    }

    fn reconcile_monitors(&mut self, screens: Vec<ScreenInfo>) {
        let current_handles: HashSet<isize> = screens.iter().map(|s| s.handle).collect();

        for screen in &screens {
            if !self.monitor_handles.contains_key(&screen.handle) {
                let id = self.hub.add_monitor(screen.name.clone(), screen.dimension);
                self.monitor_handles.insert(screen.handle, id);
                self.monitor_dimensions.insert(id, screen.dimension);
                tracing::info!(
                    name = %screen.name,
                    handle = ?screen.handle,
                    dimension = ?screen.dimension,
                    "Monitor added"
                );
            }
        }

        let to_remove: Vec<_> = self
            .monitor_handles
            .iter()
            .filter(|(h, _)| !current_handles.contains(h))
            .map(|(_, &id)| id)
            .collect();

        let fallback = screens
            .iter()
            .find(|s| s.is_primary)
            .and_then(|s| self.monitor_handles.get(&s.handle).copied());

        for monitor_id in to_remove {
            if let Some(fallback_id) = fallback
                && fallback_id != monitor_id
            {
                self.hub.remove_monitor(monitor_id, fallback_id);
                self.monitor_handles.retain(|_, &mut id| id != monitor_id);
                self.monitor_dimensions.remove(&monitor_id);
                tracing::info!(%monitor_id, fallback = %fallback_id, "Monitor removed");
            }
        }

        for screen in &screens {
            if let Some(&id) = self.monitor_handles.get(&screen.handle) {
                if self.monitor_dimensions.get(&id) != Some(&screen.dimension) {
                    let old_dim = self.monitor_dimensions.get(&id).copied();
                    tracing::info!(
                        name = %screen.name,
                        ?old_dim,
                        new_dim = ?screen.dimension,
                        "Monitor dimension changed"
                    );
                    self.monitor_dimensions.insert(id, screen.dimension);
                    self.hub.update_monitor_dimension(id, screen.dimension);
                }
            }
        }
    }
}

fn send_layout_frame(frame: LayoutFrame, app_hwnd: AppHandle) {
    let boxed = Box::new(frame);
    let ptr = Box::into_raw(boxed) as usize;
    unsafe { PostMessageW(Some(app_hwnd.hwnd()), WM_APP_LAYOUT, WPARAM(ptr), LPARAM(0)).ok() };
}

fn send_config(config: Config, app_hwnd: AppHandle) {
    let boxed = Box::new(config);
    let ptr = Box::into_raw(boxed) as usize;
    unsafe { PostMessageW(Some(app_hwnd.hwnd()), WM_APP_CONFIG, WPARAM(ptr), LPARAM(0)).ok() };
}

fn on_open_actions(
    process: &str,
    title: Option<&str>,
    rules: &[WindowsOnOpenRule],
) -> Option<Actions> {
    let rule = rules.iter().find(|r| r.window.matches(process, title))?;
    tracing::debug!(%process, ?title, actions = %rule.run, "Running on_open actions");
    Some(rule.run.clone())
}

fn should_ignore(process: &str, title: Option<&str>, rules: &[WindowsWindow]) -> bool {
    if let Some(rule) = rules.iter().find(|r| r.matches(process, title)) {
        tracing::debug!(%process, ?title, ?rule, "Window ignored by rule");
        return true;
    }
    false
}
