use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use calloop::LoopSignal;
use objc2_app_kit::NSWorkspace;
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Config, MacosOnOpenRule, MacosWindow};
use crate::core::{
    Child, Container, ContainerId, Dimension, Hub, MonitorLayout, MonitorPlacements, WindowId,
};
use crate::platform::macos::MonitorInfo;
use crate::platform::macos::accessibility::AXWindowApi;
use crate::platform::macos::running_application::RunningApp;

use super::events::{ContainerOverlayData, HubMessage, OverlayCreate, OverlayShow, RenderFrame};
use super::monitor::MonitorRegistry;
use super::recovery;
use super::registry::{Registry, WindowEntry};
use super::window::{
    Placement, PositionedState, RoundedDimension, WindowState, apply_inset, clip_to_bounds,
    hidden_monitor, move_offscreen, round_dim,
};

pub(in crate::platform::macos) struct NewWindow {
    pub(in crate::platform::macos) ax: Arc<dyn AXWindowApi>,
    pub(in crate::platform::macos) app_name: Option<String>,
    pub(in crate::platform::macos) bundle_id: Option<String>,
    pub(in crate::platform::macos) title: Option<String>,
    pub(in crate::platform::macos) x: i32,
    pub(in crate::platform::macos) y: i32,
    pub(in crate::platform::macos) w: i32,
    pub(in crate::platform::macos) h: i32,
    pub(in crate::platform::macos) is_native_fullscreen: bool,
}

pub(in crate::platform::macos) struct WindowMove {
    pub(in crate::platform::macos) window_id: WindowId,
    pub(in crate::platform::macos) x: i32,
    pub(in crate::platform::macos) y: i32,
    pub(in crate::platform::macos) w: i32,
    pub(in crate::platform::macos) h: i32,
    pub(in crate::platform::macos) observed_at: Instant,
    pub(in crate::platform::macos) is_native_fullscreen: bool,
}

pub(in crate::platform::macos) trait FrameSender: Send {
    fn send(&self, msg: HubMessage);
}

pub(in crate::platform::macos) struct Dome {
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
    sender: Box<dyn FrameSender>,
    signal: LoopSignal,
    last_focused: Option<WindowId>,
}

impl Dome {
    pub(in crate::platform::macos) fn new(
        screens: &[MonitorInfo],
        config: Config,
        sender: Box<dyn FrameSender>,
        signal: LoopSignal,
    ) -> Self {
        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_registry = MonitorRegistry::new(primary, primary_monitor_id);
        for screen in screens {
            if screen.display_id != primary.display_id {
                let id = hub.add_monitor(screen.name.clone(), screen.dimension);
                monitor_registry.insert(screen, id);
            }
        }
        Self {
            hub,
            registry: Registry::new(),
            monitor_registry,
            config,
            primary_screen: primary.dimension,
            primary_full_height: primary.full_height,
            observed_pids: HashSet::new(),
            sender,
            signal,
            last_focused: None,
        }
    }

    pub(in crate::platform::macos) fn reconcile_windows(
        &mut self,
        removed: &[CGWindowID],
        added: Vec<NewWindow>,
    ) -> Vec<WindowId> {
        for &cg_id in removed {
            self.remove_window(cg_id);
        }
        let mut ids = Vec::with_capacity(added.len());
        for new in added {
            let NewWindow {
                ax,
                app_name,
                bundle_id,
                title,
                x,
                y,
                w,
                h,
                is_native_fullscreen,
            } = new;
            if self.registry.contains(ax.cg_id()) {
                continue;
            }
            let window_id = if is_native_fullscreen {
                self.add_native_fullscreen_window(
                    ax.clone(),
                    app_name.clone(),
                    bundle_id.clone(),
                    title.clone(),
                )
            } else {
                self.add_window(
                    ax.clone(),
                    x,
                    y,
                    w,
                    h,
                    app_name.clone(),
                    bundle_id.clone(),
                    title.clone(),
                )
            };
            recovery::track(ax, self.primary_screen);
            let actions = {
                let entry = self.registry.by_id(window_id);
                on_open_actions(entry, &self.config.macos.on_open)
            };
            if let Some(actions) = actions {
                self.execute_actions(&actions);
            }
            ids.push(window_id);
        }
        self.flush_layout();
        ids
    }

