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
pub(crate) use container::Container;
pub(crate) use types::*;

use std::collections::HashMap;

use crate::config::LayoutWorkspaceConfig;
use crate::config::SizeConstraints;
use crate::core::GlobalLayoutConfig;
use crate::core::allocator::Allocator;
use crate::core::hub::HubAccess;
use crate::core::node::{Dimension, Length, Logical, WindowId, WorkspaceId};
use crate::core::strategy::{TilingAction, TilingPlacements, TilingStrategy, WorkspaceExport};

/// i3-style manual tiling strategy. Manages a container tree where windows are
/// leaves and containers define split direction (horizontal/vertical) or tabbed
/// layout. This is the default (and currently only) tiling strategy.
#[derive(Debug)]
pub(crate) struct PartitionTreeStrategy {
    containers: Allocator<Container>,
    tiling_windows: HashMap<WindowId, TilingWindowData>,
    workspaces: HashMap<WorkspaceId, WorkspaceTilingState>,
    window_slots: Allocator<PreferredWindowSlot>,
    container_slots: Allocator<PreferredContainerSlot>,
    tab_bar_height: Length<Logical>,
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

        let occupied_root = self.workspaces.get(&ws_id).unwrap().occupied_preferred_root;
        let Some(root_slot) = occupied_root else {
            // First matched window, insert via spawn mode and mark slot occupied
            self.attach_child_according_to_spawn_mode(hub, Child::Window(window_id), ws_id);

            self.occupy_window_slot(slot_id, window_id);
            self.tiling_windows.get_mut(&window_id).unwrap().occupy = Some(slot_id);
            self.workspaces
                .get_mut(&ws_id)
                .unwrap()
                .occupied_preferred_root = Some(PreferredSlot::Window(slot_id));
            tracing::debug!(%window_id, ?slot_id, "First preferred window, established as root");
            return;
        };

