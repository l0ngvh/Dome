use crate::core::hub::HubAccess;
use crate::core::node::ContainerId;
use crate::core::partition_tree::{Child, Parent, PartitionTreeStrategy, SpawnMode};

impl PartitionTreeStrategy {
    /// Delete a container with exactly one child remaining. Promotes the last
    /// child to grandparent.
    pub(super) fn delete_container(&mut self, hub: &mut HubAccess, container_id: ContainerId) {
        debug_assert_eq!(hub.containers.get(container_id).children.len(), 1);
        let grandparent = self.tiling_containers.get(&container_id).unwrap().parent;
        let ws = self.tiling_containers.get(&container_id).unwrap().workspace;
        let last_child = hub.containers.get_mut(container_id).children.pop().unwrap();

        tracing::debug!(%container_id, %last_child, "Container has one child left, cleaning up");
        self.set_parent(last_child, grandparent);
        match grandparent {
            Parent::Container(gp) => hub
                .containers
                .get_mut(gp)
                .replace_child_if_present(Child::Container(container_id), last_child),
            Parent::Workspace(ws) => self.workspaces.get_mut(&ws).unwrap().root = Some(last_child),
        }

        if self.workspaces.get(&ws).unwrap().focused_tiling == Some(Child::Container(container_id))
        {
            self.set_focus_pointer(hub, last_child);
        }

        self.clean_up_occupied_container(container_id);
        hub.free_container(container_id);
        self.tiling_containers.remove(&container_id);
        self.maintain_direction_invariance(hub, grandparent);
    }

    /// Attach child to existing container. Does not change focus.
    pub(super) fn attach_child_to_container(
        &mut self,
        hub: &mut HubAccess,
        child: Child,
        container_id: ContainerId,
        insert_pos: Option<usize>,
    ) {
        let parent = hub.containers.get_mut(container_id);
        if let Some(pos) = insert_pos {
            parent.children.insert(pos, child);
        } else {
            parent.children.push(child);
        }
        let container_spawn_mode = self
            .tiling_containers
            .get(&container_id)
            .unwrap()
            .spawn_mode();
        if let Child::Window(wid) = child {
            self.tiling_windows.get_mut(&wid).unwrap().spawn_mode =
                SpawnMode::without_history(container_spawn_mode);
        }
        self.set_parent(child, Parent::Container(container_id));
        self.maintain_direction_invariance(hub, Parent::Container(container_id));
    }

    /// Detach child from container. Deletes the container if only one child
    /// remains. Focus recovery belongs to `detach_child`, which knows whether the
    /// child is leaving the workspace or being relocated inside it.
    pub(super) fn detach_child_from_container(
        &mut self,
        hub: &mut HubAccess,
        container_id: ContainerId,
        child: Child,
    ) {
        tracing::debug!(%child, %container_id, "Detaching child from container");
        self.remove_child(hub, container_id, child);
        if hub.containers.get(container_id).children.len() == 1 {
            self.delete_container(hub, container_id);
        }
    }

    pub(super) fn active_tab(&self, hub: &HubAccess, container_id: ContainerId) -> Option<Child> {
        let data = self.tiling_containers.get(&container_id).unwrap();
        if data.is_tabbed {
            Some(hub.containers.get(container_id).children[data.active_tab_index])
        } else {
            None
        }
    }

    pub(super) fn set_active_tab_to_child(
        &mut self,
        hub: &HubAccess,
        container_id: ContainerId,
        child: Child,
    ) {
        assert!(
            self.tiling_containers.get(&container_id).unwrap().is_tabbed,
            "Calling set_active_tab_to_child on split container"
        );
        let index = hub.containers.get(container_id).position_of(child);
        self.tiling_containers
            .get_mut(&container_id)
            .unwrap()
            .active_tab_index = index;
    }

    pub(super) fn switch_tab(
        &mut self,
        hub: &HubAccess,
        container_id: ContainerId,
        forward: bool,
    ) -> Option<Child> {
        if !self.tiling_containers.get(&container_id).unwrap().is_tabbed {
            return None;
        }
        let len = hub.containers.get(container_id).children.len();
        let current = self
            .tiling_containers
            .get(&container_id)
            .unwrap()
            .active_tab_index;
        let new_tab = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        self.tiling_containers
            .get_mut(&container_id)
            .unwrap()
            .active_tab_index = new_tab;
        Some(hub.containers.get(container_id).children[new_tab])
    }

    pub(super) fn set_active_tab_by_index(
        &mut self,
        hub: &HubAccess,
        container_id: ContainerId,
        index: usize,
    ) -> Option<Child> {
        if !self.tiling_containers.get(&container_id).unwrap().is_tabbed
            || index >= hub.containers.get(container_id).children.len()
        {
            return None;
        }
        self.tiling_containers
            .get_mut(&container_id)
            .unwrap()
            .active_tab_index = index;
        Some(hub.containers.get(container_id).children[index])
    }

    pub(super) fn remove_child(
        &mut self,
        hub: &mut HubAccess,
        container_id: ContainerId,
        child: Child,
    ) {
        let pos = hub.containers.get(container_id).position_of(child);
        hub.containers.get_mut(container_id).children.remove(pos);
        let data = self.tiling_containers.get_mut(&container_id).unwrap();
        if data.is_tabbed && pos <= data.active_tab_index {
            data.active_tab_index = data.active_tab_index.saturating_sub(1);
        }
    }
}
