pub(super) mod overlay;
mod placement_tracker;
mod recovery;
mod registry;
mod window;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::action::{Actions, FocusTarget, HubAction, MoveTarget, ToggleTarget};
use crate::config::{Config, WindowsOnOpenRule, WindowsWindow};
use crate::core::{
    Child, ContainerId, ContainerPlacement, Dimension, Hub, MonitorId, MonitorLayout, WindowId,
    WindowPlacement,
};

use self::overlay::{ContainerOverlayApi, WindowOverlayApi};
use self::placement_tracker::PlacementTracker;
use self::recovery::Recovery;
use self::registry::{WindowEntry, WindowRegistry};
use self::window::{PositionedState, WindowState};
use super::ScreenInfo;
use super::external::{HwndId, ManageExternalHwnd, ZOrder};
use super::handle::is_fullscreen;
use super::taskbar::ManageTaskbar;

#[expect(
    clippy::large_enum_variant,
    reason = "These messages aren't bottleneck right now"
)]
pub(super) enum HubEvent {
    WindowCreated(Arc<dyn ManageExternalHwnd>),
    WindowDestroyed(Arc<dyn ManageExternalHwnd>),
    WindowMinimized(Arc<dyn ManageExternalHwnd>),
    WindowFocused(Arc<dyn ManageExternalHwnd>),
    WindowTitleChanged(Arc<dyn ManageExternalHwnd>),
    MoveSizeStart(Arc<dyn ManageExternalHwnd>),
    MoveSizeEnd(Arc<dyn ManageExternalHwnd>),
    LocationChanged(Arc<dyn ManageExternalHwnd>),
    Action(Actions),
    ConfigChanged(Config),
    TabClicked(ContainerId, usize),
    Shutdown,
}

struct DisplayedMonitor {
    window_ids: Vec<WindowId>,
    container_ids: Vec<ContainerId>,
}

#[derive(Clone)]
struct ContainerRender {
    placement: ContainerPlacement,
    children: Vec<Child>,
}

pub(super) trait CreateOverlay {
    fn create_window_overlay(&self) -> anyhow::Result<Box<dyn WindowOverlayApi>>;
    fn create_container_overlay(
        &self,
        config: Config,
    ) -> anyhow::Result<Box<dyn ContainerOverlayApi>>;
}

pub(super) trait QueryDisplay {
    fn get_all_screens(&self) -> anyhow::Result<Vec<ScreenInfo>>;
    /// Returns the hwnd of the foreground window if D3D exclusive fullscreen is active.
    fn get_exclusive_fullscreen_hwnd(&self) -> Option<HwndId>;
}

pub(super) struct Dome {
    hub: Hub,
    registry: WindowRegistry,
    monitor_handles: HashMap<isize, MonitorId>,
    monitor_dimensions: HashMap<MonitorId, Dimension>,
    displayed: HashMap<MonitorId, DisplayedMonitor>,
    config: Config,
    taskbar: Arc<dyn ManageTaskbar>,
    overlays: Box<dyn CreateOverlay>,
    display: Box<dyn QueryDisplay>,
    container_overlays: HashMap<ContainerId, Box<dyn ContainerOverlayApi>>,
    last_focused: Option<WindowId>,
    placement_tracker: PlacementTracker,
    recovery: Recovery,
}

impl Drop for Dome {
    fn drop(&mut self) {
        self.recovery.restore_all();
    }
}

impl Dome {
    pub(super) fn new(
        config: Config,
        taskbar: Arc<dyn ManageTaskbar>,
        overlays: Box<dyn CreateOverlay>,
        display: Box<dyn QueryDisplay>,
    ) -> anyhow::Result<Self> {
        let screens = display.get_all_screens()?;
        anyhow::ensure!(!screens.is_empty(), "No monitors detected");
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

        Ok(Self {
            hub,
            registry: WindowRegistry::new(),
            monitor_handles,
            monitor_dimensions,
            displayed: HashMap::new(),
            config,
            taskbar: taskbar.clone(),
            overlays,
            display,
            container_overlays: HashMap::new(),
            last_focused: None,
            placement_tracker: PlacementTracker::new(),
            recovery: Recovery::new(taskbar),
        })
    }

