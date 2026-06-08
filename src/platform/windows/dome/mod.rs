pub(super) mod icon;
pub(super) mod overlay;
pub(super) mod picker;
mod placement_tracker;
mod recovery;
mod registry;
mod window;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::action::Query;
use crate::action::{Actions, FocusTarget, MasterTarget, MoveTarget, TabDirection, ToggleTarget};
use crate::config::{Config, WindowMode, WindowsWindow};
use crate::core::{
    ContainerId, ContainerPlacement, Dimension, Direction, FloatWindowPlacement, Hub, MonitorId,
    MonitorLayout, Physical, TilingAction, TilingWindowPlacement, WindowId, WindowRestrictions,
    WorkspaceId,
};
use crate::font::{FontConfig, font_changed};
use crate::picker::{PickerEntry, build_picker_entries};
use crate::theme::{Flavor, theme_changed};

use self::overlay::{FloatOverlayApi, TilingOverlayApi};
use self::placement_tracker::PlacementTracker;
use self::recovery::Recovery;
use self::registry::WindowRegistry;
use self::window::{PositionedState, WindowState};

pub(super) use self::window::NewWindow;

use super::MonitorInfo;
use super::external::{HwndId, ShowCmd};
use super::taskbar::ManageTaskbar;

pub(super) enum HubEvent {
    WindowCreated(HwndId),
    WindowDestroyed(HwndId),
    WindowMinimized(HwndId),
    WindowRestored(HwndId),
    WindowFocused(HwndId),
    WindowTitleChanged(HwndId),
    MoveSizeStart(HwndId),
    MoveSizeEnd(HwndId),
    LocationChanged(HwndId),
    Action(Actions),
    Query {
        query: Query,
        sender: std::sync::mpsc::SyncSender<String>,
    },
    ConfigChanged(Box<Config>),
    TabClicked(ContainerId, usize),
    Shutdown,
}

/// Per-monitor state: physical dimension, DPI scale, and the set of windows
/// currently laid out on this monitor (rebuilt each `apply_layout` pass).
pub(super) struct MonitorState {
    /// Work area of the monitor
    dimension: Dimension,
    /// Monitor scale factor
    scale: f32,
    /// List of windows currently being displayed
    displayed: HashSet<WindowId>,
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
        monitor: Dimension,
        scale: f32,
    ) -> anyhow::Result<Box<dyn TilingOverlayApi>>;
    fn create_float_overlay(
        &self,
        flavor: Flavor,
        font: &FontConfig,
        scale: f32,
        visible_frame: Dimension,
    ) -> anyhow::Result<Box<dyn FloatOverlayApi>>;
    fn create_picker(
        &self,
        entries: Vec<PickerEntry>,
        monitor_dim: Dimension,
        flavor: Flavor,
        font: &FontConfig,
        scale: f32,
    ) -> anyhow::Result<Box<dyn overlay::PickerApi>>;
}

/// Holds Win32 foreground when Dome has no managed window to focus
/// (empty workspace, `focus_parent` container-highlight).
pub(super) trait FocusSinkApi {
    fn focus(&self);
}

