use std::collections::HashMap;

use crate::config::{LayoutConfig, Strategy};
use crate::core::allocator::Allocator;
use crate::core::hub::{ContainerPlacement, HubAccess, TilingWindowPlacement};
use crate::core::master::MasterStrategy;
use crate::core::node::{
    Child, ContainerId, Dimension, Direction, Length, WindowId, Workspace, WorkspaceId,
};
use crate::core::partition_tree::PartitionTreeStrategy;

/// Actions that are specific to the tiling strategy.
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

/// Abstraction over tiling behavior. Tiling-specific operations live here;
/// generic window management (monitors, workspaces, float, fullscreen, focus
/// priority) does not.
///
/// Each method receives `&mut HubAccess` (or `&HubAccess`) so the strategy
/// can read/write monitors, workspaces, and windows without borrowing Hub.
/// This solves the split-borrow problem: `self` (the strategy) and `hub`
/// (the access struct) are disjoint fields on Hub.
pub(crate) trait TilingStrategy: std::fmt::Debug {
    /// Insert a window into the tiling tree for the given workspace.
    fn attach_window(&mut self, hub: &mut HubAccess, window_id: WindowId, ws_id: WorkspaceId);

    /// Remove a window from its workspace's tiling tree. Returns the window's
    /// dimension in screen-absolute coordinates (translated before detach
    /// because detach triggers layout, which can change viewport_offset).
    fn detach_window(&mut self, hub: &mut HubAccess, window_id: WindowId) -> Dimension;

    /// Dispatch a tiling-specific action. Reads the current workspace from
    /// `hub.focused_monitor` internally. Both mutates state and triggers
    /// layout as needed.
    fn handle_action(&mut self, hub: &mut HubAccess, action: TilingAction);

    /// Compute layout for all tiling windows in the workspace.
    fn layout_workspace(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId);

    /// Set tiling focus to the given window, updating container focus chains
    /// and workspace focus state.
    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId);

    /// Collect tiling placements for rendering. The strategy reads
    /// offset/screen/focused_tiling from hub.workspaces internally.
    /// `highlighted`: true for the current workspace, false for others.
    /// When false, no placement is highlighted.
    fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
    ) -> TilingPlacements;

    /// Return the focused tiling window for a workspace. Returns `None` if
    /// `focused_tiling` is a `Child::Container` (container-highlight mode) or
    /// if the workspace is empty.
    fn focused_tiling_window(&self, hub: &HubAccess, ws_id: WorkspaceId) -> Option<WindowId>;

    fn detach_focused_child(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Option<Child>;

    /// Re-attach a previously-detached `Child` into `ws_id`. Sets focus
    /// to the attached child. No-op when `child` is not applicable to
    /// this strategy (e.g. `Child::Container` for MasterStrategy).

    /// Returns true if the workspace has any tiling windows (root is Some).
    fn has_tiling_windows(&self, hub: &HubAccess, ws_id: WorkspaceId) -> bool;

    /// Returns the number of tiling windows in the workspace.
    fn tiling_window_count(&self, hub: &HubAccess, ws_id: WorkspaceId) -> usize;

    fn reattach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId);

    /// Remove all per-workspace state for a workspace being deleted.
    fn prune_workspace(&mut self, ws_id: WorkspaceId);

    /// Refresh config-derived internal state and relayout the given workspace.
    fn apply_config(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId);

    /// Validate strategy-specific structural invariants (test-only).
    #[cfg(test)]
    fn validate_tree(&self, hub: &HubAccess);
}

