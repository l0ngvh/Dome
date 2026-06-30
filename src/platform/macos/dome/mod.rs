mod events;
mod inspect;
mod layout;
mod monitor;
mod recovery;
mod registry;
pub(super) mod rejection_log_filter;
mod window;

pub(super) use events::{ContainerShow, HubEvent, HubMessage};
pub(super) use inspect::{
    ExitNativeFullscreen, ExtRefresh, compute_reconcile_all, compute_reconciliation,
    compute_window_positions,
};
pub(super) use monitor::{MonitorInfo, get_all_monitors};
use window::WindowState;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use objc2_core_graphics::CGWindowID;

use crate::action::{FocusTarget, MasterTarget, MoveTarget, TabDirection, ToggleTarget};
use crate::config::{Config, LayoutConfig, MacosWindow, pattern_matches};
use crate::core::{
    ContainerId, Direction, DisplayMode, Hub, Length, Logical, OnOpenRule, TilingAction, WindowId,
    WindowMetadata, WindowRestrictions,
};
use crate::picker::build_picker_entries;
use crate::platform::macos::accessibility::ExternalWindow;

use monitor::MonitorRegistry;
use recovery::Recovery;
use registry::{ManagedWindow, WindowRegistry};
use rejection_log_filter::RejectionLogFilter;

pub(in crate::platform::macos) use window::RoundedDimension;

pub(in crate::platform::macos) struct NewWindow {
    pub(in crate::platform::macos) ax: Arc<dyn ExternalWindow>,
    pub(in crate::platform::macos) metadata: MacOSMetadata,
}

impl std::fmt::Display for NewWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[pid={}|cg={}] ", self.ax.pid(), self.ax.cg_id())?;
        write!(f, "{}", self.metadata)
    }
}

#[derive(Debug, Clone)]
pub(in crate::platform::macos) struct MacOSMetadata {
    pub title: Option<String>,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

impl std::fmt::Display for MacOSMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.app_name.as_deref().unwrap_or("Unknown"))?;
        if let Some(bid) = &self.bundle_id {
            write!(f, " ({bid})")?;
        }
        if let Some(t) = &self.title {
            write!(f, " - {t}")?;
        }
        Ok(())
    }
}

impl WindowMetadata for MacOSMetadata {
    fn icon_key(&self) -> Option<String> {
        self.bundle_id.clone()
    }
    fn app_name(&self) -> Option<String> {
        self.app_name.clone()
    }
    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }
    fn clone_box(&self) -> Box<dyn WindowMetadata> {
        Box::new(self.clone())
    }

    fn matches_on_open_rule(&self, rule: &OnOpenRule) -> bool {
        let app = self.app_name.as_deref();
        let bundle_id = self.bundle_id.as_deref();
        let title = self.title.as_deref();

        if let Some(p) = rule.app.as_deref()
            && !app.is_some_and(|a| pattern_matches(p, a))
        {
            return false;
        }
        if let Some(b) = rule.bundle_id.as_deref()
            && bundle_id != Some(b)
        {
            return false;
        }
        if let Some(p) = rule.title.as_deref()
            && !title.is_some_and(|t| pattern_matches(p, t))
        {
            return false;
        }
        if app.is_none() && bundle_id.is_none() && title.is_none() {
            return false;
        }
        rule.app.is_some() || rule.bundle_id.is_some() || rule.title.is_some()
    }
}

pub(in crate::platform::macos) enum PendingAdd {
    Positioned {
        new: NewWindow,
        dim: RoundedDimension,
    },
    /// Native fullscreen windows lives on their own space and thus has no dimension
    NativeFullscreen { new: NewWindow },
}

/// Timestamps of the first and last AX move/resize notifications in a
/// coalesced debounce burst (equal when only one fired). The first is
/// compared against the post-placement debounce window (was this burst
/// caused by our placement?), and the last against the latest placement
/// time (is this burst stale?).
#[derive(Debug, Copy, Clone)]
pub(in crate::platform::macos) struct DebounceBurst {
    pub(in crate::platform::macos) first: Instant,
    pub(in crate::platform::macos) last: Instant,
}

pub(in crate::platform::macos) struct WindowMove {
    pub(in crate::platform::macos) cg_id: CGWindowID,
    pub(in crate::platform::macos) x: i32,
    pub(in crate::platform::macos) y: i32,
    pub(in crate::platform::macos) w: i32,
    pub(in crate::platform::macos) h: i32,
    pub(in crate::platform::macos) observed_at: DebounceBurst,
}

