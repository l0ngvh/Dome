mod placement_tracker;
mod registry;
pub(super) mod throttle;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::action::{Action, Actions, FocusTarget, HubAction, MoveTarget, ToggleTarget};
use crate::config::{Config, WindowsOnOpenRule, WindowsWindow};
use crate::core::{
    Child, ContainerId, ContainerPlacement, Dimension, Hub, MonitorId, MonitorLayout, SpawnMode,
    WindowId,
};

use self::placement_tracker::PlacementTracker;
use self::registry::{WindowEntry, WindowRegistry};
use super::ScreenInfo;
use super::external::{HwndId, ManageExternalHwnd};
use super::handle::{WindowMode, is_fullscreen};

use super::{WM_APP_CONFIG, WM_APP_LAYOUT, WM_APP_TITLE};

#[expect(
    clippy::large_enum_variant,
    reason = "These messages aren't bottleneck right now"
)]
pub(super) enum HubEvent {
    AppInitialized {
        app_hwnd: AppHandle,
        windows: Vec<Arc<dyn ManageExternalHwnd>>,
    },
    WindowCreated(Arc<dyn ManageExternalHwnd>),
    WindowDestroyed(Arc<dyn ManageExternalHwnd>),
    WindowMinimized(Arc<dyn ManageExternalHwnd>),
    WindowFocused(Arc<dyn ManageExternalHwnd>),
    WindowTitleChanged(Arc<dyn ManageExternalHwnd>),
    MoveSizeStart(Arc<dyn ManageExternalHwnd>),
    MoveSizeEnd(Arc<dyn ManageExternalHwnd>),
    LocationChanged(Arc<dyn ManageExternalHwnd>),
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
/// fullscreen, exclusive fullscreen, and windows currently being moved
/// or resized. Wm must not touch them.
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
    pub(super) ext: Arc<dyn ManageExternalHwnd>,
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
    pub(super) titles: Vec<(HwndId, Option<String>)>,
    pub(super) container_renders: Vec<ContainerRender>,
}

/// Abstraction for sending layout frames, title updates, and config changes
/// from Dome to Wm. Production uses `WmSender` (PostMessageW). Tests use
/// `TestSender` (collects frames into a Vec).
pub(in crate::platform::windows) trait FrameSender: Send {
    fn send_frame(&self, frame: LayoutFrame);
    fn send_titles(&self, update: TitleUpdate);
    fn send_config(&self, config: Config);
}

/// Production sender — delivers frames to the Wm thread via PostMessageW.
struct WmSender {
    app_hwnd: AppHandle,
}

impl FrameSender for WmSender {
    fn send_frame(&self, frame: LayoutFrame) {
        let ptr = Box::into_raw(Box::new(frame)) as usize;
        unsafe {
            PostMessageW(
                Some(self.app_hwnd.hwnd()),
                WM_APP_LAYOUT,
                WPARAM(ptr),
                LPARAM(0),
            )
            .ok()
        };
    }

    fn send_titles(&self, update: TitleUpdate) {
        let ptr = Box::into_raw(Box::new(update)) as usize;
        unsafe {
            PostMessageW(
                Some(self.app_hwnd.hwnd()),
                WM_APP_TITLE,
                WPARAM(ptr),
                LPARAM(0),
            )
            .ok()
        };
    }

    fn send_config(&self, config: Config) {
        let ptr = Box::into_raw(Box::new(config)) as usize;
        unsafe {
            PostMessageW(
                Some(self.app_hwnd.hwnd()),
                WM_APP_CONFIG,
                WPARAM(ptr),
                LPARAM(0),
            )
            .ok()
        };
    }
}

pub(in crate::platform::windows) struct Dome {
    hub: Hub,
    registry: WindowRegistry,
    monitor_handles: HashMap<isize, MonitorId>,
    monitor_dimensions: HashMap<MonitorId, Dimension>,
    displayed: HashMap<MonitorId, DisplayedMonitor>,
    config: Config,
    sender: Option<Box<dyn FrameSender>>,
    placement_tracker: PlacementTracker,
}

