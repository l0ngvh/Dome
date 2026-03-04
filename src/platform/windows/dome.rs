use std::collections::HashSet;
use std::time::Duration;

use calloop::channel::{Channel, Event as ChannelEvent};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle, LoopSignal};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_QUIT};

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Config, WindowsOnOpenRule, WindowsWindow};
use crate::core::{
    Child, ContainerId, ContainerPlacement, Dimension, DisplayMode, Hub, WindowId, WindowPlacement,
};

use super::monitor::MonitorRegistry;
use super::recovery;
use super::throttle::{Throttle, ThrottleResult};
use super::window::{Registry, Taskbar, WindowHandle, enum_windows, initial_display_mode};
use super::{ScreenInfo, compute_global_bounds};

pub(super) const WM_APP_OVERLAY: u32 = 0x8001;
pub(super) const WM_APP_CONFIG: u32 = 0x8002;

const FOCUS_THROTTLE: Duration = Duration::from_millis(50);
const RESIZE_THROTTLE: Duration = Duration::from_millis(16);

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
    TabClicked(ContainerId, usize),
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
    pub(super) windows: Vec<WindowPlacement>,
    pub(super) containers: Vec<ContainerOverlayData>,
}

pub(super) struct OverlayCreate {
    pub(super) window_id: WindowId,
    pub(super) hwnd: HWND,
    pub(super) is_float: bool,
}

#[derive(Clone)]
pub(super) struct ContainerOverlayData {
    pub(super) placement: ContainerPlacement,
    pub(super) tab_titles: Vec<String>,
}

#[derive(Clone, Copy)]
enum ThrottleKind {
    Focus,
    Resize,
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
    signal: LoopSignal,
    handle: LoopHandle<'static, Self>,
    focus_throttle: Throttle<WindowHandle>,
    resize_throttle: Throttle<WindowHandle>,
}

impl Dome {
    pub(super) fn new(
        config: Config,
        screens: Vec<ScreenInfo>,
        global_bounds: Dimension,
        handle: LoopHandle<'static, Self>,
        signal: LoopSignal,
    ) -> Self {
        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_registry =
            MonitorRegistry::new(primary.handle, primary_monitor_id, primary.dimension);
        tracing::info!(
            name = %primary.name,
            handle = ?primary.handle,
            dimension = ?primary.dimension,
            "Primary monitor"
        );

        for screen in &screens {
            if screen.handle != primary.handle {
                let id = hub.add_monitor(screen.name.clone(), screen.dimension);
                monitor_registry.insert(screen.handle, id, screen.dimension);
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
            signal,
            handle,
            focus_throttle: Throttle::new(FOCUS_THROTTLE),
            resize_throttle: Throttle::new(RESIZE_THROTTLE),
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

    pub(super) fn run(
        mut self,
        channel: Channel<HubEvent>,
        mut event_loop: EventLoop<'static, Self>,
    ) {
        event_loop
            .handle()
            .insert_source(channel, |event, _, dome| match event {
                ChannelEvent::Msg(hub_event) => {
                    let last_focus = dome
                        .hub
                        .get_workspace(dome.hub.current_workspace())
                        .focused();
                    let (creates, deletes) = dome.handle_event(hub_event);
                    dome.apply_layout(creates, deletes, last_focus);
                }
                ChannelEvent::Closed => dome.signal.stop(),
            })
            .expect("Failed to insert channel source");

        event_loop
            .run(None, &mut self, |_| {})
            .expect("Event loop failed");
    }

    fn handle_event(&mut self, event: HubEvent) -> (Vec<OverlayCreate>, Vec<WindowId>) {
        let mut creates = Vec::new();
        let mut deletes = Vec::new();

        match event {
            HubEvent::AppInitialized(hwnd) => {
                self.app_hwnd = Some(hwnd);
                if let Err(e) = enum_windows(|hwnd| {
                    let handle = WindowHandle::new(hwnd);
                    if handle.is_manageable()
                        && !should_ignore(&handle, &self.config.windows.ignore)
                    {
                        // Hide before first frame — window may end up offscreen due to
                        // viewport scrolling. apply_layout will show the visible ones.
                        handle.hide();
                        let id = self.insert_window(&handle);
                        let is_float = self.hub.get_window(id).is_float();
                        creates.push(OverlayCreate {
                            window_id: id,
                            hwnd,
                            is_float,
                        });
                    }
                }) {
                    tracing::warn!("Failed to enumerate windows: {e}");
                }
            }
            HubEvent::Shutdown => self.signal.stop(),
            HubEvent::ConfigChanged(new_config) => {
                self.hub.sync_config(new_config.clone().into());
                if let Some(app_hwnd) = self.app_hwnd {
                    send_config(new_config.clone(), app_hwnd);
                }
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
                creates.push(OverlayCreate {
                    window_id: id,
                    hwnd,
                    is_float,
                });
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
                self.submit_focus(handle);
                return (creates, deletes);
            }
            HubEvent::WindowMovedOrResized(handle) => {
                self.submit_resize(handle);
                return (creates, deletes);
            }
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
                creates.push(OverlayCreate {
                    window_id: id,
                    hwnd,
                    is_float,
                });
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
            HubEvent::TabClicked(container_id, tab_idx) => {
                self.hub.focus_tab_index(container_id, tab_idx);
            }
        }

        (creates, deletes)
    }

    fn submit_focus(&mut self, handle: WindowHandle) {
        match self.focus_throttle.submit(handle.clone()) {
            ThrottleResult::Send(h) => self.handle_focus(h),
            ThrottleResult::Pending => {
                if let Some(delay) = self.focus_throttle.schedule_delay() {
                    self.schedule_throttle_timer(delay, ThrottleKind::Focus);
                }
            }
        }
    }

    fn submit_resize(&mut self, handle: WindowHandle) {
        match self.resize_throttle.submit(handle.clone()) {
            ThrottleResult::Send(h) => self.handle_resize(h),
            ThrottleResult::Pending => {
                if let Some(delay) = self.resize_throttle.schedule_delay() {
                    self.schedule_throttle_timer(delay, ThrottleKind::Resize);
                }
            }
        }
    }

    fn schedule_throttle_timer(&mut self, delay: Duration, kind: ThrottleKind) {
        let timer = Timer::from_duration(delay);
        let token = self
            .handle
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
            ThrottleKind::Focus => self.focus_throttle.set_timer_token(token),
            ThrottleKind::Resize => self.resize_throttle.set_timer_token(token),
        }
    }

    fn handle_focus(&mut self, handle: WindowHandle) {
        let _span = tracing::info_span!("window_focused", %handle).entered();
        if let Some(id) = self.registry.get_id(&handle) {
            let last_focus = self
                .hub
                .get_workspace(self.hub.current_workspace())
                .focused();
            self.hub.set_focus(id);
            tracing::info!("Window focused");
            self.apply_layout(Vec::new(), Vec::new(), last_focus);
        }
    }

    fn handle_resize(&mut self, handle: WindowHandle) {
        let last_focus = self
            .hub
            .get_workspace(self.hub.current_workspace())
            .focused();
        self.check_fullscreen_state(&handle);
        self.apply_layout(Vec::new(), Vec::new(), last_focus);
    }

    fn insert_window(&mut self, handle: &WindowHandle) -> WindowId {
        recovery::track(handle);
        let monitor = self.monitor_registry.find_monitor_dimension(handle.hwnd());
        let id = match initial_display_mode(handle, monitor.as_ref()) {
            DisplayMode::Fullscreen => self.hub.insert_fullscreen(),
            DisplayMode::Tiling => self.hub.insert_tiling(),
            DisplayMode::Float => self.hub.insert_float(handle.dimension()),
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
                    if let Some(hwnd) = self.app_hwnd {
                        unsafe {
                            PostMessageW(Some(hwnd.hwnd()), WM_QUIT, WPARAM(0), LPARAM(0)).ok()
                        };
                    }
                }
            }
        }
    }

