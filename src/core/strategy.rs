use rustc_hash::FxHashMap;
#[cfg(test)]
use rustc_hash::FxHashSet;

use crate::core::MonitorSelector;
use crate::core::TilingConfig;
#[cfg(test)]
use crate::core::float::FloatWindows;
#[cfg(test)]
use crate::core::fullscreen::FullscreenWindows;
use crate::core::hub::{HubAccess, MonitorLayout};
use crate::core::master::MasterStrategy;
use crate::core::node::{
    Child, Constraints, ContainerId, Dimension, Direction, DisplayMode, Length, LimitObservation,
    PixelRect, Pixels, Unit, WindowId, WorkspaceId,
};
use crate::core::partition_tree::PartitionTreeStrategy;
use crate::core::scrolling::ScrollingStrategy;
use crate::core::slot::SlotId;
use crate::core::{PreferredWorkspace, SizeConstraints, Strategy};

/// An action on the focused tiling child does nothing while a float or fullscreen window has
/// focus, and an action on the workspace layout or a clicked container does nothing while a
/// fullscreen window has focus.
#[derive(Debug)]
pub(crate) enum StrategyAction {
    /// Moves focus from the focused tiling child to the nearest child in `direction`.
    FocusDirection { direction: Direction, forward: bool },
    /// Moves the focused tiling child one place in `direction`.
    MoveDirection { direction: Direction, forward: bool },
    /// Flips the direction in which the focused tiling child places the next window.
    ToggleSpawnMode,
    /// Flips the split direction of the containers around the focused tiling child.
    ToggleDirection,
    /// Switches the container around the focused tiling child between split and tabbed.
    ToggleContainerLayout,
    /// Highlights the container around the focused tiling child, so that a move to another
    /// workspace takes the whole container. Does nothing at the workspace root.
    FocusParent,
    /// Activates the next or previous tab of the tabbed container around the focused tiling
    /// child.
    FocusTab { forward: bool },
    /// Activates tab `index` of `container_id` and focuses it, taking focus from a float.
    TabClicked {
        container_id: ContainerId,
        index: usize,
    },
    /// Widens the column of the focused tiling child, within the strategy's limits.
    Grow,
    /// Narrows the column of the focused tiling child, within the strategy's limits.
    Shrink,
    /// Raises the number of windows the master pane of the workspace holds.
    MoreMaster,
    /// Lowers the number of windows the master pane of the workspace holds, down to its minimum.
    FewerMaster,
    /// Floats the focused tiling window at its tile rectangle, or tiles the focused float.
    ToggleFloat,
    /// Makes the focused window fullscreen, or tiles the focused fullscreen window.
    ToggleFullscreen,
}

#[derive(Debug)]
pub(crate) enum TilingAction {
    Strategy(StrategyAction),
    FocusWorkspace {
        name: String,
        monitor: Option<String>,
    },
    MoveToWorkspace {
        name: String,
        monitor: Option<String>,
    },
    FocusMonitor {
        selector: MonitorSelector,
    },
    MoveToMonitor {
        selector: MonitorSelector,
    },
}

impl From<StrategyAction> for TilingAction {
    fn from(action: StrategyAction) -> Self {
        Self::Strategy(action)
    }
}

impl From<&crate::action::FocusTarget> for TilingAction {
    fn from(target: &crate::action::FocusTarget) -> Self {
        use crate::action::{FocusTarget, TabDirection};

        let directional =
            |direction, forward| StrategyAction::FocusDirection { direction, forward }.into();
        match target {
            FocusTarget::Up => directional(Direction::Vertical, false),
            FocusTarget::Down => directional(Direction::Vertical, true),
            FocusTarget::Left => directional(Direction::Horizontal, false),
            FocusTarget::Right => directional(Direction::Horizontal, true),
            FocusTarget::Parent => StrategyAction::FocusParent.into(),
            FocusTarget::Tab { direction } => StrategyAction::FocusTab {
                forward: matches!(direction, TabDirection::Next),
            }
            .into(),
            FocusTarget::Workspace { name, monitor } => Self::FocusWorkspace {
                name: name.clone(),
                monitor: monitor.clone(),
            },
            FocusTarget::Monitor { target } => Self::FocusMonitor {
                selector: target.into(),
            },
        }
    }
}

