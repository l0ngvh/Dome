use crate::config::SplitMode;
use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Dimension, WindowId, WorkspaceId};
use crate::core::partition_tree::{Child, Container, Parent, SpawnMode};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    /// Attach a `Child` (window or container) to a workspace. If the spawn mode is horizontal or
    /// vertical then try to insert the child next to the focused child. if it's tabbed then try to
    /// insert it into the closest tabbed container
    pub(super) fn attach_child_according_to_spawn_mode(
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

        self.compute_placement(hub, ws_id);
    }

    /// Detach a `Child` (window or container) from its workspace.
    pub(super) fn detach_child(&mut self, hub: &HubAccess, child: Child) {
        let workspace_id = self.child_workspace(hub, child);

        match self.parent(child) {
            Parent::Container(parent_id) => self.detach_child_from_container(parent_id, child),
            Parent::Workspace(_) => self.workspaces.get_mut(&workspace_id).unwrap().root = None,
        }

        if self.forget_subtree(workspace_id, child) {
            let successor = self
                .workspaces
                .get(&workspace_id)
                .unwrap()
                .focus_history
                .first()
                .copied();
            match successor {
                Some(wid) => {
                    self.set_focus_pointer(Child::Window(wid));
                }
                None => {
                    self.workspaces
                        .get_mut(&workspace_id)
                        .unwrap()
                        .focused_tiling = None
                }
            }
        }

        // Ordered after recovery so the trailing scroll_into_view clamps against the
        // surviving focus rather than the one that just left.
        self.compute_placement(hub, workspace_id);

        self.detach_preferred_slot(workspace_id, child);
    }

    /// Drop every window of `subtree` from `ws`'s focus history. Returns whether the
    /// workspace focus pointed into `subtree`, so the caller knows it owes a
    /// successor. Reads the subtree's own internals, which a structural detach
    /// leaves intact.
    fn forget_subtree(&mut self, ws: WorkspaceId, subtree: Child) -> bool {
        let nodes: Vec<_> = self.children_dfs(subtree).collect();
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

    /// Internal set_focus that works with `Child` (window or container).
    pub(super) fn set_focus(&mut self, hub: &mut HubAccess, child: Child) {
        let ws = self.set_focus_pointer(child);
        self.scroll_into_view(hub, ws);
    }

    /// The state half of `set_focus`, without the hub. Returns the workspace so
    /// callers do not re-derive it through `hub`, which can disagree with the tree
    /// mid-surgery.
    pub(super) fn set_focus_pointer(&mut self, child: Child) -> WorkspaceId {
        let path: Vec<_> = self.ancestors_of(child).collect();
        for (walk_pos, parent_id) in &path {
            let container = self.containers.get_mut(*parent_id);
            if container.is_tabbed {
                container.set_active_tab_to_child(*walk_pos);
            }
        }
        // Workspace-level focus state lives above the container tree.
        // ancestors_of terminates at the workspace boundary, so handle it here.
        let ws_child = match path.last() {
            Some((_, last_pid)) => Child::Container(*last_pid),
            None => child,
        };
        let Parent::Workspace(ws) = self.parent(ws_child) else {
            panic!("set_focus: top of ancestor path has no workspace parent");
        };
        let state = self.workspaces.get_mut(&ws).unwrap();
        state.focused_tiling = Some(child);
        if let Child::Window(wid) = child {
            state.record_focus(wid);
        }
        ws
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

    pub(super) fn containers_preorder(
        &self,
        root: ContainerId,
    ) -> impl Iterator<Item = ContainerId> + '_ {
        let mut stack = vec![root];
        let mut bound = crate::core::bounded_loop();
        std::iter::from_fn(move || {
            bound.next()?;
            let id = stack.pop()?;
            for &child in &self.containers.get(id).children {
                if let Child::Container(child_id) = child {
                    stack.push(child_id);
                }
            }
            Some(id)
        })
    }

    pub(super) fn children_dfs(&self, root: Child) -> impl Iterator<Item = Child> + '_ {
        let mut stack = vec![root];
        let mut bound = crate::core::bounded_loop();
        std::iter::from_fn(move || {
            bound.next()?;
            let child = stack.pop()?;
            if let Child::Container(cid) = child {
                for &c in &self.containers.get(cid).children {
                    stack.push(c);
                }
            }
            Some(child)
        })
    }

    pub(super) fn parent(&self, child: Child) -> Parent {
        match child {
            Child::Window(id) => self.tiling_windows.get(&id).unwrap().parent,
            Child::Container(id) => self.containers.get(id).parent,
        }
    }

    pub(super) fn set_parent(&mut self, child: Child, parent: Parent) {
        match child {
            Child::Window(id) => self.tiling_windows.get_mut(&id).unwrap().parent = parent,
            Child::Container(id) => self.containers.get_mut(id).parent = parent,
        }
    }

    pub(super) fn child_dimension(&self, child: Child) -> Dimension {
        match child {
            Child::Window(id) => self.tiling_windows.get(&id).unwrap().dimension,
            Child::Container(id) => self.containers.get(id).dimension,
        }
    }

    pub(super) fn child_workspace(&self, hub: &HubAccess, child: Child) -> WorkspaceId {
        match child {
            Child::Window(id) => hub
                .windows
                .get(id)
                .workspace()
                .expect("tiling window must have a workspace"),
            Child::Container(id) => self.containers.get(id).workspace,
        }
    }

    pub(super) fn child_spawn_mode(&self, child: Child) -> SpawnMode {
        match child {
            Child::Window(id) => self.tiling_windows.get(&id).unwrap().spawn_mode,
            Child::Container(id) => self.containers.get(id).spawn_mode(),
        }
    }

    /// Focus target when entering `subtree`. Panics if the history does not cover a
    /// container subtree.
    pub(super) fn focus_target_in(&self, subtree: Child) -> Child {
        let Child::Container(cid) = subtree else {
            return subtree;
        };
        let ws = self.containers.get(cid).workspace;
        let wid = self
            .last_focused_window_in(ws, cid)
            .expect("focus history covers every window of an in-tree subtree");
        Child::Window(wid)
    }

    /// Skips history entries that live elsewhere in the workspace.
    fn last_focused_window_in(&self, ws: WorkspaceId, subtree: ContainerId) -> Option<WindowId> {
        let history = &self.workspaces.get(&ws)?.focus_history;
        history.iter().copied().find(|&wid| {
            self.ancestors_of(Child::Window(wid))
                .any(|(_, pid)| pid == subtree)
        })
    }

    pub(super) fn assign_subtree_to_workspace(
        &mut self,
        hub: &mut HubAccess,
        child: Child,
        workspace_id: WorkspaceId,
    ) {
        let nodes: Vec<_> = self.children_dfs(child).collect();
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
                    self.containers.get_mut(cid).workspace = workspace_id;
                }
            }
        }
    }

    pub(super) fn find_tabbed_self_or_ancestor(&self, child: Child) -> Option<ContainerId> {
        if let Child::Container(id) = child
            && self.containers.get(id).is_tabbed
        {
            return Some(id);
        }
        self.ancestors_of(child)
            .map(|(_, pid)| pid)
            .find(|&pid| self.containers.get(pid).is_tabbed)
    }

    /// Ensures all child containers have different direction than their parent.
    /// Skips tabbed containers.
    pub(super) fn maintain_direction_invariance(&mut self, parent: Parent) {
        let container_id = match parent {
            Parent::Container(id) => id,
            Parent::Workspace(ws_id) => match self.workspaces.get(&ws_id).unwrap().root {
                Some(Child::Container(id)) => id,
                _ => return,
            },
        };
        let order: Vec<_> = self.containers_preorder(container_id).collect();
        for id in order {
            let Some(direction) = self.containers.get(id).direction() else {
                continue;
            };
            for &child in &self.containers.get(id).children.clone() {
                if let Child::Container(child_id) = child
                    && self.containers.get(child_id).has_direction(direction)
                {
                    self.containers.get_mut(child_id).toggle_direction();
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
        let spawn_mode = SpawnMode::from(split_mode);
        let parent = self.parent(anchor);
        let workspace_id = self.child_workspace(hub, anchor);
        let container_id = self.containers.allocate(Container::new(
            parent,
            workspace_id,
            children.clone(),
            split_mode,
        ));
        tracing::debug!("Forming container {container_id} to replace {anchor}");
        for &c in &children {
            match c {
                Child::Window(wid) => {
                    self.tiling_windows.get_mut(&wid).unwrap().spawn_mode =
                        SpawnMode::without_history(spawn_mode);
                }
                Child::Container(cid) => {
                    self.containers
                        .get_mut(cid)
                        .set_spawn_mode_reset(spawn_mode);
                }
            }
        }
        for &child in &children {
            self.set_parent(child, Parent::Container(container_id));
        }
        match parent {
            Parent::Container(cid) => self
                .containers
                .get_mut(cid)
                .replace_child_if_present(anchor, Child::Container(container_id)),
            Parent::Workspace(ws_id) => {
                self.workspaces.get_mut(&ws_id).unwrap().root =
                    Some(Child::Container(container_id));
            }
        }
        self.maintain_direction_invariance(parent);
        container_id
    }
}
