mod inspect;
mod mirror;
mod monitor;
mod placement_tracker;
mod recovery;
mod registry;
mod window;

pub(super) use monitor::get_all_screens;

use std::collections::HashSet;
use std::fmt;

use calloop::channel::{Channel, Event as ChannelEvent, Sender as CalloopSender};
use calloop::{EventLoop, LoopSignal};

use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::{Retained, autoreleasepool};
use objc2_app_kit::NSWorkspace;
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_io_surface::IOSurface;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Config, MacosOnOpenRule};
use crate::core::{ContainerId, ContainerPlacement, Dimension, Hub, WindowId, WindowPlacement};

use super::running_application::RunningApp;
use super::ui::MessageSender;
use inspect::{
    ExistingWindow, NewAxWindow, VisibleWindowsReconciled, dispatch_reconcile_all_windows,
    dispatch_refresh_app_windows,
};
use mirror::{WindowCapture, create_captures_async};
use monitor::{MonitorInfo, MonitorRegistry};
use placement_tracker::PlacementTracker;
use registry::Registry;
use window::{MacWindow, RoundedDimension};

pub(super) enum HubEvent {
    /// Visible windows changed for an app (window created/destroyed/minimized/shown/hidden).
    VisibleWindowsChanged {
        pid: i32,
    },
    /// Sync focus for an app. Separated from VisibleWindowsChanged because offscreen windows (on
    /// other workspaces) still report as "active", which would hijack focus and prevent switching
    /// to empty workspaces.
    SyncFocus {
        pid: i32,
    },
    AppTerminated {
        pid: i32,
    },
    TitleChanged(CGWindowID),
    /// One or more windows of app with pid got resized or moved.
    /// This can't be on a per CGWindowID basis, as these events are unreliable and are often fired
    /// on the wrong window. For example, Slack doesn't emit this event on the main application
    /// window. This can however create a scenario when one window in the app finishes
    /// moving/resizing and send this notification, but other windows are not finish yet.
    WindowMovedOrResized {
        pid: i32,
    },
    Action(Actions),
    ConfigChanged(Config),
    /// Periodic sync to catch missed AX notifications, as AX notifications are unreliable. Only
    /// syncs window state, not focus, as focus changes should come from user interactions. Beside
    /// we receive plenty of focus events, so missing them isn't a concern.
    Sync,
    ScreensChanged(Vec<MonitorInfo>),
    MirrorClicked(WindowId),
    TabClicked(ContainerId, usize),
    /// macOS Space changed. Used to detect native fullscreen enter/exit since
    /// native fullscreen moves windows to a separate Space.
    SpaceChanged,
    Shutdown,
}

impl fmt::Display for HubEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VisibleWindowsChanged { pid } => write!(f, "VisibleWindowsChanged(pid={pid})"),
            Self::SyncFocus { pid } => write!(f, "SyncFocus(pid={pid})"),
            Self::AppTerminated { pid } => write!(f, "AppTerminated(pid={pid})"),
            Self::TitleChanged(cg_id) => write!(f, "TitleChanged(cg_id={cg_id})"),
            Self::WindowMovedOrResized { pid } => {
                write!(f, "WindowMovedOrResized(pid={pid})")
            }
            Self::Action(actions) => write!(f, "Action({actions})"),
            Self::ConfigChanged(_) => write!(f, "ConfigChanged"),
            Self::Sync => write!(f, "Sync"),
            Self::ScreensChanged(monitors) => {
                write!(f, "ScreensChanged(count={})", monitors.len())
            }
            Self::MirrorClicked(window_id) => write!(f, "MirrorClicked({window_id})"),
            Self::TabClicked(container_id, tab_idx) => {
                write!(f, "TabClicked({container_id}, tab_idx={tab_idx})")
            }
            Self::SpaceChanged => write!(f, "SpaceChanged"),
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

