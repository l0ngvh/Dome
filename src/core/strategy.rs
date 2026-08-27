use rustc_hash::FxHashMap;
#[cfg(test)]
use rustc_hash::FxHashSet;

use crate::config::{
    LayoutWorkspaceConfig, PaneConfig, SizeConstraints, Strategy, TreeLayoutNode, WindowMatcher,
};
use crate::core::GlobalLayoutConfig;
use crate::core::hub::{ContainerPlacement, HubAccess, TilingWindowPlacement};
use crate::core::master::MasterStrategy;
use crate::core::node::{
    Child, Constraints, ContainerId, Dimension, Direction, Length, PixelRect, Pixels, Unit,
    WindowId, WindowMetadata, WorkspaceId,
};
use crate::core::partition_tree::PartitionTreeStrategy;

#[derive(Debug)]
pub(crate) enum TilingAction {
    FocusDirection {
        direction: Direction,
        forward: bool,
    },
    MoveDirection {
        direction: Direction,
        forward: bool,
    },
    ToggleSpawnMode,
    ToggleDirection,
    ToggleContainerLayout,
    FocusParent,
    FocusTab {
        forward: bool,
    },
    TabClicked {
        container_id: ContainerId,
        index: usize,
    },
    GrowMaster,
    ShrinkMaster,
    MoreMaster,
    FewerMaster,
}

/// Tiling window and container placements collected by the strategy for a
/// single workspace.
pub(crate) struct TilingPlacements {
    pub(crate) windows: Vec<TilingWindowPlacement>,
    pub(crate) containers: Vec<ContainerPlacement>,
}

/// Per-strategy export payload for serialization to layout.jsonc.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct WorkspaceExport {
    pub(crate) strategy: String,
    pub(crate) tree: Option<TreeLayoutNode>,
    pub(crate) master_ratio: Option<f32>,
    pub(crate) master_count: Option<usize>,
    pub(crate) master: PaneConfig,
    pub(crate) secondary: PaneConfig,
    pub(crate) float: Vec<WindowMatcher>,
    pub(crate) fullscreen: Vec<WindowMatcher>,
}

impl WorkspaceExport {
    pub(crate) fn to_layout_workspace_config(&self, name: &str) -> LayoutWorkspaceConfig {
        match self.strategy.as_str() {
            "partition_tree" => LayoutWorkspaceConfig::PartitionTree {
                name: name.to_owned(),
                tree: self.tree.clone(),
                float: self.float.clone(),
                fullscreen: self.fullscreen.clone(),
            },
            "master" => LayoutWorkspaceConfig::Master {
                name: name.to_owned(),
                master_ratio: self.master_ratio,
                master_count: self.master_count,
                master: self.master.clone(),
                secondary: self.secondary.clone(),
                float: self.float.clone(),
                fullscreen: self.fullscreen.clone(),
            },
            _ => unreachable!("unknown strategy"),
        }
    }
}

/// Abstraction over tiling behavior. Tiling-specific operations live here.
/// Generic window management (monitors, workspaces, float, fullscreen, focus
/// priority) does not.
pub(crate) trait TilingStrategy: std::fmt::Debug {
    /// Pre-allocate per-workspace state.
    fn prepare_workspace(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        preferred_layout: Option<&LayoutWorkspaceConfig>,
    );

    /// Insert a window into the tiling tree for the given workspace. Does not
    /// focus it: the hub decides focus.
    fn attach_window(&mut self, hub: &mut HubAccess, window_id: WindowId, ws_id: WorkspaceId);

    /// Remove a window from its workspace's tiling tree. Returns the window's
    /// dimension in screen-absolute coordinates (translated before detach
    /// because detach triggers layout, which can change viewport_offset).
    fn detach_window(&mut self, hub: &mut HubAccess, window_id: WindowId) -> PixelRect;

    /// Dispatch a tiling-specific action. Reads the current workspace from
    /// `hub.focused_monitor` internally. Both mutates state and triggers
    /// layout as needed.
    fn handle_action(&mut self, hub: &mut HubAccess, action: TilingAction);

