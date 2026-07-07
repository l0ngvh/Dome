mod container;
mod layout;
mod navigate;
mod preferred_layout;
mod tree;
mod types;
#[cfg(test)]
mod validate;

use self::preferred_layout::{PreferredLayout, PreferredSlot};
pub(crate) use crate::core::node::Child;
pub(crate) use container::Container;
pub(crate) use types::*;

use std::collections::HashMap;

use crate::config::{LayoutConfig, LayoutWorkspaceConfig};
use crate::core::allocator::Allocator;
use crate::core::hub::{ContainerPlacement, HubAccess, SpawnIndicator, TilingWindowPlacement};
use crate::core::node::{Dimension, Length, Logical, WindowId, WorkspaceId};
use crate::core::strategy::{TilingAction, TilingPlacements, TilingStrategy, clip, translate};

/// i3-style manual tiling strategy. Manages a container tree where windows are
/// leaves and containers define split direction (horizontal/vertical) or tabbed
/// layout. This is the default (and currently only) tiling strategy.
#[derive(Debug)]
pub(crate) struct PartitionTreeStrategy {
    containers: Allocator<Container>,
    tiling_windows: HashMap<WindowId, TilingWindowData>,
    workspaces: HashMap<WorkspaceId, WorkspaceTilingState>,
    tab_bar_height: Length<Logical>,
    automatic_tiling: bool,
}

impl PartitionTreeStrategy {
    pub(crate) fn new(tab_bar_height: Length<Logical>, automatic_tiling: bool) -> Self {
        Self {
            containers: Allocator::new(),
            tiling_windows: HashMap::new(),
            workspaces: HashMap::new(),
            tab_bar_height,
            automatic_tiling,
        }
    }

    /// Attach a `Child` (window or container) to a workspace. If the spwn mode is horizontal or
    /// vertical then try to insert the child next to the focused child. if it's tabbed then try to
    /// insert it into the closest tabbed container
    fn attach_child_according_to_spawn_mode(
        &mut self,
        hub: &mut HubAccess,
        child: Child,
        ws_id: WorkspaceId,
    ) {
        self.assign_subtree_to_workspace(hub, child, ws_id);
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let insert_anchor = state.focused_tiling.or(state.root);
        let Some(insert_anchor) = insert_anchor else {
            self.workspaces.get_mut(&ws_id).unwrap().root = Some(child);
            self.set_parent(child, Parent::Workspace(ws_id));
            self.set_focus_child(hub, child);
            self.layout_workspace(hub, ws_id);
            return;
        };

        let spawn_mode = self.child_spawn_mode(insert_anchor);

        if spawn_mode.is_tab()
            && let Some(tabbed_self_or_ancestor) = self.find_tabbed_self_or_ancestor(insert_anchor)
        {
            let container = self.containers.get(tabbed_self_or_ancestor);
            self.attach_child_to_container(
                child,
                tabbed_self_or_ancestor,
                Some(container.active_tab_index() + 1),
            );
        } else if let Child::Container(cid) = insert_anchor
            && self.containers.get(cid).can_accommodate(spawn_mode)
        {
            self.attach_child_to_container(child, cid, None);
        } else {
            match self.parent(insert_anchor) {
                Parent::Container(container_id) => {
                    if self
                        .containers
                        .get(container_id)
                        .can_accommodate(spawn_mode)
                    {
                        let anchor_index =
                            self.containers.get(container_id).position_of(insert_anchor);
                        self.attach_child_to_container(child, container_id, Some(anchor_index + 1));
                    } else {
                        self.replace_anchor_with_container(
                            hub,
                            insert_anchor,
                            vec![insert_anchor, child],
                            spawn_mode.into(),
                        );
                    }
                }
                Parent::Workspace(_) => {
                    self.replace_anchor_with_container(
                        hub,
                        insert_anchor,
                        vec![insert_anchor, child],
                        spawn_mode.into(),
                    );
                }
            }
        }

        self.layout_workspace(hub, ws_id);
        self.set_focus_child(hub, child);
    }