enum AsyncResult {
    AppVisibleWindows(VisibleWindowsReconciled),
    AllVisibleWindows {
        terminated_pids: Vec<i32>,
        new_apps: Vec<RunningApp>,
        apps: Vec<VisibleWindowsReconciled>,
    },
    CaptureReady {
        window_id: WindowId,
        capture: WindowCapture,
    },
}

pub(super) enum HubMessage {
    Frame(RenderFrame),
    RegisterObservers(Vec<RunningApp>),
    CaptureFrame {
        window_id: WindowId,
        surface: Retained<IOSurface>,
    },
    CaptureFailed {
        window_id: WindowId,
    },
    ConfigChanged(Config),
    Shutdown,
}

pub(super) struct RenderFrame {
    pub(super) creates: Vec<OverlayCreate>,
    pub(super) deletes: Vec<WindowId>,
    pub(super) shows: Vec<OverlayShow>,
    pub(super) container_creates: Vec<ContainerOverlayData>,
    pub(super) containers: Vec<ContainerOverlayData>,
    pub(super) deleted_containers: Vec<ContainerId>,
}

pub(super) struct OverlayCreate {
    pub(super) window_id: WindowId,
    pub(super) frame: NSRect,
}

pub(super) struct OverlayShow {
    pub(super) window_id: WindowId,
    pub(super) placement: WindowPlacement,
    pub(super) cocoa_frame: NSRect,
    pub(super) scale: f64,
    pub(super) visible_content: Option<Dimension>,
}

pub(super) struct Dome {
    hub: Hub,
    registry: Registry,
    monitor_registry: MonitorRegistry,
    config: Config,
    /// Work area of the primary monitor, used for crash recovery positioning.
    primary_screen: Dimension,
    /// Full height of the primary display (including menu bar/dock), used for Quartz→Cocoa
    /// coordinate conversion in overlay rendering.
    primary_full_height: f32,
    observed_pids: HashSet<i32>,
    sender: MessageSender,
    async_tx: CalloopSender<AsyncResult>,
    capture_queue: DispatchRetained<DispatchQueue>,
    signal: LoopSignal,
    placement_tracker: PlacementTracker,
}

impl Dome {
    pub(super) fn start(
        config: Config,
        screens: Vec<MonitorInfo>,
        sender: MessageSender,
        channel: Channel<HubEvent>,
    ) {
        recovery::install_handlers();
        let mut event_loop =
            EventLoop::<'static, Self>::try_new().expect("Failed to create event loop");
        let handle = event_loop.handle();
        let signal = event_loop.get_signal();

        let (async_tx, async_rx) = calloop::channel::channel();

        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_registry = MonitorRegistry::new(primary, primary_monitor_id);
        tracing::info!(%primary, "Primary monitor");

        for screen in &screens {
            if screen.display_id != primary.display_id {
                let id = hub.add_monitor(screen.name.clone(), screen.dimension);
                monitor_registry.insert(screen, id);
                tracing::info!(%screen, "Monitor");
            }
        }

        // Drain initial allocations from Hub::new() and add_monitor()
        hub.drain_changes();

        let mut dome = Self {
            hub,
            registry: Registry::new(monitor_registry.all_screens()),
            monitor_registry,
            config,
            primary_screen: primary.dimension,
            primary_full_height: primary.full_height,
            observed_pids: HashSet::new(),
            sender,
            async_tx,
            capture_queue: DispatchQueue::main().into(),
            signal,
            placement_tracker: PlacementTracker::new(handle.clone()),
        };

        handle
            .insert_source(channel, |event, _, dome| match event {
                ChannelEvent::Msg(hub_event) => dome.handle_event(hub_event),
                ChannelEvent::Closed => dome.signal.stop(),
            })
            .expect("Failed to insert channel source");

        handle
            .insert_source(async_rx, |event, _, dome| match event {
                ChannelEvent::Msg(result) => dome.handle_async_result(result),
                ChannelEvent::Closed => {}
            })
            .expect("Failed to insert async channel source");

        dispatch_reconcile_all_windows(
            dome.observed_pids.clone(),
            dome.registry
                .iter()
                .map(|(id, w)| (id, (w.ax().clone(), w.fullscreen())))
                .collect(),
            dome.config.macos.ignore.clone(),
            dome.async_tx.clone(),
        );
        event_loop
            .run(None, &mut dome, |_| {})
            .expect("Event loop failed");
    }