impl Dome {
    pub(in crate::platform::windows) fn new(
        config: Config,
        screens: Vec<ScreenInfo>,
        sender: Option<Box<dyn FrameSender>>,
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
            registry: WindowRegistry::new(),
            monitor_handles,
            monitor_dimensions,
            displayed: HashMap::new(),
            config,
            sender,
            placement_tracker: PlacementTracker::new(),
        }
    }

    pub(in crate::platform::windows) fn app_initialized(
        &mut self,
        app_hwnd: AppHandle,
        windows: Vec<Arc<dyn ManageExternalHwnd>>,
    ) -> Vec<Actions> {
        self.sender = Some(Box::new(WmSender { app_hwnd }));
        let mut on_open = Vec::new();
        for ext in windows {
            if let Some(actions) = self.try_manage_window(ext) {
                on_open.push(actions);
            }
        }
        self.apply_layout();
        on_open
    }

    #[cfg(test)]
    pub(in crate::platform::windows) fn config(&self) -> &Config {
        &self.config
    }

    pub(in crate::platform::windows) fn config_changed(&mut self, new_config: Config) {
        self.hub.sync_config(new_config.clone().into());
        if let Some(s) = &self.sender {
            s.send_config(new_config.clone());
        }
        self.config = new_config;
        tracing::info!("Config reloaded");
        self.apply_layout();
    }

    pub(in crate::platform::windows) fn window_created(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
    ) -> Option<Actions> {
        let actions = if !self.registry.contains_hwnd(ext.id()) {
            self.try_manage_window(ext)
        } else {
            None
        };
        self.apply_layout();
        actions
    }

    pub(in crate::platform::windows) fn window_destroyed(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
    ) {
        let _span = tracing::info_span!("window_destroyed").entered();
        self.remove_window(ext.id());
        self.apply_layout();
    }

    pub(in crate::platform::windows) fn window_minimized(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
    ) {
        let _span = tracing::info_span!("window_minimized").entered();
        let id_key = ext.id();
        let is_fullscreen = self
            .registry
            .get_id(id_key)
            .is_some_and(|id| self.hub.get_window(id).is_fullscreen());
        if !is_fullscreen {
            self.remove_window(id_key);
        }
        self.apply_layout();
    }

    pub(in crate::platform::windows) fn move_size_started(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
    ) {
        self.placement_tracker.drag_started(ext.id());
    }

    pub(in crate::platform::windows) fn move_size_ended(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
    ) {
        self.placement_tracker.drag_ended(ext.id());
        self.handle_resize(ext.id());
    }

    pub(in crate::platform::windows) fn location_changed(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
    ) -> bool {
        self.placement_tracker.location_changed(ext.id())
    }

    pub(in crate::platform::windows) fn title_changed(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
    ) -> Option<Actions> {
        let id_key = ext.id();
        let actions = if self.registry.contains_hwnd(id_key) {
            let new_title = ext.get_window_title();
            self.send_title_update(vec![(id_key, new_title)]);
            None
        } else {
            // Some apps have a brief moment where their title is empty
            self.try_manage_window(ext)
        };
        self.apply_layout();
        actions
    }

    pub(in crate::platform::windows) fn screens_changed(&mut self, screens: Vec<ScreenInfo>) {
        tracing::info!(count = screens.len(), "Screen parameters changed");
        self.update_screens(screens);
        self.apply_layout();
    }

    pub(in crate::platform::windows) fn run_hub_actions(&mut self, actions: &Actions) {
        if actions.is_empty() {
            return;
        }
        for action in actions {
            if let Action::Hub(hub) = action {
                self.execute_hub_action(hub);
            }
        }
        self.apply_layout();
    }

    pub(in crate::platform::windows) fn tab_clicked(
        &mut self,
        container_id: ContainerId,
        tab_idx: usize,
    ) {
        self.hub.focus_tab_index(container_id, tab_idx);
        self.apply_layout();
    }

    pub(in crate::platform::windows) fn set_fullscreen(&mut self, id: WindowId) {
        if let Some(info) = self.registry.get_mut(id) {
            info.mode = WindowMode::FullscreenExclusive;
            if !self.hub.get_window(id).is_fullscreen() {
                self.hub.set_fullscreen(id);
            }
        }
        self.apply_layout();
    }

    fn try_manage_window(&mut self, ext: Arc<dyn ManageExternalHwnd>) -> Option<Actions> {
        if !ext.is_manageable() {
            return None;
        }
        let title = ext.get_window_title();
        let process = ext.get_process_name().unwrap_or_default();
        if should_ignore(&process, title.as_deref(), &self.config.windows.ignore) {
            return None;
        }
        let actions = on_open_actions(&process, title.as_deref(), &self.config.windows.on_open);
        self.insert_window(ext, title, process);
        actions
    }

    fn insert_window(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
        title: Option<String>,
        process: String,
    ) {
        let id_key = ext.id();
        let dim = ext.get_dimension();
        let monitor = self.find_monitor_dimension_from_ext(&*ext);

        let mode = ext.initial_window_mode(monitor.as_ref());
        let id = match mode {
            WindowMode::FullscreenBorderless
            | WindowMode::ManagedFullscreen
            | WindowMode::FullscreenExclusive => self.hub.insert_fullscreen(),
            WindowMode::Float => self.hub.insert_float(dim),
            WindowMode::Tiling => self.hub.insert_tiling(),
        };
        self.set_constraints(id, &*ext);

        self.registry.insert(
            id_key,
            id,
            WindowEntry {
                ext,
                mode,
                title,
                process,
            },
        );
    }

    fn remove_window(&mut self, id_key: HwndId) {
        self.placement_tracker.clear(id_key);
        if let Some(id) = self.registry.remove_by_hwnd(id_key) {
            self.hub.delete_window(id);
        }
    }

    fn set_constraints(&mut self, id: WindowId, ext: &dyn ManageExternalHwnd) {
        let border = self.config.border_size;
        let (min_w, min_h, max_w, max_h) = ext.get_size_constraints();
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

    fn find_monitor_dimension_from_ext(&self, ext: &dyn ManageExternalHwnd) -> Option<Dimension> {
        let handle = ext.get_monitor_handle()?;
        let id = self.monitor_handles.get(&handle)?;
        self.monitor_dimensions.get(id).copied()
    }

    pub(in crate::platform::windows) fn handle_focus(&mut self, id_key: HwndId) {
        if let Some(id) = self.registry.get_id(id_key) {
            self.hub.set_focus(id);
            tracing::info!(?id_key, "Window focused");
            self.apply_layout();
        }
    }

    /// Called by the run loop when a drag safety timeout or resize debounce
    /// timer fires. Removes the window from the placement tracker and
    /// re-evaluates its layout.
    pub(in crate::platform::windows) fn placement_timeout(&mut self, id: HwndId) {
        self.placement_tracker.clear(id);
        self.handle_resize(id);
    }

    fn handle_resize(&mut self, id_key: HwndId) {
        let Some(id) = self.registry.get_id(id_key) else {
            return;
        };
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        if entry.mode == WindowMode::FullscreenExclusive {
            return;
        }
        let ext = entry.ext.clone();
        self.set_constraints(id, &*ext);
        self.check_fullscreen_state(id, &*ext);
        self.apply_layout();
    }

    fn check_fullscreen_state(&mut self, id: WindowId, ext: &dyn ManageExternalHwnd) {
        let Some(monitor_dim) = self.find_monitor_dimension_from_ext(ext) else {
            return;
        };

        let was_fs = self.hub.get_window(id).is_fullscreen();
        let window_dim = ext.get_dimension();
        let is_fs = is_fullscreen(&window_dim, &monitor_dim);
        if was_fs != is_fs {
            tracing::debug!(
                ?window_dim,
                ?monitor_dim,
                was_fs,
                is_fs,
                "Fullscreen state changed"
            );
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

    fn execute_hub_action(&mut self, action: &HubAction) {
        match action {
            HubAction::Focus { target } => match target {
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
            HubAction::Move { target } => match target {
                MoveTarget::Up => self.hub.move_up(),
                MoveTarget::Down => self.hub.move_down(),
                MoveTarget::Left => self.hub.move_left(),
                MoveTarget::Right => self.hub.move_right(),
                MoveTarget::Workspace { name } => self.hub.move_focused_to_workspace(name),
                MoveTarget::Monitor { target } => self.hub.move_focused_to_monitor(target),
            },
            HubAction::Toggle { target } => match target {
                ToggleTarget::SpawnDirection => self.hub.toggle_spawn_mode(),
                ToggleTarget::Direction => self.hub.toggle_direction(),
                ToggleTarget::Layout => self.hub.toggle_container_layout(),
                ToggleTarget::Float => self.hub.toggle_float(),
                ToggleTarget::Fullscreen => self.hub.toggle_fullscreen(),
            },
        }
    }

    fn apply_layout(&mut self) {
        let changes = self.hub.drain_changes();

        let created_windows: Vec<WindowCreate> = changes
            .created_windows
            .iter()
            .filter_map(|&id| {
                let info = self.registry.get(id)?;
                Some(WindowCreate {
                    ext: info.ext.clone(),
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
                    if let Some(info) = self.registry.get_mut(id) {
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
                        if let Some(info) = self.registry.get_mut(wp.id) {
                            info.mode = if wp.is_float {
                                WindowMode::Float
                            } else {
                                WindowMode::Tiling
                            };
                            if self.placement_tracker.is_moving(info.ext.id()) {
                                continue;
                            }
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

        if let Some(s) = &self.sender {
            s.send_frame(frame);
        }
    }

    fn send_title_update(&self, titles: Vec<(HwndId, Option<String>)>) {
        let Some(sender) = &self.sender else {
            return;
        };

        let affected_ids: HashSet<WindowId> = titles
            .iter()
            .filter_map(|(h, _)| self.registry.get_id(*h))
            .collect();
        let container_renders = self.build_container_renders_for(&affected_ids);

        let update = TitleUpdate {
            titles,
            container_renders,
        };
        sender.send_titles(update);
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

        let windows: Vec<_> = self.registry.iter().collect();
        for (_, id) in windows {
            if self
                .registry
                .get(id)
                .is_some_and(|i| i.mode == WindowMode::FullscreenExclusive)
            {
                continue;
            }
            if let Some(entry) = self.registry.get(id) {
                let ext = entry.ext.clone();
                self.set_constraints(id, &*ext);
            }
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
