use crate::core::hub::HubAccess;
use crate::core::node::{Child, WindowId};
use crate::core::strategy::FocusedChild;

use super::ScrollingStrategy;

impl ScrollingStrategy {
    pub(super) fn toggle_fullscreen(&mut self, hub: &mut HubAccess, focused: FocusedChild) {
        match focused {
            FocusedChild::Fullscreen(id) => self.exit_fullscreen(hub, id),
            FocusedChild::Tiling(Child::Window(id)) => self.enter_fullscreen(hub, id),
            FocusedChild::Tiling(Child::Container(_)) | FocusedChild::Float(_) => {
                tracing::debug!("No focused tiling or fullscreen window to toggle");
            }
        }
    }

    /// Makes the tiling window the topmost fullscreen window, which gives it focus. Does nothing
    /// to a window that is already fullscreen, which keeps its place in the stack.
    pub(super) fn enter_fullscreen(&mut self, hub: &mut HubAccess, id: WindowId) {
        let ws_id = hub
            .windows
            .get(id)
            .workspace()
            .expect("non-minimized window has a workspace");
        if self.workspaces[&ws_id].fullscreen_windows.contains(id) {
            tracing::debug!("Window is already fullscreen");
            return;
        }
        self.detach_tiling_window(hub, ws_id, id);
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .fullscreen_windows
            .attach(id);
    }

    /// Tiles a fullscreen window by the spawn rules and makes it the tiling focus. The
    /// fullscreen windows left in the stack keep their order. Does nothing to a window that is
    /// not fullscreen.
    pub(super) fn exit_fullscreen(&mut self, hub: &mut HubAccess, id: WindowId) {
        let ws_id = hub
            .windows
            .get(id)
            .workspace()
            .expect("non-minimized window has a workspace");
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        if !state.fullscreen_windows.detach(id) {
            return;
        }
        self.attach_tiling_window(hub, ws_id, id, None);
        self.focus_tiling(hub, ws_id, id);
    }
}