    /// Detach a `Child` (window or container) from its workspace.
    fn detach_child(&mut self, hub: &mut HubAccess, child: Child) {
        let workspace_id = self.child_workspace(hub, child);

        let parent = self.parent(child);
        match parent {
            Parent::Container(parent_id) => {
                self.detach_child_from_container(parent_id, child);
                self.layout_workspace(hub, workspace_id);
            }
            Parent::Workspace(workspace_id) => {
                self.workspaces.get_mut(&workspace_id).unwrap().root = None;
                self.workspaces
                    .get_mut(&workspace_id)
                    .unwrap()
                    .focused_tiling = None;

                let ws = hub.workspaces.get_mut(workspace_id);
                ws.is_float_focused = !ws.float_windows.is_empty();

                self.layout_workspace(hub, workspace_id);
            }
        }

        let workspace = self.workspaces.get_mut(&workspace_id).unwrap();
        if let Some(layout) = workspace.preferred_layout.as_mut() {
            match child {
                Child::Window(wid) => {
                    if let Some(slot_id) = self.tiling_windows.get(&wid).unwrap().occupy {
                        layout.clear_window_slot(slot_id);
                        if workspace.occupied_preferred_root == Some(PreferredSlot::Window(slot_id))
                        {
                            workspace.occupied_preferred_root = None;
                        }
                        self.tiling_windows.get_mut(&wid).unwrap().occupy = None;
                    }
                }
                Child::Container(cid) => {
                    if let Some(slot_id) = self.containers.get(cid).occupy {
                        layout.clear_container_slot(slot_id);
                        if workspace.occupied_preferred_root
                            == Some(PreferredSlot::Container(slot_id))
                        {
                            workspace.occupied_preferred_root = layout.top_occupied_in(slot_id);
                        }
                        self.containers.get_mut(cid).occupy = None;
                    }
                }
            }
        }
    }

    /// Internal set_focus that works with `Child` (window or container).
    ///
    /// Establish invariant 3 of `Container` for `child`: writes `child` to
    /// `container.focused` on every ancestor up to the workspace, and updates
    /// `active_tab` on each tabbed ancestor with the walk position (not
    /// `child`). At the workspace level, sets `focused_tiling = Some(child)`
    /// and clears `is_float_focused`.
    fn set_focus_child(&mut self, hub: &mut HubAccess, child: Child) {
        let path: Vec<_> = self.ancestors_of(child).collect();
        for (walk_pos, parent_id) in &path {
            let container = self.containers.get_mut(*parent_id);
            if container.is_tabbed {
                container.set_active_tab_to_child(*walk_pos);
            }
            container.focused = child;
        }
        // Workspace-level focus state lives above the container tree.
        // ancestors_of terminates at the workspace boundary, so handle it here.
        let ws_child = match path.last() {
            Some((_, last_pid)) => Child::Container(*last_pid),
            None => child,
        };
        let Parent::Workspace(ws) = self.parent(ws_child) else {
            panic!("set_focus_child: top of ancestor path has no workspace parent");
        };
        self.workspaces.get_mut(&ws).unwrap().focused_tiling = Some(child);
        hub.workspaces.get_mut(ws).is_float_focused = false;
        self.scroll_into_view(hub, ws);
    }
}

impl TilingStrategy for PartitionTreeStrategy {
    fn prepare_workspace(&mut self, ws_id: WorkspaceId, ws_name: &str, config: &LayoutConfig) {
        let preferred_layout = config.workspace.iter().find_map(|w| match w {
            LayoutWorkspaceConfig::PartitionTree { name, tree, .. } if *name == ws_name => {
                tree.as_ref().map(PreferredLayout::from_tree_layout_node)
            }
            _ => None,
        });
        self.workspaces
            .entry(ws_id)
            .or_insert(WorkspaceTilingState {
                preferred_layout,
                ..Default::default()
            });
    }

