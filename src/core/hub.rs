use crate::action::MonitorTarget;
use crate::config::{
    Config, LayoutWorkspaceConfig, MasterConfig, PartitionTreeConfig, SizeConstraints, Strategy,
    WindowMatcher, WindowMode,
};

use super::allocator::{Allocator, NodeId};
use super::matcher::{FloatFullscreenMatcherId, MatcherHit};
use super::node::{
    ContainerId, Dimension, DisplayMode, Length, LimitObservation, LimitUpdate, Logical, Monitor,
    MonitorId, PixelRect, Pixels, Unit, Window, WindowId, WindowMetadata, WindowRestrictions,
    Workspace, WorkspaceId,
};
use super::partition_tree::Child;
use super::strategy::{StrategySet, TilingAction, WorkspaceExport};

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
    pub(crate) border_box: PixelRect,
    pub(crate) visible_border_box: PixelRect,
    pub(crate) content_box: PixelRect,
    /// `content_box` trimmed to the monitor. Zero-area when nothing remains.
    #[cfg_attr(
        target_os = "windows",
        expect(
            dead_code,
            reason = "macOS trims tiling placements to the work area, Windows places them unclipped"
        )
    )]
    pub(crate) visible_content_box: PixelRect,
    /// Whether to highlight the window, for example when the window is focused. Doesn't require
    /// that the window has keyboard focus.
    pub(crate) is_highlighted: bool,
    pub(crate) spawn_indicator: Option<SpawnIndicator>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FloatWindowPlacement {
    pub(crate) id: WindowId,
    pub(crate) border_box: PixelRect,
    pub(crate) visible_border_box: PixelRect,
    pub(crate) content_box: PixelRect,
    pub(crate) is_highlighted: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ContainerPlacement {
    pub(crate) id: ContainerId,
    pub(crate) border_box: PixelRect,
    pub(crate) visible_border_box: PixelRect,
    pub(crate) is_highlighted: bool,
    pub(crate) spawn_indicator: Option<SpawnIndicator>,
    pub(crate) is_tabbed: bool,
    pub(crate) active_tab_index: usize,
    pub(crate) titles: Vec<String>,
}

pub(crate) struct MonitorPlacements {
    pub(crate) monitor_id: MonitorId,
    /// Resolved once per monitor. Emitted so the overlay paints the exact gap the
    /// inset left rather than re-deriving it from config.
    pub(crate) border_thickness: Pixels<Unit>,
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

/// Convenience bundle of the global layout fields from Config.
/// Hub and strategies use this instead of threading 9 separate fields.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GlobalLayoutConfig {
    pub(crate) strategy: Strategy,
    pub(crate) border_size: Length<Logical>,
    pub(crate) partition_tree: PartitionTreeConfig,
    pub(crate) master: MasterConfig,
    pub(crate) size_constraints: SizeConstraints,
    pub(crate) float: Vec<WindowMatcher>,
    pub(crate) fullscreen: Vec<WindowMatcher>,
    pub(crate) ignore: Vec<WindowMatcher>,
}

impl From<&Config> for GlobalLayoutConfig {
    fn from(c: &Config) -> Self {
        Self {
            strategy: c.strategy,
            border_size: c.border_size,
            partition_tree: c.partition_tree.clone(),
            master: c.master.clone(),
            size_constraints: c.size_constraints,
            float: c.float.clone(),
            fullscreen: c.fullscreen.clone(),
            ignore: c.ignore.clone(),
        }
    }
}

impl Default for GlobalLayoutConfig {
    fn default() -> Self {
        Self {
            strategy: Strategy::PartitionTree,
            border_size: Length::<Logical>::new(4.0),
            partition_tree: PartitionTreeConfig {
                tab_bar_height: Length::<Logical>::new(24.0),
                automatic_tiling: true,
            },
            master: MasterConfig {
                master_ratio: 0.5,
                master_count: 1,
            },
            size_constraints: SizeConstraints::default(),
            float: Vec::new(),
            fullscreen: Vec::new(),
            ignore: Vec::new(),
        }
    }
}

/// Non-strategy fields of Hub, extracted so that `TilingStrategy` methods can
/// receive `&mut HubAccess` while Hub holds `&mut strategy` separately. This
/// solves the split-borrow problem: strategy and access are disjoint fields.
#[derive(Debug)]
pub(crate) struct HubAccess {
    pub(super) monitors: Allocator<Monitor>,
    pub(super) focused_monitor: MonitorId,
    pub(super) layout: GlobalLayoutConfig,
    pub(super) preferred_layouts: Vec<LayoutWorkspaceConfig>,
    pub(super) workspaces: Allocator<Workspace>,
    pub(super) windows: Allocator<Window>,
}

impl HubAccess {
    /// Rounding here rather than at the call sites is what makes
    /// `border_box - content_box` exactly the thickness on every edge: a
    /// thickness ending in `.5` would otherwise round the two opposite edges
    /// apart by a pixel.
    pub(super) fn border(&self, monitor: MonitorId) -> Pixels<Unit> {
        Pixels::round(
            self.layout
                .border_size
                .to_unit(self.monitors.get(monitor).scale),
        )
    }
}

#[derive(Debug)]
pub(crate) struct Hub {
    pub(super) access: HubAccess,
    pub(super) strategies: StrategySet,
    pub(super) minimized_windows: Vec<WindowId>,
    pub(super) float_fullscreen_matchers: Allocator<WindowMatcher>,
    pub(super) global_float_matchers: Vec<FloatFullscreenMatcherId>,
    pub(super) global_fullscreen_matchers: Vec<FloatFullscreenMatcherId>,
}

impl Hub {
    pub(crate) fn new(
        primary_screen: PixelRect,
        primary_scale: f32,
        layout: GlobalLayoutConfig,
        preferred_layouts: Vec<LayoutWorkspaceConfig>,
    ) -> Self {
        let strategies = StrategySet::new(&layout);

        let mut hub = Self {
            access: HubAccess {
                monitors: Allocator::new(),
                // Placeholder id. will be changed after inserting primary monitor
                focused_monitor: MonitorId::new(0),
                layout,
                preferred_layouts,
                workspaces: Allocator::new(),
                windows: Allocator::new(),
            },
            strategies,
            minimized_windows: Vec::new(),
            float_fullscreen_matchers: Allocator::new(),
            global_float_matchers: Vec::new(),
            global_fullscreen_matchers: Vec::new(),
        };

        let primary_id = hub.add_monitor("primary".to_string(), primary_screen, primary_scale);
        hub.access.focused_monitor = primary_id;
        let preferred = hub.access.preferred_layouts.clone();
        hub.index_matchers(&preferred);
        hub
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
    pub(crate) fn focused_window(&self, ws_id: WorkspaceId) -> Option<WindowId> {
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
            .focused_tiling_window(ws_id)
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
            self.move_focused_across_workspaces(current_ws, target_ws);
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn set_focus(&mut self, window_id: WindowId) {
        tracing::debug!("Setting focus to window");
        let ws = self
            .access
            .windows
            .get(window_id)
            .workspace()
            .expect("non-minimized window has a workspace");
        self.set_workspace_focus(window_id);
        self.focus_workspace_with_id(ws);
    }

    /// Focus `window_id` within its own workspace, without switching workspace.
    pub(super) fn set_workspace_focus(&mut self, window_id: WindowId) {
        let window = self.access.windows.get(window_id);
        let ws = window
            .workspace()
            .expect("non-minimized window has a workspace");
        match window.mode {
            DisplayMode::Fullscreen { .. } => {
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
                self.access.workspaces.get_mut(ws).is_float_focused = false;
                self.strategies
                    .for_workspace_mut(ws)
                    .set_focus(&mut self.access, window_id);
            }
        }
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
    /// (creation order). Workspaces persist for the lifetime of the Hub once
    /// created, so emptied workspaces continue to appear with `window_count == 0`.
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
            .tiling_window_count(ws_id);
        tiling_count + ws.float_windows.len() + ws.fullscreen_windows.len()
    }

    pub(crate) fn export_workspace(&mut self, ws_id: WorkspaceId) -> WorkspaceExport {
        let ws_name = self.access.workspaces.get(ws_id).name.clone();
        let mut export = self
            .strategies
            .for_workspace_mut(ws_id)
            .export_workspace(&self.access, ws_id);

        let ws = self.access.workspaces.get(ws_id);
        let float_windows: Vec<WindowId> = ws.float_windows.clone();
        let fullscreen_windows: Vec<WindowId> = ws.fullscreen_windows.clone();

        let float = self.collect_display_matchers(&float_windows, |mode| match mode {
            DisplayMode::Float { occupy, .. } => *occupy,
            _ => None,
        });
        let fullscreen = self.collect_display_matchers(&fullscreen_windows, |mode| match mode {
            DisplayMode::Fullscreen { occupy } => *occupy,
            _ => None,
        });

        export.float = float;
        export.fullscreen = fullscreen;

        let config = export.to_layout_workspace_config(&ws_name);
        self.access
            .preferred_layouts
            .retain(|e| e.name() != ws_name);
        self.access.preferred_layouts.push(config);

        export
    }

    pub(crate) fn add_monitor(
        &mut self,
        name: String,
        work_area: PixelRect,
        scale: f32,
    ) -> MonitorId {
        let monitor_id = self.access.monitors.allocate(Monitor {
            name: name.clone(),
            work_area,
            scale,
            active_workspace: WorkspaceId::new(0),
        });
        // FIXME: each monitor have a dedicated set of workspaces, might be sharing the same name with the primary monitor
        let workspace_name = if name == "primary" {
            "0".to_string()
        } else {
            name.clone()
        };
        let ws_id = self
            .access
            .workspaces
            // Placeholder id. will be changed after inserting primary monitor
            .allocate(Workspace::new(workspace_name.clone(), monitor_id));
        self.access.monitors.get_mut(monitor_id).active_workspace = ws_id;
        let preferred_layout = self
            .access
            .preferred_layouts
            .iter()
            .find(|w| w.name() == workspace_name);
        self.strategies
            .register(ws_id, &self.access.layout, preferred_layout);
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
                .compute_placement(&self.access, ws_id);
        }

        if self.access.focused_monitor == monitor_id {
            self.access.focused_monitor = fallback_id;
        }
        self.access.monitors.delete(monitor_id);
    }

    pub(crate) fn update_monitor(
        &mut self,
        monitor_id: MonitorId,
        work_area: PixelRect,
        scale: f32,
    ) {
        let monitor = self.access.monitors.get_mut(monitor_id);
        monitor.work_area = work_area;
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
                .compute_placement(&self.access, ws_id);
        }
    }

    pub(crate) fn sync_configuration(&mut self, layout: GlobalLayoutConfig) {
        self.access.layout = layout.clone();
        for (ws_id, _) in self.access.workspaces.all_active() {
            self.strategies
                .for_workspace_mut(ws_id)
                .apply_config(&mut self.access, layout.clone());
        }
        let preferred_layouts = self.access.preferred_layouts.clone();

        // Change the strategy of workspages without preferred layout
        self.strategies
            .resync(&mut self.access, &preferred_layouts, layout.strategy);

        self.index_matchers(&preferred_layouts);
    }

    pub(crate) fn sync_preferred_layout(&mut self, preferred_layouts: Vec<LayoutWorkspaceConfig>) {
        self.index_matchers(&preferred_layouts);
        let default_strategy = self.access.layout.strategy;
        self.strategies
            .resync(&mut self.access, &preferred_layouts, default_strategy);
        self.access.preferred_layouts = preferred_layouts;
    }

    #[cfg(test)]
    pub(crate) fn validate(&self) {
        self.strategies.validate(&self.access);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_window(
        &mut self,
        metadata: Box<dyn WindowMetadata>,
        dimension: Dimension,
        restrictions: WindowRestrictions,
    ) -> Option<WindowId> {
        if let Some(r) = self
            .access
            .layout
            .ignore
            .iter()
            .find(|r| metadata.matches_window_matcher(r))
        {
            tracing::debug!("Window ignored by rule {r:?}");
            return None;
        }
        let matcher = self.resolve_matcher(&*metadata);
        let target_ws = matcher
            .as_ref()
            .and_then(|hit| hit.ws_id)
            .unwrap_or_else(|| self.current_workspace());

        let (mode, restrictions, occupy_id) = if restrictions == WindowRestrictions::None {
            match matcher {
                Some(MatcherHit {
                    mode, matcher_id, ..
                }) => (mode, restrictions, matcher_id),
                None => (WindowMode::Tiling, restrictions, None),
            }
        } else {
            // Restrictions force fullscreen, so the matcher only routes the
            // workspace here, and a restricted window never picks up an occupy.
            (WindowMode::Fullscreen, restrictions, None)
        };

        let window_id = match mode {
            WindowMode::Tiling => {
                let window_id = self
                    .access
                    .windows
                    .allocate(Window::tiling(target_ws, metadata));
                self.strategies.for_workspace_mut(target_ws).attach_window(
                    &mut self.access,
                    window_id,
                    target_ws,
                );
                self.set_focus(window_id);
                window_id
            }
            WindowMode::Float => {
                let window_id = self
                    .access
                    .windows
                    .allocate(Window::float(target_ws, dimension, metadata));
                tracing::debug!(%window_id, ?dimension, "Inserting float window");
                // `occupy_id` links the window back to the matcher that routed it.
                self.attach_float_to_workspace(target_ws, window_id, dimension, occupy_id);
                self.set_focus(window_id);
                window_id
            }
            WindowMode::Fullscreen => {
                let window_id = self.access.windows.allocate(Window::fullscreen(
                    target_ws,
                    restrictions,
                    metadata,
                ));
                self.attach_fullscreen_to_workspace(target_ws, window_id, occupy_id);
                self.set_focus(window_id);
                window_id
            }
        };

        Some(window_id)
    }

    pub(crate) fn set_window_title(&mut self, window_id: WindowId, title: String) -> bool {
        let window = self.access.windows.get_mut(window_id);
        if window.metadata.title() == Some(&title) {
            return false;
        }
        window.metadata.set_title(title);
        true
    }

    pub(crate) fn get_visible_placements(&self) -> VisiblePlacements {
        let current_ws = self.current_workspace();

        let monitors = self
            .visible_workspaces()
            .into_iter()
            .map(|ws_id| {
                let ws = self.access.workspaces.get(ws_id);
                let screen = self.access.monitors.get(ws.monitor).work_area;

                // Fullscreen: only return topmost, skip tiling/float
                if let Some(&fs_id) = ws.fullscreen_windows.last() {
                    return MonitorPlacements {
                        monitor_id: ws.monitor,
                        border_thickness: self.access.border(ws.monitor),
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

                let border = self.access.border(ws.monitor);
                let mut float_windows = Vec::new();
                for &id in &ws.float_windows {
                    let window = self.access.windows.get(id);
                    let DisplayMode::Float { dim, .. } = window.mode else {
                        panic!("window {id} in float_windows but mode is not Float");
                    };
                    let border_box = PixelRect::from_dimension(dim);
                    if let Some(visible_border_box) = border_box.clip(screen) {
                        let is_highlighted = focused == Some(id);
                        float_windows.push(FloatWindowPlacement {
                            id,
                            border_box,
                            visible_border_box,
                            content_box: border_box.inset_by(border),
                            is_highlighted,
                        });
                    }
                }

                MonitorPlacements {
                    monitor_id: ws.monitor,
                    border_thickness: border,
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
    pub(crate) fn delete_window(&mut self, id: WindowId) {
        let window = self.access.windows.get(id);
        let is_minimized = window.is_minimized();
        let mode = window.mode;

        if is_minimized {
            self.minimized_windows.retain(|&w| w != id);
        } else {
            let ws_id = window
                .workspace()
                .expect("non-minimized window has a workspace");
            match mode {
                DisplayMode::Float { .. } => {
                    self.detach_float_from_workspace(id);
                }
                DisplayMode::Fullscreen { .. } => self.detach_fullscreen_from_workspace(id),
                DisplayMode::Tiling => {
                    let strategy = self.strategies.for_workspace_mut(ws_id);
                    strategy.detach_window(&self.access, id);
                    if strategy.tiling_window_count(ws_id) == 0 {
                        let ws = self.access.workspaces.get_mut(ws_id);
                        if ws.fullscreen_windows.is_empty() {
                            ws.is_float_focused = !ws.float_windows.is_empty();
                        }
                    }
                }
            }
        }

        self.access.windows.delete(id);
    }

    #[tracing::instrument(skip(self))]
    /// If setting min above existing max, max is raised to match min.
    pub(crate) fn set_window_constraint(
        &mut self,
        window_id: WindowId,
        observed: LimitObservation,
    ) {
        let window = self.access.windows.get_mut(window_id);

        let update = |name: &str,
                      min: &mut Option<Length<Unit>>,
                      max: &mut Option<Length<Unit>>,
                      new_min: LimitUpdate,
                      new_max: LimitUpdate| {
            match new_min {
                LimitUpdate::Unchanged => {}
                LimitUpdate::Cleared => *min = None,
                LimitUpdate::Set(new_min) => {
                    *min = Some(new_min);
                    if max.is_some_and(|m| m < new_min) {
                        tracing::debug!(
                            "{name}: existing max {:.2} < new min {:.2}, raising max",
                            max.unwrap_or(Length::ZERO).value(),
                            new_min.value()
                        );
                        *max = Some(new_min);
                    }
                }
            }
            match new_max {
                LimitUpdate::Unchanged => {}
                LimitUpdate::Cleared => *max = None,
                LimitUpdate::Set(new_max) => {
                    *max = Some(new_max);
                    if min.is_some_and(|m| m > new_max) {
                        tracing::debug!(
                            "{name}: existing min {:.2} > new max {:.2}, lowering min",
                            min.unwrap_or(Length::ZERO).value(),
                            new_max.value()
                        );
                        *min = Some(new_max);
                    }
                }
            }
        };

        update(
            "width",
            &mut window.limits.min_width,
            &mut window.limits.max_width,
            observed.min_width,
            observed.max_width,
        );
        update(
            "height",
            &mut window.limits.min_height,
            &mut window.limits.max_height,
            observed.min_height,
            observed.max_height,
        );

        tracing::debug!("Window constraint set");

        if let Some(ws) = window.workspace() {
            self.strategies
                .for_workspace_mut(ws)
                .compute_placement(&self.access, ws);
        }
    }

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
            DisplayMode::Fullscreen { .. } => {
                self.detach_fullscreen_from_workspace(window_id);
                self.attach_fullscreen_to_workspace(target_ws, window_id, None);
                self.access.workspaces.get_mut(target_ws).is_float_focused = false;
            }
            DisplayMode::Float { .. } => {
                // Cross-workspace hop: drop occupy so the destination does not
                // export the origin workspace's authored matcher.
                let dim = self.detach_float_from_workspace(window_id);
                self.attach_float_to_workspace(target_ws, window_id, dim, None);
            }
            DisplayMode::Tiling => {
                self.move_focused_across_workspaces(current_ws, target_ws);
            }
        }

        tracing::debug!("Moved to workspace");
    }

    pub(super) fn get_or_create_workspace(&mut self, name: &str) -> WorkspaceId {
        if let Some(id) = self.access.workspaces.find(|w| w.name == name) {
            return id;
        }
        let ws_id = self.access.workspaces.allocate(Workspace::new(
            name.to_string(),
            self.access.focused_monitor,
        ));
        let preferred_layout = self
            .access
            .preferred_layouts
            .iter()
            .find(|w| w.name() == name);
        self.strategies
            .register(ws_id, &self.access.layout, preferred_layout);
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
                let current = self
                    .access
                    .monitors
                    .get(self.access.focused_monitor)
                    .work_area;
                // Doubled centres, so an odd extent does not lose half a unit to integer
                // division. Only differences of centres are used, so the factor cancels.
                let cx2 = 2 * current.x() + current.width();
                let cy2 = 2 * current.y() + current.height();

                self.access
                    .monitors
                    .all_active()
                    .iter()
                    .filter(|(id, _)| *id != self.access.focused_monitor)
                    .filter_map(|(id, m)| {
                        let m = m.work_area;
                        let dx = 2 * m.x() + m.width() - cx2;
                        let dy = 2 * m.y() + m.height() - cy2;

                        let valid = match direction {
                            MonitorTarget::Left => dx < Pixels::ZERO,
                            MonitorTarget::Right => dx > Pixels::ZERO,
                            MonitorTarget::Up => dy < Pixels::ZERO,
                            MonitorTarget::Down => dy > Pixels::ZERO,
                            MonitorTarget::Name(_) => false,
                        };
                        let dx = i64::from(dx.value());
                        let dy = i64::from(dy.value());
                        valid.then_some((*id, dx * dx + dy * dy))
                    })
                    .min_by_key(|(_, dist_sq)| *dist_sq)
                    .map(|(id, _)| id)
            }
        }
    }

    pub(super) fn move_focused_across_workspaces(&mut self, from: WorkspaceId, to: WorkspaceId) {
        let strategy = self.strategies.for_workspace_mut(from);
        let child = strategy.detach_focused_child(&self.access, from);
        let Some(child) = child else {
            return;
        };
        if strategy.tiling_window_count(from) == 0 {
            let ws = self.access.workspaces.get_mut(from);
            if ws.fullscreen_windows.is_empty() {
                ws.is_float_focused = !ws.float_windows.is_empty();
            }
        }
        self.strategies
            .for_workspace_mut(to)
            .reattach_child(&mut self.access, child, to);
        if let Child::Window(window_id) = child {
            self.set_workspace_focus(window_id);
        }
    }
}
