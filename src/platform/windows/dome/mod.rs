pub(super) mod app_window;
mod external_bar;
pub(super) mod monitor;
pub(super) mod overlay;
mod placement_tracker;
mod recovery;
mod registry;
pub(super) mod tray;
pub(super) mod window;

pub(super) use self::monitor::{MonitorInfo, QueryDisplay, Win32Display};

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use crate::action::Query;
use crate::action::{
    Actions, FocusTarget, MasterTarget, MinimizedWindow, MoveTarget, TabDirection, ToggleTarget,
};
use crate::config::{Config, LayoutConfig, LayoutWorkspaceConfig};
use crate::core::GlobalLayoutConfig;
use crate::core::{
    ContainerId, ContainerPlacement, Dimension, Direction, FloatWindowPlacement, Hub, Length,
    LimitObservation, Logical, MonitorId, MonitorLayout, Physical, PixelRect, TilingAction,
    TilingWindowPlacement, WindowId, WindowRestrictions, WorkspaceInfo,
};

use self::app_window::AppWindowApi;
use self::overlay::{FloatOverlayApi, TabBarOverlayApi, TilingOverlayApi};
use self::placement_tracker::PlacementTracker;
use self::recovery::Recovery;
use self::registry::{ManagedWindow, WindowRegistry};
use self::window::{PositionedState, WindowState};

pub(super) use self::window::NewWindow;
pub(super) use self::window::WindowsMetadata;

use self::external_bar::StatusBars;

use self::monitor::MonitorRegistry;
use super::external::{HwndId, ShowCmd};
use super::taskbar::ManageTaskbar;

pub(super) enum HubEvent {
    WindowCreated(HwndId),
    WindowDestroyed(HwndId),
    WindowMinimized(HwndId),
    WindowRestored {
        hwnd_id: HwndId,
        observed_at: Instant,
    },
    WindowFocused(HwndId),
    WindowTitleChanged(HwndId),
    MoveSizeStart(HwndId),
    MoveSizeEnd {
        hwnd_id: HwndId,
        observed_at: Instant,
    },
    LocationChanged {
        hwnd_id: HwndId,
        observed_at: Instant,
    },
    Action(Actions),
    Query {
        query: Query,
        sender: std::sync::mpsc::SyncSender<String>,
    },
    ConfigChanged(Box<Config>),
    LayoutConfigChanged(Box<LayoutConfig>),
    ExportLayout(String),
    TabClicked(ContainerId, usize),
    Shutdown,
}

struct MonitorPositionData {
    monitor_id: MonitorId,
    work_area: PixelRect,
    border_thickness: Length<Physical>,
    tiling_windows: Vec<TilingWindowPlacement>,
    float_windows: Vec<FloatWindowPlacement>,
    containers: Vec<(ContainerPlacement, Vec<String>)>,
}

pub(super) trait CreateOverlay {
    fn create_tiling_overlay(
        &self,
        config: Config,
        tab_bar_height: Length<Logical>,
        monitor: PixelRect,
        scale: f32,
    ) -> anyhow::Result<Box<dyn TilingOverlayApi>>;
    fn create_float_overlay(
        &self,
        config: Config,
        scale: f32,
        visible_border_box: PixelRect,
    ) -> anyhow::Result<Box<dyn FloatOverlayApi>>;
    fn create_tab_bar(
        &self,
        config: Config,
        container_id: ContainerId,
        rect: PixelRect,
        scale: f32,
    ) -> anyhow::Result<Box<dyn TabBarOverlayApi>>;
}

/// Platform-specific state machine that bridges Win32 window events with the core tree
/// model. Event-loop–facing methods accept `HwndId` rather than `WindowId` because callers
/// may dispatch work to background threads — by the time results arrive the window may
/// have been removed, so resolution to `WindowId` happens here where the registry can be
/// checked.
pub(super) struct Dome {
    hub: Hub,
    registry: WindowRegistry,
    monitors: MonitorRegistry,
    config: Config,
    taskbar: Rc<dyn ManageTaskbar>,
    overlay_factory: Box<dyn CreateOverlay>,
    display: Box<dyn QueryDisplay>,
    tiling_overlays: HashMap<MonitorId, Box<dyn TilingOverlayApi>>,
    tab_bars: HashMap<ContainerId, Box<dyn TabBarOverlayApi>>,
    float_overlays: HashMap<WindowId, Box<dyn FloatOverlayApi>>,
    last_focused: Option<WindowId>,
    last_focused_monitor: Option<MonitorId>,
    pending_created: Vec<WindowId>,
    placement_tracker: PlacementTracker,
    recovery: Recovery,
    app_window: Box<dyn AppWindowApi>,
    status_bars: StatusBars,
}

