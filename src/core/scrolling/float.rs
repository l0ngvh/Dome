use crate::core::hub::HubAccess;
use crate::core::node::{Child, PixelRect, WindowId, WorkspaceId};
use crate::core::strategy::{FocusedChild, translate};

use super::ScrollingStrategy;

impl ScrollingStrategy {
    /// The border box the window's tile shows, in screen coordinates.
    fn tile_border_box(&self, hub: &HubAccess, ws_id: WorkspaceId, id: WindowId) -> PixelRect {
        let state = &self.workspaces[&ws_id];
        let column = self
            .column_index_of(hub, ws_id, id)
            .expect("a tiling window sits in a column");
        translate(
            self.window_states[&id].dimension,
            state.x_offset,
            state.columns[column].y_offset,
            state.work_area.x(),
            state.work_area.y(),
        )
    }

    /// Floats the focused tiling window at the border box its tile shows, or tiles the focused
    /// float by the spawn rules and makes it the tiling focus.
    pub(super) fn toggle_float(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        focused: FocusedChild,
    ) {
        match focused {
            FocusedChild::Tiling(Child::Window(id)) => {
                // Read before the detach, whose layout pass can scroll the row and the column.
                let border_box = self.tile_border_box(hub, ws_id, id);
                self.detach_tiling_window(hub, ws_id, id);
                let floats = &mut self.workspaces.get_mut(&ws_id).unwrap().float_windows;
                floats.attach(id, border_box);
                floats.focus(id);
                tracing::debug!(window_id = %id, "Window is now floating");
            }
            FocusedChild::Float(id) => {
                let floats = &mut self.workspaces.get_mut(&ws_id).unwrap().float_windows;
                floats.detach(id);
                self.attach_tiling_window(hub, ws_id, id, None);
                self.focus_tiling(hub, ws_id, id);
                tracing::debug!(window_id = %id, "Window is now tiling");
            }
            FocusedChild::Tiling(Child::Container(_)) | FocusedChild::Fullscreen(_) => {
                tracing::debug!("No focused tiling window or float to toggle");
            }
        }
    }
}
