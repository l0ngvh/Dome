pub(super) mod icon;
pub(super) mod monitor;
pub(super) mod overlay;
pub(super) mod picker;
mod placement_tracker;
mod recovery;
mod registry;
pub(crate) mod rejection_log_filter;
mod window;

pub(super) use self::monitor::{MonitorInfo, QueryDisplay, Win32Display};

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use crate::action::Query;
use crate::action::{Actions, FocusTarget, MasterTarget, MoveTarget, TabDirection, ToggleTarget};
use crate::config::{Config, LayoutConfig, LayoutWorkspaceConfig};
use crate::core::GlobalLayoutConfig;
use crate::core::{
    ContainerId, ContainerPlacement, Dimension, Direction, FloatWindowPlacement, Hub, Length,
    Logical, MonitorId, MonitorLayout, Physical, TilingAction, TilingWindowPlacement, WindowId,
    WindowRestrictions,
};
use crate::picker::build_picker_entries;

use self::overlay::{FloatOverlayApi, TabBarOverlayApi, TilingOverlayApi};
use self::placement_tracker::PlacementTracker;
use self::recovery::Recovery;
use self::registry::{ManagedWindow, WindowRegistry};
use self::window::{PositionedState, WindowState};

pub(super) use self::window::NewWindow;
pub(super) use self::window::WindowsMetadata;

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
    TabClicked(ContainerId, usize),
    Shutdown,
}

struct MonitorPositionData {
    monitor_id: MonitorId,
    dimension: Dimension,
    tiling_windows: Vec<TilingWindowPlacement>,
    float_windows: Vec<FloatWindowPlacement>,
    containers: Vec<(ContainerPlacement, Vec<String>)>,
}

pub(super) trait CreateOverlay {
    fn create_tiling_overlay(
        &self,
        config: Config,
        tab_bar_height: Length<Logical>,
        monitor: Dimension,
        scale: f32,
    ) -> anyhow::Result<Box<dyn TilingOverlayApi>>;
    fn create_float_overlay(
        &self,
        config: Config,
        scale: f32,
        visible_frame: Dimension,
    ) -> anyhow::Result<Box<dyn FloatOverlayApi>>;
    fn create_tab_bar(
        &self,
        config: Config,
        container_id: ContainerId,
        rect: Dimension,
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
    workspace_overrides: Vec<LayoutWorkspaceConfig>,
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
    picker: Box<dyn overlay::PickerApi>,
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
        picker: Box<dyn overlay::PickerApi>,
    ) -> anyhow::Result<Self> {
        let monitors = display.get_all_monitors()?;
        anyhow::ensure!(!monitors.is_empty(), "No monitors detected");
        let primary = monitors
            .iter()
            .find(|s| s.is_primary)
            .unwrap_or(&monitors[0]);
        let mut hub = Hub::new(
            primary.dimension,
            primary.scale,
            GlobalLayoutConfig::from(&config),
            workspace_overrides.clone(),
            config.ignore.clone(),
        );
        let primary_monitor_id = hub.focused_monitor();
        let mut monitors_reg = MonitorRegistry::new();
        let mut tiling_overlays: HashMap<MonitorId, Box<dyn TilingOverlayApi>> = HashMap::new();
        monitors_reg.insert(
            primary.handle,
            primary_monitor_id,
            primary.dimension,
            primary.scale,
        );
        if let Ok(overlay) = overlay_factory.create_tiling_overlay(
            config.clone(),
            config.partition_tree.tab_bar_height,
            primary.dimension,
            primary.scale,
        ) {
            tiling_overlays.insert(primary_monitor_id, overlay);
        }
        tracing::info!(
            name = %primary.name,
            handle = ?primary.handle,
            dimension = ?primary.dimension,
            "Primary monitor"
        );

        for monitor in &monitors {
            if monitor.handle != primary.handle {
                let id = hub.add_monitor(monitor.name.clone(), monitor.dimension, monitor.scale);
                monitors_reg.insert(monitor.handle, id, monitor.dimension, monitor.scale);
                if let Ok(overlay) = overlay_factory.create_tiling_overlay(
                    config.clone(),
                    config.partition_tree.tab_bar_height,
                    monitor.dimension,
                    monitor.scale,
                ) {
                    tiling_overlays.insert(id, overlay);
                }
                tracing::info!(
                    name = %monitor.name,
                    handle = ?monitor.handle,
                    dimension = ?monitor.dimension,
                    "Monitor"
                );
            }
        }

        Ok(Self {
            hub,
            registry: WindowRegistry::new(),
            monitors: monitors_reg,
            config,
            workspace_overrides,
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
            picker,
        })
    }