impl Drop for Dome {
    fn drop(&mut self) {
        self.recovery.restore_all();
    }
}

impl Dome {
    pub(super) fn new(
        config: Config,
        workspace_overrides: Vec<LayoutWorkspaceConfig>,
        taskbar: Rc<dyn ManageTaskbar>,
        overlay_factory: Box<dyn CreateOverlay>,
        display: Box<dyn QueryDisplay>,
        app_window: Box<dyn AppWindowApi>,
    ) -> anyhow::Result<Self> {
        let monitors = display.get_all_monitors()?;
        anyhow::ensure!(!monitors.is_empty(), "No monitors detected");
        let primary = monitors
            .iter()
            .find(|s| s.is_primary)
            .unwrap_or(&monitors[0]);
        let mut hub = Hub::new(
            primary.work_area,
            primary.scale,
            GlobalLayoutConfig::from(&config),
            workspace_overrides.clone(),
        );
        let primary_monitor_id = hub.focused_monitor();
        let mut monitors_reg = MonitorRegistry::new();
        let mut tiling_overlays: HashMap<MonitorId, Box<dyn TilingOverlayApi>> = HashMap::new();
        monitors_reg.insert(
            primary.handle,
            primary_monitor_id,
            primary.work_area,
            primary.scale,
        );
        if let Ok(overlay) = overlay_factory.create_tiling_overlay(
            config.clone(),
            config.partition_tree.tab_bar_height,
            primary.work_area,
            primary.scale,
        ) {
            tiling_overlays.insert(primary_monitor_id, overlay);
        }
        tracing::info!(
            name = %primary.name,
            handle = ?primary.handle,
            work_area = ?primary.work_area,
            "Primary monitor"
        );

        for monitor in &monitors {
            if monitor.handle != primary.handle {
                let id = hub.add_monitor(monitor.name.clone(), monitor.work_area, monitor.scale);
                monitors_reg.insert(monitor.handle, id, monitor.work_area, monitor.scale);
                if let Ok(overlay) = overlay_factory.create_tiling_overlay(
                    config.clone(),
                    config.partition_tree.tab_bar_height,
                    monitor.work_area,
                    monitor.scale,
                ) {
                    tiling_overlays.insert(id, overlay);
                }
                tracing::info!(
                    name = %monitor.name,
                    handle = ?monitor.handle,
                    work_area = ?monitor.work_area,
                    "Monitor"
                );
            }
        }

        Ok(Self {
            hub,
            registry: WindowRegistry::new(),
            monitors: monitors_reg,
            config,
            taskbar: taskbar.clone(),
            overlay_factory,
            display,
            tiling_overlays,
            tab_bars: HashMap::new(),
            float_overlays: HashMap::new(),
            last_focused: None,
            last_focused_monitor: None,
            pending_created: Vec::new(),
            placement_tracker: PlacementTracker::new(),
            recovery: Recovery::new(taskbar),
            app_window,
            status_bars: StatusBars::default(),
        })
    }

    fn refresh_tray(&self) {
        self.app_window.update_tray(&self.query_workspaces());
    }

    pub(super) fn config_changed(&mut self, new_config: Config) {
        self.hub
            .sync_configuration(GlobalLayoutConfig::from(&new_config));
        self.config = new_config;
        for overlay in self.tiling_overlays.values_mut() {
            overlay.set_config(&self.config);
            overlay.set_tab_bar_height(self.config.partition_tree.tab_bar_height);
        }
        for overlay in self.float_overlays.values_mut() {
            overlay.set_config(&self.config);
        }
        for overlay in self.tab_bars.values_mut() {
            overlay.set_config(&self.config);
        }
        tracing::info!("Config reloaded");
        self.apply_layout();
    }