        self.attach_window_to_unoccupied_container(hub, window_id, ws_id, slot_id, root_slot);
    }

    fn detach_window(&mut self, hub: &HubAccess, window_id: WindowId) -> Dimension {
        let child_dim = self.tiling_windows.get(&window_id).unwrap().dimension;
        let workspace_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("detaching tiling window has a workspace");
        let (offset_x, offset_y) = self.workspaces.get(&workspace_id).unwrap().viewport_offset;
        let screen = hub
            .monitors
            .get(hub.workspaces.get(workspace_id).monitor)
            .dimension;

        // Capture offset/screen before detach because detach triggers layout,
        // which can change viewport_offset.
        self.detach_child(hub, Child::Window(window_id));
        self.tiling_windows.remove(&window_id);

        // Convert layout-space coordinates to screen-absolute. Layout positions are
        // relative to workspace origin (0,0) plus viewport offset; screen-absolute
        // includes the monitor's origin.
        Dimension::new(
            child_dim.x - offset_x + screen.x,
            child_dim.y - offset_y + screen.y,
            child_dim.width,
            child_dim.height,
        )
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
        self.compute_placement_against_constraint(hub, ws_id);
    }

    /// Update tiling focus to a window. Delegates to `set_focus_child`, which writes
    /// the window as the focused node on every ancestor container up to the workspace root.
    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        self.set_focus_child(hub, Child::Window(window_id));
    }

    fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        focused: bool,
    ) -> TilingPlacements {
        self.collect_placements(hub, ws_id, focused)
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

    fn detach_focused_child(&mut self, hub: &HubAccess, ws_id: WorkspaceId) -> Option<Child> {
        let focused = self.workspaces.get(&ws_id)?.focused_tiling?;
        self.detach_child(hub, focused);

        if let Child::Window(wid) = focused {
            self.tiling_windows.remove(&wid);
        }
        Some(focused)
    }

    fn reattach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId) {
        if let Child::Window(wid) = child {
            self.tiling_windows
                .insert(wid, TilingWindowData::new(ws_id));
        }
        self.attach_child_according_to_spawn_mode(hub, child, ws_id);
    }

    /// Counts tiling windows by walking the container tree from root.
    /// A tree walk is necessary because `self.tiling_windows` is a global map
    /// across all workspaces and cannot be filtered by workspace without it.
    fn tiling_window_count(&self, ws_id: WorkspaceId) -> usize {
        let Some(root) = self.workspaces.get(&ws_id).and_then(|s| s.root) else {
            return 0;
        };
        self.children_dfs(root)
            .filter(|c| matches!(c, Child::Window(_)))
            .count()
    }

    fn migrate(&mut self, ws_id: WorkspaceId) -> (Vec<WindowId>, Option<WindowId>) {
        let focused = self.focused_tiling_window(ws_id);
        let mut tiling: Vec<WindowId> = self
            .workspaces
            .get(&ws_id)
            .and_then(|ws| ws.root)
            .map(|root| {
                self.children_dfs(root)
                    .filter_map(|c| match c {
                        Child::Window(id) => Some(id),
                        Child::Container(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        // To return the windows in inserted order
        tiling.reverse();
        self.workspaces.remove(&ws_id);
        (tiling, focused)
    }

    fn sync_preferred_layout(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        incoming: Option<&LayoutWorkspaceConfig>,
    ) {
        let current_root = self.workspaces.get(&ws_id).and_then(|ws| ws.preferred_root);
        let changed = match current_root {
            Some(_) => match incoming {
                Some(cfg) => !self.structurally_eq(current_root, cfg),
                None => true,
            },
            None => matches!(
                incoming,
                Some(LayoutWorkspaceConfig::PartitionTree { tree: Some(_), .. })
            ),
        };

        if !changed {
            return;
        }

        tracing::debug!(%ws_id, "PartitionTree preferred layout changed, reloading");

        // Phase: immutable snapshot — collect windows, old root, and focus.
        // Mutable work (detach_child, container deletion) happens below.
        let (tiling_windows, old_root) = {
            let state = self.workspaces.get(&ws_id).unwrap();
            let windows: Vec<WindowId> = state
                .root
                .map(|r| {
                    self.children_dfs(r)
                        .filter_map(|c| match c {
                            Child::Window(id) => Some(id),
                            Child::Container(_) => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            (windows, state.root)
        };

        let focused = self.focused_tiling_window(ws_id);

        // Phase: mutable — detach root (clears bookmarks + occupation,
        // triggers one layout on the now-empty workspace).
        if let Some(root) = old_root {
            self.detach_child(hub, root);
        }

        // Set the new preferred layout.
        let new_root = incoming.and_then(|w| match w {
            LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
                tree.as_ref().map(|t| self.build_preferred_layout(t))
            }
            _ => None,
        });
        self.workspaces.get_mut(&ws_id).unwrap().preferred_root = new_root;
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .occupied_preferred_root = None;

        // Reattach windows under the new layout.
        for &wid in &tiling_windows {
            self.attach_window(hub, wid, ws_id);
        }

        if let Some(f) = focused {
            self.set_focus(hub, f);
        }
    }

    fn apply_config(&mut self, hub: &mut HubAccess, layout: GlobalLayoutConfig) {
        self.tab_bar_height = layout.partition_tree.tab_bar_height;
        self.automatic_tiling = layout.partition_tree.automatic_tiling;
        self.size_constraints = layout.size_constraints;
        for ws_id in self.workspaces.keys().copied().collect::<Vec<_>>() {
            self.compute_placement(hub, ws_id);
        }
    }

    fn export_workspace(&mut self, hub: &HubAccess, ws_id: WorkspaceId) -> Option<WorkspaceExport> {
        PartitionTreeStrategy::export_workspace(self, hub, ws_id)
    }
}

impl PartitionTreeStrategy {
    pub(crate) fn new(
        tab_bar_height: Length<Logical>,
        automatic_tiling: bool,
        size_constraints: SizeConstraints,
    ) -> Self {
        Self {
            containers: Allocator::new(),
            tiling_windows: HashMap::new(),
            workspaces: HashMap::new(),
            window_slots: Allocator::new(),
            container_slots: Allocator::new(),
            tab_bar_height,
            automatic_tiling,
            size_constraints,
        }
    }
}
