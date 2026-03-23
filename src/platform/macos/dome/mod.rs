mod events;
mod inspect;
mod monitor;
mod placement_tracker;
mod recovery;
mod registry;
mod runloop;
mod window;

pub(super) use events::{
    ContainerOverlayData, HubEvent, HubMessage, OverlayCreate, OverlayShow, RenderFrame,
};
pub(super) use monitor::get_all_screens;

use std::collections::{HashMap, HashSet};

use calloop::LoopSignal;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};
use crate::config::{Config, MacosOnOpenRule};
use crate::core::{Child, Container, Dimension, Hub, MonitorLayout, MonitorPlacements, WindowId};
use crate::platform::macos::accessibility::AXWindow;
use crate::platform::macos::dome::inspect::{GcdDispatcher, NewAxWindow};
use crate::platform::macos::dome::window::PositionedState;
use objc2_app_kit::NSWorkspace;
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::running_application::RunningApp;
use monitor::{MonitorInfo, MonitorRegistry};
use placement_tracker::PlacementTracker;
use registry::Registry;
use window::{
    RoundedDimension, WindowState, apply_inset, clip_to_bounds, hidden_monitor, move_offscreen,
};

pub(in crate::platform::macos) trait FrameSender: Send {
    fn send(&self, msg: HubMessage);
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
    sender: Box<dyn FrameSender>,
    dispatcher: GcdDispatcher,
    signal: LoopSignal,
    placement_tracker: PlacementTracker,
    last_focused: Option<WindowId>,
}

impl Dome {
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
            self.hide_window(wid);
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
            if !changes.deleted_windows.contains(&wid) && !all_displayed_windows.contains(&wid) {
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
                let dim = self.hub.get_window(wid).dimension();
                let cg_id = self.registry.by_id(wid).ax.cg_id();
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
                let new_placement = RoundedDimension {
                    x: pos.0,
                    y: pos.1,
                    width: size.0,
                    height: size.1,
                };
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
                if is_borderless_fullscreen {
                    entry.state = WindowState::BorderlessFullscreen;
                } else {
                    self.hub.unset_fullscreen(window_id);
                    entry.state = WindowState::Positioned(PositionedState::Offscreen {
                        actual: new_placement,
                    });
                }
            }
        } else if is_native_fs {
            let window_id = self.hub.insert_fullscreen();
            self.registry
                .insert(ax.clone(), window_id, WindowState::NativeFullscreen);
            tracing::info!(%ax, %window_id, "New native fullscreen window");
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
                && let Err(e) = move_offscreen(&screens, actual, &entry.ax)
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

    fn apply_windows_reconciled(&mut self, to_remove: Vec<CGWindowID>, to_add: Vec<NewAxWindow>) {
        for cg_id in to_remove {
            self.remove_window(cg_id);
        }

        for new in to_add {
            if self.registry.contains(new.ax.cg_id()) {
                continue;
            }
            let window_id = if new.is_native_fullscreen {
                self.add_native_fullscreen_window(new.ax.clone())
            } else {
                self.add_window(new.ax.clone(), new.dimension)
            };

            recovery::track(new.ax.clone(), self.primary_screen);

            let entry = self.registry.by_id(window_id);

            if let Some(actions) = on_open_actions(&entry.ax, &self.config.macos.on_open) {
                self.execute_actions(&actions);
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
}

impl Drop for Dome {
    fn drop(&mut self) {
        recovery::restore_all();
        self.sender.send(HubMessage::Shutdown);
    }
}

fn on_open_actions(ax: &AXWindow, rules: &[MacosOnOpenRule]) -> Option<Actions> {
    let rule = rules
        .iter()
        .find(|r| r.window.matches(ax.app_name(), ax.bundle_id(), ax.title()))?;
    tracing::debug!(%ax, actions = %rule.run, "Running on_open actions");
    Some(rule.run.clone())
}

fn collect_tab_titles(container: &Container, registry: &Registry) -> Vec<String> {
    container
        .children()
        .iter()
        .map(|c| match c {
            Child::Window(wid) => registry
                .by_id(*wid)
                .ax
                .title()
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
pub(super) fn to_ns_rect(primary_full_height: f32, dim: Dimension) -> NSRect {
    NSRect::new(
        NSPoint::new(
            dim.x as f64,
            (primary_full_height - dim.y - dim.height) as f64,
        ),
        NSSize::new(dim.width as f64, dim.height as f64),
    )
}