impl From<&crate::action::MoveTarget> for TilingAction {
    fn from(target: &crate::action::MoveTarget) -> Self {
        use crate::action::MoveTarget;

        let directional =
            |direction, forward| StrategyAction::MoveDirection { direction, forward }.into();
        match target {
            MoveTarget::Up => directional(Direction::Vertical, false),
            MoveTarget::Down => directional(Direction::Vertical, true),
            MoveTarget::Left => directional(Direction::Horizontal, false),
            MoveTarget::Right => directional(Direction::Horizontal, true),
            MoveTarget::Workspace { name, monitor } => Self::MoveToWorkspace {
                name: name.clone(),
                monitor: monitor.clone(),
            },
            MoveTarget::Monitor { target } => Self::MoveToMonitor {
                selector: target.into(),
            },
        }
    }
}

impl From<&crate::action::ToggleTarget> for TilingAction {
    fn from(target: &crate::action::ToggleTarget) -> Self {
        use crate::action::ToggleTarget;

        match target {
            ToggleTarget::Spawn => StrategyAction::ToggleSpawnMode.into(),
            ToggleTarget::Direction => StrategyAction::ToggleDirection.into(),
            ToggleTarget::Layout => StrategyAction::ToggleContainerLayout.into(),
            ToggleTarget::Float => StrategyAction::ToggleFloat.into(),
            ToggleTarget::Fullscreen => StrategyAction::ToggleFullscreen.into(),
        }
    }
}

impl From<&crate::action::MasterTarget> for TilingAction {
    fn from(target: &crate::action::MasterTarget) -> Self {
        use crate::action::MasterTarget;

        match target {
            MasterTarget::More => StrategyAction::MoreMaster.into(),
            MasterTarget::Fewer => StrategyAction::FewerMaster.into(),
        }
    }
}

/// The focus a strategy selects on one workspace, with its display mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum FocusedChild {
    Fullscreen(WindowId),
    Float(WindowId),
    Tiling(Child),
}

impl FocusedChild {
    pub(super) fn child(self) -> Child {
        match self {
            Self::Fullscreen(id) | Self::Float(id) => Child::Window(id),
            Self::Tiling(child) => child,
        }
    }
}

/// Owns all non-minimized windows on its workspaces, including their modes, stacking and focus.
/// Float support and the fullscreen toggle are optional. App-controlled fullscreen is required.
/// A method changing layout computes placement before returning. No method searches, holds
/// or releases a slot.
pub(crate) trait TilingStrategy: std::fmt::Debug {
    /// Builds state and allocates free slots in the hub's arena, fullscreen slots first, then
    /// float slots, then tiling slots, each in layout order. Initializes the layout work area and
    /// scale from the workspace's host monitor. Allocates optional-mode slots only for supported
    /// modes. Panics on another strategy's tiling variant.
    fn prepare_workspace(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        entry: &PreferredWorkspace,
    );

    /// Removes all windows and drops workspace state. Returns modes in reattachment order, each
    /// float and fullscreen stack bottom to top. The hub deletes the slots.
    fn clear_workspace(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
    ) -> Vec<(WindowId, DisplayMode)>;

    /// Exports live windows rather than slot matchers, with this strategy's tiling variant.
    fn export_workspace(&self, hub: &HubAccess, ws_id: WorkspaceId) -> PreferredWorkspace;

    /// Inserts and records the destination workspace. Unsupported float or Dome-controlled
    /// fullscreen becomes tiling. App-controlled fullscreen remains fullscreen. The caller passes
    /// a slot only for tiling and holds it after this call. `Some(slot)` supplies the layout
    /// position, while `None` uses default placement. The first actual tiling attachment
    /// initializes remembered tiling focus, including a conversion from an unsupported mode.
    /// Each nonempty mode retains a remembered target. Makes no explicit focus request.
    fn attach_window(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        mode: DisplayMode,
        slot: Option<SlotId>,
    );

