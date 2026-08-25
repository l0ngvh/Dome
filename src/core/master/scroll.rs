use crate::core::{
    Length,
    hub::HubAccess,
    master::{MasterStrategy, PaneDisplay},
    node::WorkspaceId,
};

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
                .y_offset = Length::ZERO;
            return;
        }
        let state = self.workspaces.get(&ws_id).unwrap();
        let pane_height = Length::from_pixels(
            hub.monitors
                .get(hub.workspaces.get(ws_id).monitor)
                .work_area
                .height(),
        );

        let offset = state.pane(kind).y_offset;

        let members = Self::pane_windows(hub, state.pane(kind).container);
        let slot_heights = self.pane_slot_heights(hub, &members, pane_height);
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
        state.pane_mut(kind).y_offset = new_offset;
    }
}
