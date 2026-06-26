mod layout;
mod navigate;
mod tree;
mod types;
#[cfg(test)]
mod validate;

pub(crate) use types::*;

use std::collections::HashMap;

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

    fn ws_state(&self, ws_id: WorkspaceId) -> &WorkspaceTilingState {
        self.workspaces
            .get(&ws_id)
            .unwrap_or_else(|| panic!("no WorkspaceTilingState for {ws_id}"))
    }

    fn ws_state_mut(&mut self, ws_id: WorkspaceId) -> &mut WorkspaceTilingState {
        self.workspaces
            .get_mut(&ws_id)
            .unwrap_or_else(|| panic!("no WorkspaceTilingState for {ws_id}"))
    }

    fn ws_state_or_default(&mut self, ws_id: WorkspaceId) -> &mut WorkspaceTilingState {
        self.workspaces.entry(ws_id).or_default()
    }

    fn tiling_data(&self, id: WindowId) -> &TilingWindowData {
        self.tiling_windows
            .get(&id)
            .unwrap_or_else(|| panic!("no TilingWindowData for {id:?} -- not a tiling window"))
    }

    fn tiling_data_mut(&mut self, id: WindowId) -> &mut TilingWindowData {
        self.tiling_windows
            .get_mut(&id)
            .unwrap_or_else(|| panic!("no TilingWindowData for {id:?} -- not a tiling window"))
    }

    /// Attach a `Child` (window or container) to a workspace.
    fn attach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId) {
        self.assign_subtree_to_workspace(hub, child, ws_id);
        if let Child::Window(wid) = child {
            self.tiling_windows.insert(
                wid,
                TilingWindowData {
                    parent: Parent::Workspace(ws_id),
                    // Zero placeholder -- layout_workspace at the end of this function
                    // computes the real rect before any reader observes this entry.
                    dimension: Dimension::default(),
                    spawn_mode: SpawnMode::default(),
                },
            );
        }
        let state = self.ws_state_or_default(ws_id);
        let insert_anchor = state.focused_tiling.or(state.root);
        let Some(insert_anchor) = insert_anchor else {
            self.ws_state_or_default(ws_id).root = Some(child);
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
                hub,
                child,
                tabbed_self_or_ancestor,
                Some(container.active_tab_index() + 1),
            );
        } else if let Child::Container(cid) = insert_anchor
            && self.containers.get(cid).can_accommodate(spawn_mode)
        {
            self.attach_child_to_container(hub, child, cid, None);
        } else {
            match self.parent(insert_anchor) {
                Parent::Container(container_id) => {
                    self.attach_child_next_to_anchor(hub, child, container_id, insert_anchor);
                }
                Parent::Workspace(workspace_id) => {
                    self.attach_child_next_to_workspace_root(hub, child, workspace_id);
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
                self.ws_state_mut(workspace_id).root = None;
                self.ws_state_mut(workspace_id).focused_tiling = None;

                let ws = hub.workspaces.get_mut(workspace_id);
                ws.is_float_focused = !ws.float_windows.is_empty();

                self.layout_workspace(hub, workspace_id);
            }
        }

        if let Child::Window(wid) = child {
            self.tiling_windows.remove(&wid);
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
        self.ws_state_or_default(ws).focused_tiling = Some(child);
        hub.workspaces.get_mut(ws).is_float_focused = false;
        self.scroll_into_view(hub, ws);
    }
}

impl TilingStrategy for PartitionTreeStrategy {
    fn attach_window(&mut self, hub: &mut HubAccess, window_id: WindowId, ws_id: WorkspaceId) {
        self.attach_child(hub, Child::Window(window_id), ws_id);
    }

    fn detach_window(&mut self, hub: &mut HubAccess, window_id: WindowId) -> Dimension {
        let child_dim = self.tiling_data(window_id).dimension;
        let workspace_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("detaching tiling window has a workspace");
        let (offset_x, offset_y) = self.ws_state(workspace_id).viewport_offset;
        let screen = hub
            .monitors
            .get(hub.workspaces.get(workspace_id).monitor)
            .dimension;

        // Capture offset/screen before detach because detach triggers layout,
        // which can change viewport_offset.
        self.detach_child(hub, Child::Window(window_id));

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
                                Some(SpawnIndicator::from_spawn_mode(
                                    self.child_spawn_mode(child),
                                ))
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
                            Some(SpawnIndicator::from_spawn_mode(
                                self.child_spawn_mode(child),
                            ))
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
        Some(focused)
    }

    fn reattach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId) {
        self.attach_child(hub, child, ws_id);
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

/// Per-window tiling state. Containers store the same fields in the container
/// allocator; this is the window equivalent, owned by the strategy rather than
/// the shared Window struct because these fields are meaningless for float and
/// fullscreen windows.
#[derive(Debug)]
struct TilingWindowData {
    parent: Parent,
    dimension: Dimension,
    spawn_mode: SpawnMode,
}

/// Per-workspace tiling state owned by the strategy. Moved out of Workspace
/// so that other strategies can manage their own per-workspace state without
/// polluting the shared Workspace struct.
#[derive(Debug, Default)]
struct WorkspaceTilingState {
    root: Option<Child>,
    /// Tiling focus pointer. Usually a `Child::Window` (the focused window). Can be
    /// `Child::Container` for container-highlight mode, where
    /// `focused_tiling_window()` returns `None`. Can only be None in an empty workspace.
    ///
    /// Anchors invariant 3 of `Container`: when this is `Some(X)`, every ancestor
    /// container of X has `focused == X`. Established by `set_focus_child`,
    /// preserved by `replace_child_focus`.
    focused_tiling: Option<Child>,
    viewport_offset: (Length, Length),
}

impl SpawnIndicator {
    fn from_spawn_mode(mode: SpawnMode) -> Self {
        Self {
            top: mode.is_tab(),
            right: mode.is_horizontal(),
            bottom: mode.is_vertical(),
            left: false,
        }
    }
}
