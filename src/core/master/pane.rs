use serde::Serialize;

use crate::core::hub::HubAccess;
use crate::core::node::{Child, ContainerId, Dimension, Length, WindowId, WorkspaceId};
use crate::core::slot::{SlotId, held_tiling_slot};

use super::{MasterStrategy, WindowState, WorkspaceState};

impl MasterStrategy {
    pub(super) fn attach_tiling_window(
        &mut self,
        hub: &mut HubAccess,
        id: WindowId,
        ws_id: WorkspaceId,
        slot: Option<SlotId>,
    ) {
        self.track_window(hub, ws_id, id);
        self.sort_window_into_pane(hub, ws_id, id, slot);
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn detach_tiling_window(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        id: WindowId,
    ) {
        self.remove_window(hub, ws_id, id);
        self.window_states
            .remove(&id)
            .unwrap_or_else(|| panic!("master: detaching {id:?} but window_states has no entry"));
        self.reconcile_master_count(hub, ws_id);
        self.compute_placement(hub, ws_id);
    }

    fn track_window(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, id: WindowId) {
        hub.windows.get_mut(id).set_workspace(Some(ws_id));
        self.window_states.insert(
            id,
            WindowState {
                dimension: Dimension::default(),
            },
        );
        self.workspaces.get_mut(&ws_id).unwrap().add_to_history(id);
    }

    /// Focus repair needs no ladder here. Dropping `window_id` from the history leaves the head
    /// on the surviving window focused before it, whichever pane that window lives in.
    fn remove_window(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        window_id: WindowId,
    ) -> Length {
        let (container, y_offset, idx) = {
            let state = self.workspaces.get(&ws_id).unwrap();
            let (kind, idx) = Self::locate(
                hub,
                state.master.container,
                state.secondary.container,
                window_id,
            );
            (state.pane(kind).container, state.pane(kind).y_offset, idx)
        };
        Self::remove_from_pane(hub, container, idx);
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .drop_from_history(window_id);
        y_offset
    }

    pub(super) fn push_unmatched_window(
        &self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        id: WindowId,
    ) {
        let state = self.workspaces.get(&ws_id).unwrap();
        let effective_count = state.master_count.unwrap_or(self.master_count);
        if Self::pane_len(hub, state.master.container) < effective_count {
            Self::push_to_pane(hub, state.master.container, id);
        } else {
            Self::push_to_pane(hub, state.secondary.container, id);
        }
    }

    pub(super) fn reconcile_master_count(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        let (effective_count, master, secondary) = {
            let Some(state) = self.workspaces.get(&ws_id) else {
                return;
            };
            (
                state.master_count.unwrap_or(self.master_count),
                state.master.container,
                state.secondary.container,
            )
        };

        while Self::pane_len(hub, master) < effective_count {
            let secondary_slots = &self.workspaces.get(&ws_id).unwrap().secondary.slots;
            let pos = hub
                .containers
                .get(secondary)
                .children()
                .iter()
                .position(|c| {
                    // A window that overflowed from master to secondary still holds its master
                    // slot. This filter accepts that window too.
                    matches!(c, Child::Window(w)
                    if held_tiling_slot(&hub.slots, *w, ws_id)
                        .is_none_or(|s| !secondary_slots.contains(&s)))
                });
            let Some(pos) = pos else {
                break;
            };
            let wid = Self::remove_from_pane(hub, secondary, pos);
            Self::push_to_pane(hub, master, wid);
        }

        while Self::pane_len(hub, master) > effective_count {
            let Some(wid) = Self::pop_from_pane(hub, master) else {
                break;
            };
            Self::insert_into_pane(hub, secondary, 0, wid);
        }
    }

    pub(super) fn pane_windows(hub: &HubAccess, container: ContainerId) -> Vec<WindowId> {
        hub.containers
            .get(container)
            .children()
            .iter()
            .filter_map(|c| match c {
                Child::Window(w) => Some(*w),
                Child::Container(_) => None,
            })
            .collect()
    }

    pub(super) fn pane_len(hub: &HubAccess, container: ContainerId) -> usize {
        hub.containers.get(container).children().len()
    }

    pub(super) fn position_in_pane(
        hub: &HubAccess,
        container: ContainerId,
        id: WindowId,
    ) -> Option<usize> {
        hub.containers
            .get(container)
            .children()
            .iter()
            .position(|c| matches!(c, Child::Window(w) if *w == id))
    }

    pub(super) fn push_to_pane(hub: &mut HubAccess, container: ContainerId, id: WindowId) {
        hub.containers
            .get_mut(container)
            .children
            .push(Child::Window(id));
    }

    pub(super) fn insert_into_pane(
        hub: &mut HubAccess,
        container: ContainerId,
        idx: usize,
        id: WindowId,
    ) {
        hub.containers
            .get_mut(container)
            .children
            .insert(idx, Child::Window(id));
    }

    pub(super) fn remove_from_pane(
        hub: &mut HubAccess,
        container: ContainerId,
        idx: usize,
    ) -> WindowId {
        match hub.containers.get_mut(container).children.remove(idx) {
            Child::Window(w) => w,
            Child::Container(_) => unreachable!("master pane holds only windows"),
        }
    }

    pub(super) fn pop_from_pane(hub: &mut HubAccess, container: ContainerId) -> Option<WindowId> {
        hub.containers
            .get_mut(container)
            .children
            .pop()
            .map(|c| match c {
                Child::Window(w) => w,
                Child::Container(_) => unreachable!("master pane holds only windows"),
            })
    }

    pub(super) fn locate(
        hub: &HubAccess,
        master: ContainerId,
        secondary: ContainerId,
        id: WindowId,
    ) -> (PaneKind, usize) {
        if let Some(i) = Self::position_in_pane(hub, master, id) {
            return (PaneKind::Master, i);
        }
        let i = Self::position_in_pane(hub, secondary, id)
            .unwrap_or_else(|| panic!("window {id:?} is in neither master nor secondary pane"));
        (PaneKind::Secondary, i)
    }
}

/// One side of the master-stack split. Windows live in `container`, a flat `Container`
/// of `Child::Window` that never nests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PaneDisplay {
    #[default]
    Tiled,
    Tabbed,
}

crate::config::lua::deserializer::string_enum!(
    PaneDisplay,
    "\"tiled\" or \"tabbed\"",
    "tiled" => PaneDisplay::Tiled,
    "tabbed" => PaneDisplay::Tabbed,
);

#[derive(Debug)]
pub(super) struct Pane {
    pub(super) container: ContainerId,
    pub(super) slots: Vec<SlotId>,
    pub(super) y_offset: Length,
    pub(super) display: PaneDisplay,
}

impl Pane {
    pub(super) fn new(container: ContainerId, slots: Vec<SlotId>, display: PaneDisplay) -> Self {
        Pane {
            container,
            slots,
            y_offset: Length::ZERO,
            display,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PaneKind {
    Master,
    Secondary,
}

impl WorkspaceState {
    pub(super) fn pane(&self, kind: PaneKind) -> &Pane {
        match kind {
            PaneKind::Master => &self.master,
            PaneKind::Secondary => &self.secondary,
        }
    }

    pub(super) fn pane_mut(&mut self, kind: PaneKind) -> &mut Pane {
        match kind {
            PaneKind::Master => &mut self.master,
            PaneKind::Secondary => &mut self.secondary,
        }
    }
}