    pub(in crate::platform::macos) fn windows_moved(&mut self, moves: Vec<WindowMove>) {
        for m in moves {
            if m.is_native_fullscreen {
                self.window_entered_native_fullscreen(m.window_id);
            } else {
                self.window_moved(m.window_id, m.x, m.y, m.w, m.h, m.observed_at);
            }
        }
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn app_terminated(&mut self, pid: i32) {
        self.remove_app_windows(pid);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn run_actions(&mut self, actions: &Actions) {
        self.execute_actions(actions);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn focused_window(&self) -> Option<WindowId> {
        match self
            .hub
            .get_workspace(self.hub.current_workspace())
            .focused()
        {
            Some(Child::Window(id)) => Some(id),
            _ => None,
        }
    }

    pub(in crate::platform::macos) fn window_id_for_cg(
        &self,
        cg_id: CGWindowID,
    ) -> Option<WindowId> {
        self.registry.get(cg_id).map(|e| e.window_id)
    }

    pub(super) fn stop(&mut self) {
        self.signal.stop();
    }

    pub(super) fn config_changed(&mut self, new_config: Config) {
        self.hub.sync_config(new_config.clone().into());
        self.sender
            .send(HubMessage::ConfigChanged(new_config.clone()));
        self.config = new_config;
        tracing::info!("Config reloaded");
        self.flush_layout();
    }

    pub(super) fn sync_focus(&mut self, pid: i32) {
        if let Some(app) = RunningApp::new(pid) {
            self.sync_app_focus(&app);
        }
        self.flush_layout();
    }

    pub(super) fn title_changed(&mut self, cg_id: CGWindowID) {
        if let Some(entry) = self.registry.get_mut(cg_id) {
            entry.title = entry.ax.read_title();
            tracing::trace!(%entry, "Title changed");
        }
        self.flush_layout();
    }

    pub(super) fn screens_changed(&mut self, screens: Vec<MonitorInfo>) {
        self.update_screens(screens);
        self.flush_layout();
    }

    pub(super) fn mirror_clicked(&mut self, window_id: WindowId) {
        let entry = self.registry.by_id(window_id);
        if let Err(e) = entry.ax.focus() {
            tracing::debug!("Failed to focus window: {e:#}");
        }
        self.hub.set_focus(window_id);
        self.flush_layout();
    }

    pub(super) fn tab_clicked(&mut self, container_id: ContainerId, tab_idx: usize) {
        self.hub.focus_tab_index(container_id, tab_idx);
        self.flush_layout();
    }

    pub(super) fn space_changed(&mut self) {
        self.handle_space_changed();
        self.flush_layout();
    }

    pub(super) fn tracked_for_pid(&self, pid: i32) -> HashMap<CGWindowID, WindowEntry> {
        self.registry
            .for_pid(pid)
            .map(|(id, e)| (id, e.clone()))
            .collect()
    }

    pub(super) fn all_tracked(&self) -> HashMap<CGWindowID, WindowEntry> {
        self.registry
            .iter()
            .map(|(id, e)| (id, e.clone()))
            .collect()
    }

    pub(super) fn ignore_rules(&self) -> Vec<MacosWindow> {
        self.config.macos.ignore.clone()
    }

    pub(super) fn observed_pids(&self) -> HashSet<i32> {
        self.observed_pids.clone()
    }

    pub(super) fn set_pid_moving(&mut self, pid: i32, moving: bool) {
        self.registry.set_pid_moving(pid, moving);
    }

    pub(super) fn mark_pid_observed(&mut self, pid: i32) {
        self.observed_pids.insert(pid);
    }

    pub(super) fn unmark_pid_observed(&mut self, pid: i32) {
        self.observed_pids.remove(&pid);
    }

    pub(super) fn remove_untracked_app(&mut self, pid: i32) {
        self.remove_app_windows(pid);
    }

    pub(super) fn register_observers(&mut self, apps: Vec<RunningApp>) {
        self.sender.send(HubMessage::RegisterObservers(apps));
    }

    /// All fullscreen -> normal and normal -> fullscreen must be resolved before this step
    #[tracing::instrument(skip_all)]
    fn flush_layout(&mut self) {
        let mut shows = Vec::new();
        let mut containers = Vec::new();
        let placements = self.hub.get_visible_placements();
        let all_displayed_windows: HashSet<WindowId> = placements
            .iter()
            .flat_map(|mp| match &mp.layout {
                MonitorLayout::Normal { windows, .. } => {
                    windows.iter().map(|p| p.id).collect::<Vec<_>>()
                }
                MonitorLayout::Fullscreen(wid) => vec![*wid],
            })
            .collect();
        let to_hide: Vec<_> = placements
            .iter()
            .flat_map(|mp| {
                let entry = self.monitor_registry.get_entry(mp.monitor_id);
                entry
                    .displayed_windows
                    .difference(&all_displayed_windows)
                    .copied()
                    .collect::<Vec<_>>()
            })
            .collect();
        for wid in to_hide {
            if self.registry.contains_id(wid) {
                self.hide_window(wid);
            }
        }
        for mp in placements {
            let displayed: HashSet<WindowId> = match &mp.layout {
                MonitorLayout::Fullscreen(window_id) => HashSet::from([*window_id]),
                MonitorLayout::Normal { windows, .. } => windows.iter().map(|p| p.id).collect(),
            };
            self.monitor_registry
                .get_entry_mut(mp.monitor_id)
                .displayed_windows = displayed;
            let (s, c) = self.apply_monitor_placements(&mp);
            shows.extend(s);
            containers.extend(c);
        }

        let focused = match self
            .hub
            .get_workspace(self.hub.current_workspace())
            .focused()
        {
            Some(Child::Window(id)) => Some(id),
            _ => None,
        };
        if focused != self.last_focused {
            self.last_focused = focused;
            if let Some(id) = focused {
                let window = self.registry.by_id(id);
                if let Err(err) = window.ax.focus() {
                    tracing::trace!("Failed to focus window: {err:#}");
                }
            }
        }
        let changes = self.hub.drain_changes();

        for &wid in &changes.created_windows {
            if !changes.deleted_windows.contains(&wid)
                && !all_displayed_windows.contains(&wid)
                && self.registry.contains_id(wid)
            {
                self.hide_window(wid);
            }
        }

        let creates = changes
            .created_windows
            .iter()
            .filter_map(|&wid| {
                if changes.deleted_windows.contains(&wid) {
                    return None;
                }
                let entry = self.registry.try_by_id(wid)?;
                let dim = self.hub.get_window(wid).dimension();
                let cg_id = entry.ax.cg_id();
                Some(OverlayCreate {
                    window_id: wid,
                    cg_id,
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
            && let Some(entry) = self.registry.get(ax.cg_id())
        {
            self.hub.set_focus(entry.window_id);
        }
    }

    fn handle_space_changed(&mut self) {
        let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
            return;
        };
        let app = RunningApp::from(app);
        // All AX APIs should be synchronous here, as we should pause everything until we know
        // whether we are dealing with native fullscreen or not.
        let Some(ax) = app.focused_window() else {
            return;
        };
        let cg_id = ax.cg_id();
        let is_native_fs = ax.is_native_fullscreen();

        if let Some(entry) = self.registry.get_mut(cg_id) {
            let _span = tracing::debug_span!("space_changed",).entered();
            let window_id = entry.window_id;
            if is_native_fs {
                entry.state = WindowState::NativeFullscreen;
                self.hub.set_fullscreen(window_id);
            } else if !is_native_fs && matches!(entry.state, WindowState::NativeFullscreen) {
                let Ok(pos) = ax.get_position() else {
                    return;
                };
                let Ok(size) = ax.get_size() else {
                    return;
                };
                self.window_moved(window_id, pos.0, pos.1, size.0, size.1, Instant::now());
            }
        } else if is_native_fs {
            let window_id = self.hub.insert_fullscreen();
            self.registry.insert(
                Arc::new(ax.clone()),
                window_id,
                WindowState::NativeFullscreen,
                ax.app_name().map(str::to_owned),
                ax.bundle_id().map(str::to_owned),
                ax.title().map(str::to_owned),
            );
            tracing::info!(%ax, %window_id, "New native fullscreen window");
        }
    }

    fn remove_app_windows(&mut self, pid: i32) {
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

    fn update_screens(&mut self, screens: Vec<MonitorInfo>) {
        if screens.is_empty() {
            tracing::warn!("Empty screen list, skipping reconciliation");
            return;
        }

        if let Some(primary) = screens.iter().find(|s| s.is_primary) {
            self.primary_screen = primary.dimension;
            self.primary_full_height = primary.full_height;
        }

        // Re-hide windows that are offscreen with updated monitor positions
        for (_, entry) in self.registry.iter() {
            if let WindowState::Positioned(PositionedState::Offscreen { actual }) = &entry.state
                && let Err(e) = move_offscreen(&screens, actual, &*entry.ax)
            {
                tracing::trace!("Failed to re-hide window: {e:#}");
            }
        }

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

    fn apply_monitor_placements(
        &mut self,
        mp: &MonitorPlacements,
    ) -> (Vec<OverlayShow>, Vec<ContainerOverlayData>) {
        match &mp.layout {
            MonitorLayout::Fullscreen(window_id) => {
                self.place_fullscreen_window(*window_id, mp.monitor_id);
                (Vec::new(), Vec::new())
            }
            MonitorLayout::Normal {
                windows,
                containers,
            } => {
                let monitors = self.monitor_registry.all_screens();
                let border_size = self.config.border_size;
                let mut shows = Vec::new();
                for wp in windows {
                    let content_dim = apply_inset(wp.frame, border_size);
                    // Clip to visible_frame bounds — macOS doesn't reliably allow
                    // placing windows partially off-screen (especially above menu bar)
                    let visible_content = clip_to_bounds(content_dim, wp.visible_frame);

                    if wp.is_float && !wp.is_focused {
                        self.move_offscreen(wp.id);
                    } else {
                        let Some(target) = visible_content else {
                            let _span = tracing::debug_span!("empty_visible_content", ?content_dim, visible_frame = ?wp.visible_frame).entered();
                            self.move_offscreen(wp.id);
                            continue;
                        };
                        self.place_window(wp.id, target);
                    }

                    shows.push(OverlayShow {
                        window_id: wp.id,
                        placement: *wp,
                        cocoa_frame: to_ns_rect(self.primary_full_height, wp.visible_frame),
                        scale: hidden_monitor(&monitors).scale,
                        content_dim,
                        visible_content,
                    });
                }

                let mut container_overlays = Vec::new();
                for cp in containers {
                    let cocoa_frame = to_ns_rect(self.primary_full_height, cp.visible_frame);
                    let tab_titles = if cp.is_tabbed {
                        let container = self.hub.get_container(cp.id);
                        collect_tab_titles(container, &self.registry)
                    } else {
                        Vec::new()
                    };
                    container_overlays.push(ContainerOverlayData {
                        placement: cp.clone(),
                        tab_titles,
                        cocoa_frame,
                    });
                }
                (shows, container_overlays)
            }
        }
    }

    fn add_window(
        &mut self,
        ax: Arc<dyn AXWindowApi>,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        app_name: Option<String>,
        bundle_id: Option<String>,
        title: Option<String>,
    ) -> WindowId {
        let dim = RoundedDimension {
            x,
            y,
            width: w,
            height: h,
        };
        let monitor = self
            .monitor_registry
            .find_monitor_at(dim.x as f32, dim.y as f32);
        let is_borderless_fullscreen = monitor.is_some_and(|m| {
            let mon = &m.dimension;
            let tolerance = 2;
            (dim.x - mon.x as i32).abs() <= tolerance
                && (dim.y - mon.y as i32).abs() <= tolerance
                && (dim.width - mon.width as i32).abs() <= tolerance
                && (dim.height - mon.height as i32).abs() <= tolerance
        });
        if is_borderless_fullscreen {
            let window_id = self.hub.insert_fullscreen();
            self.registry.insert(
                ax.clone(),
                window_id,
                WindowState::BorderlessFullscreen,
                app_name.clone(),
                bundle_id.clone(),
                title.clone(),
            );
            tracing::info!(%window_id, "New borderless fullscreen window");
            window_id
        } else {
            let window_id = self.hub.insert_tiling();
            self.registry.insert(
                ax.clone(),
                window_id,
                WindowState::Positioned(PositionedState::Offscreen { actual: dim }),
                app_name,
                bundle_id,
                title,
            );
            tracing::info!(%window_id, "New tiling window");
            window_id
        }
    }

    fn add_native_fullscreen_window(
        &mut self,
        ax: Arc<dyn AXWindowApi>,
        app_name: Option<String>,
        bundle_id: Option<String>,
        title: Option<String>,
    ) -> WindowId {
        let window_id = self.hub.insert_fullscreen();
        self.registry.insert(
            ax,
            window_id,
            WindowState::NativeFullscreen,
            app_name,
            bundle_id,
            title,
        );
        tracing::info!(%window_id, "New native fullscreen window");
        window_id
    }

    #[tracing::instrument(skip(self))]
    fn place_window(&mut self, window_id: WindowId, dim: Dimension) {
        let window = self.registry.by_id_mut(window_id);
        if window.is_moving {
            return;
        }
        let target = round_dim(dim);
        match &mut window.state {
            WindowState::Positioned(PositionedState::InView(p)) => {
                if p.set_target(target)
                    && let Err(e) =
                        window
                            .ax
                            .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ax);
                }
            }
            WindowState::Positioned(PositionedState::Offscreen { actual }) => {
                let actual = *actual;
                window.state = WindowState::Positioned(PositionedState::InView(Placement::new(
                    actual, target,
                )));
                if let Err(e) = window
                    .ax
                    .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Window {} set_frame failed: {e}", window.ax);
                }
            }
            _ => {
                debug_assert!(
                    false,
                    "We can only position windows in Positioned state, it seems core's state and platform's state differ"
                );
            }
        }
    }

    #[tracing::instrument(skip(self))]
    fn place_fullscreen_window(&mut self, window_id: WindowId, monitor_id: crate::core::MonitorId) {
        let window = self.registry.by_id_mut(window_id);
        let monitor = self.monitor_registry.get_entry_mut(monitor_id);
        let screen_dim = monitor.screen.dimension;
        match &mut window.state {
            WindowState::Minimized => {
                if let Err(err) = window.ax.unminimize() {
                    tracing::trace!("Failed to unminimize window: {err:#}");
                }
                window.state = WindowState::BorderlessFullscreen
            }
            WindowState::Positioned(PositionedState::Offscreen { actual }) => {
                let actual = *actual;
                let target = round_dim(screen_dim);
                window.state = WindowState::Positioned(PositionedState::InView(Placement::new(
                    actual, target,
                )));
                if let Err(err) =
                    window
                        .ax
                        .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Failed to set fullscreen frame: {err:#}");
                }
            }
            WindowState::Positioned(PositionedState::InView(p)) => {
                let target = round_dim(screen_dim);
                if p.set_target(target)
                    && let Err(err) =
                        window
                            .ax
                            .set_frame(target.x, target.y, target.width, target.height)
                {
                    tracing::trace!("Failed to set fullscreen frame: {err:#}");
                }
            }
            // We don't touch OS managed fullscreen windows
            _ => {}
        }
    }

    #[tracing::instrument(skip(self))]
    fn window_entered_native_fullscreen(&mut self, window_id: WindowId) {
        let window = self.registry.by_id_mut(window_id);
        window.state = WindowState::NativeFullscreen;
        self.hub.set_fullscreen(window.window_id);
    }

    #[tracing::instrument(skip(self))]
    fn window_moved(
        &mut self,
        window_id: WindowId,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        observed_at: Instant,
    ) {
        let new_placement = RoundedDimension {
            x,
            y,
            width: w,
            height: h,
        };
        let monitors = self.monitor_registry.all_screens();
        let window = self.registry.by_id_mut(window_id);
        let monitor = self
            .monitor_registry
            .find_monitor_at(new_placement.x as f32, new_placement.y as f32);
        let is_borderless_fullscreen = monitor.is_some_and(|m| {
            let mon = &m.dimension;
            let tolerance = 2;
            (new_placement.x - mon.x as i32).abs() <= tolerance
                && (new_placement.y - mon.y as i32).abs() <= tolerance
                && (new_placement.width - mon.width as i32).abs() <= tolerance
                && (new_placement.height - mon.height as i32).abs() <= tolerance
        });

        match &mut window.state {
            WindowState::Positioned(PositionedState::Offscreen { actual }) => {
                if is_borderless_fullscreen {
                    // Window turned fullscreen, but not visible, so we hide them again
                    self.hub.set_fullscreen(window_id);
                    window.state = WindowState::Minimized;
                    if let Err(e) = window.ax.minimize() {
                        tracing::trace!("Failed to minimize window: {e:#}");
                    }
                } else {
                    *actual = new_placement;
                    if let Err(e) = move_offscreen(&monitors, actual, &*window.ax) {
                        tracing::trace!("re-hide failed: {e}");
                    }
                }
            }
            WindowState::Positioned(PositionedState::InView(p)) => {
                if p.placed_at > observed_at {
                    tracing::trace!(placed_at = ?p.placed_at, "stale observation, ignoring");
                    return;
                }

                if new_placement == p.target {
                    p.actual = new_placement;
                    return;
                }

                if is_borderless_fullscreen {
                    window.state = WindowState::BorderlessFullscreen;
                    self.hub.set_fullscreen(window_id);
                    return;
                }
                let hub_window = self.hub.get_window(window_id);
                // Float can only be moved when focused (otherwise it's the mirror), and focused
                // floats are always inside viewport
                if hub_window.is_float() {
                    p.actual = new_placement;
                    // TODO: update float dimension
                    return;
                }

                if p.record_drift(new_placement) {
                    let target = p.target;
                    let should_retry = p.should_retry();
                    let just_gave_up = p.just_gave_up();
                    if should_retry {
                        tracing::trace!(?target, "window {window} drifted, correcting");
                        if let Err(e) =
                            window
                                .ax
                                .set_frame(target.x, target.y, target.width, target.height)
                        {
                            tracing::trace!("Window {} set_frame failed: {e}", window);
                        }
                    } else if just_gave_up {
                        tracing::debug!("Window {} can't be moved to {:?}", window, target,);
                    }
                    return;
                }

                p.actual = new_placement;
                let Some(c) = p.detect_constraint() else {
                    return;
                };
                // Convert actual window size back to frame size by adding border back.
                // Frame dimensions have border inset applied. If in the original frame,
                // window width is smaller than sum of borders, then we will request a size
                // that can accommodate the borders here.
                let remove_inset = |v: f32| v + 2.0 * self.config.border_size;
                self.hub.set_window_constraint(
                    window_id,
                    c.min_width.map(remove_inset),
                    c.min_height.map(remove_inset),
                    c.max_width.map(remove_inset),
                    c.max_height.map(remove_inset),
                );
            }
            WindowState::Minimized => {
                // Window somehow got brought back to screen, maybe through window focused but the
                // notification was not fired
                tracing::trace!("Previously minimized borderless fullscreen window reappeared");
                if is_borderless_fullscreen && let Err(e) = window.ax.minimize() {
                    tracing::trace!("Failed to minimize window: {e:#}");
                }
                // No longer fullscreen borderless, so bring them back and put in offscreen
                else {
                    if let Err(e) = window.ax.unminimize() {
                        tracing::debug!("Failed to unminimize window: {e:#}");
                    }
                    if let Err(e) = move_offscreen(&monitors, &new_placement, &*window.ax) {
                        tracing::trace!("hide after unminimize failed: {e}");
                    }
                    window.state = WindowState::Positioned(PositionedState::Offscreen {
                        actual: new_placement,
                    });
                    self.hub.unset_fullscreen(window_id);
                }
            }
            WindowState::BorderlessFullscreen => {
                // No longer border borderless fullscreen. Move to offscreen position as these
                // windows might now be inserted offscreen, which will be put back into view later
                // if it's in view
                if !is_borderless_fullscreen {
                    window.state = WindowState::Positioned(PositionedState::Offscreen {
                        actual: new_placement,
                    });
                    self.hub.unset_fullscreen(window_id);
                }
            }
            WindowState::NativeFullscreen => {
                // No longer native fullscreen
                if is_borderless_fullscreen {
                    window.state = WindowState::BorderlessFullscreen;
                } else {
                    window.state = WindowState::Positioned(PositionedState::Offscreen {
                        actual: new_placement,
                    });
                    self.hub.unset_fullscreen(window_id);
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    fn hide_window(&mut self, window_id: WindowId) {
        let monitors = self.monitor_registry.all_screens();
        let window = self.registry.by_id_mut(window_id);
        // Minimize borderless fullscreen windows instead of moving offscreen:
        // 1. User-zoomed windows maintain their fullscreen state, so moving them is futile
        // 2. Moving offscreen triggers handle_window_moved which detects fullscreen exit
        // Native fullscreen windows are on a separate Space and don't need hiding.
        let result = match &window.state {
            WindowState::BorderlessFullscreen => {
                window.state = WindowState::Minimized;
                window.ax.minimize()
            }
            WindowState::NativeFullscreen | WindowState::Minimized => Ok(()),
            WindowState::Positioned(positioned_state) => match positioned_state {
                PositionedState::InView(placement) => {
                    let actual = placement.actual;
                    window.state = WindowState::Positioned(PositionedState::Offscreen { actual });
                    move_offscreen(&monitors, &actual, &*window.ax)
                }
                PositionedState::Offscreen { actual } => {
                    move_offscreen(&monitors, actual, &*window.ax)
                }
            },
        };
        if let Err(e) = result {
            tracing::trace!("Failed to hide window: {e:#}");
        }
    }

    #[tracing::instrument(skip(self))]
    fn move_offscreen(&mut self, window_id: WindowId) {
        let window = self.registry.by_id_mut(window_id);
        let WindowState::Positioned(positioned_state) = window.state else {
            debug_assert!(
                false,
                "Can only move windows which dome control the positions offscreen"
            );
            return;
        };
        let monitors = self.monitor_registry.all_screens();
        match positioned_state {
            PositionedState::InView(placement) => {
                let actual = placement.actual;
                move_offscreen(&monitors, &actual, &*window.ax);
                window.state = WindowState::Positioned(PositionedState::Offscreen { actual })
            }
            PositionedState::Offscreen { actual } => {
                move_offscreen(&monitors, &actual, &*window.ax);
            }
        }
    }
}

impl Drop for Dome {
    fn drop(&mut self) {
        recovery::restore_all();
        self.sender.send(HubMessage::Shutdown);
    }
}

fn collect_tab_titles(container: &Container, registry: &Registry) -> Vec<String> {
    container
        .children()
        .iter()
        .map(|c| match c {
            Child::Window(wid) => registry
                .by_id(*wid)
                .title
                .as_deref()
                .unwrap_or("Unknown")
                .to_owned(),
            Child::Container(_) => "Container".to_owned(),
        })
        .collect()
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

// Quartz uses top-left origin, Cocoa uses bottom-left origin
fn to_ns_rect(primary_full_height: f32, dim: Dimension) -> NSRect {
    NSRect::new(
        NSPoint::new(
            dim.x as f64,
            (primary_full_height - dim.y - dim.height) as f64,
        ),
        NSSize::new(dim.width as f64, dim.height as f64),
    )
}

fn on_open_actions(entry: &WindowEntry, rules: &[MacosOnOpenRule]) -> Option<Actions> {
    let rule = rules.iter().find(|r| {
        r.window.matches(
            entry.app_name.as_deref(),
            entry.bundle_id.as_deref(),
            entry.title.as_deref(),
        )
    })?;
    tracing::debug!(%entry, actions = %rule.run, "Running on_open actions");
    Some(rule.run.clone())
}