    pub(super) fn app_initialized(
        &mut self,
        windows: Vec<Arc<dyn ManageExternalHwnd>>,
    ) -> Vec<Actions> {
        let mut on_open = Vec::new();
        for ext in windows {
            if let Some(actions) = self.try_manage_window(ext) {
                on_open.push(actions);
            }
        }
        on_open
    }

    pub(super) fn config_changed(&mut self, new_config: Config) {
        self.hub.sync_config(new_config.clone().into());
        for overlay in self.container_overlays.values_mut() {
            overlay.set_config(new_config.clone());
        }
        self.config = new_config;
        tracing::info!("Config reloaded");
    }

    pub(super) fn window_created(&mut self, ext: Arc<dyn ManageExternalHwnd>) -> Option<Actions> {
        if !self.registry.contains_hwnd(ext.id()) {
            self.try_manage_window(ext)
        } else {
            None
        }
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn window_destroyed(&mut self, ext: Arc<dyn ManageExternalHwnd>) {
        self.remove_window(ext.id());
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn window_minimized(&mut self, ext: Arc<dyn ManageExternalHwnd>) {
        let id_key = ext.id();
        let dominated_by_dome = self.registry.get_id(id_key).is_some_and(|id| {
            matches!(
                self.registry.get(id).state,
                WindowState::Minimized | WindowState::FullscreenExclusive
            )
        });
        if !dominated_by_dome {
            self.remove_window(id_key);
        }
    }

    pub(super) fn move_size_started(&mut self, ext: Arc<dyn ManageExternalHwnd>) {
        self.placement_tracker.drag_started(ext.id());
    }

    pub(super) fn move_size_ended(&mut self, ext: Arc<dyn ManageExternalHwnd>) {
        self.placement_tracker.drag_ended(ext.id());
        self.handle_resize(ext.id());
    }

    pub(super) fn location_changed(&mut self, ext: Arc<dyn ManageExternalHwnd>) -> bool {
        self.placement_tracker.location_changed(ext.id())
    }

    pub(super) fn title_changed(&mut self, ext: Arc<dyn ManageExternalHwnd>) -> Option<Actions> {
        let id_key = ext.id();
        if self.registry.contains_hwnd(id_key) {
            let new_title = ext.get_window_title();
            self.update_titles(vec![(id_key, new_title)]);
            None
        } else {
            self.try_manage_window(ext)
        }
    }

    pub(super) fn screens_changed(&mut self, screens: Vec<ScreenInfo>) {
        tracing::info!(count = screens.len(), "Screen parameters changed");
        self.update_screens(screens);
    }

    pub(super) fn tab_clicked(&mut self, container_id: ContainerId, tab_idx: usize) {
        self.hub.focus_tab_index(container_id, tab_idx);
    }

    pub(super) fn handle_display_change(&mut self) {
        match self.display.get_all_screens() {
            Ok(screens) => self.screens_changed(screens),
            Err(e) => tracing::warn!("Failed to enumerate screens: {e}"),
        }
        if let Some(fg) = self.display.get_exclusive_fullscreen_hwnd()
            && let Some(id) = self.registry.get_id(fg)
        {
            tracing::info!(%id, "D3D exclusive fullscreen entered");
            self.enter_fullscreen_exclusive(id);
        }
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

        let overlay = match self.overlays.create_window_overlay() {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(%id_key, "Failed to create window overlay, skipping: {e:#}");
                return;
            }
        };

        let (state, id) = if monitor.is_some_and(|m| is_fullscreen(&ext.get_dimension(), &m)) {
            (
                WindowState::FullscreenBorderless,
                self.hub.insert_fullscreen(),
            )
        } else if ext.should_float() {
            (
                WindowState::Positioned(PositionedState::Float),
                self.hub.insert_float(dim),
            )
        } else {
            (
                WindowState::Positioned(PositionedState::Tiling),
                self.hub.insert_tiling(),
            )
        };
        self.set_constraints(id, &*ext);
        self.recovery.track(&ext, dim);

        self.registry.insert(
            id_key,
            id,
            WindowEntry {
                ext,
                state,
                title,
                process,
                overlay,
            },
        );
        tracing::info!(%id, %id_key, %state, "Window managed");
    }

    #[tracing::instrument(skip(self))]
    fn remove_window(&mut self, id_key: HwndId) {
        self.placement_tracker.clear(id_key);
        self.taskbar.delete_tab(id_key);
        self.recovery.untrack(id_key);
        if let Some(id) = self.registry.remove_by_hwnd(id_key) {
            tracing::info!(%id, "Window removed");
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

    pub(super) fn handle_focus(&mut self, id_key: HwndId) {
        if let Some(id) = self.registry.get_id(id_key) {
            self.hub.set_focus(id);
            tracing::info!(?id_key, "Window focused");
        }
    }

    /// Called by the run loop when a drag safety timeout or resize debounce
    /// timer fires. Removes the window from the placement tracker and
    /// re-evaluates its layout.
    pub(super) fn placement_timeout(&mut self, id: HwndId) {
        self.placement_tracker.clear(id);
        self.handle_resize(id);
    }

    #[tracing::instrument(skip(self))]
    fn handle_resize(&mut self, id_key: HwndId) {
        let Some(id) = self.registry.get_id(id_key) else {
            return;
        };
        let entry = self.registry.get(id);
        if entry.state == WindowState::FullscreenExclusive {
            return;
        }
        let ext = entry.ext.clone();
        self.set_constraints(id, &*ext);
        self.check_fullscreen_state(id, &*ext);
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
                self.enter_fullscreen_borderless(id);
            }
            (true, false) => {
                self.exit_fullscreen_borderless(id);
            }
            _ => {}
        }
    }

    pub(super) fn execute_hub_action(&mut self, action: &HubAction) {
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

    #[tracing::instrument(skip_all)]
    pub(super) fn apply_layout(&mut self) {
        let changes = self.hub.drain_changes();

        // Phase 1 — Lifecycle
        for &id in &changes.created_containers {
            match self.overlays.create_container_overlay(self.config.clone()) {
                Ok(overlay) => {
                    self.container_overlays.insert(id, overlay);
                }
                Err(e) => tracing::warn!("Failed to create container overlay: {e:#}"),
            }
        }

        for &id in &changes.deleted_containers {
            self.container_overlays.remove(&id);
        }

        // Phase 2 — Compute placements
        let focused = match self
            .hub
            .get_workspace(self.hub.current_workspace())
            .focused()
        {
            Some(Child::Window(id)) => Some(id),
            _ => None,
        };

        let placements = self.hub.get_visible_placements();

        let mut to_show: Vec<WindowPlacement> = Vec::new();
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
                    self.show_fullscreen_window(id, dimension);
                }
                MonitorLayout::Normal {
                    windows,
                    containers,
                } => {
                    for wp in windows {
                        window_ids.push(wp.id);
                        if self
                            .placement_tracker
                            .is_moving(self.registry.get(wp.id).ext.id())
                        {
                            continue;
                        }
                        to_show.push(wp);
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

        // Global diff
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

        // Phase 3 — Hide
        for &id in &to_hide {
            self.taskbar.delete_tab(self.registry.get(id).ext.id());
            self.hide_window(id);
        }

        for &id in &changes.created_windows {
            if !new_window_ids.contains(&id) {
                self.hide_window(id);
            }
        }

        for &id in &containers_to_hide {
            if let Some(overlay) = self.container_overlays.get_mut(&id) {
                overlay.hide();
            }
        }

        // Phase 4 — Position
        self.position_windows(&to_show, &containers_to_show, focused);

        // Phase 5 — Taskbar
        for &id in &tabs_to_add {
            self.taskbar.add_tab(self.registry.get(id).ext.id());
        }

        // Phase 6 — Focus
        if focused != self.last_focused {
            self.last_focused = focused;
            if let Some(id) = focused {
                let entry = self.registry.get(id);
                if entry.state != WindowState::FullscreenExclusive {
                    entry.ext.set_foreground_window();
                }
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn position_windows(
        &mut self,
        to_show: &[WindowPlacement],
        containers_to_show: &[ContainerRender],
        focused: Option<WindowId>,
    ) {
        let focus_changed = focused != self.last_focused;

        let mut normal_windows: Vec<&WindowPlacement> = Vec::new();
        for wp in to_show {
            if self.registry.get(wp.id).state == WindowState::FullscreenExclusive {
                continue;
            }
            normal_windows.push(wp);
        }

        let mut newly_active_float: Vec<&WindowPlacement> = Vec::new();
        let mut steady_float: Vec<&WindowPlacement> = Vec::new();
        let mut focused_tiling: Option<&WindowPlacement> = None;
        let mut steady_tiling: Vec<&WindowPlacement> = Vec::new();

        for wp in &normal_windows {
            let is_newly_float = self.registry.get(wp.id).state
                != WindowState::Positioned(PositionedState::Float)
                && wp.is_float;
            let is_newly_focused_float = wp.is_float && focus_changed && focused == Some(wp.id);

            if wp.is_float {
                if is_newly_float || is_newly_focused_float {
                    newly_active_float.push(wp);
                } else {
                    steady_float.push(wp);
                }
            } else if focused == Some(wp.id) {
                focused_tiling = Some(wp);
            } else {
                steady_tiling.push(wp);
            }
        }

        newly_active_float.sort_by(|a, b| b.id.cmp(&a.id));
        if let Some(focused_id) = focused
            && let Some(pos) = newly_active_float.iter().position(|wp| wp.id == focused_id)
        {
            let item = newly_active_float.remove(pos);
            newly_active_float.insert(0, item);
        }
        steady_float.sort_by(|a, b| b.id.cmp(&a.id));
        steady_tiling.sort_by(|a, b| b.id.cmp(&a.id));

        let focused_container = containers_to_show.iter().find(|c| c.placement.is_focused);
        let mut steady_containers: Vec<&ContainerRender> = containers_to_show
            .iter()
            .filter(|c| !c.placement.is_focused)
            .collect();
        steady_containers.sort_by(|a, b| b.placement.id.cmp(&a.placement.id));

        let mut anchor: Option<HwndId> = None;

        for wp in newly_active_float.iter().rev() {
            anchor = Some(self.registry.get(wp.id).ext.id());
            self.show_window(wp.id, wp, ZOrder::Topmost);
        }

        for wp in &steady_float {
            let z = anchor.map(ZOrder::After).unwrap_or(ZOrder::Unchanged);
            anchor = Some(self.registry.get(wp.id).ext.id());
            self.show_window(wp.id, wp, z);
        }

        if let Some(data) = focused_container {
            let titles = self.registry.resolve_tab_titles(&data.children);
            if let Some(overlay) = self.container_overlays.get_mut(&data.placement.id) {
                overlay.update(data.placement, titles, ZOrder::NotTopmost);
                anchor = Some(overlay.id());
            }
        } else if let Some(wp) = focused_tiling {
            anchor = Some(self.registry.get(wp.id).ext.id());
            self.show_window(wp.id, wp, ZOrder::NotTopmost);
        } else {
            anchor = None;
        }

        for data in &steady_containers {
            let titles = self.registry.resolve_tab_titles(&data.children);
            if let Some(overlay) = self.container_overlays.get_mut(&data.placement.id) {
                let z = anchor.map(ZOrder::After).unwrap_or(ZOrder::Unchanged);
                overlay.update(data.placement, titles, z);
                anchor = Some(overlay.id());
            }
        }

        for wp in &steady_tiling {
            let z = anchor.map(ZOrder::After).unwrap_or(ZOrder::Unchanged);
            anchor = Some(self.registry.get(wp.id).ext.id());
            self.show_window(wp.id, wp, z);
        }
    }

    fn update_titles(&mut self, titles: Vec<(HwndId, Option<String>)>) {
        for (hwnd_id, title) in &titles {
            self.registry.set_title(*hwnd_id, title.clone());
        }
        let affected_ids: HashSet<WindowId> = titles
            .iter()
            .filter_map(|(h, _)| self.registry.get_id(*h))
            .collect();
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
                        let titles = self.registry.resolve_tab_titles(container.children());
                        if let Some(overlay) = self.container_overlays.get_mut(&cp.id) {
                            overlay.update(*cp, titles, ZOrder::Unchanged);
                        }
                    }
                }
            }
        }
    }

    fn update_screens(&mut self, screens: Vec<ScreenInfo>) {
        if screens.is_empty() {
            tracing::warn!("Empty screen list, skipping update");
            return;
        }
        self.reconcile_monitors(screens);

        let windows: Vec<_> = self.registry.iter().collect();
        for (_, id) in windows {
            if self.registry.get(id).state == WindowState::FullscreenExclusive {
                continue;
            }
            let ext = self.registry.get(id).ext.clone();
            self.set_constraints(id, &*ext);
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
            if let Some(&id) = self.monitor_handles.get(&screen.handle)
                && self.monitor_dimensions.get(&id) != Some(&screen.dimension)
            {
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
