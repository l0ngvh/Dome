use crate::core::SplitMode;
use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Dimension, Direction, WindowId, WorkspaceId};
use crate::core::partition_tree::{Child, TilingContainerData};
use crate::core::slot::SlotId;

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    pub(super) fn attach_tiling_window(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot: Option<SlotId>,
    ) {
        self.tiling_windows
            .insert(window_id, TilingWindowData::new(ws_id));
        match slot {
            Some(slot_id) => {
                tracing::debug!(%window_id, ?slot_id, "Window matched preferred layout slot");
                self.attach_window_to_slot(hub, window_id, ws_id, slot_id);
            }
            None => {
                self.attach_child_according_to_spawn_direction(hub, Child::Window(window_id), ws_id)
            }
        }
    }

    pub(super) fn detach_tiling_window(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        self.detach_child(hub, Child::Window(window_id));
        self.tiling_windows.remove(&window_id);
    }

    /// Attach a `Child` (window or container) to a workspace. Tries to insert the child next to
    /// the focused child, along that child's spawn direction.
    pub(super) fn attach_child_according_to_spawn_direction(
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
            self.compute_placement(hub, ws_id);
            return;
        };

        let spawn_direction = self.child_spawn_direction(insert_anchor);

        if let Child::Container(cid) = insert_anchor
            && self
                .tiling_containers
                .get(&cid)
                .unwrap()
                .has_direction(spawn_direction)
        {
            self.attach_child_to_container(hub, child, cid, None);
        } else {
            match self.parent(insert_anchor) {
                Parent::Container(container_id) => {
                    if self
                        .tiling_containers
                        .get(&container_id)
                        .unwrap()
                        .has_direction(spawn_direction)
                    {
                        let anchor_index =
                            hub.containers.get(container_id).position_of(insert_anchor);
                        self.attach_child_to_container(
                            hub,
                            child,
                            container_id,
                            Some(anchor_index + 1),
                        );
                    } else {
                        self.replace_anchor_with_container(
                            hub,
                            insert_anchor,
                            vec![insert_anchor, child],
                            spawn_direction.into(),
                        );
                    }
                }
                Parent::Workspace(_) => {
                    self.replace_anchor_with_container(
                        hub,
                        insert_anchor,
                        vec![insert_anchor, child],
                        spawn_direction.into(),
                    );
                }
            }
        }

        self.compute_placement(hub, ws_id);
    }

    /// Detach a `Child` (window or container) from its workspace.
    pub(super) fn detach_child(&mut self, hub: &mut HubAccess, child: Child) {
        let workspace_id = self.child_workspace(hub, child);

        match self.parent(child) {
            Parent::Container(parent_id) => self.detach_child_from_container(hub, parent_id, child),
            Parent::Workspace(_) => self.workspaces.get_mut(&workspace_id).unwrap().root = None,
        }

        if self.forget_subtree(hub, workspace_id, child) {
            let successor = self
                .workspaces
                .get(&workspace_id)
                .unwrap()
                .focus_history
                .first()
                .copied();
            match successor {
                Some(wid) => {
                    self.set_focus(hub, Child::Window(wid));
                }
                None => {
                    self.workspaces
                        .get_mut(&workspace_id)
                        .unwrap()
                        .focused_tiling = None
                }
            }
        }

        self.compute_placement(hub, workspace_id);

        self.release_container_slots_in(hub, child);
    }

    /// Drop every window of `subtree` from `ws`'s focus history. Returns whether the
    /// workspace focus pointed into `subtree`, so the caller knows it owes a
    /// successor. Reads the subtree's own internals, which a structural detach
    /// leaves intact.
    fn forget_subtree(&mut self, hub: &HubAccess, ws: WorkspaceId, subtree: Child) -> bool {
        let nodes = hub.children_dfs(subtree);
        let state = self.workspaces.get_mut(&ws).unwrap();
        let mut held_focus = false;
        for node in nodes {
            held_focus |= state.focused_tiling == Some(node);
            if let Child::Window(wid) = node {
                state.drop_from_history(wid);
            }
        }
        held_focus
    }

    /// Discard `subtree`. Returns its windows, which the caller re-homes.
    pub(super) fn free_container_subtree(
        &mut self,
        hub: &mut HubAccess,
        subtree: Child,
    ) -> Vec<WindowId> {
        // Ordered before `take_windows`, which frees the entities the walk traverses.
        for node in hub.children_dfs(subtree) {
            if let Child::Container(cid) = node {
                self.tiling_containers.remove(&cid);
            }
        }
        hub.take_windows(subtree)
    }

    pub(super) fn ancestors_of(
        &self,
        start: Child,
    ) -> impl Iterator<Item = (Child, ContainerId)> + '_ {
        let mut current = Some(start);
        let mut bound = crate::core::bounded_loop();
        std::iter::from_fn(move || {
            bound.next()?;
            let child = current?;
            match self.parent(child) {
                Parent::Container(pid) => {
                    current = Some(Child::Container(pid));
                    Some((child, pid))
                }
                Parent::Workspace(_) => {
                    current = None;
                    None
                }
            }
        })
    }

    pub(super) fn parent(&self, child: Child) -> Parent {
        match child {
            Child::Window(id) => self.tiling_windows.get(&id).unwrap().parent,
            Child::Container(id) => self.tiling_containers.get(&id).unwrap().parent,
        }
    }

    pub(super) fn set_parent(&mut self, child: Child, parent: Parent) {
        match child {
            Child::Window(id) => self.tiling_windows.get_mut(&id).unwrap().parent = parent,
            Child::Container(id) => self.tiling_containers.get_mut(&id).unwrap().parent = parent,
        }
    }

    pub(super) fn child_dimension(&self, child: Child) -> Dimension {
        match child {
            Child::Window(id) => self.tiling_windows.get(&id).unwrap().dimension,
            Child::Container(id) => self.tiling_containers.get(&id).unwrap().dimension,
        }
    }

    pub(super) fn child_workspace(&self, hub: &HubAccess, child: Child) -> WorkspaceId {
        match child {
            Child::Window(id) => hub
                .windows
                .get(id)
                .workspace()
                .expect("tiling window must have a workspace"),
            Child::Container(id) => hub.containers.get(id).workspace,
        }
    }

    pub(super) fn child_spawn_direction(&self, child: Child) -> Direction {
        match child {
            Child::Window(id) => self.tiling_windows.get(&id).unwrap().spawn_direction,
            Child::Container(id) => self.tiling_containers.get(&id).unwrap().spawn_direction(),
        }
    }

    pub(super) fn assign_subtree_to_workspace(
        &mut self,
        hub: &mut HubAccess,
        child: Child,
        workspace_id: WorkspaceId,
    ) {
        let nodes = hub.children_dfs(child);
        for node in nodes {
            match node {
                Child::Window(wid) => {
                    hub.windows.get_mut(wid).set_workspace(Some(workspace_id));
                    self.workspaces
                        .get_mut(&workspace_id)
                        .unwrap()
                        .add_to_history(wid);
                }
                Child::Container(cid) => {
                    hub.containers.get_mut(cid).workspace = workspace_id;
                }
            }
        }
    }

    pub(super) fn find_tabbed_self_or_ancestor(&self, child: Child) -> Option<ContainerId> {
        if let Child::Container(id) = child
            && self.tiling_containers.get(&id).unwrap().is_tabbed
        {
            return Some(id);
        }
        self.ancestors_of(child)
            .map(|(_, pid)| pid)
            .find(|&pid| self.tiling_containers.get(&pid).unwrap().is_tabbed)
    }

    /// Ensures all child containers have different direction than their parent.
    /// Skips tabbed containers.
    pub(super) fn maintain_direction_invariance(&mut self, hub: &HubAccess, parent: Parent) {
        let container_id = match parent {
            Parent::Container(id) => id,
            Parent::Workspace(ws_id) => match self.workspaces.get(&ws_id).unwrap().root {
                Some(Child::Container(id)) => id,
                _ => return,
            },
        };
        let order = hub.containers_preorder(container_id);
        for id in order {
            let Some(direction) = self.tiling_containers.get(&id).unwrap().direction() else {
                continue;
            };
            for &child in hub.containers.get(id).children() {
                if let Child::Container(child_id) = child
                    && self
                        .tiling_containers
                        .get(&child_id)
                        .unwrap()
                        .has_direction(direction)
                {
                    self.tiling_containers
                        .get_mut(&child_id)
                        .unwrap()
                        .toggle_direction();
                }
            }
        }
    }

    /// Replace anchor with a new container containing the given `children`.
    pub(super) fn replace_anchor_with_container(
        &mut self,
        hub: &mut HubAccess,
        anchor: Child,
        children: Vec<Child>,
        split_mode: SplitMode,
    ) -> ContainerId {
        let parent = self.parent(anchor);
        let workspace_id = self.child_workspace(hub, anchor);
        let container_id = hub.allocate_container(children.clone(), workspace_id);
        self.tiling_containers
            .insert(container_id, TilingContainerData::new(parent, split_mode));
        let spawn_direction = self
            .tiling_containers
            .get(&container_id)
            .unwrap()
            .spawn_direction();
        tracing::debug!("Forming container {container_id} to replace {anchor}");
        for &c in &children {
            match c {
                Child::Window(wid) => {
                    self.tiling_windows.get_mut(&wid).unwrap().spawn_direction = spawn_direction;
                }
                Child::Container(cid) => {
                    self.tiling_containers
                        .get_mut(&cid)
                        .unwrap()
                        .set_spawn_direction(spawn_direction);
                }
            }
        }
        for &child in &children {
            self.set_parent(child, Parent::Container(container_id));
        }
        match parent {
            Parent::Container(cid) => hub
                .containers
                .get_mut(cid)
                .replace_child_if_present(anchor, Child::Container(container_id)),
            Parent::Workspace(ws_id) => {
                self.workspaces.get_mut(&ws_id).unwrap().root =
                    Some(Child::Container(container_id));
            }
        }
        self.maintain_direction_invariance(hub, parent);
        container_id
    }
}

/// Parent role in the partition tree. A `Container` parents other nodes. A
/// `Workspace` parents only the root node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Parent {
    Container(ContainerId),
    Workspace(WorkspaceId),
}

impl std::fmt::Display for Parent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Parent::Container(id) => write!(f, "{}", id),
            Parent::Workspace(id) => write!(f, "{}", id),
        }
    }
}

/// Per-window tiling state.
#[derive(Debug)]
pub(super) struct TilingWindowData {
    pub(super) parent: Parent,
    pub(super) dimension: Dimension,
    pub(super) spawn_direction: Direction,
}

impl TilingWindowData {
    pub(super) fn new(workspace: WorkspaceId) -> Self {
        Self::with_parent(Parent::Workspace(workspace))
    }

    pub(super) fn in_container(container: ContainerId) -> Self {
        Self::with_parent(Parent::Container(container))
    }

    fn with_parent(parent: Parent) -> Self {
        TilingWindowData {
            parent,
            dimension: Dimension::default(),
            spawn_direction: Direction::default(),
        }
    }
}