    #[tracing::instrument(skip(self), fields(%event))]
    fn handle_event(&mut self, event: HubEvent) {
        autoreleasepool(|_| {
            match event {
                HubEvent::Shutdown => {
                    tracing::info!("Shutdown requested");
                    self.signal.stop();
                    return;
                }
                HubEvent::ConfigChanged(new_config) => {
                    self.hub.sync_config(new_config.clone().into());
                    self.sender
                        .send(HubMessage::ConfigChanged(new_config.clone()));
                    self.config = new_config;
                    tracing::info!("Config reloaded");
                }
                HubEvent::VisibleWindowsChanged { pid } => {
                    dispatch_refresh_app_windows(
                        pid,
                        self.registry
                            .for_pid(pid)
                            .map(|(id, w)| (id, (w.ax().clone(), w.fullscreen())))
                            .collect(),
                        self.config.macos.ignore.clone(),
                        self.async_tx.clone(),
                    );
                }
                HubEvent::SyncFocus { pid } => {
                    if let Some(app) = RunningApp::new(pid) {
                        self.sync_app_focus(&app);
                    }
                }
                HubEvent::AppTerminated { pid } => {
                    tracing::debug!(pid, "App terminated");
                    self.remove_app_windows(pid);
                }
                HubEvent::TitleChanged(cg_id) => {
                    if let Some(window) = self.registry.get_mut(cg_id) {
                        window.update_title();
                        tracing::trace!(%window, "Title changed");
                    }
                }
                HubEvent::WindowMovedOrResized { pid } => {
                    self.placement_tracker.window_moved(pid);
                    return;
                }
                HubEvent::Action(actions) => {
                    tracing::debug!(%actions, "Executing actions");
                    self.execute_actions(&actions);
                }
                HubEvent::Sync => {
                    dispatch_reconcile_all_windows(
                        self.observed_pids.clone(),
                        self.registry
                            .iter()
                            .map(|(id, w)| (id, (w.ax().clone(), w.fullscreen())))
                            .collect(),
                        self.config.macos.ignore.clone(),
                        self.async_tx.clone(),
                    );
                }
                HubEvent::ScreensChanged(screens) => {
                    tracing::info!(count = screens.len(), "Screens changed");
                    self.update_screens(screens);
                }
                HubEvent::MirrorClicked(window_id) => {
                    if let Some(window) = self.registry.by_id(window_id) {
                        if let Err(e) = window.focus() {
                            tracing::debug!("Failed to focus window: {e:#}");
                        }
                        self.hub.set_focus(window_id);
                    }
                }
                HubEvent::TabClicked(container_id, tab_idx) => {
                    self.hub.focus_tab_index(container_id, tab_idx);
                }
                HubEvent::SpaceChanged => {
                    self.handle_space_changed();
                }
            }

            self.flush_layout();
        });
    }

    fn handle_async_result(&mut self, result: AsyncResult) {
        autoreleasepool(|_| {
            match result {
                AsyncResult::AppVisibleWindows(r) => {
                    self.apply_visible_windows_change(r);
                }
                AsyncResult::AllVisibleWindows {
                    terminated_pids,
                    new_apps,
                    apps,
                } => {
                    for pid in terminated_pids {
                        self.observed_pids.remove(&pid);
                        self.remove_app_windows(pid);
                    }
                    for r in apps {
                        self.apply_visible_windows_change(r);
                    }
                    if !new_apps.is_empty() {
                        for app in &new_apps {
                            self.observed_pids.insert(app.pid());
                        }
                        self.sender.send(HubMessage::RegisterObservers(new_apps));
                    }
                }
                AsyncResult::CaptureReady { window_id, capture } => {
                    if let Some(w) = self.registry.by_id_mut(window_id) {
                        w.set_capture(capture);
                    }
                }
            }
            self.flush_layout();
        });
    }

