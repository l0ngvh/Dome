use crate::core::{
    Hub, WindowId,
    hub::HubAccess,
    matcher::{WindowMatcher, WindowMode},
    node::{WindowRestrictions, WorkspaceId},
    slot::Slot,
};

/// The fullscreen windows of one workspace. Only the topmost one shows.
#[derive(Debug, Default)]
pub(super) struct FullscreenWindows {
    /// Bottom to top.
    windows: Vec<WindowId>,
}

impl FullscreenWindows {
    /// Allocates one free fullscreen slot per matcher in the hub's arena, in matcher order.
    pub(super) fn allocate_slots(
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        matchers: &[WindowMatcher],
    ) {
        for matcher in matchers {
            hub.slots
                .allocate(Slot::new(matcher.clone(), ws_id, WindowMode::Fullscreen));
        }
    }

    /// Pushes the window on top. The caller handles a window that is already fullscreen.
    pub(super) fn attach(&mut self, window_id: WindowId) {
        self.windows.push(window_id);
    }

    /// Removes a member and keeps the rest in their order. Returns `false` when the window is
    /// not a member.
    pub(super) fn detach(&mut self, window_id: WindowId) -> bool {
        let Some(pos) = self.position(window_id) else {
            return false;
        };
        self.windows.remove(pos);
        true
    }

    /// Raises a member. Returns `false` when the window is not a member.
    pub(super) fn focus(&mut self, window_id: WindowId) -> bool {
        let Some(pos) = self.position(window_id) else {
            return false;
        };
        self.windows.remove(pos);
        self.windows.push(window_id);
        true
    }

    pub(super) fn contains(&self, window_id: WindowId) -> bool {
        self.windows.contains(&window_id)
    }

    pub(super) fn topmost(&self) -> Option<WindowId> {
        self.windows.last().copied()
    }

    /// One matcher per fullscreen window from its live metadata, bottom to top.
    pub(super) fn export(&self, hub: &HubAccess) -> Vec<WindowMatcher> {
        self.windows
            .iter()
            .map(|&id| hub.windows.get(id).metadata.to_window_matcher())
            .collect()
    }

    /// Returns the fullscreen windows bottom to top, which is the order to attach them again.
    pub(super) fn clear(&mut self) -> Vec<WindowId> {
        std::mem::take(&mut self.windows)
    }

    #[cfg(test)]
    pub(super) fn windows(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.windows.iter().copied()
    }

    fn position(&self, window_id: WindowId) -> Option<usize> {
        self.windows.iter().position(|&id| id == window_id)
    }
}

impl Hub {
    #[tracing::instrument(skip(self))]
    pub(crate) fn set_fullscreen(&mut self, window_id: WindowId, restrictions: WindowRestrictions) {
        let ws = self
            .access
            .windows
            .get(window_id)
            .workspace()
            .expect("non-minimized window has a workspace");
        self.access.windows.get_mut(window_id).restrictions = restrictions;
        self.strategies
            .for_workspace_mut(ws)
            .set_fullscreen(&mut self.access, window_id);
        tracing::info!("Fullscreen set");
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn unset_fullscreen(&mut self, window_id: WindowId) {
        // A minimized window sits in no workspace, so no fullscreen stack holds it.
        let Some(ws) = self.access.windows.get(window_id).workspace() else {
            return;
        };
        self.access.windows.get_mut(window_id).restrictions = WindowRestrictions::None;
        self.strategies
            .for_workspace_mut(ws)
            .unset_fullscreen(&mut self.access, window_id);
        tracing::info!("Fullscreen unset");
    }
}
