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
    Child, ContainerId, ContainerPlacement, Dimension, Hub, MonitorId, MonitorLayout, WindowId,
};

use super::ScreenInfo;
use super::recovery;
use super::throttle::{Throttle, ThrottleResult};
use super::window::{
    ManagedHwnd, WindowMode, enum_windows, get_dimension, get_process_name, get_size_constraints,
    get_window_title, initial_window_mode, is_fullscreen, is_manageable,
};

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

pub(super) struct LayoutFrame {
    pub(super) monitors: Vec<FrameMonitor>,
    pub(super) creates: Vec<WindowCreate>,
    pub(super) deletes: Vec<WindowId>,
    pub(super) focused: Option<WindowId>,
    pub(super) container_overlays: Vec<ContainerOverlayData>,
}

pub(super) struct FrameMonitor {
    pub(super) monitor_id: MonitorId,
    pub(super) layout: MonitorLayout,
    pub(super) dimension: Dimension,
}

pub(super) struct WindowCreate {
    pub(super) hwnd: HWND,
    pub(super) id: WindowId,
    pub(super) mode: WindowMode,
    pub(super) title: Option<String>,
    pub(super) process: String,
}

#[derive(Clone)]
pub(super) struct ContainerOverlayData {
    pub(super) placement: ContainerPlacement,
    pub(super) children: Vec<Child>,
}

pub(super) struct TitleUpdate {
    pub(super) titles: Vec<(ManagedHwnd, Option<String>)>,
    pub(super) container_overlays: Vec<ContainerOverlayData>,
}

#[derive(Clone, Copy)]
enum ThrottleKind {
    Focus,
    Resize,
}

pub(super) struct Dome {
    hub: Hub,
    window_map: HashMap<ManagedHwnd, WindowId>,
    window_info: HashMap<WindowId, bool>,
    monitor_handles: HashMap<isize, MonitorId>,
    monitor_dimensions: HashMap<MonitorId, Dimension>,
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