    fn flush_layout(&mut self) {
        let (shows, containers) = self.apply_layout();
        let changes = self.hub.drain_changes();

        let creates = changes
            .created_windows
            .iter()
            .filter_map(|&wid| {
                if changes.deleted_windows.contains(&wid) {
                    return None;
                }
                let dim = self.hub.get_window(wid).dimension();
                Some(OverlayCreate {
                    window_id: wid,
                    frame: to_ns_rect(self.primary_full_height, dim),
                })
            })
            .collect();

        let created_containers: HashSet<_> = changes.created_containers.into_iter().collect();
        let (container_creates, containers) = containers
            .into_iter()
            .partition(|c| created_containers.contains(&c.placement.id));

        self.sender.send(HubMessage::Frame(RenderFrame {
            creates,
            deletes: changes.deleted_windows,
            shows,
            container_creates,
            containers,
            deleted_containers: changes.deleted_containers,
        }));
    }

    #[tracing::instrument(skip_all, fields(pid = app.pid()))]
    fn sync_app_focus(&mut self, app: &RunningApp) {
        if !app.is_active() {
            return;
        }
        if let Some(ax) = app.focused_window()
            && let Some(window_id) = self.registry.get(ax.cg_id()).map(|w| w.window_id())
        {
            self.hub.set_focus(window_id);
        }
    }

    fn handle_space_changed(&mut self) {
        let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
            return;
        };
        let app = RunningApp::from(app);
        let Some(ax) = app.focused_window() else {
            return;
        };
        let cg_id = ax.cg_id();
        // Should be synchronous here, as we should pause everything until we know whether we are
        // dealing with native fullscreen or not.
        let is_native_fs = ax.is_native_fullscreen();