    fn attach_window(&mut self, hub: &mut HubAccess, window_id: WindowId, ws_id: WorkspaceId) {
        let metadata = hub.windows.get(window_id).metadata.as_ref();
        self.tiling_windows
            .insert(window_id, TilingWindowData::new(ws_id));

        let ws_state = self.workspaces.get(&ws_id).unwrap();
        let Some(layout) = ws_state.preferred_layout.as_ref() else {
            self.attach_child_according_to_spawn_mode(hub, Child::Window(window_id), ws_id);
            return;
        };
        let Some(slot_id) = layout.find_window_slot(metadata) else {
            self.attach_child_according_to_spawn_mode(hub, Child::Window(window_id), ws_id);
            return;
        };
        if layout.first_occupied_ancestor(slot_id).is_some() {
            // TODO: insert to the same container
            return;
        }

        let Some(root_slot) = ws_state.occupied_preferred_root else {
            // First matched window, insert via spawn mode and mark slot occupied
            self.attach_child_according_to_spawn_mode(hub, Child::Window(window_id), ws_id);

            let ws_state = self.workspaces.get_mut(&ws_id).unwrap();
            ws_state
                .preferred_layout
                .as_mut()
                .unwrap()
                .occupy_window_slot(slot_id, window_id);
            self.tiling_windows.get_mut(&window_id).unwrap().occupy = Some(slot_id);
            ws_state.occupied_preferred_root = Some(PreferredSlot::Window(slot_id));
            return;
        };

        // Matched window with existing preferred root, materialize the lowest common ancestor
        let (lowest_common_ancestor, ordering) =
            layout.lowest_common_ancestor(PreferredSlot::Window(slot_id), root_slot);
        let anchor = match root_slot {
            PreferredSlot::Window(root_slot_id) => {
                let root_window_id = layout.occupied_window(root_slot_id).unwrap();
                Child::Window(root_window_id)
            }
            PreferredSlot::Container(root_container_id) => {
                let root_container = layout.occupied_container(root_container_id).unwrap();
                Child::Container(root_container)
            }
        };

        let children = if ordering == std::cmp::Ordering::Less {
            vec![Child::Window(window_id), anchor]
        } else {
            vec![anchor, Child::Window(window_id)]
        };

        let new_container_id = self.replace_anchor_with_container(
            hub,
            anchor,
            children,
            layout.container_slot_split(lowest_common_ancestor),
        );

        let ws_state = self.workspaces.get_mut(&ws_id).unwrap();
        let layout = ws_state.preferred_layout.as_mut().unwrap();
        layout.occupy_container_slot(lowest_common_ancestor, new_container_id);
        layout.occupy_window_slot(slot_id, window_id);
        self.tiling_windows.get_mut(&window_id).unwrap().occupy = Some(slot_id);
        self.containers.get_mut(new_container_id).occupy = Some(lowest_common_ancestor);
        ws_state.occupied_preferred_root = Some(PreferredSlot::Container(lowest_common_ancestor));

        self.layout_workspace(hub, ws_id);
        self.set_focus_child(hub, Child::Window(window_id));
    }

