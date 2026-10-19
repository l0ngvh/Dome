use crate::core::hub::HubAccess;
use crate::core::node::{Child, WindowId};
use crate::core::strategy::FocusedChild;

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    pub(super) fn toggle_fullscreen(&mut self, hub: &mut HubAccess, focused: FocusedChild) {
        match focused {
            FocusedChild::Fullscreen(window_id) => self.exit_fullscreen(hub, window_id),
            FocusedChild::Float(window_id) | FocusedChild::Tiling(Child::Window(window_id)) => {
                self.enter_fullscreen(hub, window_id)
            }
            FocusedChild::Tiling(Child::Container(_)) => {
                tracing::debug!("A focused container cannot go fullscreen");
            }
        }
    }

    /// Makes the window the topmost fullscreen window and gives it focus. Does nothing to a
    /// window that is already fullscreen, which keeps its place in the stack.
    pub(super) fn enter_fullscreen(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        let ws_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("non-minimized window has a workspace");
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        if state.fullscreen_windows.contains(window_id) {
            tracing::debug!("Window is already fullscreen");
            return;
        }
        if state.float_windows.detach(window_id).is_none() {
            self.detach_tiling_window(hub, window_id);
        }
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.fullscreen_windows.attach(window_id);
    }

    /// Tiles a fullscreen window with default placement and makes it the tiling focus. The
    /// fullscreen windows left in the stack keep their order. Does nothing to a window that is
    /// not fullscreen.
    pub(super) fn exit_fullscreen(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        let ws_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("non-minimized window has a workspace");
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        if !state.fullscreen_windows.detach(window_id) {
            return;
        }
        self.attach_tiling_window(hub, window_id, ws_id, None);
        self.focus_tiling(hub, ws_id, Child::Window(window_id));
    }
}