        if let Some(mac_window) = self.registry.get_mut(cg_id) {
            let window_id = mac_window.window_id();
            let was_fs = self.hub.get_window(window_id).is_fullscreen();
            if is_native_fs && !was_fs {
                tracing::info!(%mac_window, "Entered native fullscreen");
                mac_window.set_native_fullscreen();
                self.hub.set_fullscreen(window_id);
            } else if !is_native_fs && was_fs {
                tracing::info!(%mac_window, "Exited native fullscreen");
                let pos = ax.get_position().unwrap_or((0, 0));
                let size = ax.get_size().unwrap_or((0, 0));
                mac_window.unset_fullscreen(RoundedDimension {
                    x: pos.0,
                    y: pos.1,
                    width: size.0,
                    height: size.1,
                });
                self.hub.unset_fullscreen(window_id);
            }
        } else if is_native_fs {
            tracing::info!(%ax, "New native fullscreen window");
            let window_id = self.hub.insert_fullscreen();
            self.registry.insert_native_fullscreen(ax, window_id);
        }
    }

    fn remove_app_windows(&mut self, pid: i32) {
        self.placement_tracker.cancel(pid);
        for (cg_id, window_id) in self.registry.remove_by_pid(pid) {
            recovery::untrack(cg_id);
            self.hub.delete_window(window_id);
        }
    }

    fn remove_window(&mut self, cg_id: CGWindowID) {
        if let Some(window_id) = self.registry.remove(cg_id) {
            recovery::untrack(cg_id);
            self.hub.delete_window(window_id);
        }
    }

    fn dispatch_refresh_windows(&self, pid: i32) {
        dispatch_refresh_app_windows(
            pid,
            self.registry
                .for_pid(pid)
                .map(|(id, w)| (id, (w.ax().clone(), w.fullscreen())))
                .collect(),
            self.config.macos.ignore.clone(),
            self.async_tx.clone(),
        );
    }

    fn update_screens(&mut self, screens: Vec<MonitorInfo>) {
        if screens.is_empty() {
            tracing::warn!("Empty screen list, skipping reconciliation");
            return;
        }

        if let Some(primary) = screens.iter().find(|s| s.is_primary) {
            self.primary_screen = primary.dimension;
            self.primary_full_height = primary.full_height;
        }
        self.registry.set_monitors(screens.clone());

        reconcile_monitors(&mut self.hub, &mut self.monitor_registry, &screens);
    }

    #[tracing::instrument(skip(self))]
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
                    if let Err(e) = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(command)
                        .spawn()
                    {
                        tracing::warn!(%command, "Failed to exec: {e}");
                    }
                }
                Action::Exit => {
                    tracing::debug!("Exiting hub thread");
                    self.signal.stop();
                }
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn apply_layout(&mut self) -> (Vec<OverlayShow>, Vec<ContainerOverlayData>) {
        let mut shows = Vec::new();
        let mut containers = Vec::new();
        let focused_monitor = self.hub.focused_monitor();
        for mp in self.hub.get_visible_placements() {
            let entry = self.monitor_registry.get_entry_mut(mp.monitor_id).unwrap();
            let (s, c) = entry.apply_placements(
                &mp,
                mp.monitor_id == focused_monitor,
                &self.hub,
                &mut self.registry,
                &self.config,
                self.primary_full_height,
                &self.placement_tracker,
            );
            shows.extend(s);
            containers.extend(c);
        }
        (shows, containers)
    }

    fn apply_visible_windows_change(&mut self, result: VisibleWindowsReconciled) {
        if result.is_hidden {
            let cg_ids: Vec<_> = self
                .registry
                .for_pid(result.pid)
                .map(|(id, _)| id)
                .collect();
            for cg_id in cg_ids {
                self.remove_window(cg_id);
            }
            return;
        }

        for cg_id in result.to_remove {
            self.remove_window(cg_id);
        }

        let border = self.config.border_size;
        for existing in result.existing {
            self.process_existing_window(existing, border);
        }

        let mut new_cg_ids = Vec::new();
        for ax in result.to_add {
            if let Some(ids) = self.add_window(ax) {
                new_cg_ids.push(ids);
            }
        }

        if !new_cg_ids.is_empty() {
            create_captures_async(
                new_cg_ids,
                self.async_tx.clone(),
                self.sender.clone(),
                self.capture_queue.clone(),
            );
        }
    }

    fn process_existing_window(&mut self, existing: ExistingWindow, border: f32) {
        let Some(mac_window) = self.registry.get_mut(existing.cg_id) else {
            return;
        };
        let window_id = mac_window.window_id();
        let dim = &existing.dimension;

        let monitor = self
            .monitor_registry
            .find_monitor_at(dim.x as f32, dim.y as f32);

        if existing.is_native_fullscreen {
            mac_window.set_native_fullscreen();
            self.hub.set_fullscreen(window_id);
            return;
        }

        let is_borderless = monitor.is_some_and(|m| {
            let mon = &m.dimension;
            let tolerance = 2;
            (dim.x - mon.x as i32).abs() <= tolerance
                && (dim.y - mon.y as i32).abs() <= tolerance
                && (dim.width - mon.width as i32).abs() <= tolerance
                && (dim.height - mon.height as i32).abs() <= tolerance
        });
        if is_borderless {
            mac_window.set_borderless_fullscreen(monitor.unwrap().dimension);
            self.hub.set_fullscreen(window_id);
            return;
        }
        mac_window.unset_fullscreen(existing.dimension);
        self.hub.unset_fullscreen(window_id);

        let hub_window = self.hub.get_window(window_id);
        if hub_window.is_fullscreen() {
            return;
        }
        let Some((min_w, min_h, max_w, max_h)) =
            mac_window.check_placement(hub_window, existing.dimension)
        else {
            return;
        };
        // Convert actual window size back to frame size by adding border back.
        // Frame dimensions have border inset applied. If in the original frame,
        // window width is smaller than sum of borders, then we will request a size
        // that can accommodate the borders here.
        let remove_inset = |v: f32| v + 2.0 * border;
        self.hub.set_window_constraint(
            window_id,
            min_w.map(remove_inset),
            min_h.map(remove_inset),
            max_w.map(remove_inset),
            max_h.map(remove_inset),
        );
    }

    fn add_window(&mut self, new: NewAxWindow) -> Option<(CGWindowID, WindowId)> {
        let ax = new.ax;
        let dim = new.dimension;
        if self.registry.contains(ax.cg_id()) {
            return None;
        }

        let window_id = if ax.should_tile() {
            self.hub.insert_tiling()
        } else {
            self.hub.insert_float(Dimension {
                x: dim.x as f32,
                y: dim.y as f32,
                width: dim.width as f32,
                height: dim.height as f32,
            })
        };

        recovery::track(ax.clone(), self.primary_screen);
        let cg_id = ax.cg_id();
        self.registry.insert(ax, window_id, dim);

        // Hide immediately - window may spawn outside viewport due to scrolling
        if let Some(window) = self.registry.get_mut(cg_id) {
            window.hide().ok();
        }

        let window = self.registry.by_id(window_id).unwrap();
        tracing::info!(%window, %window_id, "Window inserted");

        if let Some(actions) = on_open_actions(window, &self.config.macos.on_open) {
            self.execute_actions(&actions);
        }

        Some((cg_id, window_id))
    }
}

