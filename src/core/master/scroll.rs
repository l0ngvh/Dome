use crate::core::{
    Length,
    hub::HubAccess,
    master::{MasterStrategy, Pane},
    node::WorkspaceId,
};

impl MasterStrategy {
    pub(super) fn scroll_into_view(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let state = self.workspaces.get(&ws_id).unwrap();
        let Some((pane, idx)) = state.focused_position() else {
            return;
        };
        let pane_height = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension
            .height;

        let offset = match pane {
            Pane::Master => state.master_y_offset,
            Pane::Secondary => state.stack_y_offset,
        };

        let slot_heights = self.pane_slot_heights(hub, state.pane_vec(pane), pane_height);
        let content_h: Length = slot_heights.iter().copied().sum();
        let max_offset = (content_h - pane_height).max(Length::ZERO);

        let content_start = if content_h < pane_height {
            (pane_height - content_h) / 2.0
        } else {
            Length::ZERO
        };
        let slot_y: Length = content_start + slot_heights[..idx].iter().copied().sum::<Length>();
        let slot_height = slot_heights[idx];

        let mut new_offset = offset;
        if slot_y + slot_height - new_offset > pane_height {
            new_offset = slot_y + slot_height - pane_height;
        }
        if slot_y - new_offset < Length::ZERO {
            new_offset = slot_y;
        }
        new_offset = new_offset.clamp(Length::ZERO, max_offset);

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        match pane {
            Pane::Master => state.master_y_offset = new_offset,
            Pane::Secondary => state.stack_y_offset = new_offset,
        }
    }
}
