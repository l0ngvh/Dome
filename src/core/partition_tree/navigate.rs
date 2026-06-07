use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Direction, WorkspaceId};
use crate::core::partition_tree::{Child, Parent, SpawnMode};
use crate::core::strategy::TilingStrategy;

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

        let direct_parent = self.containers.get(direct_parent_id);
        if direct_parent.direction().is_some_and(|d| d == direction) {
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
                self.containers
                    .get_mut(direct_parent_id)
                    .children
                    .swap(pos, target_pos);
                self.layout_workspace(hub, current_ws);
                return;
            }
        }

        let mut found_ancestor = None;
        for (current_anchor, container_id) in self.ancestors_of(Child::Container(direct_parent_id))
        {
            let container = self.containers.get(container_id);
            if container.direction().is_none_or(|d| d != direction) {
                continue;
            }
            let pos = container
                .children
                .iter()
                .position(|c| *c == current_anchor)
                .unwrap();
            let insert_pos = if forward { pos + 1 } else { pos };
            found_ancestor = Some((container_id, insert_pos));
            break;
        }

        if let Some((container_id, insert_pos)) = found_ancestor {
            tracing::debug!(
                ?child, from = %direct_parent_id, to = %container_id, insert_pos, "Moving child to ancestor"
            );
            self.detach_child_from_container(direct_parent_id, child);
            self.attach_child_to_container(hub, child, container_id, Some(insert_pos));
            self.layout_workspace(hub, current_ws);
            self.set_focus_child(hub, child);
        } else {
            tracing::debug!(?child, %current_ws, "Moving child to new root container");
            self.detach_child_from_container(direct_parent_id, child);
            let root = self.ws_state(current_ws).root.unwrap();
            let children = if forward {
                vec![root, child]
            } else {
                vec![child, root]
            };
            let new_root_id = self.replace_anchor_with_container(
                hub,
                children,
                root,
                SpawnMode::from_direction(direction),
            );
            self.ws_state_mut(current_ws).root = Some(Child::Container(new_root_id));
            self.layout_workspace(hub, current_ws);
            self.set_focus_child(hub, child);
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
            let container = self.containers.get(parent_id);
            if container.direction().is_none_or(|d| d != direction) {
                continue;
            }
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
            let focus_target = self.descend_to_focused(sibling);
            tracing::debug!(?direction, forward, from = ?focused, to = ?focus_target, "Changing focus");
            self.set_focus_child(hub, focus_target);
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
            if self.containers.get(parent_id).is_tabbed {
                break;
            }
            root_id = parent_id;
        }
        self.containers.get_mut(root_id).toggle_direction();
        self.maintain_direction_invariance(Parent::Container(root_id));
        self.layout_workspace(hub, workspace_id);
    }

    pub(super) fn convert_container_layout(
        &mut self,
        hub: &mut HubAccess,
        container_id: ContainerId,
    ) {
        let container = self.containers.get_mut(container_id);
        let ws = container.workspace;
        let direction = container.direction();
        let parent = container.parent;
        container.is_tabbed = !container.is_tabbed;
        tracing::debug!(%container_id, from = ?direction, "Toggled container layout");
        if self.containers.get(container_id).is_tabbed() {
            // Toggled from split to tabbed: find the direct child matching container's focus
            let container = self.containers.get(container_id);
            let focused = container.focused;
            let active_tab = *container
                .children()
                .iter()
                .find(|c| {
                    **c == focused
                        || matches!(c, Child::Container(cid) if self.containers.get(*cid).focused == focused)
                })
                .unwrap();
            self.containers
                .get_mut(container_id)
                .set_active_tab_to_child(active_tab);
        } else {
            // Toggled from tabbed to split
            self.maintain_direction_invariance(Parent::Container(container_id));
        }
        self.maintain_direction_invariance(parent);
        self.layout_workspace(hub, ws);
    }

    pub(super) fn toggle_spawn_mode(&mut self, hub: &mut HubAccess) {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;
        let Some(focused) = self.workspaces.get(&ws_id).and_then(|s| s.focused_tiling) else {
            return;
        };

        let current_mode = match focused {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => {
                let w = hub.windows.get(id);
                if w.is_float() || w.is_fullscreen() {
                    return;
                }
                self.tiling_data(id).spawn_mode
            }
        };
        let new_mode = current_mode.toggle();

        match focused {
            Child::Container(id) => self
                .containers
                .get_mut(id)
                .set_spawn_mode_keep_history(new_mode),
            Child::Window(id) => {
                let td = self.tiling_data_mut(id);
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
        let new_child = self
            .containers
            .get_mut(container_id)
            .switch_tab(forward)
            .unwrap();
        let focus_target = self.descend_to_focused(new_child);
        tracing::debug!(forward, %container_id, ?focus_target, "Focusing tab");
        self.set_focus_child(hub, focus_target);
    }

    pub(super) fn focus_tab_index(
        &mut self,
        hub: &mut HubAccess,
        container_id: ContainerId,
        index: usize,
    ) {
        let Some(new_child) = self
            .containers
            .get_mut(container_id)
            .set_active_tab_by_index(index)
        else {
            return;
        };
        let focus_target = self.descend_to_focused(new_child);
        self.set_focus_child(hub, focus_target);
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
        self.set_focus_child(hub, Child::Container(container_id));
    }
}