impl Drop for Dome {
    fn drop(&mut self) {
        recovery::restore_all();
        self.sender.send(HubMessage::Shutdown);
    }
}

fn on_open_actions(window: &MacWindow, rules: &[MacosOnOpenRule]) -> Option<Actions> {
    let rule = rules.iter().find(|r| {
        r.window
            .matches(window.app_name(), window.bundle_id(), window.title())
    })?;
    tracing::debug!(%window, actions = %rule.run, "Running on_open actions");
    Some(rule.run.clone())
}

fn reconcile_monitors(hub: &mut Hub, registry: &mut MonitorRegistry, screens: &[MonitorInfo]) {
    let current_keys: HashSet<_> = screens.iter().map(|s| s.display_id).collect();

    // Special handling for when the primary monitor got replaced, i.e. due to mirroring to prevent
    // disruption due to removal and addition of workspaces.
    if let Some(new_primary) = screens.iter().find(|s| s.is_primary) {
        if !registry.contains(new_primary.display_id) {
            registry.replace_primary(new_primary);
            hub.update_monitor_dimension(registry.primary_monitor_id(), new_primary.dimension);
        } else {
            registry.set_primary_display_id(new_primary.display_id);
        }
    }

    // Add new monitors first to prevent exhausting all monitors
    for screen in screens {
        if !registry.contains(screen.display_id) {
            let id = hub.add_monitor(screen.name.clone(), screen.dimension);
            registry.insert(screen, id);
            tracing::info!(%screen, "Monitor added");
        }
    }

    // Remove monitors that no longer exist
    for monitor_id in registry.remove_stale(&current_keys) {
        hub.remove_monitor(monitor_id, registry.primary_monitor_id());
        tracing::info!(%monitor_id, fallback = %registry.primary_monitor_id(), "Monitor removed");
    }

    // Update screen info (dimension, scale, etc.)
    for screen in screens {
        if let Some((monitor_id, old_dim)) = registry.update_screen(screen) {
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

pub(super) struct ContainerOverlayData {
    pub(super) placement: ContainerPlacement,
    pub(super) tab_titles: Vec<String>,
    pub(super) cocoa_frame: NSRect,
}

// Quartz uses top-left origin, Cocoa uses bottom-left origin
pub(super) fn to_ns_rect(primary_full_height: f32, dim: Dimension) -> NSRect {
    NSRect::new(
        NSPoint::new(
            dim.x as f64,
            (primary_full_height - dim.y - dim.height) as f64,
        ),
        NSSize::new(dim.width as f64, dim.height as f64),
    )
}