pub(in crate::platform::macos) trait FrameSender: Send {
    fn send(&self, msg: HubMessage);
}

/// Platform-specific state machine that bridges macOS accessibility events with the
/// core tree model. Event-loop–facing methods accept `CGWindowID` rather than `WindowId`
/// because callers dispatch work to background threads that capture registry snapshots —
/// by the time results arrive the window may have been removed, so resolution to
/// `WindowId` happens here where the registry can be checked.
pub(in crate::platform::macos) struct Dome {
    hub: Hub,
    registry: WindowRegistry,
    monitor_registry: MonitorRegistry,
    config: Config,
    layout: LayoutConfig,
    /// Full height of the primary display (including menu bar/dock), used for Quartz→Cocoa
    /// coordinate conversion in overlay rendering.
    primary_full_height: f32,
    observed_pids: HashSet<i32>,
    sender: Box<dyn FrameSender>,
    last_focused: Option<WindowId>,
    recovery: Recovery,
    pending_created: Vec<WindowId>,
    pending_deleted: Vec<WindowId>,
    log_filter: Arc<RejectionLogFilter>,
}

impl Dome {
    pub(in crate::platform::macos) fn new(
        monitors: &[MonitorInfo],
        config: Config,
        layout: LayoutConfig,
        sender: Box<dyn FrameSender>,
    ) -> Self {
        let primary = monitors
            .iter()
            .find(|s| s.is_primary)
            .unwrap_or(&monitors[0]);
        let mut hub = Hub::new(primary.work_area, 1.0, layout.clone());
        hub.set_on_open_rules(convert_macos_on_open_rules(&config.macos.on_open));
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_registry = MonitorRegistry::new(primary, primary_monitor_id);
        for monitor in monitors {
            if monitor.display_id != primary.display_id {
                let id = hub.add_monitor(monitor.name.clone(), monitor.work_area, 1.0);
                monitor_registry.insert(monitor, id);
            }
        }
        Self {
            hub,
            registry: WindowRegistry::new(),
            monitor_registry,
            config,
            layout,
            primary_full_height: primary.full_height,
            observed_pids: HashSet::new(),
            sender,
            last_focused: None,
            recovery: Recovery::new(),
            pending_created: Vec::new(),
            pending_deleted: Vec::new(),
            log_filter: Arc::new(RejectionLogFilter::new()),
        }
    }

    pub(in crate::platform::macos) fn refresh_ext_cache(&mut self, refresh: &[ExtRefresh]) {
        for r in refresh {
            if self.registry.replace_ext(r.cg_id, r.ext.clone()) {
                tracing::trace!(cg_id = %r.cg_id, pid = r.ext.pid(), "Replaced stale ext handle");
            }
        }
    }

