mod container;
mod navigate;
mod placement;
mod preferred_layout;
mod scroll;
mod tree;
mod types;
#[cfg(test)]
mod validate;

use self::preferred_layout::{PreferredContainerSlot, PreferredSlot, PreferredWindowSlot};
pub(crate) use crate::core::node::Child;
pub(crate) use crate::core::node::Container;
pub(crate) use types::*;

use rustc_hash::FxHashMap;

use crate::config::LayoutWorkspaceConfig;
use crate::config::SizeConstraints;
use crate::config::SplitMode;
use crate::core::GlobalLayoutConfig;
use crate::core::allocator::Allocator;
use crate::core::hub::HubAccess;
use crate::core::node::{
    ContainerId, Logical, PixelRect, Pixels, WindowId, WindowMetadata, WorkspaceId,
};
use crate::core::strategy::{
    TilingAction, TilingPlacements, TilingStrategy, WorkspaceExport, translate,
};

/// i3-style manual tiling strategy. Manages a container tree where windows are
/// leaves and containers define split direction (horizontal/vertical) or tabbed
/// layout. This is the default (and currently only) tiling strategy.
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
    fn prepare_workspace(
        &mut self,
        ws_id: WorkspaceId,
        preferred_layout: Option<&LayoutWorkspaceConfig>,
    ) {
        let preferred_root = match preferred_layout {
            Some(LayoutWorkspaceConfig::PartitionTree { tree, .. }) => {
                tree.as_ref().map(|t| self.build_preferred_layout(t))
            }
            Some(_) => panic!("Preparing master workspace in partition tree strategy"),
            None => None,
        };
        self.workspaces.insert(
            ws_id,
            WorkspaceTilingState {
                preferred_root,
                ..Default::default()
            },
        );
    }

    fn attach_window(&mut self, hub: &mut HubAccess, window_id: WindowId, ws_id: WorkspaceId) {
        let metadata = hub.windows.get(window_id).metadata.as_ref();
        self.tiling_windows
            .insert(window_id, TilingWindowData::new(ws_id));

        let preferred_root = self.workspaces.get(&ws_id).unwrap().preferred_root;
        let Some(root) = preferred_root else {
            self.attach_child_according_to_spawn_mode(hub, Child::Window(window_id), ws_id);
            return;
        };
        let Some(slot_id) = self.find_window_slot(root, metadata) else {
            tracing::debug!(%window_id, "No preferred layout slot matched, falling back to spawn mode");
            self.attach_child_according_to_spawn_mode(hub, Child::Window(window_id), ws_id);
            return;
        };
        tracing::debug!(%window_id, ?slot_id, "Window matched preferred layout slot");
        hub.windows.get_mut(window_id).set_workspace(Some(ws_id));

        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .add_to_history(window_id);
        if let Some(ancestor_slot) = self.first_occupied_ancestor(slot_id) {
            self.attach_window_into_occupied_ancestor(
                hub,
                window_id,
                ws_id,
                slot_id,
                ancestor_slot,
            );
            return;
        }

        if !self.window_slots.get(slot_id).windows.is_empty() {
            self.attach_window_into_same_slot(hub, window_id, ws_id, slot_id);
            return;
        }

        if let Some(root_slot) = self.workspaces.get(&ws_id).unwrap().occupied_preferred_root {
            self.attach_window_to_unoccupied_container(hub, window_id, ws_id, slot_id, root_slot);
            return;
        }

        // First matched window, insert via spawn mode and mark slot occupied
        self.attach_child_according_to_spawn_mode(hub, Child::Window(window_id), ws_id);

        self.occupy_window_slot(slot_id, window_id);
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .occupied_preferred_root = Some(PreferredSlot::Window(slot_id));
        tracing::debug!(%window_id, ?slot_id, "First preferred window, established as root");
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

    fn handle_action(&mut self, hub: &mut HubAccess, action: TilingAction) {
        match action {
            TilingAction::FocusDirection { direction, forward } => {
                self.focus_in_direction(hub, direction, forward)
            }
            TilingAction::MoveDirection { direction, forward } => {
                self.move_in_direction(hub, direction, forward)
            }
            TilingAction::ToggleSpawnMode => self.toggle_spawn_mode(hub),
            TilingAction::ToggleDirection => self.toggle_focused_layout_direction(hub),
            TilingAction::ToggleContainerLayout => self.toggle_container_layout(hub),
            TilingAction::FocusParent => self.focus_parent(hub),
            TilingAction::FocusTab { forward } => self.focus_tab(hub, forward),
            TilingAction::TabClicked {
                container_id,
                index,
            } => self.focus_tab_index(hub, container_id, index),
            TilingAction::GrowMaster
            | TilingAction::ShrinkMaster
            | TilingAction::MoreMaster
            | TilingAction::FewerMaster => {}
        }
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
        self.attach_child_according_to_spawn_mode(hub, child, ws_id);
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
        self.find_window_slot(root, metadata).is_some()
    }

    fn migrate(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
    ) -> (Vec<WindowId>, Option<WindowId>) {
        let focused = self.focused_tiling_window(ws_id);
        let Some(state) = self.workspaces.remove(&ws_id) else {
            return (Vec::new(), focused);
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
        // To return the windows in inserted order
        tiling.reverse();
        (tiling, focused)
    }

    fn sync_preferred_layout(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        incoming: Option<&LayoutWorkspaceConfig>,
    ) {
        self.sync_preferred_layout(hub, ws_id, incoming)
    }

    fn apply_config(&mut self, hub: &mut HubAccess, layout: GlobalLayoutConfig) {
        self.tab_bar_height = layout.partition_tree.tab_bar_height;
        self.automatic_tiling = layout.partition_tree.automatic_tiling;
        self.size_constraints = layout.size_constraints;
        for ws_id in self.workspaces.keys().copied().collect::<Vec<_>>() {
            self.compute_placement(hub, ws_id);
        }
    }

    fn export_workspace(&mut self, hub: &HubAccess, ws_id: WorkspaceId) -> WorkspaceExport {
        PartitionTreeStrategy::export_workspace(self, hub, ws_id)
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
}
