use crate::action::MonitorTarget;
use crate::config::{LayoutConfig, SizeConstraint, Strategy};

use super::allocator::{Allocator, NodeId};
use super::master::MasterStrategy;
use super::node::{
    ContainerId, Dimension, DisplayMode, Length, Monitor, MonitorId, Window, WindowId,
    WindowRestrictions, Workspace, WorkspaceId,
};
use super::partition_tree::PartitionTreeStrategy;
use super::strategy::{TilingAction, TilingStrategy, clip};

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
    pub(super) config: HubConfig,
    pub(super) workspaces: Allocator<Workspace>,
    pub(super) windows: Allocator<Window>,
}

#[derive(Debug)]
pub(crate) struct Hub {
    pub(super) access: HubAccess,
    pub(super) strategy: Box<dyn TilingStrategy>,
    pub(super) minimized_windows: Vec<WindowId>,
}

impl Hub {
    pub(crate) fn new(primary_screen: Dimension, primary_scale: f32, config: HubConfig) -> Self {
        let mut monitors: Allocator<Monitor> = Allocator::new();
        let mut workspaces: Allocator<Workspace> = Allocator::new();

        let primary_id = monitors.allocate(Monitor {
            name: "primary".to_string(),
            dimension: primary_screen,
            scale: primary_scale,
            active_workspace: WorkspaceId::new(0),
        });

        let ws_id = workspaces.allocate(Workspace::new("0".to_string(), primary_id));
        monitors.get_mut(primary_id).active_workspace = ws_id;

        let strategy = build_strategy(&config.layout);

        Self {
            access: HubAccess {
                monitors,
                focused_monitor: primary_id,
                config,
                workspaces,
                windows: Allocator::new(),
            },
            strategy,
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
            && let Some(&(id, _)) = workspace.float_windows.last()
        {
            return Some(id);
        }
        self.strategy.focused_tiling_window(&self.access, ws_id)
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
        self.strategy.handle_action(&mut self.access, action);
    }

    pub(crate) fn focus_tab_index(&mut self, container_id: ContainerId, index: usize) {
        self.handle_tiling_action(TilingAction::TabClicked {
            container_id,
            index,
        });
    }

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
        tracing::debug!(?target, "Focusing monitor");
        self.access.focused_monitor = target_id;
    }

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
        tracing::debug!(?target, "Moving to monitor");
        let current_ws = self.current_workspace();
        if let Some(window_id) = self.focused_window(current_ws) {
            self.move_child_to_workspace_with_id(window_id, target_ws);
        } else if self.strategy.has_tiling_windows(&self.access, current_ws) {
            // Container highlighted: bypass focused_window() and move directly.
            tracing::debug!(?current_ws, ?target_ws, "Moving container to monitor");
            self.strategy
                .move_focused_to_workspace(&mut self.access, current_ws, target_ws);
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn set_focus(&mut self, window_id: WindowId) {
        if self.access.windows.get(window_id).mode == DisplayMode::Minimized {
            self.unminimize_window(window_id);
            return;
        }
        tracing::debug!(%window_id, "Setting focus to window");
        let window = self.access.windows.get(window_id);
        let ws = window.workspace;
        match window.mode {
            DisplayMode::Fullscreen => {
                let fs = &mut self.access.workspaces.get_mut(ws).fullscreen_windows;
                if let Some(pos) = fs.iter().position(|&w| w == window_id) {
                    fs.remove(pos);
                    fs.push(window_id);
                }
                self.access.workspaces.get_mut(ws).is_float_focused = false;
            }
            DisplayMode::Float => {
                self.focus_float(ws, window_id);
            }
            DisplayMode::Tiling => {
                self.strategy.set_focus(&mut self.access, window_id);
            }
            DisplayMode::Minimized => unreachable!("guarded above"),
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
        self.strategy.tiling_window_count(&self.access, ws_id)
            + ws.float_windows.len()
            + ws.fullscreen_windows.len()
    }

    #[cfg(test)]
    pub(super) fn all_monitors(&self) -> Vec<(MonitorId, Monitor)> {
        self.access.monitors.all_active()
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
            .allocate(Workspace::new(name, monitor_id));
        self.access.monitors.get_mut(monitor_id).active_workspace = ws_id;
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
            self.strategy.layout_workspace(&mut self.access, ws_id);
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
            self.strategy.layout_workspace(&mut self.access, ws_id);
        }
    }

    pub(crate) fn sync_config(&mut self, config: HubConfig) {
        // A full strategy rebuild is needed only when the active layout kind
        // changes (partition-tree <-> master-stack). Scalar param changes
        // (master_ratio, master_count) are pushed into the running strategy
        // via apply_config, preserving per-workspace window ordering and focus.
        let rebuild = self.access.config.layout.strategy != config.layout.strategy;
        self.access.config = config;

        if !rebuild {
            self.strategy.apply_config(&mut self.access);
            return;
        }

        // Collect IDs first to avoid borrowing self.access.workspaces while
        // passing &mut self.access to the strategy.
        let ws_ids: Vec<WorkspaceId> = self
            .access
            .workspaces
            .all_active()
            .iter()
            .map(|(id, _)| *id)
            .collect();

        // Rebuild path. Entered only when `layout.strategy` changed (e.g.
        // partition-tree <-> master-stack). Strategies differ in tree topology
        // (partition-tree uses containers and tabs; master-stack uses a flat
        // ordered list), so there is no meaningful cross-strategy migration.
        // Consequences:
        //   - Container groupings, tabbed containers, and split directions
        //     from partition-tree are lost.
        //   - Master-stack window order within master/stack areas is reset.
        //   - Runtime-tuned master-stack params (GrowMaster / ShrinkMaster /
        //     MoreMaster / FewerMaster actions) are wiped because the new
        //     strategy is built fresh from config.
        // Focus is preserved per workspace: the previously-focused tiling
        // window is restored via set_focus, and `is_float_focused` is
        // restored inline after the strategy calls that would otherwise clear
        // it. Fullscreen, float, and minimized windows are untouched: their
        // placement is managed by Hub, not the strategy.

        // Snapshot each workspace's tiling windows, previous tiling focus,
        // and float-focus flag before the old strategy is dropped.
        let mut snapshots: Vec<(WorkspaceId, Vec<WindowId>, Option<WindowId>, bool)> = Vec::new();
        for ws_id in &ws_ids {
            let tiling_windows: Vec<WindowId> = self
                .access
                .windows
                .all_active()
                .iter()
                .filter(|(_, w)| w.mode == DisplayMode::Tiling && w.workspace == *ws_id)
                .map(|(id, _)| *id)
                .collect();
            let prev_focus = self.strategy.focused_tiling_window(&self.access, *ws_id);
            let was_float_focused = self.access.workspaces.get(*ws_id).is_float_focused;
            snapshots.push((*ws_id, tiling_windows, prev_focus, was_float_focused));
        }

        // Replace the strategy. The old Box<dyn TilingStrategy> is dropped,
        // deallocating all per-workspace state (both PartitionTreeStrategy
        // and MasterStrategy are pure owned-data structs).
        self.strategy = build_strategy(&self.access.config.layout);

        // Re-attach tiling windows in WindowId-ascending order (creation
        // order, the canonical deterministic ordering).
        for (ws_id, wids, prev_focus, was_float_focused) in snapshots {
            for wid in &wids {
                self.strategy.attach_window(&mut self.access, *wid, ws_id);
            }
            if let Some(f) = prev_focus {
                self.strategy.set_focus(&mut self.access, f);
            }
            // Restore is_float_focused after all strategy calls that clear it.
            // The Workspace invariant (flag=true => float_windows non-empty)
            // holds because the rebuild never mutates float_windows.
            if was_float_focused {
                self.access.workspaces.get_mut(ws_id).is_float_focused = true;
            }
        }
    }

    #[cfg(test)]
    pub(super) fn all_workspaces(&self) -> Vec<(WorkspaceId, Workspace)> {
        self.access.workspaces.all_active()
    }

    #[cfg(test)]
    pub(crate) fn validate_tree(&self) {
        self.strategy.validate_tree(&self.access);
    }

    #[cfg(test)]
    pub(crate) fn minimized_windows(&self) -> &[WindowId] {
        &self.minimized_windows
    }

    #[cfg(test)]
    pub(crate) fn focused_tiling_window(&self, ws_id: WorkspaceId) -> Option<WindowId> {
        self.strategy.focused_tiling_window(&self.access, ws_id)
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "used in test validators"))]
    pub(crate) fn get_workspace(&self, id: WorkspaceId) -> &Workspace {
        self.access.workspaces.get(id)
    }

    pub(crate) fn get_window(&self, id: WindowId) -> &Window {
        self.access.windows.get(id)
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

                let tiling = self.strategy.collect_tiling_placements(
                    &self.access,
                    ws_id,
                    ws_id == current_ws,
                );
                let tiling_windows = tiling.windows;
                let containers = tiling.containers;

                let focused = if ws_id == current_ws {
                    self.focused_window(ws_id)
                } else {
                    None
                };

                let mut float_windows = Vec::new();
                for &(id, dim) in &ws.float_windows {
                    // Float dimensions are already screen-absolute (stored in the workspace
                    // tuple), so no translate() call needed. clip() works because both dim
                    // and screen are in absolute screen coordinates.
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

    /// Insert a new window as tiling to the current workspace.
    /// Update workspace focus to the newly inserted window.
    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_tiling(&mut self) -> WindowId {
        let current_ws = self.current_workspace();
        let window_id = self.access.windows.allocate(Window::tiling(current_ws));
        self.strategy
            .attach_window(&mut self.access, window_id, current_ws);
        window_id
    }

    /// Insert a new window as float to the current workspace.
    /// Update workspace focus to the newly inserted window.
    #[cfg_attr(
        all(target_os = "macos", not(test)),
        expect(dead_code, reason = "used on Windows")
    )]
    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_float(&mut self, dimension: Dimension) -> WindowId {
        let current_ws = self.current_workspace();
        let window_id = self.access.windows.allocate(Window::float(current_ws));
        tracing::debug!("Inserting float window {window_id} with dimension {dimension:?}");
        self.attach_float_to_workspace(current_ws, window_id, dimension);
        window_id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_fullscreen(&mut self, restrictions: WindowRestrictions) -> WindowId {
        let current_ws = self.current_workspace();
        let window_id = self
            .access
            .windows
            .allocate(Window::fullscreen(current_ws, restrictions));
        self.attach_fullscreen_to_workspace(current_ws, window_id);
        self.set_focus(window_id);
        window_id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn delete_window(&mut self, id: WindowId) {
        let window = self.access.windows.get(id);
        let ws = window.workspace;
        let mode = window.mode;
        match mode {
            DisplayMode::Float => {
                let _dim = self.detach_float_from_workspace(id);
            }
            DisplayMode::Fullscreen => self.detach_fullscreen_from_workspace(id),
            DisplayMode::Tiling => {
                self.strategy.detach_window(&mut self.access, id);
            }
            DisplayMode::Minimized => {
                self.minimized_windows.retain(|&w| w != id);
            }
        }
        // Minimized windows have a stale workspace field (the workspace may
        // have been pruned already), so skip prune_workspace for them.
        if mode != DisplayMode::Minimized {
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
                    tracing::debug!(window_id = %window_id, "{name}: existing max {:.2} < new min {:.2}, raising max", *max, new_min);
                    *max = new_min;
                }
            }
            if let Some(new_max) = new_max {
                *max = if new_max > 0.0 { new_max } else { 0.0 };
                if *max > 0.0 && *min > *max {
                    tracing::debug!(window_id = %window_id, "{name}: existing min {:.2} > new max {:.2}, lowering min", *min, *max);
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

        tracing::debug!(%window_id, ?min_width, ?min_height, ?max_width, ?max_height, "Window constraint set");

        let mode = window.mode;
        let workspace_id = window.workspace;
        // Minimized windows have a stale workspace field, so skip relayout.
        if mode != DisplayMode::Minimized {
            self.strategy
                .layout_workspace(&mut self.access, workspace_id);
        }
    }

    /// Move a window to a target workspace. For tiling windows, delegates to
    /// strategy.move_focused_to_workspace which handles both window and container
    /// moves. For fullscreen/float, moves the specific window.
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
        match window.mode {
            DisplayMode::Fullscreen => {
                self.detach_fullscreen_from_workspace(window_id);
                self.attach_fullscreen_to_workspace(target_ws, window_id);
                self.access.workspaces.get_mut(target_ws).is_float_focused = false;
            }
            DisplayMode::Float => {
                let dim = self.detach_float_from_workspace(window_id);
                self.attach_float_to_workspace(target_ws, window_id, dim);
            }
            DisplayMode::Tiling => {
                self.strategy
                    .move_focused_to_workspace(&mut self.access, current_ws, target_ws);
            }
            DisplayMode::Minimized => return,
        }

        tracing::debug!(?window_id, ?target_ws, "Moved to workspace");
    }

    pub(super) fn get_or_create_workspace(&mut self, name: &str) -> WorkspaceId {
        match self.access.workspaces.find(|w| w.name == name) {
            Some(id) => id,
            None => self.access.workspaces.allocate(Workspace::new(
                name.to_string(),
                self.access.focused_monitor,
            )),
        }
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
}

#[derive(Debug, Clone)]
pub(crate) struct HubConfig {
    pub(super) layout: LayoutConfig,
    pub(super) min_width: SizeConstraint,
    pub(super) min_height: SizeConstraint,
    pub(super) max_width: SizeConstraint,
    pub(super) max_height: SizeConstraint,
}

impl From<crate::config::Config> for HubConfig {
    fn from(config: crate::config::Config) -> Self {
        Self {
            layout: config.layout,
            min_width: config.min_width,
            min_height: config.min_height,
            max_width: config.max_width,
            max_height: config.max_height,
        }
    }
}

fn build_strategy(layout: &LayoutConfig) -> Box<dyn TilingStrategy> {
    match layout.strategy {
        Strategy::PartitionTree => Box::new(PartitionTreeStrategy::new()),
        Strategy::Master => Box::new(MasterStrategy::new(
            layout.master.master_ratio,
            layout.master.master_count,
        )),
    }
}