    #[tracing::instrument(skip_all)]
    pub(in crate::platform::macos) fn reconcile_windows(
        &mut self,
        refresh: &[ExtRefresh],
        removed: &[CGWindowID],
        minimized: &[CGWindowID],
        added: Vec<PendingAdd>,
        to_enter_native_fullscreen: &[CGWindowID],
        to_exit_native_fullscreen: &[ExitNativeFullscreen],
    ) {
        self.refresh_ext_cache(refresh);
        for &cg_id in removed {
            if let Some(entry) = self.registry.get(cg_id) {
                self.remove_window(entry.window_id);
            }
        }
        for &cg_id in minimized {
            if let Some(entry) = self.registry.get(cg_id) {
                self.minimize_window(entry.window_id);
            }
        }
        for pending in added {
            let new_ref = match &pending {
                PendingAdd::Positioned { new, .. } | PendingAdd::NativeFullscreen { new } => new,
            };
            if self.registry.contains(new_ref.ax.cg_id()) {
                continue;
            }

            let workspace_name = self
                .hub
                .resolve_on_open(&new_ref.metadata)
                .and_then(|r| r.workspace.clone());

            match pending {
                PendingAdd::NativeFullscreen { new } => {
                    let target_ws = self.hub.resolve_workspace(workspace_name.as_deref());
                    self.add_native_fullscreen_window(new, target_ws);
                }
                PendingAdd::Positioned { new, dim } => {
                    let ax_for_recovery = new.ax.clone();
                    let borderless_fs = self.is_borderless_fullscreen_at(dim);
                    let restrictions = if borderless_fs {
                        WindowRestrictions::ProtectFullscreen
                    } else {
                        WindowRestrictions::None
                    };
                    let (id, display_mode) = self.hub.insert_window(
                        Box::new(new.metadata.clone()),
                        dim.to_dimension(),
                        borderless_fs,
                        restrictions,
                    );
                    tracing::info!(%id, ?display_mode, %new, "New window");
                    let state = match display_mode {
                        DisplayMode::Tiling => {
                            WindowState::Positioned(window::PositionedState::Offscreen(
                                window::OffscreenPlacement::new(dim),
                            ))
                        }
                        DisplayMode::Float { .. } => WindowState::Positioned(
                            window::PositionedState::Float(window::FloatPlacement::new(dim)),
                        ),
                        DisplayMode::Fullscreen => WindowState::BorderlessFullscreen,
                    };
                    self.finalize_added_window(new, id, state);
                    self.recovery.track(
                        ax_for_recovery,
                        dim.width,
                        dim.height,
                        self.monitor_registry.primary_monitor().work_area(),
                    );
                }
            }
        }
        for &cg_id in to_enter_native_fullscreen {
            if let Some(entry) = self.registry.get(cg_id) {
                let window_id = entry.window_id;
                self.window_entered_native_fullscreen(window_id);
            }
        }
        for e in to_exit_native_fullscreen {
            if let Some(entry) = self.registry.get(e.cg_id)
                && matches!(entry.state, WindowState::NativeFullscreen)
            {
                let window_id = entry.window_id;
                let now = Instant::now();
                // NativeFullscreen doesn't emit any move/resize event, so we need to simulate one
                self.window_moved(
                    window_id,
                    e.x,
                    e.y,
                    e.w,
                    e.h,
                    DebounceBurst {
                        first: now,
                        last: now,
                    },
                );
            }
        }
        self.flush_layout();
    }

