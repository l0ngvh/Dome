use crate::core::hub::HubAccess;
use crate::core::node::{Child, ContainerId, WindowId, WorkspaceId};
use crate::core::strategy::FocusedChild;

use super::{MasterStrategy, PaneKind, WorkspaceState};

impl MasterStrategy {
    /// The topmost fullscreen window, otherwise the topmost float when float focus is selected
    /// or no tiling window remains, otherwise the most recently focused tiling window.
    pub(super) fn focused(&self, ws_id: WorkspaceId) -> Option<FocusedChild> {
        let state = self.workspaces.get(&ws_id)?;
        if let Some(id) = state.fullscreen_windows.topmost() {
            return Some(FocusedChild::Fullscreen(id));
        }
        let tiling = state.focused_window();
        if let Some(id) = state.float_windows.topmost()
            && (state.float_windows.is_float_focused || tiling.is_none())
        {
            return Some(FocusedChild::Float(id));
        }
        tiling.map(|id| FocusedChild::Tiling(Child::Window(id)))
    }

    /// Selecting a tiling window clears float focus.
    pub(super) fn focus_tiling(&mut self, hub: &HubAccess, ws_id: WorkspaceId, id: WindowId) {
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.float_windows.is_float_focused = false;
        state.record_focus(id);
        self.scroll_into_view(hub, ws_id);
    }

    pub(super) fn focused_panes(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> Option<FocusedPanes> {
        let (kind, idx) = self.focused_position(hub, ws_id)?;
        let (master_container, secondary_container) = {
            let state = self.workspaces.get(&ws_id).unwrap();
            (state.master.container, state.secondary.container)
        };
        Some(FocusedPanes {
            kind,
            idx,
            master_container,
            secondary_container,
            master_len: Self::pane_len(hub, master_container),
            stack_len: Self::pane_len(hub, secondary_container),
        })
    }

    /// `None` only for an empty workspace.
    pub(super) fn focused_position(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> Option<(PaneKind, usize)> {
        let state = self.workspaces.get(&ws_id)?;
        let focus = state.focused_window()?;
        Some(Self::locate(
            hub,
            state.master.container,
            state.secondary.container,
            focus,
        ))
    }

    /// Returns the most recently focused window that the `kind` pane's container holds now,
    /// or the first window of that pane. Panics when the pane is empty.
    pub(super) fn last_focused_in(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        kind: PaneKind,
    ) -> WindowId {
        let state = self.workspaces.get(&ws_id).unwrap();
        let members = Self::pane_windows(hub, state.pane(kind).container);
        state
            .focus_history
            .iter()
            .find(|w| members.contains(w))
            .copied()
            .or_else(|| members.first().copied())
            .unwrap_or_else(|| panic!("last_focused_in called on empty {kind:?} pane"))
    }
}

impl WorkspaceState {
    pub(super) fn focused_window(&self) -> Option<WindowId> {
        self.focus_history.first().copied()
    }

    pub(super) fn record_focus(&mut self, window_id: WindowId) {
        self.drop_from_history(window_id);
        self.focus_history.insert(0, window_id);
    }

    /// Appends as least recently focused, keeping `focus_history` set-equal to the
    /// panes without claiming focus. Idempotent, so a rebuild preserves order.
    pub(super) fn add_to_history(&mut self, window_id: WindowId) {
        if !self.focus_history.contains(&window_id) {
            self.focus_history.push(window_id);
        }
    }

    pub(super) fn drop_from_history(&mut self, window_id: WindowId) {
        if let Some(pos) = self.focus_history.iter().position(|&w| w == window_id) {
            self.focus_history.remove(pos);
        }
    }
}

pub(super) struct FocusedPanes {
    pub(super) kind: PaneKind,
    pub(super) idx: usize,
    pub(super) master_container: ContainerId,
    pub(super) secondary_container: ContainerId,
    pub(super) master_len: usize,
    pub(super) stack_len: usize,
}
