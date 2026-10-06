use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Direction, WorkspaceId};

use super::config::{MIN_MASTER_COUNT, clamp_master_ratio};
use super::focus::FocusedPanes;
use super::{MasterStrategy, PaneDisplay, PaneKind};

impl MasterStrategy {
    pub(super) fn focus_direction(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        direction: Direction,
        forward: bool,
    ) {
        let Some(FocusedPanes {
            kind,
            idx,
            master_container,
            secondary_container,
            master_len,
            stack_len,
        }) = self.focused_panes(hub, ws_id)
        else {
            return;
        };
        if master_len + stack_len <= 1 {
            return;
        }
        match (direction, forward) {
            (Direction::Horizontal, false) => {
                if kind == PaneKind::Secondary && master_len > 0 {
                    let target = self.last_focused_in(hub, ws_id, PaneKind::Master);
                    self.workspaces
                        .get_mut(&ws_id)
                        .unwrap()
                        .record_focus(target);
                }
            }
            (Direction::Horizontal, true) => {
                if kind == PaneKind::Master && stack_len > 0 {
                    let target = self.last_focused_in(hub, ws_id, PaneKind::Secondary);
                    self.workspaces
                        .get_mut(&ws_id)
                        .unwrap()
                        .record_focus(target);
                }
            }
            (Direction::Vertical, _) => {
                let cid = if kind == PaneKind::Master {
                    master_container
                } else {
                    secondary_container
                };
                let members = Self::pane_windows(hub, cid);
                let len = members.len();
                if len <= 1 {
                    return;
                }
                let target = members[wrap_index(idx, len, forward)];
                self.workspaces
                    .get_mut(&ws_id)
                    .unwrap()
                    .record_focus(target);
            }
        }
        self.scroll_into_view(hub, ws_id);
    }

    pub(super) fn move_direction(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        direction: Direction,
        forward: bool,
    ) {
        let Some(FocusedPanes {
            kind,
            idx,
            master_container,
            secondary_container,
            master_len,
            stack_len,
        }) = self.focused_panes(hub, ws_id)
        else {
            return;
        };
        if master_len + stack_len <= 1 {
            return;
        }
        let effective = {
            let state = self.workspaces.get(&ws_id).unwrap();
            state.master_count.unwrap_or(self.master_count)
        };
        match (direction, forward) {
            (Direction::Horizontal, false) => {
                if kind == PaneKind::Secondary {
                    let moved = Self::remove_from_pane(hub, secondary_container, idx);
                    if Self::pane_len(hub, master_container) >= effective && master_len > 0 {
                        let swapped = Self::pop_from_pane(hub, master_container).unwrap();
                        Self::push_to_pane(hub, master_container, moved);
                        Self::push_to_pane(hub, secondary_container, swapped);
                    } else if Self::pane_len(hub, master_container) < effective {
                        Self::push_to_pane(hub, master_container, moved);
                    }
                }
            }
            (Direction::Horizontal, true) => {
                if kind == PaneKind::Master && stack_len > 0 {
                    let moved = Self::remove_from_pane(hub, master_container, idx);
                    let swapped = Self::remove_from_pane(hub, secondary_container, 0);
                    Self::push_to_pane(hub, master_container, swapped);
                    Self::push_to_pane(hub, secondary_container, moved);
                }
            }
            (Direction::Vertical, _) => {
                let cid = if kind == PaneKind::Master {
                    master_container
                } else {
                    secondary_container
                };
                let len = Self::pane_len(hub, cid);
                if len <= 1 {
                    return;
                }
                let target = wrap_index(idx, len, forward);
                hub.containers.get_mut(cid).children.swap(idx, target);
            }
        }
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn toggle_container_layout(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        let Some(FocusedPanes { kind, .. }) = self.focused_panes(hub, ws_id) else {
            return;
        };
        let pane = self.workspaces.get_mut(&ws_id).unwrap().pane_mut(kind);
        pane.display = match pane.display {
            PaneDisplay::Tiled => PaneDisplay::Tabbed,
            PaneDisplay::Tabbed => PaneDisplay::Tiled,
        };
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn focus_tab(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, forward: bool) {
        let Some(FocusedPanes {
            kind,
            idx,
            master_container,
            secondary_container,
            ..
        }) = self.focused_panes(hub, ws_id)
        else {
            return;
        };
        let cid = if kind == PaneKind::Master {
            master_container
        } else {
            secondary_container
        };
        let is_tabbed =
            self.workspaces.get(&ws_id).unwrap().pane(kind).display == PaneDisplay::Tabbed;
        let members = Self::pane_windows(hub, cid);
        if !is_tabbed || members.len() < 2 {
            return;
        }
        let target = members[wrap_index(idx, members.len(), forward)];
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .record_focus(target);
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn tab_clicked(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        container_id: ContainerId,
        index: usize,
    ) {
        let state = self.workspaces.get(&ws_id).unwrap();
        let clicked_kind = if container_id == state.master.container {
            PaneKind::Master
        } else if container_id == state.secondary.container {
            PaneKind::Secondary
        } else {
            unreachable!("container {container_id} is neither pane of workspace {ws_id}");
        };
        let is_tabbed = state.pane(clicked_kind).display == PaneDisplay::Tabbed;
        let members = Self::pane_windows(hub, container_id);
        if !is_tabbed || members.len() < 2 {
            return;
        }
        let Some(&target) = members.get(index) else {
            return;
        };
        self.focus_tiling(hub, ws_id, target);
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn grow(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        if self.focused_position(hub, ws_id).is_none() {
            return;
        }
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let global_ratio = self.master_ratio;
        let current = state.master_ratio.unwrap_or(global_ratio);
        state.master_ratio = Some(clamp_master_ratio(current + 0.05));
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn shrink(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        if self.focused_position(hub, ws_id).is_none() {
            return;
        }
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let global_ratio = self.master_ratio;
        let current = state.master_ratio.unwrap_or(global_ratio);
        state.master_ratio = Some(clamp_master_ratio(current - 0.05));
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn more(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        if self.focused_position(hub, ws_id).is_none() {
            return;
        }
        let global_count = self.master_count;
        {
            let state = self.workspaces.get_mut(&ws_id).unwrap();
            let current = state.master_count.unwrap_or(global_count);
            state.master_count = Some(current + 1);
        }
        self.reconcile_master_count(hub, ws_id);
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn fewer(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        if self.focused_position(hub, ws_id).is_none() {
            return;
        }
        let global_count = self.master_count;
        let current = self
            .workspaces
            .get(&ws_id)
            .and_then(|s| s.master_count)
            .unwrap_or(global_count);
        if current <= MIN_MASTER_COUNT {
            return;
        }
        {
            let state = self.workspaces.get_mut(&ws_id).unwrap();
            state.master_count = Some(current - 1);
        }
        self.reconcile_master_count(hub, ws_id);
        self.compute_placement(hub, ws_id);
    }
}

fn wrap_index(idx: usize, len: usize, forward: bool) -> usize {
    if forward {
        if idx + 1 == len { 0 } else { idx + 1 }
    } else if idx == 0 {
        len - 1
    } else {
        idx - 1
    }
}
