use crate::core::hub::HubAccess;
use crate::core::node::{Child, Length, PixelRect, WindowId, WorkspaceId};
use crate::core::strategy::{FocusedChild, translate};

use super::MasterStrategy;

impl MasterStrategy {
    /// The border box of the window's tile in screen coordinates.
    fn tile_border_box(&self, hub: &HubAccess, ws_id: WorkspaceId, id: WindowId) -> PixelRect {
        let state = self.workspaces.get(&ws_id).unwrap();
        let (kind, _) = Self::locate(hub, state.master.container, state.secondary.container, id);
        let y_offset = state.pane(kind).y_offset;
        let dimension = self.window_states[&id].dimension;
        translate(
            dimension,
            Length::ZERO,
            y_offset,
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
            FocusedChild::Tiling(Child::Window(id)) => {
                // Read before the detach, whose layout pass can scroll the pane.
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
                self.attach_tiling_window(hub, id, ws_id, None);
                self.focus_tiling(hub, ws_id, id);
                tracing::debug!(window_id = %id, "Window is now tiling");
            }
            FocusedChild::Tiling(Child::Container(_)) | FocusedChild::Fullscreen(_) => {
                tracing::debug!("No focused tiling window or float to toggle");
            }
        }
    }
}