    fn apply_layout(
        &mut self,
        creates: Vec<OverlayCreate>,
        deletes: Vec<WindowId>,
        last_focus: Option<Child>,
    ) {
        let focused = self
            .hub
            .get_workspace(self.hub.current_workspace())
            .focused();

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

        for mp in self.hub.get_visible_placements() {
            if let Some(entry) = self.monitor_registry.get_entry_mut(mp.monitor_id) {
                let (wins, conts) = entry.apply_placements(
                    &mp.layout,
                    &mut self.registry,
                    &mut self.taskbar,
                    &mut self.hub,
                    &self.config,
                );
                frame.windows.extend(wins);
                frame.containers.extend(conts);
            }
        }

        // Focus window if focus changed
        if focused != last_focus
            && let Some(Child::Window(id)) = focused
            && let Some(handle) = self.registry.get_handle(id)
        {
            handle.focus();
        }

        if let Some(app_hwnd) = self.app_hwnd {
            send_overlay_frame(frame, app_hwnd);
        }
    }

    fn check_fullscreen_state(&mut self, handle: &WindowHandle) {
        let Some(window_id) = self.registry.get_id(handle) else {
            return;
        };
        let Some(monitor) = self.monitor_registry.find_monitor_dimension(handle.hwnd()) else {
            return;
        };
        let is_fs = handle.is_fullscreen(&monitor);
        let was_fs = self
            .registry
            .get_handle(window_id)
            .is_some_and(|h| h.fullscreen());
        match (was_fs, is_fs) {
            (false, true) => {
                if let Some(h) = self.registry.get_handle_mut(window_id) {
                    h.sync_fullscreen(true);
                }
                self.hub.set_fullscreen(window_id);
            }
            (true, false) => {
                if let Some(h) = self.registry.get_handle_mut(window_id) {
                    h.sync_fullscreen(false);
                }
                self.hub.unset_fullscreen(window_id);
            }
            _ => {}
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
    unsafe {
        PostMessageW(
            Some(app_hwnd.hwnd()),
            WM_APP_OVERLAY,
            WPARAM(ptr),
            LPARAM(0),
        )
        .ok()
    };
}

fn send_config(config: Config, app_hwnd: AppHandle) {
    let boxed = Box::new(config);
    let ptr = Box::into_raw(boxed) as usize;
    unsafe { PostMessageW(Some(app_hwnd.hwnd()), WM_APP_CONFIG, WPARAM(ptr), LPARAM(0)).ok() };
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

    for screen in &screens {
        if !registry.map.contains_key(&screen.handle) {
            let id = hub.add_monitor(screen.name.clone(), screen.dimension);
            registry.insert(screen.handle, id, screen.dimension);
            tracing::info!(
                name = %screen.name,
                handle = ?screen.handle,
                dimension = ?screen.dimension,
                "Monitor added"
            );
        }
    }

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

    for screen in &screens {
        if let Some(entry) = registry.map.get_mut(&screen.handle) {
            let monitor_id = entry.id;
            entry.dimension = screen.dimension;
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