    pub(super) fn layout_changed(&mut self, new_layout: LayoutConfig) {
        self.hub.sync_preferred_layout(new_layout.workspace);
        tracing::info!("Layout reloaded");
        self.apply_layout();
    }

    #[tracing::instrument(
        skip(self, id_key),
        fields(hwnd = %id_key),
    )]
    pub(super) fn window_destroyed(&mut self, id_key: HwndId) {
        self.clear_move_state(id_key);
        self.taskbar.delete_tab(id_key);
        self.recovery.untrack(id_key);
        if let Some(id) = self.registry.remove_by_hwnd(id_key) {
            tracing::info!(%id, "Window removed");
            self.float_overlays.remove(&id);
            self.monitors.remove_window_from_displayed(id);
            self.hub.delete_window(id);
            self.apply_layout();
        }
    }

    #[tracing::instrument(
        skip(self, id_key),
        fields(hwnd = %id_key),
    )]
    pub(super) fn window_minimized(&mut self, id_key: HwndId) {
        let Some(id) = self.registry.get_id(id_key) else {
            return;
        };
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        // Dome-initiated minimize
        if matches!(entry.state, WindowState::BorderlessMinimized { .. }) {
            return;
        }
        self.hub.minimize_window(id);
        if let Some(entry) = self.registry.get_mut(id) {
            entry.is_minimized = true;
        }
        self.apply_layout();
    }

    pub(super) fn move_size_started(&mut self, id_key: HwndId) {
        self.placement_tracker.drag_started(id_key);
    }

    pub(super) fn clear_move_state(&mut self, id_key: HwndId) {
        self.placement_tracker.clear(id_key);
    }

    pub(super) fn location_changed(&mut self, id_key: HwndId) -> bool {
        self.placement_tracker.location_changed(id_key)
    }

    pub(super) fn monitors_changed(&mut self, monitors: Vec<MonitorInfo>) -> Vec<HwndId> {
        tracing::info!(count = monitors.len(), "Monitor parameters changed");
        self.update_monitors(monitors)
    }

    pub(super) fn tab_clicked(&mut self, container_id: ContainerId, tab_idx: usize) {
        self.hub.focus_tab_index(container_id, tab_idx);
        self.apply_layout();
    }

    pub(super) fn export_layout(&mut self, path: &std::path::Path) {
        if let Err(e) = self.hub.export_layout(path) {
            tracing::error!("Export layout failed: {e:#}");
        }
    }

    pub(super) fn handle_display_change(&mut self) -> Vec<HwndId> {
        let to_refresh = match self.display.get_all_monitors() {
            Ok(monitors) => self.monitors_changed(monitors),
            Err(e) => {
                tracing::warn!("Failed to enumerate monitors: {e}");
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

    pub(super) fn handle_work_area_change(&mut self) -> Vec<HwndId> {
        match self.display.get_all_monitors() {
            Ok(monitors) => {
                tracing::info!("Work area changed, refreshing monitor geometry");
                self.monitors_changed(monitors)
            }
            Err(e) => {
                tracing::warn!("Failed to enumerate monitors on work area change: {e}");
                Vec::new()
            }
        }
    }

    /// Adding a manageable window.
    #[tracing::instrument(skip_all, fields(pid = ext.pid(), hwnd = %ext.id(), metadata = %metadata))]
    pub(super) fn add_window(
        &mut self,
        NewWindow {
            ext,
            metadata,
            constraints,
        }: NewWindow,
        rect: Dimension<Physical>,
        monitor: isize,
    ) {
        if self.registry.contains_hwnd(ext.id()) {
            return;
        }
        let borderless_fs = self
            .monitors
            .is_borderless_fullscreen_at(PixelRect::from_dimension(rect), monitor);
        let restrictions = if borderless_fs {
            WindowRestrictions::ProtectFullscreen
        } else {
            WindowRestrictions::None
        };
        let Some(id) = self
            .hub
            .insert_window(Box::new(metadata.clone()), rect, restrictions)
        else {
            tracing::trace!(hwnd = %ext.id(), pid = ext.pid(), "ignored by rule");
            return;
        };
        tracing::info!(%id, "New window");
        let state = if borderless_fs {
            WindowState::BorderlessFullscreen
        } else {
            WindowState::Positioned(PositionedState::Offscreen {
                retries: 0,
                actual: PixelRect::from_dimension(rect),
            })
        };
        let id_key = ext.id();
        self.hub.set_window_constraint(id, constraints);
        self.recovery.track(&ext);
        self.registry.insert(
            id_key,
            id,
            ManagedWindow {
                ext,
                state,
                is_minimized: false,
            },
        );
        self.pending_created.push(id);
        self.apply_layout();
    }

    pub(super) fn set_constraints_for(&mut self, hwnd_id: HwndId, constraints: LimitObservation) {
        let Some(id) = self.registry.get_id(hwnd_id) else {
            return;
        };
        self.hub.set_window_constraint(id, constraints);
    }

    #[tracing::instrument(
        skip(self, id_key),
        fields(hwnd = %id_key),
    )]
    pub(super) fn handle_focus(&mut self, id_key: HwndId) {
        let Some(id) = self.registry.get_id(id_key) else {
            return;
        };
        let was_minimized = self
            .registry
            .get(id)
            .map(|entry| entry.is_minimized)
            .unwrap_or(false);
        if was_minimized {
            self.hub.unminimize_window(id);
            if let Some(entry) = self.registry.get_mut(id) {
                entry.is_minimized = false;
            }
        }
        self.hub.set_focus(id);
        tracing::info!("Window focused");
        self.apply_layout();
    }

    pub(super) fn query_workspaces(&self) -> Vec<WorkspaceInfo> {
        self.hub.query_workspaces()
    }

    pub(super) fn query_workspaces_json(&self) -> String {
        serde_json::to_string(&self.hub.query_workspaces())
            .expect("WorkspaceInfo is infallibly serializable")
    }

    pub(super) fn query_minimized_windows_json(&self) -> String {
        let entries: Vec<MinimizedWindow> = self
            .hub
            .minimized_window_entries()
            .into_iter()
            .map(|e| MinimizedWindow {
                id: e.id,
                title: e.title,
                app_name: e.app_name,
                bundle_id: e.bundle_id,
                executable_path: e.executable_path,
            })
            .collect();
        serde_json::to_string(&entries).expect("MinimizedWindow is infallibly serializable")
    }

    pub(super) fn apply_focus(&mut self, target: &FocusTarget) {
        match target {
            FocusTarget::Up => self.hub.handle_tiling_action(TilingAction::FocusDirection {
                direction: Direction::Vertical,
                forward: false,
            }),
            FocusTarget::Down => self.hub.handle_tiling_action(TilingAction::FocusDirection {
                direction: Direction::Vertical,
                forward: true,
            }),
            FocusTarget::Left => self.hub.handle_tiling_action(TilingAction::FocusDirection {
                direction: Direction::Horizontal,
                forward: false,
            }),
            FocusTarget::Right => self.hub.handle_tiling_action(TilingAction::FocusDirection {
                direction: Direction::Horizontal,
                forward: true,
            }),
            FocusTarget::Parent => self.hub.handle_tiling_action(TilingAction::FocusParent),
            FocusTarget::Tab { direction } => {
                self.hub.handle_tiling_action(TilingAction::FocusTab {
                    forward: matches!(direction, TabDirection::Next),
                })
            }
            FocusTarget::Workspace { name } => self.hub.focus_workspace(name),
            FocusTarget::Monitor { target } => self.hub.focus_monitor(target),
        }
    }

    pub(super) fn apply_move(&mut self, target: &MoveTarget) {
        match target {
            MoveTarget::Up => self.hub.handle_tiling_action(TilingAction::MoveDirection {
                direction: Direction::Vertical,
                forward: false,
            }),
            MoveTarget::Down => self.hub.handle_tiling_action(TilingAction::MoveDirection {
                direction: Direction::Vertical,
                forward: true,
            }),
            MoveTarget::Left => self.hub.handle_tiling_action(TilingAction::MoveDirection {
                direction: Direction::Horizontal,
                forward: false,
            }),
            MoveTarget::Right => self.hub.handle_tiling_action(TilingAction::MoveDirection {
                direction: Direction::Horizontal,
                forward: true,
            }),
            MoveTarget::Workspace { name } => self.hub.move_focused_to_workspace(name),
            MoveTarget::Monitor { target } => self.hub.move_focused_to_monitor(target),
        }
    }

    pub(super) fn apply_toggle(&mut self, target: &ToggleTarget) {
        match target {
            ToggleTarget::Spawn => self.hub.handle_tiling_action(TilingAction::ToggleSpawnMode),
            ToggleTarget::Direction => self.hub.handle_tiling_action(TilingAction::ToggleDirection),
            ToggleTarget::Layout => self
                .hub
                .handle_tiling_action(TilingAction::ToggleContainerLayout),
            ToggleTarget::Float => self.hub.toggle_float(),
            ToggleTarget::Fullscreen => self.hub.toggle_fullscreen(),
        }
    }

    pub(super) fn apply_master(&mut self, target: &MasterTarget) {
        let action = match target {
            MasterTarget::Grow => TilingAction::GrowMaster,
            MasterTarget::Shrink => TilingAction::ShrinkMaster,
            MasterTarget::More => TilingAction::MoreMaster,
            MasterTarget::Fewer => TilingAction::FewerMaster,
        };
        self.hub.handle_tiling_action(action);
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub(super) fn unminimize_window(&mut self, id: WindowId) {
        self.hub.unminimize_window(id);
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        if entry.is_minimized {
            entry.ext.show_cmd(ShowCmd::Restore);
            entry.is_minimized = false;
            // entry.state holds the prior Positioned(Tiling/Float/Offscreen) or
            // BorderlessFullscreen variant. The next apply_layout dispatches
            // through show_fullscreen_window / show_tiling / show_float against
            // that preserved state.
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn close_focused_window(&mut self) {
        let Some(window_id) = self.hub.focused_window(self.hub.current_workspace()) else {
            return;
        };
        let Some(entry) = self.registry.get(window_id) else {
            return;
        };
        entry.ext.close();
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) fn apply_layout(&mut self) {
        let created = std::mem::take(&mut self.pending_created);

        let result = self.hub.get_visible_placements();
        let focused_window = result.focused_window;
        let focused_monitor = result.focused_monitor;
        let focused = focused_window;

        let mut per_monitor: Vec<MonitorPositionData> = Vec::new();
        let mut new_displayed: HashMap<MonitorId, HashSet<WindowId>> = HashMap::new();

        for mp in result.monitors {
            let work_area = self.monitors.monitor(mp.monitor_id).work_area();

            let mut window_ids = HashSet::new();

            match &mp.layout {
                MonitorLayout::Fullscreen(id) => {
                    window_ids.insert(*id);
                    self.show_fullscreen_window(*id, work_area, mp.monitor_id);
                }
                MonitorLayout::Normal {
                    tiling_windows,
                    float_windows: fw,
                    containers,
                } => {
                    let mut placed_tiling = Vec::new();
                    let mut placed_floats = Vec::new();
                    let mut container_data = Vec::new();

                    // Windows places tiles unclipped, so a tile extending past the work
                    // area stays where core put it. That is a current choice, not an
                    // invariant -- macOS trims instead (macos/dome/layout.rs).
                    for wp in tiling_windows {
                        window_ids.insert(wp.id);
                        if self.registry.get(wp.id).is_none() {
                            continue;
                        }
                        if wp.content_box.is_empty() {
                            tracing::debug!(
                                window_id = %wp.id,
                                border_box = ?wp.border_box,
                                "Content box entirely border, hiding window"
                            );
                            self.hide_window(wp.id);
                            continue;
                        }
                        placed_tiling.push(*wp);
                    }
                    for wp in fw {
                        window_ids.insert(wp.id);
                        if self.registry.get(wp.id).is_none() {
                            continue;
                        }
                        if wp.content_box.is_empty() {
                            tracing::debug!(
                                window_id = %wp.id,
                                border_box = ?wp.border_box,
                                "Float content box entirely border, hiding window"
                            );
                            self.hide_window(wp.id);
                            continue;
                        }
                        placed_floats.push(*wp);
                    }
                    for cp in containers {
                        if !cp.is_tabbed && !cp.is_highlighted {
                            continue;
                        }
                        let titles = cp.titles.clone();
                        container_data.push((cp.clone(), titles));
                    }

                    per_monitor.push(MonitorPositionData {
                        monitor_id: mp.monitor_id,
                        work_area,
                        border_thickness: mp.border_thickness,
                        tiling_windows: placed_tiling,
                        float_windows: placed_floats,
                        containers: container_data,
                    });
                }
            }

            new_displayed.insert(mp.monitor_id, window_ids);
        }

        // Global diff
        let old_window_ids: HashSet<WindowId> = self
            .monitors
            .monitors()
            .flat_map(|m| m.displayed().iter())
            .copied()
            .collect();
        let new_window_ids: HashSet<WindowId> = new_displayed.values().flatten().copied().collect();
        let to_hide: Vec<WindowId> = old_window_ids
            .difference(&new_window_ids)
            .copied()
            .collect();
        let tabs_to_add: Vec<WindowId> = new_window_ids
            .difference(&old_window_ids)
            .copied()
            .collect();

        // Update displayed state on each monitor.
        // Clear all first, then set the ones that have placements this pass.
        self.monitors.clear_all_displayed();
        for (mid, dm) in new_displayed {
            self.monitors.set_displayed_windows(mid, dm);
        }

        for &id in &to_hide {
            // Keep taskbar tab for user-minimized windows so the user can
            // click it to restore. Dome-hidden windows get their tab removed.
            if let Some(entry) = self.registry.get(id)
                && !entry.is_minimized
            {
                self.taskbar.delete_tab(entry.ext.id());
            }
            self.hide_window(id);
        }

        for &id in &created {
            if !new_window_ids.contains(&id) {
                self.hide_window(id);
            }
        }

        // Position
        self.position_windows(&per_monitor, focused);

        // Clean up float overlays for windows that are no longer float
        let current_float_ids: HashSet<WindowId> = per_monitor
            .iter()
            .flat_map(|m| m.float_windows.iter().map(|wp| wp.id))
            .collect();
        self.float_overlays
            .retain(|id, _| current_float_ids.contains(id));

        // Taskbar
        for &id in &tabs_to_add {
            if let Some(entry) = self.registry.get(id) {
                self.taskbar.add_tab(entry.ext.id());
            }
        }

        // Focus
        let current_monitor = focused_monitor;
        let monitor_changed = self
            .last_focused_monitor
            .is_some_and(|m| m != current_monitor);

        if focused != self.last_focused || monitor_changed {
            self.last_focused = focused;
            if let Some(id) = focused {
                if let Some(entry) = self.registry.get(id)
                    && !matches!(entry.state, WindowState::ExclusiveFullscreen)
                {
                    entry.ext.set_foreground_window();
                }
            } else if let Some(overlay) = self.tiling_overlays.get(&focused_monitor) {
                overlay.focus();
            }
        }
        self.last_focused_monitor = Some(current_monitor);
        self.refresh_tray();
    }

    #[tracing::instrument(level = "trace", skip_all)]
    fn position_windows(&mut self, per_monitor: &[MonitorPositionData], focused: Option<WindowId>) {
        let focus_changed = focused != self.last_focused;

        for data in per_monitor {
            for wp in &data.float_windows {
                let Some(entry) = self.registry.get(wp.id) else {
                    tracing::debug!(id = ?wp.id, "position_windows: float window missing from registry");
                    continue;
                };
                let hwnd_id = entry.ext.id();
                if self.placement_tracker.is_moving(hwnd_id) {
                    continue;
                }
                if !self.float_overlays.contains_key(&wp.id) {
                    match self.overlay_factory.create_float_overlay(
                        self.config.clone(),
                        self.monitors.monitor(data.monitor_id).scale(),
                        wp.visible_border_box,
                    ) {
                        Ok(o) => {
                            self.float_overlays.insert(wp.id, o);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create float overlay: {e:#}");
                            continue;
                        }
                    }
                }
                self.show_float(
                    wp.id,
                    wp,
                    focus_changed,
                    focused == Some(wp.id),
                    data.monitor_id,
                    data.border_thickness,
                );
            }

            if !self.tiling_overlays.contains_key(&data.monitor_id) {
                continue;
            }
            if data.tiling_windows.is_empty() && data.containers.is_empty() {
                self.tiling_overlays
                    .get_mut(&data.monitor_id)
                    .unwrap()
                    .clear();
                continue;
            }
            for wp in &data.tiling_windows {
                let Some(entry) = self.registry.get(wp.id) else {
                    tracing::debug!(id = ?wp.id, "position_windows: tiling window missing from registry");
                    continue;
                };
                let hwnd_id = entry.ext.id();
                // Mid-move: skip SetWindowPos but overlay still gets target rect below.
                if self.placement_tracker.is_moving(hwnd_id) {
                    continue;
                }
                self.show_tiling(wp.id, wp, data.monitor_id);
            }
            let scale = self.monitors.monitor(data.monitor_id).scale();
            self.tiling_overlays
                .get_mut(&data.monitor_id)
                .unwrap()
                .update(
                    data.work_area,
                    &data.tiling_windows,
                    &data.containers,
                    scale,
                    data.border_thickness,
                );
            let tab_bar_h_logical = self.config.partition_tree.tab_bar_height;
            for (placement, titles) in data.containers.iter().filter(|(p, _)| p.is_tabbed) {
                let rect = compute_tab_bar_rect(placement.border_box, tab_bar_h_logical, scale);
                let tab_bar = match self.tab_bars.entry(placement.id) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        match self.overlay_factory.create_tab_bar(
                            self.config.clone(),
                            placement.id,
                            rect,
                            scale,
                        ) {
                            Ok(o) => e.insert(o),
                            Err(err) => {
                                tracing::warn!(?err, "failed to create tab bar");
                                continue;
                            }
                        }
                    }
                };
                tab_bar.update(
                    rect,
                    titles.clone(),
                    placement.active_tab_index,
                    placement.is_highlighted,
                    scale,
                    data.border_thickness,
                );
            }
        }
        let active: HashSet<ContainerId> = per_monitor
            .iter()
            .flat_map(|d| {
                d.containers
                    .iter()
                    .filter(|(p, _)| p.is_tabbed)
                    .map(|(p, _)| p.id)
            })
            .collect();
        self.tab_bars.retain(|id, _| active.contains(id));
    }

    pub(super) fn handle_window_moved(
        &mut self,
        id_key: HwndId,
        new_placement: Dimension<Physical>,
        monitor_handle: isize,
        observed_at: Instant,
    ) {
        let Some(id) = self.registry.get_id(id_key) else {
            return;
        };
        self.window_moved(
            id,
            PixelRect::from_dimension(new_placement),
            monitor_handle,
            observed_at,
        );
        self.apply_layout();
    }

    pub(super) fn update_titles(&mut self, titles: Vec<(HwndId, Option<String>)>) {
        for (hwnd_id, title) in &titles {
            if let (Some(window_id), Some(title)) = (self.registry.get_id(*hwnd_id), title)
                && self.hub.set_window_title(window_id, title.clone())
            {
                tracing::trace!(%window_id, ?hwnd_id, title = %title, "Title changed");
            }
        }
        // TODO: full re-layout on every title change is expensive — we should
        // selectively re-render only the affected tiling overlay instead.
        self.apply_layout();
    }

    fn update_monitors(&mut self, mut monitors: Vec<MonitorInfo>) -> Vec<HwndId> {
        if monitors.is_empty() {
            tracing::warn!("Empty monitor list, skipping update");
            return Vec::new();
        }
        self.status_bars.reserve(&mut monitors, &self.monitors);
        let change = self.monitors.reconcile(&mut self.hub, &monitors);
        for id in change.added {
            let m = self.monitors.monitor(id);
            if let Ok(overlay) = self.overlay_factory.create_tiling_overlay(
                self.config.clone(),
                self.config.partition_tree.tab_bar_height,
                m.work_area(),
                m.scale(),
            ) {
                self.tiling_overlays.insert(id, overlay);
            }
        }
        for id in change.removed {
            self.tiling_overlays.remove(&id);
        }

        self.registry
            .iter()
            .filter(|(_, id)| {
                self.registry
                    .get(*id)
                    .is_none_or(|e| !matches!(e.state, WindowState::ExclusiveFullscreen))
            })
            .map(|(hwnd_id, _)| hwnd_id)
            .collect()
    }

    pub(super) fn capture_bar(
        &mut self,
        hwnd_id: HwndId,
        monitor: isize,
        rect: Dimension<Physical>,
    ) {
        if let Some(mid) = self.monitors.id_for_handle(monitor) {
            self.status_bars.capture(hwnd_id, mid, rect);
            tracing::info!(%hwnd_id, %mid, ?rect, "Status bar recognized, reserving work area");
            self.recompute_work_areas();
        } else {
            tracing::warn!(handle = monitor, "known bar on unknown monitor handle");
        }
    }

    pub(super) fn remove_bar(&mut self, hwnd_id: HwndId) -> bool {
        if self.status_bars.remove(hwnd_id).is_some() {
            self.recompute_work_areas();
            true
        } else {
            false
        }
    }

    pub(super) fn is_known_bar(metadata: &WindowsMetadata) -> bool {
        StatusBars::is_known_bar(metadata)
    }

    pub(super) fn is_tracked_bar(&self, hwnd_id: HwndId) -> bool {
        self.status_bars.is_tracked(hwnd_id)
    }

    pub(in crate::platform::windows) fn bar_moved(
        &mut self,
        hwnd_id: HwndId,
        monitor_handle: isize,
        rect: Dimension<Physical>,
    ) {
        if let Some(mid) = self.monitors.id_for_handle(monitor_handle) {
            self.status_bars.move_to(hwnd_id, mid, rect);
            self.recompute_work_areas();
        }
    }

    fn recompute_work_areas(&mut self) {
        match self.display.get_all_monitors() {
            Ok(monitors) => {
                self.update_monitors(monitors);
                self.apply_layout();
            }
            Err(e) => tracing::warn!("Failed to enumerate monitors for bar reservation: {e}"),
        }
    }

    /// Updates the DPI scale for a monitor identified by its Win32 HMONITOR handle.
    /// Called from the dome-thread message loop when WM_APP_DPI_CHANGE arrives.
    ///
    /// Early-returns silently when the computed scale equals the stored value.
    /// This absorbs duplicate posts from multiple Dome-owned wnd-procs on the
    /// same monitor (all four HWNDs default to the primary monitor, so a
    /// primary-monitor DPI change posts WM_APP_DPI_CHANGE four times).
    pub(super) fn monitor_dpi_changed(&mut self, handle: isize, dpi: u32) {
        self.monitors.apply_dpi_change(handle, dpi, &mut self.hub);
    }

    pub(super) fn retry_drifted_windows(&mut self) {
        let window_ids: Vec<(HwndId, WindowId)> = self.registry.iter().collect();
        for (_hwnd_id, window_id) in window_ids {
            self.retry_drift(window_id);
        }
    }

    pub(super) fn is_managed(&self, id_key: HwndId) -> bool {
        self.registry.contains_hwnd(id_key)
    }
}

// Fallback display string derived from the executable name. Prefer
// FileDescription from version info when available (see get_app_display_name).
pub(super) fn display_from_process(process: &str) -> String {
    process.strip_suffix(".exe").unwrap_or(process).to_string()
}

// The bar hugs the container's top edge, with the configured logical height rounded
// into the platform's `Unit`.
fn compute_tab_bar_rect(
    border_box: PixelRect,
    tab_bar_h_logical: Length<Logical>,
    scale: f32,
) -> PixelRect {
    let h_phys = tab_bar_h_logical.to_unit(scale).round();
    PixelRect::new(
        border_box.x(),
        border_box.y(),
        border_box.width(),
        h_phys.value() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_from_process_strips_exe() {
        assert_eq!(display_from_process("chrome.exe"), "chrome");
        assert_eq!(display_from_process("notepad"), "notepad");
    }
}
