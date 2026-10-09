use crate::config::lua::deserializer::string_enum;
use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Dimension, Direction};
use crate::core::partition_tree::{Child, Parent, PartitionTreeStrategy};

use super::preferred_layout::PreferredContainerSlotId;

impl PartitionTreeStrategy {
    /// Partition-tree invariant: a container holds at least two children. When one child
    /// remains, dissolve it and promote the survivor to the grandparent.
    pub(super) fn delete_container(&mut self, hub: &mut HubAccess, container_id: ContainerId) {
        debug_assert_eq!(hub.containers.get(container_id).children.len(), 1);
        let grandparent = self.tiling_containers.get(&container_id).unwrap().parent;
        let ws = hub.containers.get(container_id).workspace;
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
            self.set_focus(hub, last_child);
        }

        self.release_container_slot(container_id);
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
        let container_spawn_direction = self
            .tiling_containers
            .get(&container_id)
            .unwrap()
            .spawn_direction();
        if let Child::Window(wid) = child {
            self.tiling_windows.get_mut(&wid).unwrap().spawn_direction = container_spawn_direction;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SplitMode {
    Horizontal,
    Vertical,
    Tabbed,
}

string_enum!(
    SplitMode,
    "\"horizontal\", \"vertical\" or \"tabbed\"",
    "horizontal" => SplitMode::Horizontal,
    "vertical" => SplitMode::Vertical,
    "tabbed" => SplitMode::Tabbed,
);

/// Per-container tiling state.
///
/// Invariant: a non-tabbed container's `direction` differs from its non-tabbed
/// parent's direction. A tabbed container is exempt: `direction()` returns
/// `None` for it, so the alternation rule does not apply across a tabbed
/// boundary. `validate_container_direction` enforces this.
#[derive(Debug)]
pub(super) struct TilingContainerData {
    pub(super) parent: Parent,
    pub(super) dimension: Dimension,
    /// Split axis. Read through `direction()`, which returns `None` when
    /// `is_tabbed` is set. A value is stored while tabbed to keep the field
    /// initialised, but it is unused until the container converts back to split.
    direction: Direction,
    /// Direction the next child extends. Automatic tiling derives it from the
    /// container's shape, so it can differ from `direction`.
    spawn_direction: Direction,
    pub(super) is_tabbed: bool,
    pub(super) active_tab_index: usize,
    /// Preferred container slot this live container materializes, if any.
    pub(super) held_slot: Option<PreferredContainerSlotId>,
}

impl TilingContainerData {
    pub(super) fn new(parent: Parent, split_mode: SplitMode) -> Self {
        let (direction, is_tabbed) = match split_mode {
            SplitMode::Horizontal => (Direction::Horizontal, false),
            SplitMode::Vertical => (Direction::Vertical, false),
            SplitMode::Tabbed => (Direction::Horizontal, true),
        };
        Self {
            parent,
            dimension: Dimension::default(),
            direction,
            spawn_direction: direction,
            is_tabbed,
            active_tab_index: 0,
            held_slot: None,
        }
    }

    pub(super) fn is_tabbed(&self) -> bool {
        self.is_tabbed
    }

    pub(super) fn active_tab_index(&self) -> usize {
        self.active_tab_index
    }

    pub(super) fn direction(&self) -> Option<Direction> {
        if self.is_tabbed {
            None
        } else {
            Some(self.direction)
        }
    }

    pub(super) fn has_direction(&self, direction: Direction) -> bool {
        if self.is_tabbed {
            false
        } else {
            self.direction == direction
        }
    }

    pub(super) fn spawn_direction(&self) -> Direction {
        self.spawn_direction
    }

    pub(super) fn set_spawn_direction(&mut self, spawn_direction: Direction) {
        self.spawn_direction = spawn_direction
    }

    pub(super) fn toggle_direction(&mut self) -> Direction {
        self.direction = match self.direction {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal,
        };
        self.direction
    }
}

impl From<Direction> for SplitMode {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Horizontal => SplitMode::Horizontal,
            Direction::Vertical => SplitMode::Vertical,
        }
    }
}