    fn detach_window(&mut self, hub: &mut HubAccess, window_id: WindowId) -> Dimension {
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

    fn layout_workspace(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        self.do_layout_workspace(hub, ws_id);
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
        highlighted: bool,
    ) -> TilingPlacements {
        let Some(ws_state) = self.workspaces.get(&ws_id) else {
            return TilingPlacements {
                windows: Vec::new(),
                containers: Vec::new(),
            };
        };
        let ws = hub.workspaces.get(ws_id);
        let (offset_x, offset_y) = ws_state.viewport_offset;
        let screen = hub.monitors.get(ws.monitor).dimension;
        // Only highlight tiling focus when this is the current workspace AND
        // the workspace's effective focus is on tiling (not float). Fullscreen
        // workspaces never reach here (hub returns early with MonitorLayout::Fullscreen).
        let focused = if highlighted && !ws.is_float_focused {
            ws_state.focused_tiling
        } else {
            None
        };
        let mut windows = Vec::new();
        let mut containers = Vec::new();

        // Hand-rolled DFS kept because tabbed containers push only the active
        // tab, not all children. This visible-only traversal differs from the
        // full pre-order that children_dfs provides.
        let mut stack: Vec<Child> = ws_state.root.into_iter().collect();
        for _ in crate::core::bounded_loop() {
            let Some(child) = stack.pop() else { break };
            match child {
                Child::Window(id) => {
                    let frame = translate(self.child_dimension(child), offset_x, offset_y, screen);
                    if let Some(visible_frame) = clip(frame, screen) {
                        let is_highlighted = focused == Some(Child::Window(id));
                        windows.push(TilingWindowPlacement {
                            id,
                            frame,
                            visible_frame,
                            is_highlighted,
                            spawn_indicator: if is_highlighted {
                                Some(SpawnIndicator::from(self.child_spawn_mode(child)))
                            } else {
                                None
                            },
                        });
                    }
                }
                Child::Container(id) => {
                    let container = self.containers.get(id);
                    let frame = translate(self.child_dimension(child), offset_x, offset_y, screen);
                    let Some(visible_frame) = clip(frame, screen) else {
                        continue;
                    };
                    let is_highlighted = focused == Some(Child::Container(id));
                    containers.push(ContainerPlacement {
                        id,
                        frame,
                        visible_frame,
                        is_highlighted,
                        spawn_indicator: if is_highlighted {
                            Some(SpawnIndicator::from(self.child_spawn_mode(child)))
                        } else {
                            None
                        },
                        is_tabbed: container.is_tabbed(),
                        active_tab_index: container.active_tab_index(),
                        titles: container
                            .children()
                            .iter()
                            .map(|c| match c {
                                Child::Window(wid) => hub.windows.get(*wid).title().to_owned(),
                                Child::Container(_) => "Container".to_string(),
                            })
                            .collect(),
                    });
                    if let Some(active) = container.active_tab() {
                        stack.push(active);
                    } else {
                        for &c in container.children() {
                            stack.push(c);
                        }
                    }
                }
            }
        }

        TilingPlacements {
            windows,
            containers,
        }
    }

    fn focused_tiling_window(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> Option<WindowId> {
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

    fn has_tiling_windows(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> bool {
        self.workspaces
            .get(&ws_id)
            .is_some_and(|s| s.root.is_some())
    }

    /// Counts tiling windows by walking the container tree from root.
    /// A tree walk is necessary because `self.tiling_windows` is a global map
    /// across all workspaces and cannot be filtered by workspace without it.
    fn tiling_window_count(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> usize {
        let Some(root) = self.workspaces.get(&ws_id).and_then(|s| s.root) else {
            return 0;
        };
        self.children_dfs(root)
            .filter(|c| matches!(c, Child::Window(_)))
            .count()
    }

    fn prune_workspace(&mut self, ws_id: WorkspaceId) {
        self.workspaces.remove(&ws_id);
    }

    fn apply_config(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        self.tab_bar_height = hub.config.partition_tree.tab_bar_height;
        self.automatic_tiling = hub.config.partition_tree.automatic_tiling;
        self.layout_workspace(hub, ws_id);
    }

    #[cfg(test)]
    fn validate_tree(&self, hub: &HubAccess) {
        for (workspace_id, workspace) in hub.workspaces.all_active() {
            self.validate_workspace_focus(hub, workspace_id, &workspace);

            let Some(root) = self.workspaces.get(&workspace_id).and_then(|s| s.root) else {
                continue;
            };
            // Hand-rolled DFS kept because the walk threads expected_parent
            // derived from the traversal structure. Using children_dfs plus
            // parent would check the parent field against itself.
            let mut stack = vec![(root, Parent::Workspace(workspace_id))];
            for _ in crate::core::bounded_loop() {
                let Some((child, expected_parent)) = stack.pop() else {
                    break;
                };
                match child {
                    Child::Window(wid) => {
                        self.validate_window(hub, wid, expected_parent, workspace_id)
                    }
                    Child::Container(cid) => {
                        self.validate_container(
                            hub,
                            cid,
                            expected_parent,
                            workspace_id,
                            &mut stack,
                        );
                    }
                }
            }
        }
    }
}