    /// Compute layout for all tiling windows in the workspace.
    fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId);

    /// Move tiling focus to the given window. Never touches
    /// `Workspace::is_float_focused`, so a focused float keeps keyboard focus.
    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId);

    /// Collect tiling placements for rendering.
    fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
    ) -> TilingPlacements;

    /// Return the focused tiling window for a workspace. Returns `None` if
    /// `focused_tiling` is a `Child::Container` (container-highlight mode) or
    /// if the workspace is empty.
    fn focused_tiling_window(&self, ws_id: WorkspaceId) -> Option<WindowId>;

    fn detach_focused_child(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Option<Child>;

    /// Returns the number of tiling windows in the workspace.
    fn tiling_window_count(&self, hub: &HubAccess, ws_id: WorkspaceId) -> usize;

    /// Return true if this workspace's tiling layout has a matcher that matches
    /// the given window. Read-only routing query used by resolve_matcher on
    /// window insert.
    fn matches_tiling(&self, ws_id: WorkspaceId, metadata: &dyn WindowMetadata) -> bool;

    /// Re-attach a previously-detached `Child` into `ws_id` and set focus within the
    /// workspace. A strategy that cannot host containers flattens `child` into its
    /// windows, so the resulting focus need not be `child`.
    fn reattach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId);

    /// Migrate windows out of a workspace being rebuilt after a strategy
    /// change. Returns the list of tiling window IDs and the focused tiling
    /// window (if any), then removes all per-workspace state.
    fn migrate(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
    ) -> (Vec<WindowId>, Option<WindowId>);

    /// Synchronize the preferred layout for a single workspace from an incoming
    /// workspace override.
    /// `incoming` is `None` when the workspace no longer has an override
    /// in the new config. The strategy clears its per-workspace state and
    /// falls back to global defaults.
    fn sync_preferred_layout(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        incoming: Option<&LayoutWorkspaceConfig>,
    );

    /// Refresh config-derived internal state and relayout the given workspace.
    fn apply_config(&mut self, hub: &mut HubAccess, layout: GlobalLayoutConfig);

    /// Export the current layout for a workspace, updating the strategy's
    /// internal preferred-layout representation to match the live tree.
    fn export_workspace(&mut self, hub: &HubAccess, ws_id: WorkspaceId) -> WorkspaceExport;
}

#[cfg(test)]
pub(super) trait ValidateStrategy {
    fn validate(&self, hub: &HubAccess);

    /// Container ids this strategy reaches from its workspace roots.
    fn reachable_containers(&self, hub: &HubAccess) -> FxHashSet<ContainerId>;
}

/// Absorbs the f32 error a constraint accumulates while being distributed.
#[cfg(test)]
pub(super) const VALIDATION_TOLERANCE: Length = Length::new(0.01);

/// Resolve one tiling window's effective constraints, in border-box space.
///
/// `Window::limits` records what the app asked for, which describes its content
/// area, so each per-window limit gains `2 * border` here. The global
/// `size_constraints` are already border-box and must not be outset, or what a
/// percentage means would start depending on `border_size`.
pub(crate) fn window_constraints(
    hub: &HubAccess,
    size_constraints: &SizeConstraints,
    wid: WindowId,
) -> Constraints {
    let window = hub.windows.get(wid);
    let ws_id = window.workspace().expect("tiling window has a workspace");
    let monitor_id = hub.workspaces.get(ws_id).monitor;
    let monitor = hub.monitors.get(monitor_id);
    let scale = monitor.scale;
    let work_area = monitor.work_area;
    let screen_width = Length::from_pixels(work_area.width());
    let screen_height = Length::from_pixels(work_area.height());

    let global_min_w = size_constraints.minimum_width.resolve(screen_width, scale);
    let global_min_h = size_constraints
        .minimum_height
        .resolve(screen_height, scale);
    let global_max_w = size_constraints.maximum_width.resolve(screen_width, scale);
    let global_max_h = size_constraints
        .maximum_height
        .resolve(screen_height, scale);

    let outset = Length::from_pixels(hub.border_for_scale(scale) * 2);
    let limits = window.limits();
    // Filter before the outset: a non-positive stored limit is not a limit at all, and outsetting
    // it first would turn it into a spurious `2 * border` cap that collapses the slot.
    let outset_limit = |v: Option<Length<Unit>>| {
        v.filter(|v| *v > Length::ZERO)
            .map_or(Length::ZERO, |v| v + outset)
    };
    let win_min_w = outset_limit(limits.min_width);
    let win_min_h = outset_limit(limits.min_height);
    let win_max_w = outset_limit(limits.max_width);
    let win_max_h = outset_limit(limits.max_height);

    let max_w = if win_max_w > Length::ZERO {
        win_max_w
    } else {
        global_max_w
    };
    let max_h = if win_max_h > Length::ZERO {
        win_max_h
    } else {
        global_max_h
    };

    let min_w = if max_w > Length::ZERO {
        win_min_w.max(global_min_w).min(max_w)
    } else {
        win_min_w.max(global_min_w)
    };
    let min_h = if max_h > Length::ZERO {
        win_min_h.max(global_min_h).min(max_h)
    } else {
        win_min_h.max(global_min_h)
    };

    Constraints {
        min_width: min_w,
        min_height: min_h,
        max_width: max_w,
        max_height: max_h,
    }
}