    /// Inserts a detached group and gives it tiling focus, even when a float had focus. The
    /// strategy chooses whether the group itself or one of its windows holds that focus.
    fn attach_container(
        &mut self,
        hub: &mut HubAccess,
        container_id: ContainerId,
        ws_id: WorkspaceId,
    ) {
        let windows = hub.take_windows(Child::Container(container_id));
        for &window_id in &windows {
            self.attach_window(hub, window_id, ws_id, DisplayMode::Tiling, None);
        }
        if let Some(&first) = windows.first() {
            self.set_focus(hub, first);
        }
    }

    /// Removes a window in any mode, returning its mode and a float's border box. Does not alter
    /// its held slot.
    fn detach_window(&mut self, hub: &mut HubAccess, window_id: WindowId) -> DisplayMode;

    /// Leaves the detached group and its windows in the hub arenas for attachment. Only a
    /// strategy reporting a focused container receives this call and must override the panic.
    fn detach_container(
        &mut self,
        _hub: &mut HubAccess,
        _container_id: ContainerId,
        _ws_id: WorkspaceId,
    ) {
        unreachable!("this strategy never reports a focused container");
    }

    /// Accepts app-controlled fullscreen even without toggle support. A window entering
    /// fullscreen becomes topmost and focused. An already-fullscreen window is unchanged.
    fn set_fullscreen(&mut self, hub: &mut HubAccess, window_id: WindowId);

    /// Ends app-controlled fullscreen and tiles with default placement. The other fullscreen
    /// windows keep their stack order, so the topmost of them keeps focus, and the window leaving
    /// fullscreen takes focus once no fullscreen window remains. A non-fullscreen window is
    /// unchanged. Required without toggle support.
    fn unset_fullscreen(&mut self, hub: &mut HubAccess, window_id: WindowId);

    /// Accepts the rectangle only for a current float.
    fn update_float_rect(
        &mut self,
        _hub: &mut HubAccess,
        _window_id: WindowId,
        _border_box: PixelRect,
    ) -> bool {
        false
    }

    /// Unsupported actions do nothing, and so does an action whose target does not have focus,
    /// as `StrategyAction` describes. For a tab click, `ws_id` is the workspace of the clicked
    /// container, which can be on a monitor without focus.
    fn handle_action(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, action: StrategyAction);

    /// Requests focus in any mode. Raises a float or fullscreen target in its own stack, or
    /// updates remembered tiling focus. A target that fullscreen covers does not take keyboard
    /// focus from the topmost fullscreen window.
    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId);

    /// Called once a reset has attached every window. Focuses the topmost fullscreen window,
    /// otherwise the topmost float, otherwise the tiling focus that attachment initialized, then
    /// recomputes placement.
    fn reset_focus(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId);

    /// Returns effective focus in any mode, including a container representing a group. `None`
    /// means exactly that the workspace has no window.
    fn focused_child(&self, ws_id: WorkspaceId) -> Option<Child>;

    /// Updates this workspace's layout work area and scale, then recomputes placement
    /// internally before returning. The caller has already recorded the monitor report
    /// or reassigned the workspace to its new host.
    fn update_work_area(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        work_area: PixelRect,
        scale: f32,
    );

    /// Applies an observation to the window's existing limits, then recomputes its
    /// workspace's placement before returning. The window can be in any display mode, but is
    /// never minimized.
    fn update_window_size_limits(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        observed: LimitObservation,
    );

    /// Reads computed placements. Fullscreen shows only its topmost window. `highlighted`
    /// enables focus indicators and is true only for the current workspace.
    fn collect_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
    ) -> MonitorLayout;

    /// Reads this strategy's config and recomputes placement for all its workspaces.
    fn apply_config(&mut self, hub: &mut HubAccess, tiling: &TilingConfig);
}

