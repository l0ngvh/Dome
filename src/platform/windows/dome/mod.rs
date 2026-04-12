pub(super) mod overlay;
mod placement_tracker;
mod recovery;
mod registry;
mod window;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use crate::action::{Actions, FocusTarget, HubAction, MoveTarget, ToggleTarget};
use crate::config::{Config, WindowsOnOpenRule, WindowsWindow};
use crate::core::{
    Child, ContainerId, ContainerPlacement, Dimension, Hub, MonitorId, MonitorLayout, WindowId,
    WindowPlacement, WindowRestrictions,
};

use self::overlay::{FloatOverlayApi, TilingOverlayApi};
use self::placement_tracker::PlacementTracker;
use self::recovery::Recovery;
use self::registry::{WindowEntry, WindowRegistry};
use self::window::{PositionedState, WindowState};

#[derive(Clone, Copy)]
pub(super) enum ObservedPosition {
    Fullscreen,
    Visible(i32, i32, i32, i32),
}
use super::ScreenInfo;
use super::external::{HwndId, ManageExternalHwnd, ZOrder};
use super::taskbar::ManageTaskbar;

pub(super) enum HubEvent {
    WindowCreated(HwndId),
    WindowDestroyed(HwndId),
    WindowMinimized(HwndId),
    WindowFocused(HwndId),
    WindowTitleChanged(HwndId),
    MoveSizeStart(HwndId),
    MoveSizeEnd(HwndId),
    LocationChanged(HwndId),
    Action(Actions),
    ConfigChanged(Box<Config>),
    TabClicked(ContainerId, usize),
    Shutdown,
}

struct DisplayedMonitor {
    window_ids: HashSet<WindowId>,
}

struct MonitorPositionData {
    monitor_id: MonitorId,
    dimension: Dimension,
    tiling_windows: Vec<WindowPlacement>,
    containers: Vec<(ContainerPlacement, Vec<String>)>,
}

pub(super) trait CreateOverlay {
    fn create_tiling_overlay(&self, config: Config) -> anyhow::Result<Box<dyn TilingOverlayApi>>;
    fn create_float_overlay(&self) -> anyhow::Result<Box<dyn FloatOverlayApi>>;
}

pub(super) trait QueryDisplay {
    fn get_all_screens(&self) -> anyhow::Result<Vec<ScreenInfo>>;
    /// Returns the hwnd of the foreground window if D3D exclusive fullscreen is active.
    fn get_exclusive_fullscreen_hwnd(&self) -> Option<HwndId>;
}

