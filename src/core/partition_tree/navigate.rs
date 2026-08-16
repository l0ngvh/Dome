use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Direction, WorkspaceId};
use crate::core::partition_tree::{Child, Parent, SpawnMode};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    pub(super) fn focused_child(&self, hub: &HubAccess) -> Option<Child> {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;
        self.workspaces.get(&ws_id).and_then(|s| s.focused_tiling)
    }

    pub(super) fn focused_child_in(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> Option<Child> {
        self.workspaces.get(&ws_id).and_then(|s| s.focused_tiling)
    }

    pub(super) fn move_in_direction(
        &mut self,
        hub: &mut HubAccess,
        direction: Direction,
        forward: bool,
    ) {
        let current_ws = hub.monitors.get(hub.focused_monitor).active_workspace;
        let Some(child) = self.focused_child_in(hub, current_ws) else {
            return;
        };
        let Parent::Container(direct_parent_id) = self.parent(child) else {
            return;
        };

        let direct_parent_direction = self
            .tiling_containers
            .get(&direct_parent_id)
            .unwrap()
            .direction();
        let direct_parent = hub.containers.get(direct_parent_id);
        if direct_parent_direction.is_some_and(|d| d == direction) {
            let pos = direct_parent.position_of(child);
            let target_pos = if forward {
                pos + 1
            } else {
                pos.saturating_sub(1)
            };
            if target_pos != pos && target_pos < direct_parent.children.len() {
                tracing::debug!(
                    ?child, from = pos, to = target_pos, %direct_parent_id, "Swapping child position"
                );
                hub.containers
                    .get_mut(direct_parent_id)
                    .children
                    .swap(pos, target_pos);
                self.compute_placement(hub, current_ws);
                return;
            }
        }

        let mut found_ancestor = None;
        for (current_anchor, container_id) in self.ancestors_of(Child::Container(direct_parent_id))
        {
            if self
                .tiling_containers
                .get(&container_id)
                .unwrap()
                .direction()
                .is_none_or(|d| d != direction)
            {
                continue;
            }
            let pos = hub.containers.get(container_id).position_of(current_anchor);
            let insert_pos = if forward { pos + 1 } else { pos };
            found_ancestor = Some((container_id, insert_pos));
            break;
        }

        if let Some((container_id, insert_pos)) = found_ancestor {
            tracing::debug!(
                ?child, from = %direct_parent_id, to = %container_id, insert_pos, "Moving child to ancestor"
            );
            self.detach_child_from_container(hub, direct_parent_id, child);
            self.attach_child_to_container(hub, child, container_id, Some(insert_pos));
            self.compute_placement(hub, current_ws);
            self.set_focus(hub, child);
        } else {
            tracing::debug!(?child, %current_ws, "Moving child to new root container");
            self.detach_child_from_container(hub, direct_parent_id, child);
            let root = self.workspaces.get(&current_ws).unwrap().root.unwrap();
            let children = if forward {
                vec![root, child]
            } else {
                vec![child, root]
            };
            let spawn_mode = SpawnMode::from_direction(direction);
            self.replace_anchor_with_container(hub, root, children, spawn_mode.into());
            self.compute_placement(hub, current_ws);
            self.set_focus(hub, child);
        }
    }

    pub(super) fn focus_in_direction(
        &mut self,
        hub: &mut HubAccess,
        direction: Direction,
        forward: bool,
    ) {
        let Some(focused) = self.focused_child(hub) else {
            return;
        };

        let mut sibling_found = None;
        for (current, parent_id) in self.ancestors_of(focused) {
            if self
                .tiling_containers
                .get(&parent_id)
                .unwrap()
                .direction()
                .is_none_or(|d| d != direction)
            {
                continue;
            }
            let container = hub.containers.get(parent_id);
            let pos = container.position_of(current);
            let has_sibling = if forward {
                pos + 1 < container.children.len()
            } else {
                pos > 0
            };
            if has_sibling {
                let sibling_pos = if forward { pos + 1 } else { pos - 1 };
                sibling_found = Some(container.children[sibling_pos]);
                break;
            }
        }
        if let Some(sibling) = sibling_found {
            let focus_target = self.focus_target_in(sibling);
            tracing::debug!(?direction, forward, from = ?focused, to = ?focus_target, "Changing focus");
            self.set_focus(hub, focus_target);
        }
    }

    pub(super) fn toggle_focused_layout_direction(&mut self, hub: &mut HubAccess) {
        let workspace_id = hub.monitors.get(hub.focused_monitor).active_workspace;
        let Some(focused) = self.focused_child_in(hub, workspace_id) else {
            return;
        };
        let mut root_id = match focused {
            Child::Container(id) => id,
            Child::Window(_) => {
                let Parent::Container(id) = self.parent(focused) else {
                    return;
                };
                id
            }
        };
        for (_, parent_id) in self.ancestors_of(Child::Container(root_id)) {
            if self.tiling_containers.get(&parent_id).unwrap().is_tabbed {
                break;
            }
            root_id = parent_id;
        }
        self.tiling_containers
            .get_mut(&root_id)
            .unwrap()
            .toggle_direction();
        self.maintain_direction_invariance(hub, Parent::Container(root_id));
        self.compute_placement(hub, workspace_id);
    }

    pub(super) fn convert_container_layout(
        &mut self,
        hub: &mut HubAccess,
        container_id: ContainerId,
    ) {
        let container = self.tiling_containers.get_mut(&container_id).unwrap();
        let ws = container.workspace;
        let direction = container.direction();
        let parent = container.parent;
        container.is_tabbed = !container.is_tabbed;
        tracing::debug!(%container_id, from = ?direction, "Toggled container layout");
        if self
            .tiling_containers
            .get(&container_id)
            .unwrap()
            .is_tabbed()
        {
            // Resolved through focus_target_in because highlight mode on this very
            // container is not on its own ancestor path. None leaves the tab alone.
            let focused = self.workspaces.get(&ws).unwrap().focused_tiling;
            let active_tab = focused.and_then(|f| {
                let target = self.focus_target_in(f);
                self.ancestors_of(target)
                    .find(|(_, pid)| *pid == container_id)
                    .map(|(child, _)| child)
            });
            if let Some(active_tab) = active_tab {
                self.set_active_tab_to_child(hub, container_id, active_tab);
            }
        } else {
            // Toggled from tabbed to split
            self.maintain_direction_invariance(hub, Parent::Container(container_id));
        }
        self.maintain_direction_invariance(hub, parent);
        self.compute_placement(hub, ws);
    }

    pub(super) fn toggle_spawn_mode(&mut self, hub: &mut HubAccess) {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;
        let Some(focused) = self.workspaces.get(&ws_id).and_then(|s| s.focused_tiling) else {
            return;
        };

        let current_mode = match focused {
            Child::Container(id) => self.tiling_containers.get(&id).unwrap().spawn_mode(),
            Child::Window(id) => {
                let w = hub.windows.get(id);
                if w.is_float() || w.is_fullscreen() {
                    return;
                }
                self.tiling_windows.get(&id).unwrap().spawn_mode
            }
        };
        let new_mode = current_mode.toggle();

        match focused {
            Child::Container(id) => self
                .tiling_containers
                .get_mut(&id)
                .unwrap()
                .set_spawn_mode_keep_history(new_mode),
            Child::Window(id) => {
                let td = self.tiling_windows.get_mut(&id).unwrap();
                td.spawn_mode = td.spawn_mode.switch_to(new_mode);
            }
        }
        tracing::debug!(?focused, ?new_mode, "Toggled spawn mode");
    }

    pub(super) fn toggle_container_layout(&mut self, hub: &mut HubAccess) {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;
        let Some(focused) = self.workspaces.get(&ws_id).and_then(|s| s.focused_tiling) else {
            return;
        };
        let container_id = match focused {
            Child::Container(id) => id,
            Child::Window(id) => {
                let w = hub.windows.get(id);
                if w.is_float() || w.is_fullscreen() {
                    return;
                }
                match self.parent(Child::Window(id)) {
                    Parent::Container(cid) => cid,
                    Parent::Workspace(_) => return,
                }
            }
        };
        self.convert_container_layout(hub, container_id);
    }

    pub(super) fn focus_tab(&mut self, hub: &mut HubAccess, forward: bool) {
        let Some(focused) = self.focused_child(hub) else {
            return;
        };
        let Some(container_id) = self.find_tabbed_self_or_ancestor(focused) else {
            return;
        };
        let new_child = self.switch_tab(hub, container_id, forward).unwrap();
        let focus_target = self.focus_target_in(new_child);
        tracing::debug!(forward, %container_id, ?focus_target, "Focusing tab");
        self.set_focus(hub, focus_target);
    }

    pub(super) fn focus_tab_index(
        &mut self,
        hub: &mut HubAccess,
        container_id: ContainerId,
        index: usize,
    ) {
        let Some(new_child) = self.set_active_tab_by_index(hub, container_id, index) else {
            return;
        };
        let focus_target = self.focus_target_in(new_child);
        self.set_focus(hub, focus_target);
    }

    /// Move tiling focus from the current child to its parent container. Sets
    /// `focused_tiling` to `Child::Container`, entering container-highlight mode.
    /// No managed windows should receive keyboard focus in this mode.
    /// Move-to-workspace operates on the whole container.
    pub(super) fn focus_parent(&mut self, hub: &mut HubAccess) {
        let Some(focused) = self.focused_child(hub) else {
            return;
        };
        let Parent::Container(container_id) = self.parent(focused) else {
            tracing::debug!("Cannot focus parent of workspace root, ignoring");
            return;
        };
        tracing::debug!(parent = %container_id, %focused, "Focusing parent");
        self.set_focus(hub, Child::Container(container_id));
    }
}
