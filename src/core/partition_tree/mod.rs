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
use crate::core::node::{Dimension, WindowId, WorkspaceId};
use crate::core::strategy::{TilingAction, TilingPlacements, TilingStrategy, clip, translate};

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
    /// `Child::Container` after `focus_parent`, entering container-highlight mode where
    /// `focused_tiling_window()` returns `None`. Can only be None in an empty workspace.
    ///
    /// Invariant: if `focused_tiling == Some(X)`, every ancestor container of X has
    /// `focused == X`. Walking `container.focused` from root reaches X directly.
    /// Established by `set_focus_child`, preserved by `replace_split_child_focus`.
    focused_tiling: Option<Child>,
    viewport_offset: (f32, f32),
}

/// i3-style manual tiling strategy. Manages a container tree where windows are
/// leaves and containers define split direction (horizontal/vertical) or tabbed
/// layout. This is the default (and currently only) tiling strategy.
#[derive(Debug)]
pub(crate) struct PartitionTreeStrategy {
    containers: Allocator<Container>,
    tiling_windows: HashMap<WindowId, TilingWindowData>,
    workspaces: HashMap<WorkspaceId, WorkspaceTilingState>,
}

impl PartitionTreeStrategy {
    pub(crate) fn new() -> Self {
        Self {
            containers: Allocator::new(),
            tiling_windows: HashMap::new(),
            workspaces: HashMap::new(),
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

    /// Internal attach that works with `Child` (window or container). The public
    /// `attach_window` wraps a `WindowId` and delegates here. `move_focused_to_workspace`
    /// calls this directly to handle container moves.
    fn attach_child_internal(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId) {
        self.set_workspace(hub, child, ws_id);
        if let Child::Window(wid) = child {
            self.tiling_windows.insert(
                wid,
                TilingWindowData {
                    parent: Parent::Workspace(ws_id),
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

        let spawn_mode = match insert_anchor {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => self.tiling_data(id).spawn_mode,
        };

        if spawn_mode.is_tab()
            && let Some(tabbed_ancestor) = self.find_tabbed_ancestor(insert_anchor)
        {
            let container = self.containers.get(tabbed_ancestor);
            self.attach_split_child_to_container(
                hub,
                child,
                tabbed_ancestor,
                Some(container.active_tab_index() + 1),
            );
        } else if let Child::Container(cid) = insert_anchor
            && self.containers.get(cid).can_accomodate(spawn_mode)
        {
            self.attach_split_child_to_container(hub, child, cid, None);
        } else {
            match self.get_parent(insert_anchor) {
                Parent::Container(container_id) => {
                    self.try_attach_split_child_to_container_next_to(
                        hub,
                        child,
                        container_id,
                        insert_anchor,
                    );
                }
                Parent::Workspace(workspace_id) => {
                    self.attach_split_child_next_to_workspace_root(hub, child, workspace_id);
                }
            }
        }

        self.layout_workspace(hub, ws_id);
        self.set_focus_child(hub, child);
    }

    /// Internal detach that works with `Child` (window or container). The public
    /// `detach_window` wraps a `WindowId` and delegates here. `move_focused_to_workspace`
    /// calls this directly to handle container moves.
    fn detach_child_internal(&mut self, hub: &mut HubAccess, child: Child) {
        let workspace_id = match child {
            Child::Window(id) => hub.windows.get(id).workspace,
            Child::Container(id) => self.containers.get(id).workspace,
        };

        let parent = self.get_parent(child);
        match parent {
            Parent::Container(parent_id) => {
                self.detach_split_child_from_container(parent_id, child);
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

    /// Internal set_focus that works with `Child` (window or container). The public
    /// `set_focus` wraps a `WindowId` and delegates here.
    ///
    /// Writes `child` (the original argument, which can be a window or container) to
    /// `container.focused` on every ancestor from `child` up to the workspace root. This
    /// means all ancestors share the same `focused` value. For tabbed containers, also calls
    /// `set_active_tab(current)` with the direct child (the walk position), not the target.
    ///
    /// At the workspace level, sets `focused_tiling = Some(child)` and clears
    /// `is_float_focused`.
    fn set_focus_child(&mut self, hub: &mut HubAccess, child: Child) {
        let mut current = child;
        for _ in crate::core::bounded_loop() {
            match self.get_parent(current) {
                Parent::Container(cid) => {
                    let container = self.containers.get_mut(cid);
                    if container.is_tabbed {
                        container.set_active_tab(current);
                    }
                    container.focused = child;
                    current = Child::Container(cid);
                }
                Parent::Workspace(ws) => {
                    self.ws_state_or_default(ws).focused_tiling = Some(child);
                    hub.workspaces.get_mut(ws).is_float_focused = false;
                    self.scroll_into_view(hub, ws);
                    break;
                }
            }
        }
    }
}

impl TilingStrategy for PartitionTreeStrategy {
    fn attach_window(&mut self, hub: &mut HubAccess, window_id: WindowId, ws_id: WorkspaceId) {
        self.attach_child_internal(hub, Child::Window(window_id), ws_id);
    }

    fn detach_window(&mut self, hub: &mut HubAccess, window_id: WindowId) -> Dimension {
        let child_dim = self.tiling_data(window_id).dimension;
        let workspace_id = hub.windows.get(window_id).workspace;
        let (offset_x, offset_y) = self.ws_state(workspace_id).viewport_offset;
        let screen = hub
            .monitors
            .get(hub.workspaces.get(workspace_id).monitor)
            .dimension;

        // Capture offset/screen before detach because detach triggers layout,
        // which can change viewport_offset.
        self.detach_child_internal(hub, Child::Window(window_id));

        // Convert layout-space coordinates to screen-absolute. Layout positions are
        // relative to workspace origin (0,0) plus viewport offset; screen-absolute
        // includes the monitor's origin.
        Dimension {
            x: child_dim.x - offset_x + screen.x,
            y: child_dim.y - offset_y + screen.y,
            ..child_dim
        }
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
            TilingAction::ToggleDirection => self.toggle_direction(hub),
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

        let mut stack: Vec<Child> = ws_state.root.into_iter().collect();
        for _ in crate::core::bounded_loop() {
            let Some(child) = stack.pop() else { break };
            match child {
                Child::Window(id) => {
                    let td = self.tiling_data(id);
                    let frame = translate(td.dimension, offset_x, offset_y, screen);
                    if let Some(visible_frame) = clip(frame, screen) {
                        let is_highlighted = focused == Some(Child::Window(id));
                        windows.push(TilingWindowPlacement {
                            id,
                            frame,
                            visible_frame,
                            is_highlighted,
                            spawn_indicator: if is_highlighted {
                                Some(SpawnIndicator::from_spawn_mode(td.spawn_mode))
                            } else {
                                None
                            },
                        });
                    }
                }
                Child::Container(id) => {
                    let container = self.containers.get(id);
                    let frame = translate(container.dimension, offset_x, offset_y, screen);
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
                            Some(SpawnIndicator::from_spawn_mode(container.spawn_mode()))
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
                        children: container.children().to_vec(),
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

    fn move_focused_to_workspace(
        &mut self,
        hub: &mut HubAccess,
        from_ws: WorkspaceId,
        to_ws: WorkspaceId,
    ) {
        let Some(focused) = self.workspaces.get(&from_ws).and_then(|s| s.focused_tiling) else {
            return;
        };
        self.detach_child_internal(hub, focused);
        self.attach_child_internal(hub, focused, to_ws);
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
        let mut count = 0;
        let mut stack = vec![root];
        for _ in crate::core::bounded_loop() {
            let Some(child) = stack.pop() else { break };
            match child {
                Child::Window(_) => count += 1,
                Child::Container(id) => {
                    for &c in self.containers.get(id).children() {
                        stack.push(c);
                    }
                }
            }
        }
        count
    }

    fn prune_workspace(&mut self, ws_id: WorkspaceId) {
        self.workspaces.remove(&ws_id);
    }

    #[cfg(test)]
    fn validate_tree(&self, hub: &HubAccess) {
        for (workspace_id, workspace) in hub.workspaces.all_active() {
            self.validate_workspace_focus(hub, workspace_id, &workspace);

            let Some(root) = self.workspaces.get(&workspace_id).and_then(|s| s.root) else {
                continue;
            };
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
