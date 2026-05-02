pub(super) mod icon;
pub(super) mod overlay;
pub(super) mod picker;
mod placement_tracker;
mod recovery;
mod registry;
mod window;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use crate::action::Query;
use crate::action::{
    Actions, FocusTarget, HubAction, MasterTarget, MoveTarget, TabDirection, ToggleTarget,
};
use crate::config::{Config, WindowsOnOpenRule, WindowsWindow};
use crate::core::{
    ContainerId, ContainerPlacement, Dimension, Direction, FloatWindowPlacement, Hub, MonitorId,
    MonitorLayout, TilingAction, TilingWindowPlacement, WindowId, WindowRestrictions,
};
use crate::font::{FontConfig, font_changed};
use crate::picker::{PickerEntry, build_picker_entries};
use crate::theme::{Flavor, theme_changed};

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
use super::external::{HwndId, ManageExternalHwnd, ShowCmd};
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

struct DisplayedMonitor {
    window_ids: HashSet<WindowId>,
}

/// Per-monitor state bundling dimension, DPI scale, and current display info.
/// `dimension` and `scale` are populated when the monitor is first seen (in
/// `new` or `reconcile_monitors`). `displayed` is rebuilt each `apply_layout`
/// pass and tracks which windows are currently visible on this monitor.
pub(super) struct MonitorState {
    dimension: Dimension,
    // Exposed as pub(super) because window.rs and lifecycle tests read it.
    // The lifecycle invariant documented above applies to `displayed`, not `scale`,
    // so a plain field access is fine here.
    pub(super) scale: f32,
    displayed: Option<DisplayedMonitor>,
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

/// Holds Win32 foreground when Dome has no managed window to focus (empty
/// workspace, `focus_parent` container-highlight). A dedicated invisible HWND
/// avoids raising the tiling overlay, which would swallow clicks on managed
/// windows until the next layout pass pushes it back down.
pub(super) trait KeyboardSinkApi {
    fn focus(&self);
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
    pub(super) monitors: HashMap<MonitorId, MonitorState>,
    config: Config,
    taskbar: Rc<dyn ManageTaskbar>,
    overlay_factory: Box<dyn CreateOverlay>,
    display: Box<dyn QueryDisplay>,
    tiling_overlays: HashMap<MonitorId, Box<dyn TilingOverlayApi>>,
    float_overlays: HashMap<WindowId, Box<dyn FloatOverlayApi>>,
    keyboard_sink: Box<dyn KeyboardSinkApi>,
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
        keyboard_sink: Box<dyn KeyboardSinkApi>,
    ) -> anyhow::Result<Self> {
        let screens = display.get_all_screens()?;
        anyhow::ensure!(!screens.is_empty(), "No monitors detected");
        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, primary.scale, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_handles = HashMap::new();
        let mut monitors = HashMap::new();
        let mut tiling_overlays: HashMap<MonitorId, Box<dyn TilingOverlayApi>> = HashMap::new();
        monitor_handles.insert(primary.handle, primary_monitor_id);
        monitors.insert(
            primary_monitor_id,
            MonitorState {
                dimension: primary.dimension,
                scale: primary.scale,
                displayed: None,
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

        for screen in &screens {
            if screen.handle != primary.handle {
                let id = hub.add_monitor(screen.name.clone(), screen.dimension, screen.scale);
                monitor_handles.insert(screen.handle, id);
                monitors.insert(
                    id,
                    MonitorState {
                        dimension: screen.dimension,
                        scale: screen.scale,
                        displayed: None,
                    },
                );
                if let Ok(overlay) = overlay_factory.create_tiling_overlay(
                    config.clone(),
                    screen.dimension,
                    screen.scale,
                ) {
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
            monitors,
            config,
            taskbar: taskbar.clone(),
            overlay_factory,
            display,
            tiling_overlays,
            float_overlays: HashMap::new(),
            keyboard_sink,
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
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn window_destroyed(&mut self, id_key: HwndId) {
        self.remove_window(id_key);
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn window_minimized(&mut self, id_key: HwndId) {
        let Some(id) = self.registry.get_id(id_key) else {
            return;
        };
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        match entry.state {
            // Dome-initiated minimize or exclusive fullscreen -- ignore.
            WindowState::Minimized | WindowState::FullscreenExclusive => {}
            _ => {
                self.hub.minimize_window(id);
                if let Some(entry) = self.registry.get_mut(id) {
                    entry.state = WindowState::UserMinimized;
                }
            }
        }
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn window_restored(&mut self, id_key: HwndId) {
        let Some(id) = self.registry.get_id(id_key) else {
            return;
        };
        if !self
            .registry
            .get(id)
            .is_some_and(|e| matches!(e.state, WindowState::UserMinimized))
        {
            return;
        }
        self.hub.unminimize_window(id);
        if let Some(entry) = self.registry.get_mut(id) {
            entry.state = WindowState::Positioned(PositionedState::Offscreen {
                retries: 0,
                actual: (0, 0, 0, 0),
            });
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
        app_name: Option<String>,
    ) -> Option<Actions> {
        if should_ignore(&process, title.as_deref(), &self.config.windows.ignore) {
            return None;
        }
        let actions = on_open_actions(&process, title.as_deref(), &self.config.windows.on_open);
        self.insert_window(ext, title, process, constraints, observation, app_name);
        actions
    }

    fn insert_window(
        &mut self,
        ext: Arc<dyn ManageExternalHwnd>,
        title: Option<String>,
        process: String,
        constraints: (f32, f32, f32, f32),
        observation: ObservedPosition,
        app_name: Option<String>,
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
        if let Some(title) = &title {
            self.hub.set_window_title(id, title.clone());
        }
        self.recovery.track(&ext);

        self.registry.insert(
            id_key,
            id,
            WindowEntry {
                ext,
                state,
                title,
                process,
                app_name,
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
            for ms in self.monitors.values_mut() {
                if let Some(dm) = &mut ms.displayed {
                    dm.window_ids.remove(&id);
                }
            }
            self.hub.delete_window(id);
        }
    }

    pub(super) fn set_constraints(&mut self, id: WindowId, constraints: (f32, f32, f32, f32)) {
        // TODO(DPI): `border` is logical here but `constraints` arrive in physical
        // pixels (see handle::get_size_constraints). Needs scaling by
        // `self.monitors[...].scale`, but fixing it is a behavior change tracked
        // separately from the window.rs border-scaling dedup.
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

    pub(super) fn query_workspaces_json(&self) -> String {
        serde_json::to_string(&self.hub.query_workspaces())
            .expect("WorkspaceInfo is infallibly serializable")
    }

    pub(super) fn execute_hub_action(&mut self, action: &HubAction) {
        match action {
            HubAction::Focus { target } => match target {
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
            },
            HubAction::Move { target } => match target {
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
            },
            HubAction::Toggle { target } => match target {
                ToggleTarget::Spawn => self.hub.handle_tiling_action(TilingAction::ToggleSpawnMode),
                ToggleTarget::Direction => {
                    self.hub.handle_tiling_action(TilingAction::ToggleDirection)
                }
                ToggleTarget::Layout => self
                    .hub
                    .handle_tiling_action(TilingAction::ToggleContainerLayout),
                ToggleTarget::Float => self.hub.toggle_float(),
                ToggleTarget::Fullscreen => self.hub.toggle_fullscreen(),
            },
            HubAction::Master { target } => {
                let action = match target {
                    MasterTarget::Grow => TilingAction::GrowMaster,
                    MasterTarget::Shrink => TilingAction::ShrinkMaster,
                    MasterTarget::More => TilingAction::MoreMaster,
                    MasterTarget::Fewer => TilingAction::FewerMaster,
                };
                self.hub.handle_tiling_action(action);
            }
        }
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

    /// Unminimize a window selected via the picker. Unlike `window_restored`
    /// (driven by a Win32 event after the user clicks the taskbar), the picker
    /// path must drive both the core state and the OS state: tell the hub the
    /// window is back, ask Windows to restore it, and transition the registry
    /// state to Positioned(Offscreen) so apply_layout can place it.
    pub(super) fn picker_unminimize_window(&mut self, id: WindowId) {
        self.hub.unminimize_window(id);
        let Some(entry) = self.registry.get_mut(id) else {
            return;
        };
        if let WindowState::UserMinimized = entry.state {
            entry.ext.show_cmd(ShowCmd::Restore);
            entry.state = WindowState::Positioned(PositionedState::Offscreen {
                retries: 0,
                actual: (0, 0, 0, 0),
            });
        }
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn apply_layout(&mut self) {
        let created = std::mem::take(&mut self.pending_created);

        let result = self.hub.get_visible_placements();
        let focused_window = result.focused_window;
        let focused_monitor = result.focused_monitor;
        let focused = focused_window;

        let mut per_monitor: Vec<MonitorPositionData> = Vec::new();
        let mut new_displayed: HashMap<MonitorId, DisplayedMonitor> = HashMap::new();

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

            new_displayed.insert(mp.monitor_id, DisplayedMonitor { window_ids });
        }

        // Global diff
        let old_window_ids: HashSet<WindowId> = self
            .monitors
            .values()
            .filter_map(|ms| ms.displayed.as_ref())
            .flat_map(|dm| &dm.window_ids)
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

        // Update displayed state on each monitor.
        // Clear all first, then set the ones that have placements this pass.
        for ms in self.monitors.values_mut() {
            ms.displayed = None;
        }
        for (mid, dm) in new_displayed {
            if let Some(ms) = self.monitors.get_mut(&mid) {
                ms.displayed = Some(dm);
            }
        }

        // Hide
        for &id in &to_hide {
            // Keep taskbar tab for user-minimized windows so the user can
            // click it to restore. Dome-hidden windows get their tab removed.
            if let Some(entry) = self.registry.get(id)
                && !matches!(entry.state, WindowState::UserMinimized)
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
                    && !matches!(entry.state, WindowState::FullscreenExclusive)
                {
                    entry.ext.set_foreground_window();
                }
            } else {
                self.keyboard_sink.focus();
            }
        }
        self.last_focused_monitor = Some(current_monitor);
    }

    #[tracing::instrument(skip_all)]
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
            }
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
                self.registry
                    .get(*id)
                    .is_none_or(|e| !matches!(e.state, WindowState::FullscreenExclusive))
            })
            .map(|(hwnd_id, _)| hwnd_id)
            .collect()
    }

    fn reconcile_monitors(&mut self, screens: Vec<ScreenInfo>) {
        let current_handles: HashSet<isize> = screens.iter().map(|s| s.handle).collect();

        for screen in &screens {
            if !self.monitor_handles.contains_key(&screen.handle) {
                let id = self
                    .hub
                    .add_monitor(screen.name.clone(), screen.dimension, screen.scale);
                self.monitor_handles.insert(screen.handle, id);
                self.monitors.insert(
                    id,
                    MonitorState {
                        dimension: screen.dimension,
                        scale: screen.scale,
                        displayed: None,
                    },
                );
                if let Ok(overlay) = self.overlay_factory.create_tiling_overlay(
                    self.config.clone(),
                    screen.dimension,
                    screen.scale,
                ) {
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
                self.monitors.remove(&monitor_id);
                self.tiling_overlays.remove(&monitor_id);
                tracing::info!(%monitor_id, fallback = %fallback_id, "Monitor removed");
            }
        }

        for screen in &screens {
            if let Some(&id) = self.monitor_handles.get(&screen.handle)
                && let Some(ms) = self.monitors.get(&id)
                && (ms.dimension != screen.dimension || ms.scale != screen.scale)
            {
                let old_dim = Some(ms.dimension);
                let old_scale = Some(ms.scale);
                tracing::info!(
                    name = %screen.name,
                    ?old_dim,
                    new_dim = ?screen.dimension,
                    ?old_scale,
                    new_scale = ?screen.scale,
                    "Monitor dimension changed"
                );
                let ms = self.monitors.get_mut(&id).expect("just checked");
                ms.dimension = screen.dimension;
                ms.scale = screen.scale;
                self.hub.update_monitor(id, screen.dimension, screen.scale);
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
        let scale = dpi as f32 / crate::platform::windows::dpi::BASE_DPI;
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

    /// Test-only: look up the MonitorId for a given HMONITOR handle.
    #[cfg(test)]
    pub(super) fn monitor_id_for_handle(&self, handle: isize) -> Option<MonitorId> {
        self.monitor_handles.get(&handle).copied()
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

// Fallback display string derived from the executable name. Prefer
// FileDescription from version info when available (see get_app_display_name).
fn display_from_process(process: &str) -> String {
    process.strip_suffix(".exe").unwrap_or(process).to_string()
}

#[cfg(test)]
impl Dome {
    /// Test-only: returns the outer frame dimension stored in core for a floating window.
    pub(super) fn float_frame(&self, hwnd_id: HwndId) -> Option<Dimension> {
        let window_id = self.registry.get_id(hwnd_id)?;
        let ws_id = self.hub.current_workspace();
        let ws = self.hub.get_workspace(ws_id);
        ws.float_windows()
            .iter()
            .find(|&&(id, _)| id == window_id)
            .map(|&(_, dim)| dim)
    }

    /// Returns the shell-side FloatPlacement.monitor for a floating window.
    #[cfg(test)]
    pub(super) fn float_monitor(&self, hwnd_id: HwndId) -> Option<MonitorId> {
        let wid = self.registry.get_id(hwnd_id)?;
        let entry = self.registry.get(wid)?;
        match &entry.state {
            WindowState::Positioned(PositionedState::Float(fp)) => Some(fp.monitor),
            _ => None,
        }
    }
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