        Self {
            hub,
            window_map: HashMap::new(),
            window_info: HashMap::new(),
            monitor_handles,
            monitor_dimensions,
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
                    if let Some((creates, deletes)) = dome.handle_event(hub_event) {
                        dome.apply_layout(creates, deletes);
                    }
                }
                ChannelEvent::Closed => dome.signal.stop(),
            })
            .expect("Failed to insert channel source");

        event_loop
            .run(None, &mut self, |_| {})
            .expect("Event loop failed");
    }

    fn handle_event(&mut self, event: HubEvent) -> Option<(Vec<WindowCreate>, Vec<WindowId>)> {
        let mut creates = Vec::new();
        let mut deletes = Vec::new();

        match event {
            HubEvent::AppInitialized(hwnd) => {
                self.app_hwnd = Some(hwnd);
                if let Err(e) = enum_windows(|hwnd| {
                    if let Some(wc) = self.try_manage_window(hwnd) {
                        creates.push(wc);
                    }
                }) {
                    tracing::warn!("Failed to enumerate windows: {e}");
                }
            }
            HubEvent::Shutdown => {
                self.signal.stop();
                return None;
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
                    return Some((creates, deletes));
                }
                if let Some(wc) = self.try_manage_window(h.hwnd()) {
                    creates.push(wc);
                }
            }
            HubEvent::WindowDestroyed(h) => {
                let _span = tracing::info_span!("window_destroyed").entered();
                if let Some(id) = self.remove_window(h) {
                    deletes.push(id);
                }
            }
            HubEvent::WindowMinimized(h) => {
                let _span = tracing::info_span!("window_minimized").entered();
                let is_fullscreen = self
                    .window_map
                    .get(&h)
                    .map(|&id| self.hub.get_window(id).is_fullscreen())
                    .unwrap_or(false);
                if !is_fullscreen {
                    if let Some(id) = self.remove_window(h) {
                        deletes.push(id);
                    }
                }
            }
            HubEvent::WindowFocused(h) => {
                self.submit_focus(h);
                return Some((creates, deletes));
            }
            HubEvent::WindowMovedOrResized(h) => {
                self.submit_resize(h);
                return Some((creates, deletes));
            }
            HubEvent::WindowTitleChanged(h) => {
                if self.window_map.contains_key(&h) {
                    let new_title = get_window_title(h.hwnd());
                    self.send_title_update(vec![(h, new_title)]);
                    return Some((creates, deletes));
                }
                // Some apps have a brief moment where their title is empty
                if let Some(wc) = self.try_manage_window(h.hwnd()) {
                    creates.push(wc);
                }
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
                if let Some(exclusive) = self.window_info.get_mut(&id) {
                    *exclusive = true;
                    if !self.hub.get_window(id).is_fullscreen() {
                        self.hub.set_fullscreen(id);
                    }
                }
            }
        }

        Some((creates, deletes))
    }

    fn try_manage_window(&mut self, hwnd: HWND) -> Option<WindowCreate> {
        if !is_manageable(hwnd) {
            return None;
        }
        let title = get_window_title(hwnd);
        let process = get_process_name(hwnd).unwrap_or_default();
        if should_ignore(&process, title.as_deref(), &self.config.windows.ignore) {
            return None;
        }
        let wc = self.insert_window(hwnd, title, process);
        if let Some(actions) = on_open_actions(
            &wc.process,
            wc.title.as_deref(),
            &self.config.windows.on_open,
        ) {
            self.execute_actions(&actions);
        }
        Some(wc)
    }

    fn insert_window(
        &mut self,
        hwnd: HWND,
        title: Option<String>,
        process: String,
    ) -> WindowCreate {
        let managed = ManagedHwnd::new(hwnd);
        let dim = get_dimension(hwnd);
        let monitor = self.find_monitor_dimension(hwnd);
        recovery::track(hwnd, dim);

        let mode = initial_window_mode(hwnd, monitor.as_ref());
        let id = match mode {
            WindowMode::FullscreenBorderless | WindowMode::FullscreenExclusive => {
                self.hub.insert_fullscreen()
            }
            WindowMode::Float => self.hub.insert_float(dim),
            WindowMode::Tiling => self.hub.insert_tiling(),
        };
        self.set_constraints(id, hwnd);

        self.window_map.insert(managed, id);
        self.window_info
            .insert(id, matches!(mode, WindowMode::FullscreenExclusive));
        WindowCreate {
            hwnd,
            id,
            mode,
            title,
            process,
        }
    }

    fn remove_window(&mut self, h: ManagedHwnd) -> Option<WindowId> {
        if let Some(id) = self.window_map.remove(&h) {
            self.window_info.remove(&id);
            recovery::untrack(h);
            self.hub.delete_window(id);
            return Some(id);
        }
        None
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
            self.apply_layout(Vec::new(), Vec::new());
        }
    }

    fn handle_resize(&mut self, h: ManagedHwnd) {
        let Some(&id) = self.window_map.get(&h) else {
            return;
        };
        if self.window_info.get(&id) == Some(&true) {
            return;
        }
        self.set_constraints(id, h.hwnd());
        self.check_fullscreen_state(h);
        self.apply_layout(Vec::new(), Vec::new());
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

    fn apply_layout(&mut self, creates: Vec<WindowCreate>, deletes: Vec<WindowId>) {
        let focused = self
            .hub
            .get_workspace(self.hub.current_workspace())
            .focused();

        let placements = self.hub.get_visible_placements();

        let monitors: Vec<FrameMonitor> = placements
            .into_iter()
            .map(|mp| {
                let dimension = self
                    .monitor_dimensions
                    .get(&mp.monitor_id)
                    .copied()
                    .unwrap_or_default();
                FrameMonitor {
                    monitor_id: mp.monitor_id,
                    layout: mp.layout,
                    dimension,
                }
            })
            .collect();

        let mut container_overlays = Vec::new();
        for fm in &monitors {
            if let MonitorLayout::Normal { containers, .. } = &fm.layout {
                for cp in containers {
                    if !cp.is_tabbed && !cp.is_focused {
                        continue;
                    }
                    let children = if cp.is_tabbed {
                        self.hub.get_container(cp.id).children().to_vec()
                    } else {
                        vec![]
                    };
                    container_overlays.push(ContainerOverlayData {
                        placement: *cp,
                        children,
                    });
                }
            }
        }

        let frame = LayoutFrame {
            monitors,
            creates,
            deletes,
            focused: match focused {
                Some(Child::Window(id)) => Some(id),
                _ => None,
            },
            container_overlays,
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
        let container_overlays = self.build_container_overlays_for(&affected_ids);

        let update = TitleUpdate {
            titles,
            container_overlays,
        };
        let boxed = Box::new(update);
        let ptr = Box::into_raw(boxed) as usize;
        unsafe { PostMessageW(Some(app_hwnd.hwnd()), WM_APP_TITLE, WPARAM(ptr), LPARAM(0)).ok() };
    }

    fn build_container_overlays_for(
        &self,
        affected_ids: &HashSet<WindowId>,
    ) -> Vec<ContainerOverlayData> {
        let mut overlays = Vec::new();
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
                        overlays.push(ContainerOverlayData {
                            placement: *cp,
                            children,
                        });
                    }
                }
            }
        }
        overlays
    }

    fn update_screens(&mut self, screens: Vec<ScreenInfo>) {
        if screens.is_empty() {
            tracing::warn!("Empty screen list, skipping update");
            return;
        }
        self.reconcile_monitors(screens);

        let windows: Vec<_> = self.window_map.iter().map(|(&h, &id)| (h, id)).collect();
        for (managed, id) in windows {
            if self.window_info.get(&id) == Some(&true) {
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

impl Drop for Dome {
    fn drop(&mut self) {
        recovery::restore_all();
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