/// What one strategy's validator reached.
#[cfg(test)]
pub(super) struct Reachable {
    pub(super) containers: FxHashSet<ContainerId>,
    /// The windows of each workspace the strategy holds, in every mode. A workspace with no
    /// window has an empty entry.
    pub(super) windows: FxHashMap<WorkspaceId, Vec<WindowId>>,
}

#[cfg(test)]
pub(super) trait ValidateStrategy {
    /// Returns the container ids this strategy reaches from its workspace roots, with the windows
    /// of each of its workspaces.
    fn validate(&self, hub: &HubAccess) -> Reachable;
}

/// Absorbs the f32 error a constraint accumulates while being distributed.
#[cfg(test)]
pub(super) const VALIDATION_TOLERANCE: Length = Length::new(0.01);

/// Panics when float focus is selected with no float to give it to, when a fullscreen window no
/// longer exists, or when a tiling window or a float has restrictions, which only a fullscreen
/// window can carry.
#[cfg(test)]
pub(super) fn validate_display_modes(
    hub: &HubAccess,
    ws_id: WorkspaceId,
    tiling_windows: &[WindowId],
    float_windows: &FloatWindows,
    fullscreen_windows: &FullscreenWindows,
) {
    assert!(
        !float_windows.is_float_focused || float_windows.topmost().is_some(),
        "{ws_id}: float focus is selected but the workspace has no float"
    );
    for id in tiling_windows
        .iter()
        .copied()
        .chain(float_windows.windows())
    {
        assert_eq!(
            hub.windows.get(id).restrictions,
            crate::core::WindowRestrictions::None,
            "{ws_id}: {id} has restrictions but is not fullscreen"
        );
    }
    for id in fullscreen_windows.windows() {
        assert!(
            hub.windows.contains(id),
            "{ws_id}: fullscreen window {id} no longer exists"
        );
    }
}

