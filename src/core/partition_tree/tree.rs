use crate::config::SplitMode;
use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Dimension, WorkspaceId};
use crate::core::partition_tree::{Child, Container, Parent, SpawnMode};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
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
                .expect("tiling window has a workspace"),
            Child::Container(id) => self.containers.get(id).workspace,
        }
    }

    pub(super) fn child_spawn_mode(&self, child: Child) -> SpawnMode {
        match child {
            Child::Window(id) => self.tiling_windows.get(&id).unwrap().spawn_mode,
            Child::Container(id) => self.containers.get(id).spawn_mode(),
        }
    }

    /// One step down the focus chain. Does not recurse.
    pub(super) fn descend_to_focused(&self, child: Child) -> Child {
        match child {
            Child::Window(_) => child,
            Child::Container(id) => self.containers.get(id).focused,
        }
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

    /// Replace anchor with a new container containing `anchor` and `other`.
    pub(super) fn replace_anchor_with_container(
        &mut self,
        hub: &mut HubAccess,
        anchor: Child,
        other: Child,
        split_mode: SplitMode,
    ) -> ContainerId {
        let spawn_mode = SpawnMode::from(split_mode);
        let children = vec![anchor, other];
        let parent = self.parent(anchor);
        let workspace_id = self.child_workspace(hub, anchor);
        let container_id = self.containers.allocate(Container::new(
            parent,
            workspace_id,
            children.clone(),
            anchor,
            split_mode,
        ));
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

    /// Attach child to existing container. Does not change focus.
    pub(super) fn attach_child_to_container(
        &mut self,
        child: Child,
        container_id: ContainerId,
        insert_pos: Option<usize>,
    ) {
        let parent = self.containers.get_mut(container_id);
        if let Some(pos) = insert_pos {
            parent.children.insert(pos, child);
        } else {
            parent.children.push(child);
        }
        let container_spawn_mode = self.containers.get(container_id).spawn_mode();
        if let Child::Window(wid) = child {
            self.tiling_windows.get_mut(&wid).unwrap().spawn_mode =
                SpawnMode::without_history(container_spawn_mode);
        }
        self.set_parent(child, Parent::Container(container_id));
        self.maintain_direction_invariance(Parent::Container(container_id));
    }

    /// Maintain invariant 3 of `Container` after replacing `old_child` with
    /// `new_child` in the tree. Two-walk algorithm:
    ///
    /// Walk 1 (scope): from `old_child`, find the highest ancestor still
    /// focusing `old_child`. Stops at the first ancestor that does not focus
    /// `old_child`. If the walk reaches the workspace, scope is the entire
    /// path (and `focused_tiling` is also checked in walk 2).
    ///
    /// Walk 2 (replace): from `new_child`, rewrite `focused = new_child` and
    /// update active tabs on every ancestor that still has `focused ==
    /// old_child`. Stops at the scope boundary from walk 1. If scope reached
    /// the workspace, also replaces `focused_tiling` if it pointed to
    /// `old_child`.
    fn replace_child_focus(&mut self, old_child: Child, new_child: Child) {
        // Walk 1 (scope): find how far up old_child's focus extends.
        // focus_chain_top = None means scope reaches the workspace.
        let mut focus_chain_top = None;
        let mut reached_workspace = true;
        for (_, parent_id) in self.ancestors_of(old_child) {
            if self.containers.get(parent_id).focused == old_child {
                focus_chain_top = Some(parent_id);
            } else {
                reached_workspace = false;
                break;
            }
        }
        if reached_workspace {
            focus_chain_top = None;
        }

        // Walk 2 (replace): walk up from new_child, replacing focus references.
        let path: Vec<_> = self.ancestors_of(new_child).collect();
        let mut hit_boundary = false;
        for (walk_pos, parent_id) in &path {
            let container = self.containers.get_mut(*parent_id);
            if container.focused == old_child {
                if container.is_tabbed {
                    container.set_active_tab_to_child(*walk_pos);
                }
                container.focused = new_child;
            }
            if focus_chain_top.is_some_and(|c| c == *parent_id) {
                hit_boundary = true;
                break;
            }
        }
        // If scope reached workspace and walk 2 didn't hit a boundary, update workspace focus.
        if !hit_boundary {
            let ws_child = match path.last() {
                Some((_, last_pid)) => Child::Container(*last_pid),
                None => new_child,
            };
            if let Parent::Workspace(ws) = self.parent(ws_child)
                && self.workspaces.get(&ws).unwrap().focused_tiling == Some(old_child)
            {
                self.workspaces.get_mut(&ws).unwrap().focused_tiling = Some(new_child);
                tracing::debug!(?old_child, ?new_child, "Workspace focus replaced");
            }
        }
    }

    /// Detach child from container and replace focus to sibling.
    /// Deletes container if only one child remains.
    pub(super) fn detach_child_from_container(&mut self, container_id: ContainerId, child: Child) {
        tracing::debug!(%child, %container_id, "Detaching child from container");
        let children = &self.containers.get(container_id).children;
        let pos = children.iter().position(|c| *c == child).unwrap();
        let sibling = if pos > 0 {
            children[pos - 1]
        } else {
            children[pos + 1]
        };
        let new_focus = self.descend_to_focused(sibling);
        self.replace_child_focus(child, new_focus);

        self.containers.get_mut(container_id).remove_child(child);
        if self.containers.get(container_id).children.len() == 1 {
            self.delete_container(container_id);
        }
    }

    /// Delete a container with exactly one child remaining. Promotes the last
    /// child to grandparent.
    fn delete_container(&mut self, container_id: ContainerId) {
        debug_assert_eq!(self.containers.get(container_id).children.len(), 1);
        let grandparent = self.containers.get(container_id).parent;
        let last_child = self
            .containers
            .get_mut(container_id)
            .children
            .pop()
            .unwrap();

        tracing::debug!(%container_id, %last_child, "Container has one child left, cleaning up");
        self.set_parent(last_child, grandparent);
        match grandparent {
            Parent::Container(gp) => self
                .containers
                .get_mut(gp)
                .replace_child_if_present(Child::Container(container_id), last_child),
            Parent::Workspace(ws) => self.workspaces.get_mut(&ws).unwrap().root = Some(last_child),
        }

        self.replace_child_focus(Child::Container(container_id), last_child);

        self.containers.delete(container_id);
        self.maintain_direction_invariance(grandparent);
    }
}
