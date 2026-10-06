use crate::core::hub::HubAccess;
use crate::core::node::{Child, PixelRect, WindowId, WorkspaceId};
use crate::core::strategy::{FocusedChild, translate};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    /// The border box of the window's tile in screen coordinates.
    fn tile_border_box(&self, ws_id: WorkspaceId, window_id: WindowId) -> PixelRect {
        let state = self.workspaces.get(&ws_id).unwrap();
        let (offset_x, offset_y) = state.viewport_offset;
        let dimension = self.tiling_windows.get(&window_id).unwrap().dimension;
        translate(
            dimension,
            offset_x,
            offset_y,
            state.work_area.x(),
            state.work_area.y(),
        )
    }

    pub(super) fn toggle_float(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        focused: FocusedChild,
    ) {
        match focused {
            FocusedChild::Tiling(Child::Window(window_id)) => {
                // Read before the detach, whose layout pass can move the viewport.
                let border_box = self.tile_border_box(ws_id, window_id);
                self.detach_tiling_window(hub, window_id);
                let floats = &mut self.workspaces.get_mut(&ws_id).unwrap().float_windows;
                floats.attach(window_id, border_box);
                floats.focus(window_id);
                tracing::debug!(%window_id, "Window is now floating");
            }
            FocusedChild::Float(window_id) => {
                let floats = &mut self.workspaces.get_mut(&ws_id).unwrap().float_windows;
                floats.detach(window_id);
                self.attach_tiling_window(hub, window_id, ws_id, None);
                self.focus_tiling(hub, ws_id, Child::Window(window_id));
                tracing::debug!(%window_id, "Window is now tiling");
            }
            FocusedChild::Tiling(Child::Container(_)) | FocusedChild::Fullscreen(_) => {
                tracing::debug!("No focused tiling window or float to toggle");
            }
        }
    }
}
