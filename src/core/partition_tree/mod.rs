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

use crate::config::LayoutWorkspaceConfig;
use crate::core::GlobalLayoutConfig;
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

impl TilingStrategy for PartitionTreeStrategy {
    fn prepare_workspace(
        &mut self,
        ws_id: WorkspaceId,
        ws_name: &str,
        _layout: &GlobalLayoutConfig,
        workspace_overrides: &[LayoutWorkspaceConfig],
    ) {
        let preferred_layout = workspace_overrides.iter().find_map(|w| match w {
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
            tracing::debug!(%window_id, "No preferred layout slot matched, falling back to spawn mode");
            self.attach_child_according_to_spawn_mode(hub, Child::Window(window_id), ws_id);
            return;
        };
        tracing::debug!(%window_id, ?slot_id, "Window matched preferred layout slot");
        hub.windows.get_mut(window_id).set_workspace(Some(ws_id));
        if let Some(ancestor_slot) = layout.first_occupied_ancestor(slot_id) {
            self.attach_window_into_occupied_ancestor(
                hub,
                window_id,
                ws_id,
                slot_id,
                ancestor_slot,
            );
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
            tracing::debug!(%window_id, ?slot_id, "First preferred window, established as root");
            return;
        };

        self.attach_window_to_unoccupied_container(hub, window_id, ws_id, slot_id, root_slot);
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

    fn sync_preferred_layout(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        incoming: Option<&LayoutWorkspaceConfig>,
    ) {
        let changed = match self.workspaces.get(&ws_id) {
            Some(ws) => match ws.preferred_layout.as_ref() {
                Some(current) => match incoming {
                    Some(cfg) => !current.structurally_eq(cfg),
                    None => true,
                },
                None => matches!(
                    incoming,
                    Some(LayoutWorkspaceConfig::PartitionTree { tree: Some(_), .. })
                ),
            },
            None => incoming.is_some(),
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

        let focused = self.focused_tiling_window(hub, ws_id);

        // Phase: mutable — detach root (clears bookmarks + occupation,
        // triggers one layout on the now-empty workspace).
        if let Some(root) = old_root {
            self.detach_child(hub, root);
        }

        // Set the new preferred layout.
        let new_layout = incoming.and_then(|w| match w {
            LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
                tree.as_ref().map(PreferredLayout::from_tree_layout_node)
            }
            _ => None,
        });
        self.workspaces.get_mut(&ws_id).unwrap().preferred_layout = new_layout;

        // Reattach windows under the new layout.
        for &wid in &tiling_windows {
            self.attach_window(hub, wid, ws_id);
        }

        if let Some(f) = focused {
            self.set_focus(hub, f);
        }
    }

    fn apply_config(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        self.tab_bar_height = hub.layout.partition_tree.tab_bar_height;
        self.automatic_tiling = hub.layout.partition_tree.automatic_tiling;
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
}