/// Convert layout-space coordinates to screen-absolute. Layout positions are
/// relative to workspace origin (0,0); this applies viewport offset and
/// monitor origin.
pub(crate) fn translate<U>(
    dim: Dimension<U>,
    offset_x: Length<U>,
    offset_y: Length<U>,
    screen: Dimension<U>,
) -> Dimension<U> {
    Dimension::new(
        dim.x - offset_x + screen.x,
        dim.y - offset_y + screen.y,
        dim.width,
        dim.height,
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

/// Diff entry produced by `StrategySet::resync` for one workspace whose kind
/// changed across a config reload. The caller drives the cross-kind rebuild.
pub(super) struct WorkspaceKindChange {
    pub(super) ws_id: WorkspaceId,
    pub(super) old: Strategy,
    pub(super) new: Strategy,
}

/// Owns one shared instance per tiling strategy and the per-workspace mapping
/// from `WorkspaceId` to `Strategy`. Hub holds this as a single field disjoint
/// from `HubAccess`, so dispatch (`for_workspace_mut`) borrows only this field
/// and leaves `HubAccess` free for the strategy method to take by `&mut`.
#[derive(Debug)]
pub(super) struct StrategySet {
    partition_tree: PartitionTreeStrategy,
    master: MasterStrategy,
    kinds: HashMap<WorkspaceId, Strategy>,
}

impl StrategySet {
    pub(super) fn new(config: &LayoutConfig) -> Self {
        let partition_tree = PartitionTreeStrategy::new(
            config.partition_tree.tab_bar_height,
            config.partition_tree.automatic_tiling,
        );
        let master = MasterStrategy::new();
        Self {
            partition_tree,
            master,
            kinds: HashMap::new(),
        }
    }

    pub(super) fn register(&mut self, ws_id: WorkspaceId, name: &str, config: &LayoutConfig) {
        self.kinds.insert(ws_id, kind_for(name, config));
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

    /// Recompute kinds against `new_config`, returning entries only for
    /// workspaces whose kind changed. The caller drives the cross-kind
    /// rebuild. This method only updates the map.
    pub(super) fn resync(
        &mut self,
        workspaces: &Allocator<Workspace>,
        new_config: &LayoutConfig,
    ) -> Vec<WorkspaceKindChange> {
        let mut changes = Vec::new();
        for (ws_id, ws) in workspaces.all_active() {
            let old = *self
                .kinds
                .get(&ws_id)
                .unwrap_or_else(|| panic!("workspace {ws_id:?} not registered with StrategySet"));
            let new = kind_for(&ws.name, new_config);
            self.kinds.insert(ws_id, new);
            if old != new {
                changes.push(WorkspaceKindChange { ws_id, old, new });
            }
        }
        changes
    }

    #[cfg(test)]
    pub(super) fn validate_tree(&self, hub: &HubAccess) {
        self.partition_tree.validate_tree(hub);
        self.master.validate_tree(hub);
    }
}

fn kind_for(name: &str, config: &LayoutConfig) -> Strategy {
    config
        .workspace
        .iter()
        .find(|w| w.name == name)
        .map(|w| w.strategy)
        .unwrap_or(config.strategy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        GapsConfig, LayoutWorkspaceConfig, MasterConfig, PartitionTreeConfig, SizeConstraint,
    };
    use crate::core::node::Length;

    fn config_with(strategy: Strategy, overrides: Vec<LayoutWorkspaceConfig>) -> LayoutConfig {
        LayoutConfig {
            strategy,
            gaps: GapsConfig::default(),
            partition_tree: PartitionTreeConfig {
                tab_bar_height: Length::ZERO,
                automatic_tiling: false,
            },
            master: MasterConfig {
                master_ratio: 0.5,
                master_count: 1,
                workspace: vec![],
            },
            min_width: SizeConstraint::default(),
            min_height: SizeConstraint::default(),
            max_width: SizeConstraint::default(),
            max_height: SizeConstraint::default(),
            workspace: overrides,
        }
    }

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

    #[test]
    fn kind_for_returns_override_when_name_matches() {
        let config = config_with(
            Strategy::PartitionTree,
            vec![LayoutWorkspaceConfig {
                name: "1".into(),
                strategy: Strategy::Master,
            }],
        );
        assert_eq!(kind_for("1", &config), Strategy::Master);
    }

    #[test]
    fn kind_for_falls_back_to_global_when_no_match() {
        let config = config_with(
            Strategy::PartitionTree,
            vec![LayoutWorkspaceConfig {
                name: "1".into(),
                strategy: Strategy::Master,
            }],
        );
        assert_eq!(kind_for("2", &config), Strategy::PartitionTree);
    }

    #[test]
    fn kind_for_falls_back_to_global_when_no_overrides() {
        let config = config_with(Strategy::Master, vec![]);
        assert_eq!(kind_for("anything", &config), Strategy::Master);
    }
}