/// Resolve one tiling window's effective constraints, in border-box space.
///
/// `Window::limits` records what the app asked for, which describes its content
/// area, so each per-window limit gains `2 * border` here. The global
/// `size_constraints` are already border-box and must not be outset, or what a
/// percentage means would start depending on `border_size`.
///
/// A per-window max tightens the global max, it does not replace it. An app that
/// declares a huge cap would otherwise escape the configured one.
pub(crate) fn window_constraints(
    hub: &HubAccess,
    size_constraints: &SizeConstraints,
    wid: WindowId,
    work_area: PixelRect,
    scale: f32,
) -> Constraints {
    let window = hub.windows.get(wid);
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

    let max_w = tighter_max(win_max_w, global_max_w);
    let max_h = tighter_max(win_max_h, global_max_h);

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

/// `Length::ZERO` means no cap, so it cannot take part in a plain `min`.
pub(crate) fn tighter_max(a: Length, b: Length) -> Length {
    if a > Length::ZERO && b > Length::ZERO {
        a.min(b)
    } else {
        a.max(b)
    }
}

/// The size a capped child takes in `slot_extent`, with the offset that centers it there.
pub(crate) fn apply_max_constraint(max: Length, slot_extent: Length) -> (Length, Length) {
    let size = if max > Length::ZERO && max < slot_extent {
        max
    } else {
        slot_extent
    };
    let offset = (slot_extent - size) / 2.0;
    (size, offset.max(Length::ZERO))
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

/// Zero height when the container is not tabbed.
pub(crate) fn tab_bar_band(
    border_box: PixelRect,
    dim: Dimension,
    screen: PixelRect,
    tab_bar_length: Length,
    is_tabbed: bool,
) -> PixelRect {
    let band_height = if is_tabbed {
        let content_top = Pixels::round(dim.y + tab_bar_length) + screen.y();
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

pub(crate) fn visible_tab_bar_band(
    tab_bar_band: PixelRect,
    visible_border_box: PixelRect,
) -> PixelRect {
    tab_bar_band
        .clip(visible_border_box)
        .unwrap_or(PixelRect::ZERO)
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
    scrolling: ScrollingStrategy,
    kinds: FxHashMap<WorkspaceId, Strategy>,
}

impl StrategySet {
    pub(super) fn new(tiling: &TilingConfig) -> Self {
        Self {
            partition_tree: PartitionTreeStrategy::new(tiling),
            master: MasterStrategy::new(tiling),
            scrolling: ScrollingStrategy::new(tiling),
            kinds: FxHashMap::default(),
        }
    }

    /// Assigns the workspace to the strategy that its entry names, replacing any earlier
    /// assignment, and has that strategy prepare the workspace.
    pub(super) fn prepare_workspace(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        entry: &PreferredWorkspace,
    ) {
        let kind = entry.tiling.strategy();
        self.kinds.insert(ws_id, kind);
        self.get_mut(kind).prepare_workspace(hub, ws_id, entry);
    }

    pub(super) fn kind_of(&self, ws_id: WorkspaceId) -> Strategy {
        *self
            .kinds
            .get(&ws_id)
            .unwrap_or_else(|| panic!("workspace {ws_id:?} not registered with StrategySet"))
    }

    fn get(&self, kind: Strategy) -> &dyn TilingStrategy {
        match kind {
            Strategy::PartitionTree => &self.partition_tree,
            Strategy::Master => &self.master,
            Strategy::Scrolling => &self.scrolling,
        }
    }

    fn get_mut(&mut self, kind: Strategy) -> &mut dyn TilingStrategy {
        match kind {
            Strategy::PartitionTree => &mut self.partition_tree,
            Strategy::Master => &mut self.master,
            Strategy::Scrolling => &mut self.scrolling,
        }
    }

    pub(super) fn for_workspace(&self, ws_id: WorkspaceId) -> &dyn TilingStrategy {
        self.get(self.kind_of(ws_id))
    }

    pub(super) fn for_workspace_mut(&mut self, ws_id: WorkspaceId) -> &mut dyn TilingStrategy {
        let kind = self.kind_of(ws_id);
        self.get_mut(kind)
    }

    /// A strategy that owns no workspace still takes the config, so a workspace
    /// that later moves to it starts from the current values.
    pub(super) fn apply_config(&mut self, hub: &mut HubAccess, tiling: &TilingConfig) {
        self.partition_tree.apply_config(hub, tiling);
        self.master.apply_config(hub, tiling);
        self.scrolling.apply_config(hub, tiling);
    }

    /// Returns the workspace that holds each open window, after checking that every workspace
    /// sits in the strategy it is assigned to and that no window sits in two places.
    #[cfg(test)]
    pub(super) fn validate(&self, hub: &HubAccess) -> FxHashMap<WindowId, WorkspaceId> {
        let tree = self.partition_tree.validate(hub);
        let master = self.master.validate(hub);
        let scrolling = self.scrolling.validate(hub);

        // The container arena is shared across strategies, so union every strategy's reachable
        // set before the leak sweep, or one strategy's containers look leaked to another.
        let mut reachable = tree.containers;
        reachable.extend(master.containers);
        reachable.extend(scrolling.containers);
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

        let mut owners: FxHashMap<WindowId, WorkspaceId> = FxHashMap::default();
        let mut held_workspaces: FxHashSet<WorkspaceId> = FxHashSet::default();
        for (kind, windows) in [
            (Strategy::PartitionTree, tree.windows),
            (Strategy::Master, master.windows),
            (Strategy::Scrolling, scrolling.windows),
        ] {
            for (ws_id, ids) in windows {
                assert_eq!(
                    self.kind_of(ws_id),
                    kind,
                    "{ws_id} has state in the {kind:?} strategy but is assigned to another"
                );
                held_workspaces.insert(ws_id);
                for id in ids {
                    if let Some(other) = owners.insert(id, ws_id) {
                        panic!("{id} is listed twice, in {other} and in {ws_id}");
                    }
                }
            }
        }
        for ws_id in hub.workspaces.sorted_ids() {
            assert!(
                held_workspaces.contains(&ws_id),
                "{ws_id} has no state in the strategy it is assigned to"
            );
        }
        owners
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