    #[tracing::instrument(skip_all)]
    pub(in crate::platform::macos) fn windows_moved(&mut self, moves: Vec<WindowMove>) {
        for m in moves {
            let Some(entry) = self.registry.get(m.cg_id) else {
                continue;
            };
            let window_id = entry.window_id;
            self.window_moved(window_id, m.x, m.y, m.w, m.h, m.observed_at);
        }
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn app_terminated(&mut self, pid: i32) {
        self.remove_app_windows(pid);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn config_changed(&mut self, new_config: Config) {
        self.hub.sync_config(self.layout.clone());
        self.sender
            .send(HubMessage::ConfigChanged(Box::new(new_config.clone())));
        self.config = new_config;
        self.hub
            .set_on_open_rules(convert_macos_on_open_rules(&self.config.macos.on_open));
        tracing::info!("Config reloaded");
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn layout_changed(&mut self, new_layout: LayoutConfig) {
        self.layout = new_layout;
        self.hub.sync_config(self.layout.clone());
        tracing::info!("Layout reloaded");
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn tracked_window(
        &self,
        cg_id: CGWindowID,
    ) -> Option<ManagedWindow> {
        self.registry.get(cg_id).cloned()
    }

    #[tracing::instrument(skip(self), fields(cg_id = %cg_id))]
    pub(in crate::platform::macos) fn focus_window_by_cg(&mut self, cg_id: CGWindowID) {
        let Some(entry) = self.registry.get(cg_id) else {
            return;
        };
        let window_id = entry.window_id;
        let was_minimized = entry.is_minimized;
        if was_minimized {
            self.hub.unminimize_window(window_id);
        }
        self.hub.set_focus(window_id);
        self.flush_layout();
    }

    #[tracing::instrument(skip(self, title), fields(cg_id = %cg_id))]
    pub(in crate::platform::macos) fn update_title(
        &mut self,
        cg_id: CGWindowID,
        title: Option<String>,
    ) {
        if let Some(entry) = self.registry.get_mut(cg_id)
            && let Some(title) = title
        {
            if self.hub.set_window_title(entry.window_id, title) {
                tracing::trace!("Title changed");
            }
            self.flush_layout();
        }
    }

    pub(in crate::platform::macos) fn monitors_changed(&mut self, monitors: Vec<MonitorInfo>) {
        if monitors.is_empty() {
            tracing::warn!("Empty monitor list, skipping reconciliation");
            return;
        }
        self.rehide_offscreen_windows(&monitors);
        self.update_monitors(&monitors);
        self.flush_layout();
    }

    #[tracing::instrument(skip(self), fields(cg_id = %cg_id))]
    pub(in crate::platform::macos) fn mirror_clicked(&mut self, cg_id: CGWindowID) {
        let Some(entry) = self.registry.get(cg_id) else {
            return;
        };
        let window_id = entry.window_id;
        let was_minimized = entry.is_minimized;
        let ext = entry.ext.clone();
        if was_minimized {
            self.hub.unminimize_window(window_id);
        }
        if let Err(e) = ext.focus() {
            tracing::debug!("Failed to focus window: {e:#}");
        }
        self.hub.set_focus(window_id);
        self.flush_layout();
    }

    #[tracing::instrument(skip(self), fields(container_id = %container_id, tab_idx))]
    pub(in crate::platform::macos) fn tab_clicked(
        &mut self,
        container_id: ContainerId,
        tab_idx: usize,
    ) {
        self.hub.focus_tab_index(container_id, tab_idx);
        self.flush_layout();
    }

    /// Handles the frontmost window entering native fullscreen after a space
    /// change. If `cg_id` is tracked, transitions it to `NativeFullscreen`
    /// state. If untracked, inserts it as a new fullscreen window.
    #[tracing::instrument(skip(self, new), fields(cg_id = %cg_id))]
    pub(in crate::platform::macos) fn enter_native_fullscreen(
        &mut self,
        cg_id: CGWindowID,
        new: NewWindow,
    ) {
        if let Some(entry) = self.registry.get(cg_id) {
            let window_id = entry.window_id;
            self.window_entered_native_fullscreen(window_id);
        } else {
            let target_ws = self.hub.current_workspace();
            self.add_native_fullscreen_window(new, target_ws);
        }
        self.flush_layout();
    }

    /// Handles the frontmost window exiting native fullscreen after a space
    /// change. Only acts if `cg_id` is tracked and currently in
    /// `NativeFullscreen` state, routing through `window_moved` so the window
    /// re-enters tiling via the same path as reconcile-detected exits.
    #[tracing::instrument(skip(self, pos, size), fields(cg_id = %cg_id))]
    pub(in crate::platform::macos) fn exit_native_fullscreen(
        &mut self,
        cg_id: CGWindowID,
        pos: (Length<Logical>, Length<Logical>),
        size: (Length<Logical>, Length<Logical>),
    ) {
        if let Some(entry) = self.registry.get(cg_id)
            && matches!(entry.state, WindowState::NativeFullscreen)
        {
            let window_id = entry.window_id;
            let now = Instant::now();
            self.window_moved(
                window_id,
                pos.0.value() as i32,
                pos.1.value() as i32,
                size.0.value() as i32,
                size.1.value() as i32,
                DebounceBurst {
                    first: now,
                    last: now,
                },
            );
            self.flush_layout();
        }
    }

    pub(in crate::platform::macos) fn tracked_for_pid(
        &self,
        pid: i32,
    ) -> HashMap<CGWindowID, ManagedWindow> {
        self.registry
            .for_pid(pid)
            .map(|(id, e)| (id, e.clone()))
            .collect()
    }

    pub(in crate::platform::macos) fn all_tracked(&self) -> HashMap<CGWindowID, ManagedWindow> {
        self.registry
            .iter()
            .map(|(id, e)| (id, e.clone()))
            .collect()
    }

    pub(in crate::platform::macos) fn ignore_rules(&self) -> Vec<MacosWindow> {
        self.config.macos.ignore.clone()
    }

    pub(in crate::platform::macos) fn observed_pids(&self) -> HashSet<i32> {
        self.observed_pids.clone()
    }

    pub(in crate::platform::macos) fn log_filter(&self) -> Arc<RejectionLogFilter> {
        Arc::clone(&self.log_filter)
    }

    pub(in crate::platform::macos) fn set_pid_moving(&mut self, pid: i32, moving: bool) {
        self.registry.set_pid_moving(pid, moving);
    }

    pub(in crate::platform::macos) fn mark_pid_observed(&mut self, pid: i32) {
        self.observed_pids.insert(pid);
    }

    /// Replaces `observed_pids` wholesale with the given set. Called after
    /// `refresh_all_observers` completes on the main thread.
    pub(in crate::platform::macos) fn set_observed_pids(&mut self, pids: HashSet<i32>) {
        self.observed_pids = pids;
    }

    pub(in crate::platform::macos) fn remove_untracked_app(&mut self, pid: i32) {
        self.remove_app_windows(pid);
    }

    /// Sends a message to the main thread to tear down all observers and
    /// re-register from scratch.
    pub(in crate::platform::macos) fn refresh_observers(&self) {
        self.sender.send(HubMessage::RefreshObservers);
    }

    fn remove_window(&mut self, wid: WindowId) {
        self.hub.delete_window(wid);
        self.pending_deleted.push(wid);
    }

    fn remove_app_windows(&mut self, pid: i32) {
        let window_ids: Vec<WindowId> = self
            .registry
            .for_pid(pid)
            .map(|(_, entry)| entry.window_id)
            .collect();
        for wid in window_ids {
            self.remove_window(wid);
        }
    }

    pub(in crate::platform::macos) fn query_workspaces_json(&self) -> String {
        serde_json::to_string(&self.hub.query_workspaces())
            .expect("WorkspaceInfo is infallibly serializable")
    }

    /// Sends picker data to the UI thread, which toggles the picker window:
    /// creates it if absent, closes it if already open.
    pub(in crate::platform::macos) fn toggle_picker(&mut self) {
        let minimized = self.hub.minimized_window_entries();
        let entries = build_picker_entries(&minimized);
        let focused_monitor = self.hub.focused_monitor();
        let m = self.monitor_registry.monitor(focused_monitor);
        let monitor_dim = m.work_area();
        let scale = m.egui_scale();
        let cocoa_frame = crate::platform::macos::objc2_wrapper::dimension_to_ns_rect_cocoa(
            Length::new(self.primary_full_height),
            monitor_dim,
        );
        self.sender.send(HubMessage::PickerToggle {
            entries,
            monitor_dim,
            cocoa_frame,
            scale,
        });
    }

    /// Unminimize a window selected via the picker. Clears the minimize flag
    /// and drives the OS-side restore.
    #[tracing::instrument(skip(self), fields(window_id = %window_id))]
    pub(in crate::platform::macos) fn picker_unminimize_window(&mut self, window_id: WindowId) {
        self.hub.unminimize_window(window_id);
        let Some(window) = self.registry.by_id_mut(window_id) else {
            return;
        };
        if window.is_minimized {
            window.is_minimized = false;
            if let Err(e) = window.ext.unminimize() {
                tracing::debug!("Failed to unminimize window from picker: {e:#}");
            }
        }
        self.flush_layout();
    }

    #[tracing::instrument(skip(self), fields(target = ?target))]
    pub(in crate::platform::macos) fn apply_focus(&mut self, target: &FocusTarget) {
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

    #[tracing::instrument(skip(self), fields(target = ?target))]
    pub(in crate::platform::macos) fn apply_move(&mut self, target: &MoveTarget) {
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

    #[tracing::instrument(skip(self), fields(target = ?target))]
    pub(in crate::platform::macos) fn apply_toggle(&mut self, target: &ToggleTarget) {
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

    #[tracing::instrument(skip(self), fields(target = ?target))]
    pub(in crate::platform::macos) fn apply_master(&mut self, target: &MasterTarget) {
        let action = match target {
            MasterTarget::Grow => TilingAction::GrowMaster,
            MasterTarget::Shrink => TilingAction::ShrinkMaster,
            MasterTarget::More => TilingAction::MoreMaster,
            MasterTarget::Fewer => TilingAction::FewerMaster,
        };
        self.hub.handle_tiling_action(action);
    }
}

impl Drop for Dome {
    fn drop(&mut self) {
        self.recovery.restore_all();
        self.sender.send(HubMessage::Shutdown);
    }
}

pub(super) fn convert_macos_on_open_rules(
    rules: &[crate::config::MacosOnOpenRule],
) -> Vec<OnOpenRule> {
    rules
        .iter()
        .map(|r| OnOpenRule {
            mode: r.mode,
            workspace: r.workspace.clone(),
            app: r.app.clone(),
            bundle_id: r.bundle_id.clone(),
            title: r.title.clone(),
            process: None,
            class: None,
            aumid: None,
        })
        .collect()
}
