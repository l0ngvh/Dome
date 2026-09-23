mod container;
mod navigate;
mod placement;
mod preferred_layout;
mod scroll;
mod tree;
mod types;
#[cfg(test)]
mod validate;

use self::preferred_layout::{PreferredContainerSlot, PreferredWindowSlot};
pub(crate) use crate::core::node::Child;
pub(crate) use crate::core::node::Container;
pub(crate) use preferred_layout::TreeLayoutNode;
pub(crate) use types::*;

use rustc_hash::FxHashMap;

use crate::core::SizeConstraints;
use crate::core::TilingConfig;
use crate::core::allocator::Allocator;
use crate::core::hub::HubAccess;
use crate::core::node::{
    ContainerId, Direction, Logical, PixelRect, Pixels, WindowId, WindowMetadata, WorkspaceId,
};
use crate::core::strategy::{TilingPlacements, TilingStrategy, translate};

/// i3-style manual tiling strategy. Manages a container tree where windows are
/// leaves and containers define split direction (horizontal/vertical) or tabbed
/// layout. This is the default tiling strategy.
#[derive(Debug)]
pub(crate) struct PartitionTreeStrategy {
    tiling_containers: FxHashMap<ContainerId, TilingContainerData>,
    tiling_windows: FxHashMap<WindowId, TilingWindowData>,
    workspaces: FxHashMap<WorkspaceId, WorkspaceTilingState>,
    window_slots: Allocator<PreferredWindowSlot>,
    container_slots: Allocator<PreferredContainerSlot>,
    tab_bar_height: Pixels<Logical>,
    automatic_tiling: bool,
    size_constraints: SizeConstraints,
}

impl TilingStrategy for PartitionTreeStrategy {
    fn attach_window(&mut self, hub: &mut HubAccess, window_id: WindowId, ws_id: WorkspaceId) {
        let metadata = hub.windows.get(window_id).metadata.as_ref();
        self.tiling_windows
            .insert(window_id, TilingWindowData::new(ws_id));

        let preferred_root = self.workspaces.get(&ws_id).unwrap().preferred_root;
        let Some(root) = preferred_root else {
            self.attach_child_according_to_spawn_direction(hub, Child::Window(window_id), ws_id);
            return;
        };
        let Some(slot_id) = self.find_free_slot(root, metadata) else {
            tracing::debug!(%window_id, "No preferred layout slot matched, falling back to spawn direction");
            self.attach_child_according_to_spawn_direction(hub, Child::Window(window_id), ws_id);
            return;
        };
        tracing::debug!(%window_id, ?slot_id, "Window matched preferred layout slot");
        self.attach_window_to_slot(hub, window_id, ws_id, slot_id);
    }

    fn detach_window(&mut self, hub: &mut HubAccess, window_id: WindowId) -> PixelRect {
        let child_dim = self.tiling_windows.get(&window_id).unwrap().dimension;
        let workspace_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("detaching tiling window has a workspace");
        let (offset_x, offset_y) = self.workspaces.get(&workspace_id).unwrap().viewport_offset;
        let work_area = hub
            .monitors
            .get(hub.workspaces.get(workspace_id).monitor)
            .work_area;

        // Capture the offset before detach because detach triggers layout, which can
        // change viewport_offset.
        self.detach_child(hub, Child::Window(window_id));
        self.tiling_windows.remove(&window_id);

        translate(child_dim, offset_x, offset_y, work_area.x(), work_area.y())
    }

    fn focus_direction(&mut self, hub: &mut HubAccess, direction: Direction, forward: bool) {
        self.focus_in_direction(hub, direction, forward)
    }

    fn move_direction(&mut self, hub: &mut HubAccess, direction: Direction, forward: bool) {
        self.move_in_direction(hub, direction, forward)
    }

    fn toggle_container_layout(&mut self, hub: &mut HubAccess) {
        self.toggle_focused_container_layout(hub)
    }

    fn focus_tab(&mut self, hub: &mut HubAccess, forward: bool) {
        self.focus_tab_in_direction(hub, forward)
    }

    fn tab_clicked(&mut self, hub: &mut HubAccess, container_id: ContainerId, index: usize) {
        self.focus_tab_index(hub, container_id, index)
    }

    fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        self.compute_placement(hub, ws_id);
    }

    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        self.set_focus(hub, Child::Window(window_id));
    }

    fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        focused: bool,
    ) -> TilingPlacements {
        self.collect_tiling_placements(hub, ws_id, focused)
    }

    fn focused_tiling_window(&self, ws_id: WorkspaceId) -> Option<WindowId> {
        // Read focused_tiling directly instead of walking from root.
        // When focused_tiling is Child::Container (focus_parent highlight),
        // returns None so toggle_float/toggle_fullscreen become no-ops.
        // No fallback needed when None: the validator enforces
        // root.is_some() => focused_tiling.is_some(), so None means empty workspace.
        match self.workspaces.get(&ws_id)?.focused_tiling? {
            Child::Window(id) => Some(id),
            Child::Container(_) => None,
        }
    }

    fn detach_focused_child(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Option<Child> {
        let focused = self.workspaces.get(&ws_id)?.focused_tiling?;
        self.detach_child(hub, focused);

        // Ordered after the detach, which still reads the state being dropped.
        for node in hub.children_dfs(focused) {
            match node {
                Child::Window(wid) => {
                    self.tiling_windows.remove(&wid);
                }
                Child::Container(cid) => {
                    self.tiling_containers.remove(&cid);
                }
            }
        }
        Some(focused)
    }

    fn reattach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId) {
        match child {
            Child::Window(wid) => {
                self.tiling_windows
                    .insert(wid, TilingWindowData::new(ws_id));
            }
            Child::Container(root) => {
                // Reversed because a preorder walk yields parents first, and a container must
                // exist before its parent links to it. The root's parent is a placeholder,
                // overwritten by the attach below.
                for cid in hub.containers_preorder(root).into_iter().rev() {
                    self.tiling_containers.insert(
                        cid,
                        TilingContainerData::new(
                            Parent::Workspace(ws_id),
                            ws_id,
                            SplitMode::Horizontal,
                        ),
                    );
                    for &member in hub.containers.get(cid).children() {
                        match member {
                            Child::Window(wid) => {
                                self.tiling_windows
                                    .insert(wid, TilingWindowData::in_container(cid));
                            }
                            Child::Container(nested) => {
                                self.tiling_containers.get_mut(&nested).unwrap().parent =
                                    Parent::Container(cid);
                            }
                        }
                    }
                }
                // Every container was rebuilt with the same default direction, so a nested
                // subtree arrives with a container inside a same-direction container. The
                // attach path only re-derives direction when it wraps an anchor, which an
                // empty destination skips.
                self.maintain_direction_invariance(hub, Parent::Container(root));
            }
        }
        self.attach_child_according_to_spawn_direction(hub, child, ws_id);
        self.set_focus(hub, child);
    }

    /// Counts tiling windows by walking the container tree from root.
    /// A tree walk is necessary because `self.tiling_windows` is a global map
    /// across all workspaces and cannot be filtered by workspace without it.
    fn tiling_window_count(&self, hub: &HubAccess, ws_id: WorkspaceId) -> usize {
        let Some(root) = self.workspaces.get(&ws_id).and_then(|s| s.root) else {
            return 0;
        };
        hub.children_dfs(root)
            .into_iter()
            .filter(|c| matches!(c, Child::Window(_)))
            .count()
    }

    fn matches_tiling(&self, ws_id: WorkspaceId, metadata: &dyn WindowMetadata) -> bool {
        let Some(root) = self.workspaces.get(&ws_id).and_then(|w| w.preferred_root) else {
            return false;
        };
        self.find_free_slot(root, metadata).is_some()
    }

    fn migrate(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Vec<WindowId> {
        let Some(state) = self.workspaces.remove(&ws_id) else {
            return Vec::new();
        };
        let mut tiling = match state.root {
            Some(root) => self.free_container_subtree(hub, root),
            None => Vec::new(),
        };
        if let Some(preferred_root) = state.preferred_root {
            self.free_preferred_subtree(preferred_root);
        }
        for wid in &tiling {
            self.tiling_windows.remove(wid);
        }
        tiling.reverse();
        tiling
    }

    fn apply_config(&mut self, hub: &mut HubAccess, tiling: TilingConfig) {
        self.tab_bar_height = tiling.partition_tree.tab_bar_height;
        self.automatic_tiling = tiling.partition_tree.automatic_tiling;
        self.size_constraints = tiling.size_constraints;
        for ws_id in self.workspaces.keys().copied().collect::<Vec<_>>() {
            self.compute_placement(hub, ws_id);
        }
    }
}

impl PartitionTreeStrategy {
    pub(crate) fn new(
        tab_bar_height: Pixels<Logical>,
        automatic_tiling: bool,
        size_constraints: SizeConstraints,
    ) -> Self {
        Self {
            tiling_containers: FxHashMap::default(),
            tiling_windows: FxHashMap::default(),
            workspaces: FxHashMap::default(),
            window_slots: Allocator::new(),
            container_slots: Allocator::new(),
            tab_bar_height,
            automatic_tiling,
            size_constraints,
        }
    }

    pub(super) fn prepare_workspace(&mut self, ws_id: WorkspaceId, tree: Option<&TreeLayoutNode>) {
        let preferred_root = tree.map(|t| self.build_preferred_layout(t));
        self.workspaces.insert(
            ws_id,
            WorkspaceTilingState {
                preferred_root,
                ..Default::default()
            },
        );
    }
}