/// Converts layout-space coordinates to screen-absolute. Layout positions are relative to
/// the workspace origin plus the viewport offset, so the monitor origin is what makes them
/// absolute. The origin is added after rounding rather than before, which is exact because
/// it is integral.
pub(crate) fn translate<U>(
    dim: Dimension<U>,
    offset_x: Length<U>,
    offset_y: Length<U>,
    screen_x: Pixels<U>,
    screen_y: Pixels<U>,
) -> PixelRect<U> {
    let local = PixelRect::from_dimension(Dimension::new(
        dim.x - offset_x,
        dim.y - offset_y,
        dim.width,
        dim.height,
    ));
    PixelRect::from_pixels(
        local.x() + screen_x,
        local.y() + screen_y,
        local.width(),
        local.height(),
    )
}

/// Clip a dimension to screen bounds. Returns None if entirely outside.
pub(crate) fn clip<U>(dim: Dimension<U>, bounds: Dimension<U>) -> Option<Dimension<U>> {
    let x1 = dim.x.max(bounds.x);
    let y1 = dim.y.max(bounds.y);
    let x2 = (dim.x + dim.width).min(bounds.x + bounds.width);
    let y2 = (dim.y + dim.height).min(bounds.y + bounds.height);
    if x1 >= x2 || y1 >= y2 {
        return None;
    }
    Some(Dimension::new(x1, y1, x2 - x1, y2 - y1))
}

/// Zero-height when the container is not tabbed. The band top comes from the container's own
/// dimension, not a separately rounded height, so round(y) + round(band) cannot drift a unit from
/// the round(y + band) the content box uses.
pub(crate) fn tab_bar_band(
    border_box: PixelRect,
    dim: Dimension,
    offset_y: Length,
    screen: PixelRect,
    tab_bar_length: Length,
    is_tabbed: bool,
) -> PixelRect {
    let band_height = if is_tabbed {
        let content_top = Pixels::round(dim.y + tab_bar_length - offset_y) + screen.y();
        content_top - border_box.y()
    } else {
        Pixels::ZERO
    };
    PixelRect::from_pixels(
        border_box.x(),
        border_box.y(),
        border_box.width(),
        band_height,
    )
}

pub(crate) fn container_titles(hub: &HubAccess, id: ContainerId) -> Vec<String> {
    hub.containers
        .get(id)
        .children()
        .iter()
        .map(|c| match c {
            Child::Window(wid) => hub.windows.get(*wid).title().to_owned(),
            Child::Container(_) => "Container".to_string(),
        })
        .collect()
}

/// Distribute `container_size` across `constraints` so every child whose
/// (min, max) range straddles the result receives the same uniform size.
pub(crate) fn distribute_space(
    constraints: &[(Length, Length)],
    container_size: Length,
) -> Vec<Length> {
    let constraints: Vec<(Length, Length)> = constraints
        .iter()
        .map(|&(min, max)| {
            let max = if max == Length::ZERO {
                Length::new(f32::INFINITY)
            } else {
                max
            };
            (min, max)
        })
        .collect();

    let sum_mins: Length = constraints.iter().map(|(min, _)| *min).sum();
    if sum_mins >= container_size {
        return constraints.iter().map(|(min, _)| *min).collect();
    }

    let all_finite = constraints.iter().all(|(_, max)| max.value().is_finite());
    if all_finite {
        let sum_maxes: Length = constraints.iter().map(|(_, max)| *max).sum();
        if sum_maxes <= container_size {
            return constraints.iter().map(|(_, max)| *max).collect();
        }
    }

    let mut uniform_low = 0.0_f32;
    let mut uniform_high = container_size.value();
    const EPSILON: f32 = 0.001;

    // Binary search converges in ~log2(container_size / EPSILON) iterations,
    // typically ~24 for monitor-sized inputs. Cap at 64 per AGENTS.md no-unbounded-loop rule.
    for _ in 0..64 {
        if uniform_high - uniform_low <= EPSILON {
            break;
        }
        let uniform_candidate = (uniform_low + uniform_high) / 2.0;
        let total: f32 = constraints
            .iter()
            .map(|(min, max)| uniform_candidate.clamp(min.value(), max.value()))
            .sum();
        if total > container_size.value() {
            uniform_high = uniform_candidate;
        } else {
            uniform_low = uniform_candidate;
        }
    }

    constraints
        .iter()
        .map(|(min, max)| Length::new(uniform_low.clamp(min.value(), max.value())))
        .collect()
}

