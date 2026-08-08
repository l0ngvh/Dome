use crate::config::{
    Config, LayoutWorkspaceConfig, MasterConfig, PartitionTreeConfig, SizeConstraints, Strategy,
    WindowMatcher, WindowMode, default_border_size, default_master_config,
    default_partition_tree_config, default_strategy,
};

use super::allocator::{Allocator, NodeId};
use super::matcher::{FloatFullscreenMatcherId, MatcherHit};
use super::monitor::Monitor;
use super::node::{
    Container, ContainerId, DisplayMode, Length, LimitObservation, LimitUpdate, Logical, MonitorId,
    PixelRect, Pixels, Unit, Window, WindowId, WindowMetadata, WindowRestrictions, WorkspaceId,
};
use super::partition_tree::Child;
use super::strategy::{StrategySet, TilingAction, WorkspaceExport};
use super::workspace::{Attachment, Workspace};

pub(crate) struct VisiblePlacements {
    pub(crate) focused_window: Option<WindowId>,
    pub(crate) focused_monitor: MonitorId,
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
    /// Highlighting does not require keyboard focus.
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
    /// Top band of `border_box` reserved for the tab strip, zero-height when the container
    /// is not tabbed.
    pub(crate) tab_bar_band: PixelRect,
    pub(crate) is_highlighted: bool,
    pub(crate) spawn_indicator: Option<SpawnIndicator>,
    pub(crate) is_tabbed: bool,
    pub(crate) active_tab_index: usize,
    pub(crate) titles: Vec<String>,
}

pub(crate) struct MonitorPlacements {
    pub(crate) monitor_id: MonitorId,
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
/// `left` is always false today but included so we don't need a struct change
/// if a future spawn mode uses it.
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
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GlobalLayoutConfig {
    pub(crate) strategy: Strategy,
    pub(crate) border_size: Pixels<Logical>,
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
            strategy: default_strategy(),
            border_size: default_border_size(),
            partition_tree: default_partition_tree_config(),
            master: default_master_config(),
            size_constraints: SizeConstraints::default(),
            // Empty rather than `Config::default()`'s bundled matcher lists, so a fixture
            // manages every window it inserts.
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
    pub(super) containers: Allocator<Container>,
}

impl HubAccess {
    pub(super) fn allocate_container(&mut self, container: Container) -> ContainerId {
        self.containers.allocate(container)
    }

    pub(super) fn free_container(&mut self, id: ContainerId) {
        self.containers.delete(id);
    }

    pub(super) fn containers_preorder(&self, root: ContainerId) -> Vec<ContainerId> {
        let mut stack = vec![root];
        let mut order = Vec::new();
        for _ in super::bounded_loop() {
            let Some(id) = stack.pop() else { break };
            order.push(id);
            for &child in &self.containers.get(id).children {
                if let Child::Container(child_id) = child {
                    stack.push(child_id);
                }
            }
        }
        order
    }

    pub(super) fn children_dfs(&self, root: Child) -> Vec<Child> {
        let mut stack = vec![root];
        let mut order = Vec::new();
        for _ in super::bounded_loop() {
            let Some(child) = stack.pop() else { break };
            order.push(child);
            if let Child::Container(cid) = child {
                for &c in &self.containers.get(cid).children {
                    stack.push(c);
                }
            }
        }
        order
    }

    pub(super) fn take_windows(&mut self, root: Child) -> Vec<WindowId> {
        let mut windows = Vec::new();
        for child in self.children_dfs(root) {
            match child {
                Child::Window(wid) => windows.push(wid),
                Child::Container(cid) => self.free_container(cid),
            }
        }
        windows
    }
}

impl HubAccess {
    /// Rounding here rather than at the call sites is what makes
    /// `border_box - content_box` exactly the thickness on every edge: a
    /// thickness ending in `.5` would otherwise round the two opposite edges
    /// apart by a pixel.
    pub(super) fn border(&self, monitor: MonitorId) -> Pixels<Unit> {
        Pixels::round(
            Length::from_pixels(self.layout.border_size).to_unit(self.monitors.get(monitor).scale),
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
                containers: Allocator::new(),
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

    /// Single entry point for tiling actions.
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
    pub(crate) fn query_workspaces(&self) -> Vec<crate::action::WorkspaceInfo> {
        let focused_ws = self.current_workspace();
        let visible: Vec<WorkspaceId> = self.visible_workspaces();
        self.access
            .workspaces
            .all_active()
            .into_iter()
            .map(|(ws_id, ws)| {
                let (monitor, state) = match &ws.attachment {
                    Attachment::Attached => (
                        self.access.monitors.get(ws.monitor).unique_name.clone(),
                        crate::action::WorkspaceState::Attached,
                    ),
                    Attachment::Parked { origin } => {
                        (origin.clone(), crate::action::WorkspaceState::Parked)
                    }
                };
                crate::action::WorkspaceInfo {
                    name: ws.name.clone(),
                    monitor,
                    state,
                    is_focused: ws_id == focused_ws,
                    is_visible: visible.contains(&ws_id),
                    window_count: self.count_workspace_windows(ws_id, &ws),
                }
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

    pub(crate) fn sync_configuration(&mut self, layout: GlobalLayoutConfig) {
        self.access.layout = layout.clone();
        for (ws_id, _) in self.access.workspaces.all_active() {
            self.strategies
                .for_workspace_mut(ws_id)
                .apply_config(&mut self.access, layout.clone());
        }
        let preferred_layouts = self.access.preferred_layouts.clone();

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
        rect: PixelRect,
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
                    .allocate(Window::float(target_ws, rect, metadata));
                tracing::debug!(%window_id, ?rect, "Inserting float window");
                self.attach_float_to_workspace(target_ws, window_id, rect, occupy_id);
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
                    let DisplayMode::Float { border_box, .. } = window.mode else {
                        panic!("window {id} in float_windows but mode is not Float");
                    };
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
                    strategy.detach_window(&mut self.access, id);
                    if strategy.tiling_window_count(&self.access, ws_id) == 0 {
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
}