    pub(super) fn config_changed(&mut self, new_config: Config) {
        let workspace_overrides = self.workspace_overrides.clone();
        self.hub
            .sync_config(GlobalLayoutConfig::from(&new_config), workspace_overrides);
        self.hub.set_ignore_rules(new_config.ignore.clone());
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
        self.picker.set_config(&self.config);
        tracing::info!("Config reloaded");
        self.apply_layout();
    }

    pub(super) fn layout_changed(&mut self, new_layout: LayoutConfig) {
        let layout_settings = GlobalLayoutConfig::from(&self.config);
        self.workspace_overrides = new_layout.workspace;
        self.hub
            .sync_config(layout_settings, self.workspace_overrides.clone());
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
        let borderless_fs = self.monitors.is_borderless_fullscreen_at(rect, monitor);
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
                actual: rect,
            })
        };
        let id_key = ext.id();
        self.set_constraints(id, constraints);
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

    fn resolve_window_monitor(&self, id: WindowId) -> MonitorId {
        let Some(entry) = self.registry.get(id) else {
            return self.hub.focused_monitor();
        };
        if entry.is_minimized {
            return self.hub.focused_monitor();
        }
        match entry.state {
            WindowState::Positioned(PositionedState::Tiling(d)) => d.monitor,
            WindowState::Positioned(PositionedState::Float(fp)) => fp.monitor,
            // Offscreen, BorderlessFullscreen, ExclusiveFullscreen, or unregistered:
            // best-effort fallback to focused monitor.
            // The next apply_layout retriggers set_constraints via the Tiling/Float branch.
            _ => self.hub.focused_monitor(),
        }
    }

    pub(super) fn set_constraints(&mut self, id: WindowId, constraints: (f32, f32, f32, f32)) {
        // FIXME: resolve_window_monitor is best effort, so it can return the wrong monitor. If the
        // window is immediately minimized after spawn, then we'd get the wrong border
        let monitor = self.resolve_window_monitor(id);
        let border = self
            .monitors
            .physical_border(monitor, self.config.border_size)
            .value();
        let (min_w, min_h, max_w, max_h) = constraints;
        if min_w > 0.0 || min_h > 0.0 || max_w > 0.0 || max_h > 0.0 {
            let to_frame = |v: f32| {
                if v > 0.0 {
                    Some(v + 2.0 * border)
                } else {
                    None
                }
            };
            // No pre-check against stored values: calling set_window_constraint with
            // unchanged values is cheap (the runner's apply_layout diffs against cached
            // placements and skips windows whose target is unchanged).
            self.hub.set_window_constraint(
                id,
                to_frame(min_w),
                to_frame(min_h),
                to_frame(max_w),
                to_frame(max_h),
            );
        }
    }

    pub(super) fn set_constraints_for(
        &mut self,
        hwnd_id: HwndId,
        constraints: (f32, f32, f32, f32),
    ) {
        let Some(id) = self.registry.get_id(hwnd_id) else {
            return;
        };
        self.set_constraints(id, constraints);
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

    pub(super) fn query_workspaces_json(&self) -> String {
        serde_json::to_string(&self.hub.query_workspaces())
            .expect("WorkspaceInfo is infallibly serializable")
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

    pub(super) fn toggle_picker(&mut self) {
        if self.picker.is_visible() {
            self.picker.hide();
        } else {
            let minimized = self.hub.minimized_window_entries();
            let entries = build_picker_entries(&minimized);
            let focused_monitor = self.hub.focused_monitor();
            let m = self.monitors.monitor(focused_monitor);
            let monitor_dim = m.dimension();
            let scale = m.scale();
            self.picker.show(entries, monitor_dim, scale);
        }
    }

    pub(super) fn picker_icons_to_load(&mut self) -> Vec<(String, super::external::HwndId)> {
        let registry = &self.registry;
        self.picker
            .icons_to_load(&|wid| registry.get(wid).map(|e| e.ext.id()))
    }

    pub(super) fn picker_receive_icon(&mut self, app_id: String, image: egui::ColorImage) {
        self.picker.receive_icon(app_id, image);
    }

    pub(super) fn picker_visible(&self) -> bool {
        self.picker.is_visible()
    }

    pub(super) fn picker_scale(&self) -> Option<f32> {
        if !self.picker.is_visible() {
            return None;
        }
        let focused = self.hub.focused_monitor();
        Some(self.monitors.monitor(focused).scale())
    }

    pub(super) fn picker_rerender(&mut self) {
        self.picker.rerender();
    }

    /// Unminimize a window selected via the picker.
    pub(super) fn picker_unminimize_window(&mut self, id: WindowId) {
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
            let dimension = self.monitors.monitor(mp.monitor_id).dimension();

            let mut window_ids = HashSet::new();

            match &mp.layout {
                MonitorLayout::Fullscreen(id) => {
                    window_ids.insert(*id);
                    self.show_fullscreen_window(*id, dimension, mp.monitor_id);
                }
                MonitorLayout::Normal {
                    tiling_windows,
                    float_windows: fw,
                    containers,
                } => {
                    let mut placed_tiling = Vec::new();
                    let mut placed_floats = Vec::new();
                    let mut container_data = Vec::new();

                    for wp in tiling_windows {
                        window_ids.insert(wp.id);
                        if self.registry.get(wp.id).is_none() {
                            continue;
                        }
                        placed_tiling.push(*wp);
                    }
                    for wp in fw {
                        window_ids.insert(wp.id);
                        if self.registry.get(wp.id).is_none() {
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
                        dimension,
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

        // Hide
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
                        wp.visible_frame,
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
                    data.dimension,
                    &data.tiling_windows,
                    &data.containers,
                    scale,
                );
            let tab_bar_h_logical = self.config.partition_tree.tab_bar_height;
            for (placement, titles) in data.containers.iter().filter(|(p, _)| p.is_tabbed) {
                let rect = compute_tab_bar_rect(placement.frame, tab_bar_h_logical, scale);
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
        self.window_moved(id, new_placement, monitor_handle, observed_at);
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

    fn update_monitors(&mut self, monitors: Vec<MonitorInfo>) -> Vec<HwndId> {
        if monitors.is_empty() {
            tracing::warn!("Empty monitor list, skipping update");
            return Vec::new();
        }
        let change = self.monitors.reconcile(&mut self.hub, &monitors);
        for id in change.added {
            let m = self.monitors.monitor(id);
            if let Ok(overlay) = self.overlay_factory.create_tiling_overlay(
                self.config.clone(),
                self.config.partition_tree.tab_bar_height,
                m.dimension(),
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
}

// Fallback display string derived from the executable name. Prefer
// FileDescription from version info when available (see get_app_display_name).
pub(super) fn display_from_process(process: &str) -> String {
    process.strip_suffix(".exe").unwrap_or(process).to_string()
}

// Tab bar rect from a tabbed container's physical-pixel `frame`. The bar
// hugs the container's top edge with the configured logical height
// rounded into the platform's `Unit` (physical pixels on Windows).
fn compute_tab_bar_rect(
    frame: Dimension,
    tab_bar_h_logical: Length<Logical>,
    scale: f32,
) -> Dimension {
    let h_phys = tab_bar_h_logical.to_unit(scale).round();
    Dimension::new(frame.x, frame.y, frame.width, h_phys)
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
