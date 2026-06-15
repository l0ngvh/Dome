use crate::action::MonitorTarget;
use crate::config::{LayoutConfig, Strategy};

use std::collections::HashSet;

use super::allocator::{Allocator, NodeId};
use super::node::{
    ContainerId, Dimension, DisplayMode, Length, Monitor, MonitorId, Window, WindowId,
    WindowRestrictions, Workspace, WorkspaceId,
};
use super::strategy::{StrategySet, TilingAction, clip};

pub(crate) struct VisiblePlacements {
    /// Window that should receive keyboard focus
    pub(crate) focused_window: Option<WindowId>,
    pub(crate) focused_monitor: MonitorId,
    /// Placement of windows per monitor
    pub(crate) monitors: Vec<MonitorPlacements>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TilingWindowPlacement {
    pub(crate) id: WindowId,
    pub(crate) frame: Dimension,
    pub(crate) visible_frame: Dimension,
    /// Whether to highlight the window, for example when the window is focused. Doesn't require
    /// that the window has keyboard focus.
    pub(crate) is_highlighted: bool,
    pub(crate) spawn_indicator: Option<SpawnIndicator>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FloatWindowPlacement {
    pub(crate) id: WindowId,
    pub(crate) frame: Dimension,
    pub(crate) visible_frame: Dimension,
    pub(crate) is_highlighted: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ContainerPlacement {
    pub(crate) id: ContainerId,
    pub(crate) frame: Dimension,
    pub(crate) visible_frame: Dimension,
    pub(crate) is_highlighted: bool,
    pub(crate) spawn_indicator: Option<SpawnIndicator>,
    pub(crate) is_tabbed: bool,
    pub(crate) active_tab_index: usize,
    pub(crate) titles: Vec<String>,
}

pub(crate) struct MonitorPlacements {
    pub(crate) monitor_id: MonitorId,
    pub(crate) layout: MonitorLayout,
}

pub(crate) enum MonitorLayout {
    Normal {
        tiling_windows: Vec<TilingWindowPlacement>,
        float_windows: Vec<FloatWindowPlacement>,
        containers: Vec<ContainerPlacement>,
    },
    Fullscreen(WindowId),
}

/// Which border edges to highlight with the spawn indicator color.
/// Each bool means "highlight this edge." `left` is always false today
/// but included so we don't need a struct change if a future spawn mode uses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpawnIndicator {
    pub(crate) top: bool,
    pub(crate) right: bool,
    pub(crate) bottom: bool,
    pub(crate) left: bool,
}

/// Categorizes restricted operations by what they do, so each restriction level
/// (BlockAll, ProtectFullscreen) can allow or deny them independently.
pub(super) enum RestrictedAction {
    /// Navigate or rearrange within the current tiling paradigm.
    /// Blocked by: BlockAll.
    TilingNavigation,
    /// Change the window's display mode (float, fullscreen).
    /// Blocked by: BlockAll, ProtectFullscreen.
    DisplayModeChange,
    /// Move the window to a different workspace (same or different monitor).
    /// Blocked by: BlockAll only. ProtectFullscreen does NOT block this -- on macOS
    /// and Windows, fullscreen windows can freely move across workspaces.
    WorkspaceMove,
    /// Move the window to a different monitor's active workspace.
    /// Blocked by: BlockAll, ProtectFullscreen. Fullscreen windows are bound to their
    /// monitor -- moving them cross-monitor would break the fullscreen association.
    MonitorMove,
}

/// Non-strategy fields of Hub, extracted so that `TilingStrategy` methods can
/// receive `&mut HubAccess` while Hub holds `&mut strategy` separately. This
/// solves the split-borrow problem: strategy and access are disjoint fields.
#[derive(Debug)]
pub(crate) struct HubAccess {
    pub(super) monitors: Allocator<Monitor>,
    pub(super) focused_monitor: MonitorId,
    pub(super) config: LayoutConfig,
    pub(super) workspaces: Allocator<Workspace>,
    pub(super) windows: Allocator<Window>,
}

#[derive(Debug)]
pub(crate) struct Hub {
    pub(super) access: HubAccess,
    pub(super) strategies: StrategySet,
    pub(super) minimized_windows: Vec<WindowId>,
}

impl Hub {
    pub(crate) fn new(primary_screen: Dimension, primary_scale: f32, config: LayoutConfig) -> Self {
        let mut monitors: Allocator<Monitor> = Allocator::new();
        let mut workspaces: Allocator<Workspace> = Allocator::new();

        let primary_id = monitors.allocate(Monitor {
            name: "primary".to_string(),
            dimension: primary_screen,
            scale: primary_scale,
            active_workspace: WorkspaceId::new(0),
        });

        let primary_ws_name = "0".to_string();
        let primary_ws_id =
            workspaces.allocate(Workspace::new(primary_ws_name.clone(), primary_id));
        monitors.get_mut(primary_id).active_workspace = primary_ws_id;

        let mut strategies = StrategySet::new(&config);
        strategies.register(primary_ws_id, &primary_ws_name, &config);

        // Pre-allocate every workspace name listed in [[layout.workspace]]
        // (skipping any that collide with the primary workspace's name) so
        // that named workspaces have stable IDs from boot. They live on the
        // primary monitor and are pinned: prune_workspace leaves them alone.
        for entry in &config.workspace {
            if entry.name == primary_ws_name {
                continue;
            }
            let ws_id = workspaces.allocate(Workspace::new(entry.name.clone(), primary_id));
            strategies.register(ws_id, &entry.name, &config);
        }

        Self {
            access: HubAccess {
                monitors,
                focused_monitor: primary_id,
                config,
                workspaces,
                windows: Allocator::new(),
            },
            strategies,
            minimized_windows: Vec::new(),
        }
    }

    pub(crate) fn current_workspace(&self) -> WorkspaceId {
        self.access
            .monitors
            .get(self.access.focused_monitor)
            .active_workspace
    }

    /// Return the window that should get keyboard focus.
    ///
    /// The top most fullscreen window will get the focus, if any, as fullscreen windows take over
    /// the whole workspaces they are in.
    /// If none is present, focus between float and tiling windows will be decided by is_float_focused
    pub(super) fn focused_window(&self, ws_id: WorkspaceId) -> Option<WindowId> {
        let workspace = self.access.workspaces.get(ws_id);

        if let Some(&id) = workspace.fullscreen_windows.last() {
            return Some(id);
        }
        if workspace.is_float_focused
            && let Some(&id) = workspace.float_windows.last()
        {
            return Some(id);
        }
        self.strategies
            .for_workspace(ws_id)
            .focused_tiling_window(&self.access, ws_id)
    }

    pub(super) fn is_restricted(&self, action: RestrictedAction) -> bool {
        let ws_id = self.current_workspace();
        let Some(id) = self.focused_window(ws_id) else {
            return false;
        };
        let restrictions = self.access.windows.get(id).restrictions;
        match action {
            RestrictedAction::TilingNavigation | RestrictedAction::WorkspaceMove => {
                restrictions == WindowRestrictions::BlockAll
            }
            RestrictedAction::DisplayModeChange | RestrictedAction::MonitorMove => {
                restrictions != WindowRestrictions::None
            }
        }
    }

    /// Single entry point for tiling actions. Checks restrictions and delegates
    /// to the strategy.
    #[tracing::instrument(skip(self))]
    pub(crate) fn handle_tiling_action(&mut self, action: TilingAction) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let ws_id = self.current_workspace();
        self.strategies
            .for_workspace_mut(ws_id)
            .handle_action(&mut self.access, action);
    }

    pub(crate) fn focus_tab_index(&mut self, container_id: ContainerId, index: usize) {
        self.handle_tiling_action(TilingAction::TabClicked {
            container_id,
            index,
        });
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_monitor(&mut self, target: &MonitorTarget) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        let Some(target_id) = self.find_monitor_by_target(target) else {
            return;
        };
        if target_id == self.access.focused_monitor {
            return;
        }
        tracing::debug!("Focusing monitor");
        self.access.focused_monitor = target_id;
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn move_focused_to_monitor(&mut self, target: &MonitorTarget) {
        if self.is_restricted(RestrictedAction::MonitorMove) {
            return;
        }
        let Some(target_id) = self.find_monitor_by_target(target) else {
            return;
        };
        if target_id == self.access.focused_monitor {
            return;
        }

        let target_ws = self.access.monitors.get(target_id).active_workspace;
        tracing::debug!("Moving to monitor");
        let current_ws = self.current_workspace();
        if let Some(window_id) = self.focused_window(current_ws) {
            self.move_child_to_workspace_with_id(window_id, target_ws);
        } else {
            let has_tiling = self
                .strategies
                .for_workspace(current_ws)
                .has_tiling_windows(&self.access, current_ws);
            if has_tiling {
                tracing::debug!(?current_ws, ?target_ws, "Moving container to monitor");
                self.move_focused_across_workspaces(current_ws, target_ws);
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn set_focus(&mut self, window_id: WindowId) {
        tracing::debug!("Setting focus to window");
        let window = self.access.windows.get(window_id);
        let ws = window
            .workspace()
            .expect("non-minimized window has a workspace");
        match window.mode {
            DisplayMode::Fullscreen => {
                let fs = &mut self.access.workspaces.get_mut(ws).fullscreen_windows;
                if let Some(pos) = fs.iter().position(|&w| w == window_id) {
                    fs.remove(pos);
                    fs.push(window_id);
                }
                self.access.workspaces.get_mut(ws).is_float_focused = false;
            }
            DisplayMode::Float { .. } => {
                self.focus_float(ws, window_id);
            }
            DisplayMode::Tiling => {
                self.strategies
                    .for_workspace_mut(ws)
                    .set_focus(&mut self.access, window_id);
            }
        }
        self.focus_workspace_with_id(ws);
    }

    pub(crate) fn focused_monitor(&self) -> MonitorId {
        self.access.focused_monitor
    }

    pub(crate) fn visible_workspaces(&self) -> Vec<WorkspaceId> {
        self.access
            .monitors
            .all_active()
            .into_iter()
            .map(|(_, m)| m.active_workspace)
            .collect()
    }

    /// Returns metadata for all active workspaces, ordered by WorkspaceId
    /// (creation order). Pruned workspaces (empty and not active on any
    /// monitor) never appear because `prune_workspace` deletes them.
    pub(crate) fn query_workspaces(&self) -> Vec<super::WorkspaceInfo> {
        let focused_ws = self.current_workspace();
        let visible: Vec<WorkspaceId> = self.visible_workspaces();
        self.access
            .workspaces
            .all_active()
            .into_iter()
            .map(|(ws_id, ws)| super::WorkspaceInfo {
                name: ws.name.clone(),
                is_focused: ws_id == focused_ws,
                is_visible: visible.contains(&ws_id),
                window_count: self.count_workspace_windows(ws_id, &ws),
            })
            .collect()
    }

    fn count_workspace_windows(&self, ws_id: WorkspaceId, ws: &Workspace) -> usize {
        let tiling_count = self
            .strategies
            .for_workspace(ws_id)
            .tiling_window_count(&self.access, ws_id);
        tiling_count + ws.float_windows.len() + ws.fullscreen_windows.len()
    }

    pub(crate) fn add_monitor(
        &mut self,
        name: String,
        dimension: Dimension,
        scale: f32,
    ) -> MonitorId {
        let monitor_id = self.access.monitors.allocate(Monitor {
            name: name.clone(),
            dimension,
            scale,
            active_workspace: WorkspaceId::new(0),
        });
        let ws_id = self
            .access
            .workspaces
            .allocate(Workspace::new(name.clone(), monitor_id));
        self.access.monitors.get_mut(monitor_id).active_workspace = ws_id;
        self.strategies.register(ws_id, &name, &self.access.config);
        monitor_id
    }

    pub(crate) fn remove_monitor(&mut self, monitor_id: MonitorId, fallback_id: MonitorId) {
        assert!(
            fallback_id != monitor_id,
            "fallback must differ from removed monitor"
        );

        let workspaces_to_migrate: Vec<WorkspaceId> = self
            .access
            .workspaces
            .all_active()
            .iter()
            .filter(|(_, ws)| ws.monitor == monitor_id)
            .map(|(id, _)| *id)
            .collect();

        for ws_id in workspaces_to_migrate {
            self.access.workspaces.get_mut(ws_id).monitor = fallback_id;
            self.strategies
                .for_workspace_mut(ws_id)
                .layout_workspace(&mut self.access, ws_id);
        }

        if self.access.focused_monitor == monitor_id {
            self.access.focused_monitor = fallback_id;
        }
        self.access.monitors.delete(monitor_id);
    }

    pub(crate) fn update_monitor(
        &mut self,
        monitor_id: MonitorId,
        dimension: Dimension,
        scale: f32,
    ) {
        let monitor = self.access.monitors.get_mut(monitor_id);
        monitor.dimension = dimension;
        monitor.scale = scale;
        // Collect IDs first to avoid borrowing self.access.workspaces while
        // passing &mut self.access to the strategy.
        let ws_ids: Vec<WorkspaceId> = self
            .access
            .workspaces
            .all_active()
            .iter()
            .filter(|(_, ws)| ws.monitor == monitor_id)
            .map(|(id, _)| *id)
            .collect();
        for ws_id in ws_ids {
            self.strategies
                .for_workspace_mut(ws_id)
                .layout_workspace(&mut self.access, ws_id);
        }
    }

    pub(crate) fn sync_config(&mut self, config: LayoutConfig) {
        self.access.config = config;
        let changes = self
            .strategies
            .resync(&self.access.workspaces, &self.access.config);
        let changed_ids: HashSet<WorkspaceId> = changes.iter().map(|c| c.ws_id).collect();

        let unchanged_ids: Vec<WorkspaceId> = self
            .access
            .workspaces
            .all_active()
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| !changed_ids.contains(id))
            .collect();
        for ws_id in unchanged_ids {
            self.strategies
                .for_workspace_mut(ws_id)
                .apply_config(&mut self.access, ws_id);
        }

        for change in changes {
            tracing::debug!(
                ws_id = %change.ws_id,
                old = ?change.old,
                new = ?change.new,
                "Per-workspace strategy changed, rebuilding",
            );
            let snapshot = self.snapshot_workspace_for_rebuild(change.ws_id, change.old);
            self.strategies
                .get_mut(change.old)
                .prune_workspace(change.ws_id);
            self.reattach_workspace_after_rebuild(change.ws_id, snapshot);
        }
    }
    #[cfg(test)]
    pub(crate) fn validate_tree(&self) {
        self.strategies.validate_tree(&self.access);
    }

    pub(crate) fn set_window_title(&mut self, window_id: WindowId, title: String) {
        self.access.windows.get_mut(window_id).title = title;
    }

    pub(crate) fn get_visible_placements(&self) -> VisiblePlacements {
        let current_ws = self.current_workspace();

        let monitors = self
            .visible_workspaces()
            .into_iter()
            .map(|ws_id| {
                let ws = self.access.workspaces.get(ws_id);
                let screen = self.access.monitors.get(ws.monitor).dimension;

                // Fullscreen: only return topmost, skip tiling/float
                if let Some(&fs_id) = ws.fullscreen_windows.last() {
                    return MonitorPlacements {
                        monitor_id: ws.monitor,
                        layout: MonitorLayout::Fullscreen(fs_id),
                    };
                }

                let tiling = self
                    .strategies
                    .for_workspace(ws_id)
                    .collect_tiling_placements(&self.access, ws_id, ws_id == current_ws);
                let tiling_windows = tiling.windows;
                let containers = tiling.containers;

                let focused = if ws_id == current_ws {
                    self.focused_window(ws_id)
                } else {
                    None
                };

                let mut float_windows = Vec::new();
                for &id in &ws.float_windows {
                    let window = self.access.windows.get(id);
                    let DisplayMode::Float { dim } = window.mode else {
                        panic!("window {id} in float_windows but mode is not Float");
                    };
                    let frame = dim;
                    if let Some(visible_frame) = clip(frame, screen) {
                        let is_highlighted = focused == Some(id);
                        float_windows.push(FloatWindowPlacement {
                            id,
                            frame,
                            visible_frame,
                            is_highlighted,
                        });
                    }
                }

                MonitorPlacements {
                    monitor_id: ws.monitor,
                    layout: MonitorLayout::Normal {
                        tiling_windows,
                        float_windows,
                        containers,
                    },
                }
            })
            .collect();

        let focused_window = self.focused_window(current_ws);

        VisiblePlacements {
            focused_window,
            focused_monitor: self.access.focused_monitor,
            monitors,
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_tiling(&mut self, target_ws: WorkspaceId) -> WindowId {
        let window_id = self.access.windows.allocate(Window::tiling(target_ws));
        self.strategies.for_workspace_mut(target_ws).attach_window(
            &mut self.access,
            window_id,
            target_ws,
        );
        window_id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_float(
        &mut self,
        target_ws: WorkspaceId,
        dimension: Dimension,
    ) -> WindowId {
        let window_id = self
            .access
            .windows
            .allocate(Window::float(target_ws, dimension));
        tracing::debug!(%window_id, ?dimension, "Inserting float window");
        self.attach_float_to_workspace(target_ws, window_id, dimension);
        window_id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_fullscreen(
        &mut self,
        target_ws: WorkspaceId,
        restrictions: WindowRestrictions,
    ) -> WindowId {
        let window_id = self
            .access
            .windows
            .allocate(Window::fullscreen(target_ws, restrictions));
        self.attach_fullscreen_to_workspace(target_ws, window_id);
        self.set_focus(window_id);
        window_id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn delete_window(&mut self, id: WindowId) {
        let window = self.access.windows.get(id);
        let is_minimized = window.is_minimized();
        let mode = window.mode;

        if is_minimized {
            self.minimized_windows.retain(|&w| w != id);
        } else {
            let ws = window
                .workspace()
                .expect("non-minimized window has a workspace");
            match mode {
                DisplayMode::Float { .. } => {
                    let _dim = self.detach_float_from_workspace(id);
                }
                DisplayMode::Fullscreen => self.detach_fullscreen_from_workspace(id),
                DisplayMode::Tiling => {
                    self.strategies
                        .for_workspace_mut(ws)
                        .detach_window(&mut self.access, id);
                }
            }
            self.prune_workspace(ws);
        }

        self.access.windows.delete(id);
    }

    #[tracing::instrument(skip(self))]
    /// Set size constraints for a window.
    ///
    /// - `None`: don't change existing value
    /// - `Some(0.0)`: clear constraint
    /// - `Some(x)`: set constraint to x
    ///
    /// If setting min above existing max, max is raised to match min.
    pub(crate) fn set_window_constraint(
        &mut self,
        window_id: WindowId,
        min_width: Option<f32>,
        min_height: Option<f32>,
        max_width: Option<f32>,
        max_height: Option<f32>,
    ) {
        let window = self.access.windows.get_mut(window_id);

        let update = |name: &str,
                      min: &mut f32,
                      max: &mut f32,
                      new_min: Option<f32>,
                      new_max: Option<f32>| {
            if let Some(new_min) = new_min {
                *min = new_min;
                if *max > 0.0 && *max < new_min {
                    tracing::debug!(
                        "{name}: existing max {:.2} < new min {:.2}, raising max",
                        *max,
                        new_min
                    );
                    *max = new_min;
                }
            }
            if let Some(new_max) = new_max {
                *max = if new_max > 0.0 { new_max } else { 0.0 };
                if *max > 0.0 && *min > *max {
                    tracing::debug!(
                        "{name}: existing min {:.2} > new max {:.2}, lowering min",
                        *min,
                        *max
                    );
                    *min = *max;
                }
            }
        };

        update(
            "width",
            &mut window.min_width,
            &mut window.max_width,
            min_width,
            max_width,
        );
        update(
            "height",
            &mut window.min_height,
            &mut window.max_height,
            min_height,
            max_height,
        );

        tracing::debug!("Window constraint set");

        if let Some(ws) = window.workspace() {
            self.strategies
                .for_workspace_mut(ws)
                .layout_workspace(&mut self.access, ws);
        }
    }

    /// Move a window to a target workspace. For tiling windows, delegates to
    /// `Hub::move_focused_across_workspaces` which handles both window and container
    /// moves. For fullscreen/float, moves the specific window.
    #[tracing::instrument(skip(self))]
    pub(super) fn move_child_to_workspace_with_id(
        &mut self,
        window_id: WindowId,
        target_ws: WorkspaceId,
    ) {
        let current_ws = self.current_workspace();
        if current_ws == target_ws {
            return;
        }

        let window = self.access.windows.get(window_id);
        if window.is_minimized() {
            panic!("Minimized window can't be moved");
        }
        match window.mode {
            DisplayMode::Fullscreen => {
                self.detach_fullscreen_from_workspace(window_id);
                self.attach_fullscreen_to_workspace(target_ws, window_id);
                self.access.workspaces.get_mut(target_ws).is_float_focused = false;
            }
            DisplayMode::Float { .. } => {
                let dim = self.detach_float_from_workspace(window_id);
                self.attach_float_to_workspace(target_ws, window_id, dim);
            }
            DisplayMode::Tiling => {
                self.move_focused_across_workspaces(current_ws, target_ws);
            }
        }

        tracing::debug!("Moved to workspace");
    }

    pub(crate) fn resolve_workspace(&mut self, name: Option<&str>) -> WorkspaceId {
        match name {
            Some(n) => self.get_or_create_workspace(n),
            None => self.current_workspace(),
        }
    }

    pub(super) fn get_or_create_workspace(&mut self, name: &str) -> WorkspaceId {
        if let Some(id) = self.access.workspaces.find(|w| w.name == name) {
            return id;
        }
        let ws_id = self.access.workspaces.allocate(Workspace::new(
            name.to_string(),
            self.access.focused_monitor,
        ));
        self.strategies.register(ws_id, name, &self.access.config);
        ws_id
    }

    pub(super) fn find_monitor_by_target(&self, target: &MonitorTarget) -> Option<MonitorId> {
        match target {
            MonitorTarget::Name(name) => self
                .access
                .monitors
                .all_active()
                .iter()
                .find(|(_, m)| m.name == *name)
                .map(|(id, _)| *id),
            direction => {
                let current = self.access.monitors.get(self.access.focused_monitor);
                let cx = current.dimension.x + current.dimension.width / 2.0;
                let cy = current.dimension.y + current.dimension.height / 2.0;

                self.access
                    .monitors
                    .all_active()
                    .iter()
                    .filter(|(id, _)| *id != self.access.focused_monitor)
                    .filter_map(|(id, m)| {
                        let mx = m.dimension.x + m.dimension.width / 2.0;
                        let my = m.dimension.y + m.dimension.height / 2.0;
                        let dx = mx - cx;
                        let dy = my - cy;

                        let valid = match direction {
                            MonitorTarget::Left => dx < Length::ZERO,
                            MonitorTarget::Right => dx > Length::ZERO,
                            MonitorTarget::Up => dy < Length::ZERO,
                            MonitorTarget::Down => dy > Length::ZERO,
                            MonitorTarget::Name(_) => false,
                        };
                        // Use raw f32 for distance² comparison (unit is irrelevant for ordering)
                        let dist_sq = dx.value() * dx.value() + dy.value() * dy.value();
                        valid.then_some((*id, dist_sq))
                    })
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .map(|(id, _)| id)
            }
        }
    }

    pub(super) fn move_focused_across_workspaces(&mut self, from: WorkspaceId, to: WorkspaceId) {
        let from_kind = self.strategies.kind_of(from);
        let to_kind = self.strategies.kind_of(to);

        if from_kind == to_kind {
            self.strategies
                .get_mut(from_kind)
                .move_focused_to_workspace(&mut self.access, from, to);
        } else {
            let detached = self
                .strategies
                .get_mut(from_kind)
                .detach_focused(&mut self.access, from);
            self.strategies
                .get_mut(to_kind)
                .attach_detached(&mut self.access, to, &detached);
        }
    }

    fn snapshot_workspace_for_rebuild(
        &self,
        ws_id: WorkspaceId,
        old_kind: Strategy,
    ) -> WorkspaceRebuildSnapshot {
        let tiling_windows: Vec<WindowId> = self
            .access
            .windows
            .all_active()
            .iter()
            .filter(|(_, w)| w.mode == DisplayMode::Tiling && w.workspace() == Some(ws_id))
            .map(|(id, _)| *id)
            .collect();
        // Dispatch on the passed-in old_kind because StrategySet::resync has
        // already updated the kind map to the NEW kind, but the OLD strategy
        // still owns this workspace's tiling state.
        let focused = self
            .strategies
            .get(old_kind)
            .focused_tiling_window(&self.access, ws_id);
        let was_float_focused = self.access.workspaces.get(ws_id).is_float_focused;
        WorkspaceRebuildSnapshot {
            tiling_windows,
            focused,
            was_float_focused,
        }
    }

    fn reattach_workspace_after_rebuild(
        &mut self,
        ws_id: WorkspaceId,
        snapshot: WorkspaceRebuildSnapshot,
    ) {
        for wid in &snapshot.tiling_windows {
            self.strategies
                .for_workspace_mut(ws_id)
                .attach_window(&mut self.access, *wid, ws_id);
        }
        if let Some(f) = snapshot.focused {
            self.strategies
                .for_workspace_mut(ws_id)
                .set_focus(&mut self.access, f);
        }
        if snapshot.was_float_focused {
            self.access.workspaces.get_mut(ws_id).is_float_focused = true;
        }
    }
}

struct WorkspaceRebuildSnapshot {
    tiling_windows: Vec<WindowId>,
    focused: Option<WindowId>,
    was_float_focused: bool,
}