/// Platform-specific state machine that bridges Win32 window events with the core tree
/// model. Event-loop–facing methods accept `HwndId` rather than `WindowId` because callers
/// may dispatch work to background threads — by the time results arrive the window may
/// have been removed, so resolution to `WindowId` happens here where the registry can be
/// checked.
pub(super) struct Dome {
    hub: Hub,
    registry: WindowRegistry,
    monitor_handles: HashMap<isize, MonitorId>,
    monitor_dimensions: HashMap<MonitorId, Dimension>,
    displayed: HashMap<MonitorId, DisplayedMonitor>,
    config: Config,
    taskbar: Rc<dyn ManageTaskbar>,
    overlay_factory: Box<dyn CreateOverlay>,
    display: Box<dyn QueryDisplay>,
    tiling_overlays: HashMap<MonitorId, Box<dyn TilingOverlayApi>>,
    float_overlays: HashMap<WindowId, Box<dyn FloatOverlayApi>>,
    last_focused: Option<WindowId>,
    last_focused_monitor: Option<MonitorId>,
    pending_created: Vec<WindowId>,
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
        taskbar: Rc<dyn ManageTaskbar>,
        overlay_factory: Box<dyn CreateOverlay>,
        display: Box<dyn QueryDisplay>,
    ) -> anyhow::Result<Self> {
        let screens = display.get_all_screens()?;
        anyhow::ensure!(!screens.is_empty(), "No monitors detected");
        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_handles = HashMap::new();
        let mut monitor_dimensions = HashMap::new();
        let mut tiling_overlays: HashMap<MonitorId, Box<dyn TilingOverlayApi>> = HashMap::new();
        monitor_handles.insert(primary.handle, primary_monitor_id);
        monitor_dimensions.insert(primary_monitor_id, primary.dimension);
        if let Ok(overlay) = overlay_factory.create_tiling_overlay(config.clone()) {
            tiling_overlays.insert(primary_monitor_id, overlay);
        }
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
                if let Ok(overlay) = overlay_factory.create_tiling_overlay(config.clone()) {
                    tiling_overlays.insert(id, overlay);
                }
                tracing::info!(
                    name = %screen.name,
                    handle = ?screen.handle,
                    dimension = ?screen.dimension,
                    "Monitor"
                );
            }
        }

        Ok(Self {
            hub,
            registry: WindowRegistry::new(),
            monitor_handles,
            monitor_dimensions,
            displayed: HashMap::new(),
            config,
            taskbar: taskbar.clone(),
            overlay_factory,
            display,
            tiling_overlays,
            float_overlays: HashMap::new(),
            last_focused: None,
            last_focused_monitor: None,
            pending_created: Vec::new(),
            placement_tracker: PlacementTracker::new(),
            recovery: Recovery::new(taskbar),
        })
    }

    pub(super) fn config_changed(&mut self, new_config: Config) {
        self.hub.sync_config(new_config.clone().into());
        for overlay in self.tiling_overlays.values_mut() {
            overlay.set_config(new_config.clone());
        }
        self.config = new_config;
        tracing::info!("Config reloaded");
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn window_destroyed(&mut self, id_key: HwndId) {
        self.remove_window(id_key);
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn window_minimized(&mut self, id_key: HwndId) {
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

    pub(super) fn move_size_started(&mut self, id_key: HwndId) {
        self.placement_tracker.drag_started(id_key);
    }

    pub(super) fn move_size_ended(&mut self, id_key: HwndId) {
        self.placement_tracker.drag_ended(id_key);
    }

    pub(super) fn location_changed(&mut self, id_key: HwndId) -> bool {
        self.placement_tracker.location_changed(id_key)
    }

    pub(super) fn screens_changed(&mut self, screens: Vec<ScreenInfo>) -> Vec<HwndId> {
        tracing::info!(count = screens.len(), "Screen parameters changed");
        self.update_screens(screens)
    }

    pub(super) fn tab_clicked(&mut self, container_id: ContainerId, tab_idx: usize) {
        self.hub.focus_tab_index(container_id, tab_idx);
    }

    pub(super) fn handle_display_change(&mut self) -> Vec<HwndId> {
        let to_refresh = match self.display.get_all_screens() {
            Ok(screens) => self.screens_changed(screens),
            Err(e) => {
                tracing::warn!("Failed to enumerate screens: {e}");
                Vec::new()
            }
        };
        if let Some(fg) = self.display.get_exclusive_fullscreen_hwnd()
            && let Some(id) = self.registry.get_id(fg)
        {
            tracing::info!(%id, "D3D exclusive fullscreen entered");
            self.enter_fullscreen_exclusive(id);
        }
        to_refresh
    }

    pub(super) fn registry_contains_hwnd(&self, id: HwndId) -> bool {
        self.registry.contains_hwnd(id)
    }

    pub(super) fn registry_get_id(&self, id: HwndId) -> Option<WindowId> {
        self.registry.get_id(id)
    }

    pub(super) fn try_manage_window(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
        title: Option<String>,
        process: String,
        constraints: (f32, f32, f32, f32),
        observation: ObservedPosition,
    ) -> Option<Actions> {
        if should_ignore(&process, title.as_deref(), &self.config.windows.ignore) {
            return None;
        }
        let actions = on_open_actions(&process, title.as_deref(), &self.config.windows.on_open);
        self.insert_window(ext, title, process, constraints, observation);
        actions
    }

    fn insert_window(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
        title: Option<String>,
        process: String,
        constraints: (f32, f32, f32, f32),
        observation: ObservedPosition,
    ) {
        let id_key = ext.id();

        let (state, id) = match observation {
            ObservedPosition::Fullscreen => (
                WindowState::FullscreenBorderless,
                self.hub
                    .insert_fullscreen(WindowRestrictions::ProtectFullscreen),
            ),
            ObservedPosition::Visible(x, y, w, h) => {
                let dim = Dimension {
                    x: x as f32,
                    y: y as f32,
                    width: w as f32,
                    height: h as f32,
                };
                let offscreen = WindowState::Positioned(PositionedState::Offscreen {
                    retries: 0,
                    actual: (x, y, w, h),
                });
                if ext.should_float() {
                    (offscreen, self.hub.insert_float(dim))
                } else {
                    (offscreen, self.hub.insert_tiling())
                }
            }
        };
        self.set_constraints(id, constraints);
        self.recovery.track(&ext);

        self.registry.insert(
            id_key,
            id,
            WindowEntry {
                ext,
                state,
                title,
                process,
            },
        );
        tracing::info!(%id, %id_key, %state, "Window managed");
        self.pending_created.push(id);
    }

    #[tracing::instrument(skip(self))]
    fn remove_window(&mut self, id_key: HwndId) {
        self.placement_tracker.clear(id_key);
        self.taskbar.delete_tab(id_key);
        self.recovery.untrack(id_key);
        if let Some(id) = self.registry.remove_by_hwnd(id_key) {
            tracing::info!(%id, "Window removed");
            self.float_overlays.remove(&id);
            for dm in self.displayed.values_mut() {
                dm.window_ids.remove(&id);
            }
            self.hub.delete_window(id);
        }
    }

    pub(super) fn set_constraints(&mut self, id: WindowId, constraints: (f32, f32, f32, f32)) {
        let border = self.config.border_size;
        let (min_w, min_h, max_w, max_h) = constraints;
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

    pub(super) fn handle_focus(&mut self, id_key: HwndId) {
        if let Some(id) = self.registry.get_id(id_key) {
            self.hub.set_focus(id);
            tracing::info!(?id_key, "Window focused");
        }
    }

    /// Called by the run loop when a drag safety timeout or resize debounce
    /// timer fires. Removes the window from the placement tracker.
    pub(super) fn placement_timeout(&mut self, id: HwndId) {
        self.placement_tracker.clear(id);
    }

    pub(super) fn window_moved(&mut self, id_key: HwndId, observation: ObservedPosition) {
        let Some(id) = self.registry.get_id(id_key) else {
            return;
        };
        match observation {
            ObservedPosition::Fullscreen => self.window_entered_borderless_fullscreen(id),
            ObservedPosition::Visible(x, y, w, h) => self.window_drifted(id, x, y, w, h),
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
        let created = std::mem::take(&mut self.pending_created);

        let focused = match self
            .hub
            .get_workspace(self.hub.current_workspace())
            .focused()
        {
            Some(Child::Window(id)) => Some(id),
            _ => None,
        };

        let placements = self.hub.get_visible_placements();

        let mut float_windows: Vec<WindowPlacement> = Vec::new();
        let mut per_monitor: Vec<MonitorPositionData> = Vec::new();
        let mut new_displayed: HashMap<MonitorId, DisplayedMonitor> = HashMap::new();

        for mp in placements {
            let dimension = self
                .monitor_dimensions
                .get(&mp.monitor_id)
                .copied()
                .unwrap_or_default();

            let mut window_ids = HashSet::new();

            match mp.layout {
                MonitorLayout::Fullscreen(id) => {
                    window_ids.insert(id);
                    self.show_fullscreen_window(id, dimension);
                }
                MonitorLayout::Normal {
                    windows,
                    containers,
                } => {
                    let mut tiling_windows = Vec::new();
                    let mut container_data = Vec::new();

                    for wp in windows {
                        window_ids.insert(wp.id);
                        if self
                            .placement_tracker
                            .is_moving(self.registry.get(wp.id).ext.id())
                        {
                            continue;
                        }
                        if wp.is_float {
                            float_windows.push(wp);
                        } else {
                            tiling_windows.push(wp);
                        }
                    }
                    for cp in &containers {
                        if !cp.is_tabbed && !cp.is_focused {
                            continue;
                        }
                        let children = if cp.is_tabbed {
                            self.hub.get_container(cp.id).children().to_vec()
                        } else {
                            vec![]
                        };
                        let titles = self.registry.resolve_tab_titles(&children);
                        container_data.push((*cp, titles));
                    }

                    per_monitor.push(MonitorPositionData {
                        monitor_id: mp.monitor_id,
                        dimension,
                        tiling_windows,
                        containers: container_data,
                    });
                }
            }

            new_displayed.insert(mp.monitor_id, DisplayedMonitor { window_ids });
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

        self.displayed = new_displayed;

        // Hide
        for &id in &to_hide {
            self.taskbar.delete_tab(self.registry.get(id).ext.id());
            self.hide_window(id);
        }

        for &id in &created {
            if !new_window_ids.contains(&id) {
                self.hide_window(id);
            }
        }

        // Position
        self.position_windows(&float_windows, &per_monitor, focused);

        // Clean up float overlays for windows that are no longer float
        let current_float_ids: HashSet<WindowId> = float_windows.iter().map(|wp| wp.id).collect();
        self.float_overlays
            .retain(|id, _| current_float_ids.contains(id));

        // Taskbar
        for &id in &tabs_to_add {
            self.taskbar.add_tab(self.registry.get(id).ext.id());
        }

        // Focus
        let current_monitor = self.hub.focused_monitor();
        let monitor_changed = self
            .last_focused_monitor
            .is_some_and(|m| m != current_monitor);

        if focused != self.last_focused || monitor_changed {
            self.last_focused = focused;
            if let Some(id) = focused {
                let entry = self.registry.get(id);
                if !matches!(entry.state, WindowState::FullscreenExclusive) {
                    entry.ext.set_foreground_window();
                }
            } else if let Some(overlay) = self.tiling_overlays.get(&current_monitor) {
                overlay.focus();
            }
        }
        self.last_focused_monitor = Some(current_monitor);
    }

    #[tracing::instrument(skip_all)]
    fn position_windows(
        &mut self,
        float_windows: &[WindowPlacement],
        per_monitor: &[MonitorPositionData],
        focused: Option<WindowId>,
    ) {
        let focus_changed = focused != self.last_focused;

        // Float windows — ensure overlay exists, then position
        for wp in float_windows {
            if !self.float_overlays.contains_key(&wp.id) {
                match self.overlay_factory.create_float_overlay() {
                    Ok(o) => {
                        self.float_overlays.insert(wp.id, o);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create float overlay: {e:#}");
                        continue;
                    }
                }
            }
            self.show_float(wp.id, wp, focus_changed);
        }

        // Tiling windows — per monitor, chained after tiling overlay
        for data in per_monitor {
            if let Some(overlay) = self.tiling_overlays.get_mut(&data.monitor_id) {
                if data.tiling_windows.is_empty() && data.containers.is_empty() {
                    overlay.clear();
                } else {
                    overlay.update(data.dimension, &data.tiling_windows, &data.containers);
                }
                let mut anchor = overlay.id();
                // Focused window first so it's highest in z-order among tiling
                let focused_first = data
                    .tiling_windows
                    .iter()
                    .filter(|wp| focused == Some(wp.id))
                    .chain(
                        data.tiling_windows
                            .iter()
                            .filter(|wp| focused != Some(wp.id)),
                    );
                for wp in focused_first {
                    self.show_tiling(wp.id, wp, ZOrder::After(anchor));
                    anchor = self.registry.get(wp.id).ext.id();
                }
            }
        }
    }

    pub(super) fn update_titles(&mut self, titles: Vec<(HwndId, Option<String>)>) {
        for (hwnd_id, title) in &titles {
            self.registry.set_title(*hwnd_id, title.clone());
        }
        // TODO: full re-layout on every title change is expensive — we should
        // selectively re-render only the affected tiling overlay instead.
        self.apply_layout();
    }

    fn update_screens(&mut self, screens: Vec<ScreenInfo>) -> Vec<HwndId> {
        if screens.is_empty() {
            tracing::warn!("Empty screen list, skipping update");
            return Vec::new();
        }
        self.reconcile_monitors(screens);

        self.registry
            .iter()
            .filter(|(_, id)| {
                !matches!(
                    self.registry.get(*id).state,
                    WindowState::FullscreenExclusive
                )
            })
            .map(|(hwnd_id, _)| hwnd_id)
            .collect()
    }

    fn reconcile_monitors(&mut self, screens: Vec<ScreenInfo>) {
        let current_handles: HashSet<isize> = screens.iter().map(|s| s.handle).collect();

        for screen in &screens {
            if !self.monitor_handles.contains_key(&screen.handle) {
                let id = self.hub.add_monitor(screen.name.clone(), screen.dimension);
                self.monitor_handles.insert(screen.handle, id);
                self.monitor_dimensions.insert(id, screen.dimension);
                if let Ok(overlay) = self
                    .overlay_factory
                    .create_tiling_overlay(self.config.clone())
                {
                    self.tiling_overlays.insert(id, overlay);
                }
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
                self.tiling_overlays.remove(&monitor_id);
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