/// Owns one shared instance per tiling strategy and the per-workspace mapping
/// from `WorkspaceId` to `Strategy`. Hub holds this as a single field disjoint
/// from `HubAccess`, so dispatch (`for_workspace_mut`) borrows only this field
/// and leaves `HubAccess` free for the strategy method to take by `&mut`.
#[derive(Debug)]
pub(super) struct StrategySet {
    partition_tree: PartitionTreeStrategy,
    master: MasterStrategy,
    kinds: FxHashMap<WorkspaceId, Strategy>,
}

impl StrategySet {
    pub(super) fn new(layout: &GlobalLayoutConfig) -> Self {
        let partition_tree = PartitionTreeStrategy::new(
            layout.partition_tree.tab_bar_height,
            layout.partition_tree.automatic_tiling,
            layout.size_constraints,
        );
        let master = MasterStrategy::new(
            layout.master.master_count,
            layout.master.master_ratio,
            layout.size_constraints,
            layout.partition_tree.tab_bar_height,
        );
        Self {
            partition_tree,
            master,
            kinds: FxHashMap::default(),
        }
    }

    pub(super) fn register(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        let ws_name = hub.workspaces.get(ws_id).name.clone();
        // Clone so the `&mut hub` below does not alias a borrow into `hub.preferred_layouts`.
        let preferred = hub
            .preferred_layouts
            .iter()
            .find(|w| w.name() == ws_name)
            .cloned();
        let preferred_strategy = preferred
            .as_ref()
            .map(|w| match w {
                LayoutWorkspaceConfig::PartitionTree { .. } => Strategy::PartitionTree,
                LayoutWorkspaceConfig::Master { .. } => Strategy::Master,
            })
            .unwrap_or(hub.layout.strategy);

        self.kinds.insert(ws_id, preferred_strategy);
        let kind = self.kind_of(ws_id);
        self.get_mut(kind)
            .prepare_workspace(hub, ws_id, preferred.as_ref());
    }

    pub(super) fn kind_of(&self, ws_id: WorkspaceId) -> Strategy {
        *self
            .kinds
            .get(&ws_id)
            .unwrap_or_else(|| panic!("workspace {ws_id:?} not registered with StrategySet"))
    }

    pub(super) fn get(&self, kind: Strategy) -> &dyn TilingStrategy {
        match kind {
            Strategy::PartitionTree => &self.partition_tree,
            Strategy::Master => &self.master,
        }
    }

    pub(super) fn get_mut(&mut self, kind: Strategy) -> &mut dyn TilingStrategy {
        match kind {
            Strategy::PartitionTree => &mut self.partition_tree,
            Strategy::Master => &mut self.master,
        }
    }

    pub(super) fn for_workspace(&self, ws_id: WorkspaceId) -> &dyn TilingStrategy {
        self.get(self.kind_of(ws_id))
    }

    pub(super) fn for_workspace_mut(&mut self, ws_id: WorkspaceId) -> &mut dyn TilingStrategy {
        let kind = self.kind_of(ws_id);
        self.get_mut(kind)
    }

