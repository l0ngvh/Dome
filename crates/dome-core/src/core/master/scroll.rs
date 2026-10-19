use crate::core::{
    FractionalLength, WindowId,
    hub::HubAccess,
    master::{MasterStrategy, PaneDisplay},
    node::WorkspaceId,
};
use crate::geometry::FractionalOffset;

impl MasterStrategy {
    pub(super) fn scroll_into_view(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some((kind, idx)) = self.focused_position(hub, ws_id) else {
            return;
        };
        let tabbed = {
            let pane = self.workspaces.get(&ws_id).unwrap().pane(kind);
            pane.display == PaneDisplay::Tabbed && Self::pane_len(hub, pane.container) >= 2
        };
        if tabbed {
            self.workspaces
                .get_mut(&ws_id)
                .unwrap()
                .pane_mut(kind)
                .y_offset = FractionalOffset::ZERO;
            return;
        }
        let state = self.workspaces.get(&ws_id).unwrap();
        let pane_height = FractionalLength::from_pixels(state.work_area.height());

        let offset = state.pane(kind).y_offset;

        let members = Self::pane_windows(hub, state.pane(kind).container);
        let slot_heights = self.pane_slot_heights(hub, &members, pane_height);
        let content_h: FractionalLength = slot_heights.iter().copied().sum();
        let max_offset = (content_h - pane_height).max(FractionalOffset::ZERO);

        let content_start = if content_h < pane_height {
            (pane_height - content_h) / 2.0
        } else {
            FractionalOffset::ZERO
        };
        let slot_y: FractionalOffset = content_start
            + slot_heights[..idx]
                .iter()
                .copied()
                .sum::<FractionalLength>();
        let slot_height = slot_heights[idx];

        let mut new_offset = offset;
        if slot_y + slot_height - new_offset > pane_height.offset() {
            new_offset = slot_y + slot_height - pane_height;
        }
        if slot_y - new_offset < FractionalOffset::ZERO {
            new_offset = slot_y;
        }
        new_offset = new_offset.clamp(FractionalOffset::ZERO, max_offset);

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.pane_mut(kind).y_offset = new_offset;
    }

    pub(super) fn clamp_scroll(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let state = self.workspaces.get(&ws_id).unwrap();
        let pane_height = FractionalLength::from_pixels(state.work_area.height());

        let master_ids: Vec<WindowId> = Self::pane_windows(hub, state.master.container);
        let master_tabbed = state.master.display == PaneDisplay::Tabbed && master_ids.len() >= 2;
        let master_max = if !master_ids.is_empty() && !master_tabbed {
            let content_h = self.pane_content_height(hub, &master_ids, pane_height);
            (content_h - pane_height).max(FractionalOffset::ZERO)
        } else {
            FractionalOffset::ZERO
        };

        let stack_ids: Vec<WindowId> = Self::pane_windows(hub, state.secondary.container);
        let stack_tabbed = state.secondary.display == PaneDisplay::Tabbed && stack_ids.len() >= 2;
        let stack_max = if !stack_ids.is_empty() && !stack_tabbed {
            let content_h = self.pane_content_height(hub, &stack_ids, pane_height);
            (content_h - pane_height).max(FractionalOffset::ZERO)
        } else {
            FractionalOffset::ZERO
        };

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.master.y_offset = state
            .master
            .y_offset
            .clamp(FractionalOffset::ZERO, master_max);
        state.secondary.y_offset = state
            .secondary
            .y_offset
            .clamp(FractionalOffset::ZERO, stack_max);
    }
}
