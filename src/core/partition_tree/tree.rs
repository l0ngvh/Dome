use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, WorkspaceId};
use crate::core::partition_tree::{Child, Parent, SpawnMode};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    pub(super) fn get_parent(&self, child: Child) -> Parent {
        match child {
            Child::Window(id) => self.tiling_data(id).parent,
            Child::Container(id) => self.containers.get(id).parent,
        }
    }

    pub(super) fn set_parent(&mut self, child: Child, parent: Parent) {
        match child {
            Child::Window(id) => self.tiling_data_mut(id).parent = parent,
            Child::Container(id) => self.containers.get_mut(id).parent = parent,
        }
    }

    pub(super) fn set_workspace(
        &mut self,
        hub: &mut HubAccess,
        child: Child,
        workspace_id: WorkspaceId,
    ) {
        let mut stack = vec![child];
        for _ in crate::core::bounded_loop() {
            let Some(current) = stack.pop() else { break };
            match current {
                Child::Window(wid) => {
                    hub.windows.get_mut(wid).workspace = workspace_id;
                }
                Child::Container(cid) => {
                    self.containers.get_mut(cid).workspace = workspace_id;
                    stack.extend(self.containers.get(cid).children.iter().copied());
                }
            }
        }
    }

    pub(super) fn find_tabbed_ancestor(&self, child: Child) -> Option<ContainerId> {
        let mut current = child;
        for _ in crate::core::bounded_loop() {
            if let Child::Container(id) = current
                && self.containers.get(id).is_tabbed
            {
                return Some(id);
            }
            match self.get_parent(current) {
                Parent::Container(id) => current = Child::Container(id),
                Parent::Workspace(_) => return None,
            }
        }
        unreachable!()
    }

    /// Ensures all child containers have different direction than their parent.
    /// Skips tabbed containers.
    pub(super) fn maintain_direction_invariance(&mut self, parent: Parent) {
        let container_id = match parent {
            Parent::Container(id) => id,
            Parent::Workspace(ws_id) => match self.ws_state(ws_id).root {
                Some(Child::Container(id)) => id,
                _ => return,
            },
        };
        let mut stack = vec![container_id];
        for _ in crate::core::bounded_loop() {
            let Some(id) = stack.pop() else {
                return;
            };
            let Some(direction) = self.containers.get(id).direction() else {
                continue;
            };

            for &child in &self.containers.get(id).children.clone() {
                if let Child::Container(child_id) = child {
                    if self.containers.get(child_id).has_direction(direction) {
                        self.containers.get_mut(child_id).toggle_direction();
                    }
                    stack.push(child_id);
                }
            }
        }
    }

    /// Replace anchor with a new container containing children.
    /// Gets parent, workspace, and dimension from anchor.
    pub(super) fn replace_anchor_with_container(
        &mut self,
        hub: &mut HubAccess,
        children: Vec<Child>,
        anchor: Child,
        spawn_mode: SpawnMode,
    ) -> ContainerId {
        use crate::core::partition_tree::Container;

        let (parent, workspace_id, dimension) = match anchor {
            Child::Window(wid) => {
                let td = self.tiling_data(wid);
                (td.parent, hub.windows.get(wid).workspace, td.dimension)
            }
            Child::Container(cid) => {
                let c = self.containers.get(cid);
                (c.parent, c.workspace, c.dimension)
            }
        };
        let container_id = if let Some(direction) = spawn_mode.as_direction() {
            let container_id = self.containers.allocate(Container::split(
                parent,
                workspace_id,
                children.clone(),
                anchor,
                dimension,
                direction,
            ));
            for child in children {
                match child {
                    Child::Window(wid) => {
                        let td = self.tiling_data_mut(wid);
                        td.spawn_mode = SpawnMode::clean(spawn_mode);
                        td.parent = Parent::Container(container_id);
                    }
                    Child::Container(cid) => {
                        self.containers.get_mut(cid).parent = Parent::Container(container_id);
                    }
                }
            }
            container_id
        } else {
            let container_id = self.containers.allocate(Container::tabbed(
                parent,
                workspace_id,
                children.clone(),
                anchor,
                dimension,
            ));
            for child in children {
                match child {
                    Child::Window(wid) => {
                        let td = self.tiling_data_mut(wid);
                        td.spawn_mode = SpawnMode::clean(spawn_mode);
                        td.parent = Parent::Container(container_id);
                    }
                    Child::Container(cid) => {
                        self.containers.get_mut(cid).set_spawn_mode(spawn_mode);
                        self.containers.get_mut(cid).parent = Parent::Container(container_id);
                    }
                }
            }
            container_id
        };
        self.maintain_direction_invariance(Parent::Container(container_id));
        container_id
    }

    /// Attach child to existing container. Does not change focus.
    pub(super) fn attach_split_child_to_container(
        &mut self,
        _hub: &mut HubAccess,
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
        match child {
            Child::Window(wid) => {
                let td = self.tiling_data_mut(wid);
                td.spawn_mode = SpawnMode::clean(container_spawn_mode);
                td.parent = Parent::Container(container_id);
            }
            Child::Container(cid) => {
                self.containers.get_mut(cid).parent = Parent::Container(container_id);
            }
        }
        self.maintain_direction_invariance(Parent::Container(container_id));
    }

    /// Attach `child` next to `anchor` in container, or create a new parent
    /// to house both if the container can't accommodate the spawn mode.
    pub(super) fn try_attach_split_child_to_container_next_to(
        &mut self,
        hub: &mut HubAccess,
        child: Child,
        container_id: ContainerId,
        anchor: Child,
    ) {
        let spawn_mode = match anchor {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => self.tiling_data(id).spawn_mode,
        };
        let parent_container = self.containers.get(container_id);
        if parent_container.can_accomodate(spawn_mode) {
            let anchor_index = self.containers.get(container_id).position_of(anchor);
            self.attach_split_child_to_container(hub, child, container_id, Some(anchor_index + 1));
        } else {
            let new_container_id =
                self.replace_anchor_with_container(hub, vec![anchor, child], anchor, spawn_mode);
            self.containers
                .get_mut(container_id)
                .replace_child(anchor, Child::Container(new_container_id));
        }
    }

    pub(super) fn attach_split_child_next_to_workspace_root(
        &mut self,
        hub: &mut HubAccess,
        child: Child,
        workspace_id: WorkspaceId,
    ) {
        let anchor = self.ws_state(workspace_id).root.unwrap();
        let spawn_mode = match anchor {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => self.tiling_data(id).spawn_mode,
        };
        let new_container_id =
            self.replace_anchor_with_container(hub, vec![anchor, child], anchor, spawn_mode);
        self.ws_state_mut(workspace_id).root = Some(Child::Container(new_container_id));
    }

    /// Replace all focus references from old_child to new_child. Walks up
    /// from old_child to find the highest container focusing it, then walks
    /// up from new_child updating focus pointers and active tabs.
    ///
    /// Always checks focused_tiling even if the container focus
    /// chain stops early. focused_tiling can point to old_child even when
    /// the focus chain from root doesn't lead to it (e.g., after
    /// focus_parent changes the chain without clearing focused_tiling).
    fn replace_split_child_focus(
        &mut self,
        hub: &mut HubAccess,
        old_child: Child,
        new_child: Child,
    ) {
        let mut current = old_child;
        let mut highest_focusing_container = None;
        for _ in crate::core::bounded_loop() {
            match self.get_parent(current) {
                Parent::Container(cid) => {
                    if self.containers.get(cid).focused == old_child {
                        highest_focusing_container = Some(cid)
                    } else {
                        break;
                    }
                    current = Child::Container(cid);
                }
                Parent::Workspace(_) => {
                    highest_focusing_container = None;
                    break;
                }
            }
        }

        let mut current = new_child;
        let mut reached_workspace = false;
        for _ in crate::core::bounded_loop() {
            match self.get_parent(current) {
                Parent::Container(cid) => {
                    let container = self.containers.get_mut(cid);
                    if container.focused == old_child {
                        if container.is_tabbed {
                            container.set_active_tab(current);
                        }
                        container.focused = new_child;
                    }

                    if highest_focusing_container.is_some_and(|c| c == cid) {
                        break;
                    }
                    current = Child::Container(cid);
                }
                Parent::Workspace(ws) => {
                    reached_workspace = true;
                    if self.ws_state(ws).focused_tiling == Some(old_child) {
                        self.ws_state_mut(ws).focused_tiling = Some(new_child);
                        tracing::debug!(?old_child, ?new_child, "Workspace focus replaced");
                    }
                    break;
                }
            }
        }

        // The container focus chain can diverge from focused_tiling when
        // focus_parent sets focused_tiling to a container, then set_focus
        // on a different subtree changes the chain without touching the
        // original leaf. When that happens the second loop stops at
        // highest_focusing_container and never reaches the workspace.
        // Patch up focused_tiling directly.
        if !reached_workspace {
            let ws_id = match old_child {
                Child::Window(id) => hub.windows.get(id).workspace,
                Child::Container(id) => self.containers.get(id).workspace,
            };
            if self.ws_state(ws_id).focused_tiling == Some(old_child) {
                self.ws_state_mut(ws_id).focused_tiling = Some(new_child);
                tracing::debug!(
                    ?old_child,
                    ?new_child,
                    "Workspace focus replaced (fallback)"
                );
            }
        }
    }

    /// Detach child from container and replace focus to sibling.
    /// Deletes container if only one child remains.
    pub(super) fn detach_split_child_from_container(
        &mut self,
        hub: &mut HubAccess,
        container_id: ContainerId,
        child: Child,
    ) {
        tracing::debug!(%child, %container_id, "Detaching child from container");
        let children = &self.containers.get(container_id).children;
        let pos = children.iter().position(|c| *c == child).unwrap();
        let sibling = if pos > 0 {
            children[pos - 1]
        } else {
            children[pos + 1]
        };
        let new_focus = match sibling {
            Child::Window(_) => sibling,
            Child::Container(c) => self.containers.get(c).focused,
        };
        self.replace_split_child_focus(hub, child, new_focus);

        self.containers.get_mut(container_id).remove_child(child);
        if self.containers.get(container_id).children.len() == 1 {
            self.delete_container(hub, container_id);
        }
    }

    /// Delete a container with exactly one child remaining. Promotes the last
    /// child to grandparent.
    fn delete_container(&mut self, hub: &mut HubAccess, container_id: ContainerId) {
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
                .replace_child(Child::Container(container_id), last_child),
            Parent::Workspace(ws) => self.ws_state_mut(ws).root = Some(last_child),
        }

        self.replace_split_child_focus(hub, Child::Container(container_id), last_child);

        self.containers.delete(container_id);
        self.maintain_direction_invariance(grandparent);
    }
}