    /// Recompute kinds and drive the full sync. All cross-kind rebuilds and
    /// same-kind syncs happen here.
    pub(super) fn resync(
        &mut self,
        hub: &mut HubAccess,
        preferred_layouts: &[LayoutWorkspaceConfig],
        default_strategy: Strategy,
    ) {
        for ws_id in hub.workspaces.sorted_ids() {
            let old = *self
                .kinds
                .get(&ws_id)
                .unwrap_or_else(|| panic!("workspace {ws_id:?} not registered with StrategySet"));
            let ws_name = hub.workspaces.get(ws_id).name.clone();
            let new = preferred_layouts
                .iter()
                .find(|w| w.name() == ws_name)
                .map(|w| match w {
                    LayoutWorkspaceConfig::PartitionTree { .. } => Strategy::PartitionTree,
                    LayoutWorkspaceConfig::Master { .. } => Strategy::Master,
                })
                .unwrap_or(default_strategy);
            self.kinds.insert(ws_id, new);
            let incoming = preferred_layouts
                .iter()
                .find(|o| o.name() == ws_name.as_str());
            if old != new {
                tracing::debug!(
                    ws_id = %ws_id,
                    old = ?old,
                    new = ?new,
                    "Per-workspace strategy changed, rebuilding",
                );
                let (tiling_windows, focused) = self.get_mut(old).migrate(hub, ws_id);
                self.get_mut(new).prepare_workspace(hub, ws_id, incoming);

                for wid in &tiling_windows {
                    self.for_workspace_mut(ws_id)
                        .attach_window(hub, *wid, ws_id);
                }
                if let Some(f) = focused {
                    self.for_workspace_mut(ws_id).set_focus(hub, f);
                }
            } else {
                let cfg = incoming.cloned();
                self.get_mut(new)
                    .sync_preferred_layout(hub, ws_id, cfg.as_ref());
            }
        }
    }

    #[cfg(test)]
    pub(super) fn validate(&self, hub: &HubAccess) {
        // The container arena is shared across strategies, so union every strategy's reachable
        // set before the leak sweep, or one strategy's containers look leaked to another.
        let mut reachable = self.partition_tree.reachable_containers(hub);
        reachable.extend(self.master.reachable_containers(hub));
        let allocated: FxHashSet<ContainerId> = hub.containers.sorted_ids().into_iter().collect();

        let mut leaked: Vec<ContainerId> = allocated.difference(&reachable).copied().collect();
        leaked.sort_unstable();
        assert!(
            leaked.is_empty(),
            "Containers allocated but reachable from no workspace root, so they leaked: {leaked:?}"
        );
        let mut dangling: Vec<ContainerId> = reachable.difference(&allocated).copied().collect();
        dangling.sort_unstable();
        assert!(
            dangling.is_empty(),
            "Containers reachable from a workspace root but not allocated: {dangling:?}"
        );

        self.partition_tree.validate(hub);
        self.master.validate(hub);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::Length;

    #[test]
    fn distribute_space_returns_mins_when_sum_exceeds_container() {
        let constraints = vec![
            (Length::new(60.0), Length::ZERO),
            (Length::new(60.0), Length::ZERO),
        ];
        let result = distribute_space(&constraints, Length::new(100.0));
        assert_eq!(result, vec![Length::new(60.0), Length::new(60.0)]);
    }

    #[test]
    fn distribute_space_returns_maxes_when_all_fit() {
        let constraints = vec![
            (Length::new(10.0), Length::new(20.0)),
            (Length::new(10.0), Length::new(20.0)),
        ];
        let result = distribute_space(&constraints, Length::new(100.0));
        assert_eq!(result, vec![Length::new(20.0), Length::new(20.0)]);
    }

    #[test]
    fn distribute_space_splits_uniformly_with_mixed_caps() {
        // Child 0: uncapped (max=0 -> infinity), child 1: max=20, child 2: uncapped
        let constraints = vec![
            (Length::ZERO, Length::ZERO),
            (Length::ZERO, Length::new(20.0)),
            (Length::ZERO, Length::ZERO),
        ];
        let result = distribute_space(&constraints, Length::new(100.0));
        // Child 1 pins at 20. Remaining 80 splits evenly between children 0 and 2.
        assert!((result[1].value() - 20.0).abs() < 0.01);
        assert!((result[0].value() - 40.0).abs() < 0.01);
        assert!((result[2].value() - 40.0).abs() < 0.01);
    }

    #[test]
    fn distribute_space_pins_min_when_below_uniform() {
        // Child 0 has min=50, so it stays at 50 when uniform target is ~35.
        let constraints = vec![
            (Length::new(50.0), Length::ZERO),
            (Length::ZERO, Length::ZERO),
            (Length::ZERO, Length::ZERO),
        ];
        let result = distribute_space(&constraints, Length::new(120.0));
        assert!((result[0].value() - 50.0).abs() < 0.01);
        assert!((result[1].value() - 35.0).abs() < 0.01);
        assert!((result[2].value() - 35.0).abs() < 0.01);
    }
}