pub(super) trait QueryDisplay {
    fn get_all_monitors(&self) -> anyhow::Result<Vec<MonitorInfo>>;
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
    monitors: HashMap<MonitorId, MonitorState>,
    config: Config,
    taskbar: Rc<dyn ManageTaskbar>,
    overlay_factory: Box<dyn CreateOverlay>,
    display: Box<dyn QueryDisplay>,
    tiling_overlays: HashMap<MonitorId, Box<dyn TilingOverlayApi>>,
    float_overlays: HashMap<WindowId, Box<dyn FloatOverlayApi>>,
    focus_sink: Box<dyn FocusSinkApi>,
    last_focused: Option<WindowId>,
    last_focused_monitor: Option<MonitorId>,
    pending_created: Vec<WindowId>,
    placement_tracker: PlacementTracker,
    recovery: Recovery,
    picker: Option<Box<dyn overlay::PickerApi>>,
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
        focus_sink: Box<dyn FocusSinkApi>,
    ) -> anyhow::Result<Self> {
        let monitors = display.get_all_monitors()?;
        anyhow::ensure!(!monitors.is_empty(), "No monitors detected");
        let primary = monitors
            .iter()
            .find(|s| s.is_primary)
            .unwrap_or(&monitors[0]);
        let mut hub = Hub::new(primary.dimension, primary.scale, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_handles = HashMap::new();
        let mut monitor_states = HashMap::new();
        let mut tiling_overlays: HashMap<MonitorId, Box<dyn TilingOverlayApi>> = HashMap::new();
        monitor_handles.insert(primary.handle, primary_monitor_id);
        monitor_states.insert(
            primary_monitor_id,
            MonitorState {
                dimension: primary.dimension,
                scale: primary.scale,
                displayed: HashSet::new(),
            },
        );
        if let Ok(overlay) =
            overlay_factory.create_tiling_overlay(config.clone(), primary.dimension, primary.scale)
        {
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
                monitor_handles.insert(monitor.handle, id);
                monitor_states.insert(
                    id,
                    MonitorState {
                        dimension: monitor.dimension,
                        scale: monitor.scale,
                        displayed: HashSet::new(),
                    },
                );
                if let Ok(overlay) = overlay_factory.create_tiling_overlay(
                    config.clone(),
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
            monitor_handles,
            monitors: monitor_states,
            config,
            taskbar: taskbar.clone(),
            overlay_factory,
            display,
            tiling_overlays,
            float_overlays: HashMap::new(),
            focus_sink,
            last_focused: None,
            last_focused_monitor: None,
            pending_created: Vec::new(),
            placement_tracker: PlacementTracker::new(),
            recovery: Recovery::new(taskbar),
            picker: None,
        })
    }

    pub(super) fn config_changed(&mut self, new_config: Config) {
        let old_flavor = self.config.theme;
        let old_font = self.config.font.clone();
        self.hub.sync_config(new_config.clone().into());
        for overlay in self.tiling_overlays.values_mut() {
            overlay.set_config(new_config.clone());
        }
        self.config = new_config;
        if theme_changed(old_flavor, self.config.theme) {
            for overlay in self.tiling_overlays.values_mut() {
                overlay.apply_theme(self.config.theme);
            }
            for overlay in self.float_overlays.values_mut() {
                overlay.apply_theme(self.config.theme);
            }
            if let Some(picker) = self.picker.as_mut() {
                picker.apply_theme(self.config.theme);
            }
        }
        if font_changed(&old_font, &self.config.font) {
            for overlay in self.tiling_overlays.values_mut() {
                overlay.apply_font(&self.config.font);
            }
            for overlay in self.float_overlays.values_mut() {
                overlay.apply_font(&self.config.font);
            }
        }
        tracing::info!("Config reloaded");
        self.apply_layout();
    }

    #[tracing::instrument(
        skip(self, id_key),
        fields(hwnd = %id_key, window = tracing::field::Empty),
    )]
    pub(super) fn window_destroyed(&mut self, id_key: HwndId) {
        if let Some(id) = self.registry.get_id(id_key)
            && let Some(entry) = self.registry.get(id)
        {
            tracing::Span::current().record("window", entry.to_string());
        }

        self.placement_tracker.clear(id_key);
        self.taskbar.delete_tab(id_key);
        self.recovery.untrack(id_key);
        if let Some(id) = self.registry.remove_by_hwnd(id_key) {
            tracing::info!(%id, "Window removed");
            self.float_overlays.remove(&id);
            for ms in self.monitors.values_mut() {
                ms.displayed.remove(&id);
            }
            self.hub.delete_window(id);
            self.apply_layout();
        }
    }

    #[tracing::instrument(
        skip(self, id_key),
        fields(hwnd = %id_key, window = tracing::field::Empty),
    )]
    pub(super) fn window_minimized(&mut self, id_key: HwndId) {
        let Some(id) = self.registry.get_id(id_key) else {
            return;
        };
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        tracing::Span::current().record("window", entry.to_string());
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

    pub(super) fn move_size_ended(&mut self, id_key: HwndId) {
        self.placement_tracker.drag_ended(id_key);
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

    pub(super) fn registry_contains_hwnd(&self, id: HwndId) -> bool {
        self.registry.contains_hwnd(id)
    }

    pub(super) fn registry_get_id(&self, id: HwndId) -> Option<WindowId> {
        self.registry.get_id(id)
    }

    /// Single entry point for adding a newly-detected external window.
    ///
    /// Mirrors macOS's `reconcile_windows` insert path: applies the
    /// already-known shell-side filters (ignore-rules, on-open lookup,
    /// borderless-fullscreen detection, `should_float`) and dispatches to the
    /// matching `insert_*_window` helper, then flushes layout. The
    /// `is_manageable` filter still lives on the inspection side (the worker
    /// thread that produces this call's arguments) so unmanageable windows
    /// never reach this function.
    #[tracing::instrument(skip_all, fields(window = %new))]
    pub(super) fn add_window(&mut self, new: NewWindow, rect: Dimension<Physical>, monitor: isize) {
        if self.registry.contains_hwnd(new.ext.id()) {
            return;
        }
        if should_ignore(&new, &self.config.windows.ignore) {
            return;
        }
        let (target_ws, mode_override) = self.resolve_on_open(&new);
        let resolved_mode = mode_override.unwrap_or_else(|| {
            if self.is_borderless_fullscreen_at(rect, monitor) {
                WindowMode::Fullscreen
            } else if new.ext.should_float() {
                WindowMode::Float
            } else {
                WindowMode::Tiling
            }
        });
        match resolved_mode {
            WindowMode::Tiling => {
                self.insert_tiling_window(new, rect, target_ws);
            }
            WindowMode::Float => {
                self.insert_float_window(new, rect, target_ws);
            }
            WindowMode::Fullscreen => {
                // mode_override == Some(Fullscreen) is a user-explicit rule -> no protection.
                // mode_override == None && is_borderless_fullscreen_at is shell-detected -> protect.
                let restrictions = if mode_override.is_some() {
                    WindowRestrictions::None
                } else {
                    WindowRestrictions::ProtectFullscreen
                };
                self.insert_fullscreen_window(new, target_ws, restrictions);
            }
        }
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
        let border = self.physical_border(monitor).value();
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

    #[tracing::instrument(
        skip(self, id_key),
        fields(hwnd = %id_key, window = tracing::field::Empty),
    )]
    pub(super) fn handle_focus(&mut self, id_key: HwndId) {
        if let Some(id) = self.registry.get_id(id_key) {
            if let Some(entry) = self.registry.get(id) {
                tracing::Span::current().record("window", entry.to_string());
            }
            self.hub.set_focus(id);
            tracing::info!("Window focused");
            self.apply_layout();
        }
    }

    /// Called by the run loop when a drag safety timeout or resize debounce
    /// timer fires. Removes the window from the placement tracker.
    pub(super) fn placement_timeout(&mut self, id: HwndId) {
        self.placement_tracker.clear(id);
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
        match &mut self.picker {
            Some(pw) if pw.is_visible() => {
                pw.hide();
            }
            Some(pw) => {
                let minimized = self.hub.minimized_window_entries();
                let entries = build_picker_entries(&minimized, |wid| {
                    let Some(e) = self.registry.get(wid) else {
                        return (None, None);
                    };
                    let display = e
                        .app_name
                        .clone()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| display_from_process(&e.process));
                    (Some(e.process.clone()), Some(display))
                });
                let focused_monitor = self.hub.focused_monitor();
                let ms = &self.monitors[&focused_monitor];
                let monitor_dim = ms.dimension;
                let scale = ms.scale;
                pw.show(entries, monitor_dim, scale);
            }
            None => {
                let minimized = self.hub.minimized_window_entries();
                let entries = build_picker_entries(&minimized, |wid| {
                    let Some(e) = self.registry.get(wid) else {
                        return (None, None);
                    };
                    let display = e
                        .app_name
                        .clone()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| display_from_process(&e.process));
                    (Some(e.process.clone()), Some(display))
                });
                let focused_monitor = self.hub.focused_monitor();
                let monitor_dim = self.monitors[&focused_monitor].dimension;
                match self.overlay_factory.create_picker(
                    entries,
                    monitor_dim,
                    self.config.theme,
                    &self.config.font,
                    self.monitors[&focused_monitor].scale,
                ) {
                    Ok(pw) => {
                        self.picker = Some(pw);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create picker window: {e:#}");
                    }
                }
            }
        }
    }

    pub(super) fn picker_icons_to_load(&mut self) -> Vec<(String, super::external::HwndId)> {
        let Some(picker) = &mut self.picker else {
            return Vec::new();
        };
        let registry = &self.registry;
        picker.icons_to_load(&|wid| registry.get(wid).map(|e| e.ext.id()))
    }

    pub(super) fn picker_receive_icon(&mut self, app_id: String, image: egui::ColorImage) {
        if let Some(picker) = &mut self.picker {
            picker.receive_icon(app_id, image);
        }
    }

    pub(super) fn picker_visible(&self) -> bool {
        self.picker.as_ref().is_some_and(|p| p.is_visible())
    }

    pub(super) fn picker_scale(&self) -> Option<f32> {
        let picker = self.picker.as_ref()?;
        if !picker.is_visible() {
            return None;
        }
        let focused = self.hub.focused_monitor();
        Some(self.monitors[&focused].scale)
    }

    pub(super) fn picker_rerender(&mut self) {
        if let Some(picker) = &mut self.picker {
            picker.rerender();
        }
    }

    /// Unminimize a window selected via the picker. Unlike the Win32-driven
    /// taskbar path (where `EVENT_SYSTEM_MINIMIZEEND` triggers a placement
    /// read whose result drives the unminimize fold inside `window_moved`),
    /// the picker path must drive both the core
    /// state and the OS state: tell the hub the window is back, ask Windows
    /// to restore it, and clear the minimize flag so apply_layout dispatches
    /// against the preserved WindowState.
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
            let dimension = self.monitors[&mp.monitor_id].dimension;

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
            .values()
            .flat_map(|ms| &ms.displayed)
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
        for ms in self.monitors.values_mut() {
            ms.displayed.clear();
        }
        for (mid, dm) in new_displayed {
            if let Some(ms) = self.monitors.get_mut(&mid) {
                ms.displayed = dm;
            }
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
            } else {
                self.focus_sink.focus();
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
                        self.config.theme,
                        &self.config.font,
                        self.monitors[&data.monitor_id].scale,
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
            let scale = self.monitors[&data.monitor_id].scale;
            self.tiling_overlays
                .get_mut(&data.monitor_id)
                .unwrap()
                .update(
                    data.dimension,
                    &data.tiling_windows,
                    &data.containers,
                    scale,
                );
        }
    }

    pub(super) fn update_titles(&mut self, titles: Vec<(HwndId, Option<String>)>) {
        for (hwnd_id, title) in &titles {
            self.registry.set_title(*hwnd_id, title.clone());
            if let (Some(window_id), Some(title)) = (self.registry.get_id(*hwnd_id), title) {
                self.hub.set_window_title(window_id, title.clone());
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
        self.reconcile_monitors(monitors);

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

    fn reconcile_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        let current_handles: HashSet<isize> = monitors.iter().map(|s| s.handle).collect();

        for monitor in &monitors {
            if !self.monitor_handles.contains_key(&monitor.handle) {
                let id =
                    self.hub
                        .add_monitor(monitor.name.clone(), monitor.dimension, monitor.scale);
                self.monitor_handles.insert(monitor.handle, id);
                self.monitors.insert(
                    id,
                    MonitorState {
                        dimension: monitor.dimension,
                        scale: monitor.scale,
                        displayed: HashSet::new(),
                    },
                );
                if let Ok(overlay) = self.overlay_factory.create_tiling_overlay(
                    self.config.clone(),
                    monitor.dimension,
                    monitor.scale,
                ) {
                    self.tiling_overlays.insert(id, overlay);
                }
                tracing::info!(
                    name = %monitor.name,
                    handle = ?monitor.handle,
                    dimension = ?monitor.dimension,
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

        let fallback = monitors
            .iter()
            .find(|s| s.is_primary)
            .and_then(|s| self.monitor_handles.get(&s.handle).copied());

        for monitor_id in to_remove {
            if let Some(fallback_id) = fallback
                && fallback_id != monitor_id
            {
                self.hub.remove_monitor(monitor_id, fallback_id);
                self.monitor_handles.retain(|_, &mut id| id != monitor_id);
                self.monitors.remove(&monitor_id);
                self.tiling_overlays.remove(&monitor_id);
                tracing::info!(%monitor_id, fallback = %fallback_id, "Monitor removed");
            }
        }

        for monitor in &monitors {
            if let Some(&id) = self.monitor_handles.get(&monitor.handle)
                && let Some(ms) = self.monitors.get(&id)
                && (ms.dimension != monitor.dimension || ms.scale != monitor.scale)
            {
                let old_dim = Some(ms.dimension);
                let old_scale = Some(ms.scale);
                tracing::info!(
                    name = %monitor.name,
                    ?old_dim,
                    new_dim = ?monitor.dimension,
                    ?old_scale,
                    new_scale = ?monitor.scale,
                    "Monitor dimension changed"
                );
                let ms = self.monitors.get_mut(&id).expect("just checked");
                ms.dimension = monitor.dimension;
                ms.scale = monitor.scale;
                self.hub
                    .update_monitor(id, monitor.dimension, monitor.scale);
            }
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
        let Some(&id) = self.monitor_handles.get(&handle) else {
            tracing::warn!(handle, dpi, "DPI change for unknown monitor handle");
            return;
        };
        let scale = dpi as f32 / crate::platform::windows::display::BASE_DPI;
        // Same-scale early return: absorbs duplicate posts without log noise.
        if self.monitors.get(&id).is_some_and(|ms| ms.scale == scale) {
            return;
        }
        let previous = self.monitors.get_mut(&id).map(|ms| {
            let prev = ms.scale;
            ms.scale = scale;
            prev
        });
        // Propagate the new scale into core so layout math uses the updated
        // multiplier when the caller-scheduled apply_layout reruns.
        let current_dim = self.monitors[&id].dimension;
        self.hub.update_monitor(id, current_dim, scale);
        tracing::info!(%id, dpi, scale, ?previous, "Monitor scale updated via DPI change");
    }

    fn resolve_on_open(&mut self, new: &NewWindow) -> (WorkspaceId, Option<WindowMode>) {
        let rule = self
            .config
            .windows
            .on_open
            .iter()
            .find(|r| r.matches(&new.process, new.title.as_deref()));
        let target_ws = self
            .hub
            .resolve_workspace(rule.and_then(|r| r.workspace.as_deref()));
        let mode_override = rule.and_then(|r| r.mode);
        (target_ws, mode_override)
    }
}

pub(super) fn should_ignore(new: &NewWindow, rules: &[WindowsWindow]) -> bool {
    if let Some(rule) = rules
        .iter()
        .find(|r| r.matches(&new.process, new.title.as_deref()))
    {
        tracing::debug!(%new, ?rule, "Window ignored by rule");
        return true;
    }
    false
}

// Fallback display string derived from the executable name. Prefer
// FileDescription from version info when available (see get_app_display_name).
fn display_from_process(process: &str) -> String {
    process.strip_suffix(".exe").unwrap_or(process).to_string()
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
