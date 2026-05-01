use crate::core::hub::{ContainerPlacement, HubAccess, TilingWindowPlacement};
use crate::core::node::{ContainerId, Dimension, Direction, WindowId, WorkspaceId};

/// Actions that are specific to the tiling strategy. Hub routes these through
/// `TilingStrategy::handle_action` after checking window restrictions.
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
/// single workspace. Hub merges these with float placements to build the
/// final `MonitorLayout`.
pub(crate) struct TilingPlacements {
    pub(crate) windows: Vec<TilingWindowPlacement>,
    pub(crate) containers: Vec<ContainerPlacement>,
}

/// Abstraction over tiling behavior. Hub delegates all tiling-specific
/// operations to the active strategy, keeping generic window management
/// (monitors, workspaces, float, fullscreen, focus priority) on Hub itself.
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

    /// Move the focused tiling child (window or container) from one workspace
    /// to another. The strategy reads its own focus state to determine what to
    /// move. If no focused tiling child exists in `from_ws`, silently returns.
    fn move_focused_to_workspace(
        &mut self,
        hub: &mut HubAccess,
        from_ws: WorkspaceId,
        to_ws: WorkspaceId,
    );

    /// Returns true if the workspace has any tiling windows (root is Some).
    fn has_tiling_windows(&self, hub: &HubAccess, ws_id: WorkspaceId) -> bool;

    /// Returns the number of tiling windows in the workspace.
    fn tiling_window_count(&self, hub: &HubAccess, ws_id: WorkspaceId) -> usize;

    /// Remove per-workspace tiling state. Called before workspace deletion.
    fn prune_workspace(&mut self, ws_id: WorkspaceId);

    /// Validate strategy-specific structural invariants (test-only).
    #[cfg(test)]
    fn validate_tree(&self, hub: &HubAccess);
}

/// Convert layout-space coordinates to screen-absolute. Layout positions are
/// relative to workspace origin (0,0); this applies viewport offset and
/// monitor origin.
pub(crate) fn translate(
    dim: Dimension,
    offset_x: f32,
    offset_y: f32,
    screen: Dimension,
) -> Dimension {
    Dimension {
        x: dim.x - offset_x + screen.x,
        y: dim.y - offset_y + screen.y,
        width: dim.width,
        height: dim.height,
    }
}

/// Clip a dimension to screen bounds. Returns None if entirely outside.
pub(crate) fn clip(dim: Dimension, bounds: Dimension) -> Option<Dimension> {
    let x1 = dim.x.max(bounds.x);
    let y1 = dim.y.max(bounds.y);
    let x2 = (dim.x + dim.width).min(bounds.x + bounds.width);
    let y2 = (dim.y + dim.height).min(bounds.y + bounds.height);
    if x1 >= x2 || y1 >= y2 {
        return None;
    }
    Some(Dimension {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
    })
}
