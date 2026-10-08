use crate::core::hub::HubAccess;
use crate::core::node::{Child, Direction, WindowId, WorkspaceId};
use crate::core::strategy::FocusedChild;

use super::{ScrollingStrategy, WorkspaceState};

impl ScrollingStrategy {
    /// The topmost fullscreen window, otherwise the most recently focused tiling window.
    pub(super) fn focused(&self, ws_id: WorkspaceId) -> Option<FocusedChild> {
        let state = self.workspaces.get(&ws_id)?;
        if let Some(id) = state.fullscreen_windows.topmost() {
            return Some(FocusedChild::Fullscreen(id));
        }
        state
            .focused_window()
            .map(|id| FocusedChild::Tiling(Child::Window(id)))
    }

    /// Makes the window the tiling focus and scrolls it into view.
    pub(super) fn focus_tiling(&mut self, hub: &HubAccess, ws_id: WorkspaceId, id: WindowId) {
        self.workspaces.get_mut(&ws_id).unwrap().record_focus(id);
        self.scroll_into_view(hub, ws_id);
    }

    /// A press first reveals the part of the focused window that lies past the work area edge in
    /// that direction, and moves focus only once no such part is left.
    pub(super) fn focus_direction(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        direction: Direction,
        forward: bool,
    ) {
        if self.reveal_hidden_part(hub, ws_id, direction, forward) {
            return;
        }
        let Some(column) = self.focused_column_index(hub, ws_id) else {
            return;
        };
        let target = match direction {
            Direction::Vertical => {
                let Some(target) = self.vertical_neighbor(hub, ws_id, column, forward) else {
                    return;
                };
                target
            }
            Direction::Horizontal => {
                let count = self.workspaces[&ws_id].columns.len();
                let target_column = match forward {
                    true if column + 1 < count => column + 1,
                    false if column > 0 => column - 1,
                    _ => return,
                };
                self.last_focused_in_column(hub, ws_id, target_column)
            }
        };
        self.focus_tiling(hub, ws_id, target);
    }

    pub(super) fn focused_column_index(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> Option<usize> {
        let focus = self.workspaces.get(&ws_id)?.focused_window()?;
        self.column_index_of(hub, ws_id, focus)
    }

    /// The first window in `focus_history` that the column holds. `focus_history` holds every
    /// tiling window of the workspace, so the search always finds one.
    pub(super) fn last_focused_in_column(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        column: usize,
    ) -> WindowId {
        let state = &self.workspaces[&ws_id];
        let members = Self::column_windows(hub, state.columns[column].container);
        state
            .focus_history
            .iter()
            .copied()
            .find(|window_id| members.contains(window_id))
            .unwrap_or_else(|| {
                panic!("scrolling {ws_id}: focus_history holds no window of column {column}")
            })
    }

    /// The window above or below the focused window in its column. `None` at the column's edge.
    fn vertical_neighbor(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        column: usize,
        forward: bool,
    ) -> Option<WindowId> {
        let state = &self.workspaces[&ws_id];
        let focus = state.focused_window()?;
        let windows = Self::column_windows(hub, state.columns[column].container);
        let row = windows.iter().position(|&window_id| window_id == focus)?;
        let target_row = if forward {
            row + 1
        } else {
            row.checked_sub(1)?
        };
        windows.get(target_row).copied()
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

    /// Appends as least recently focused, so the first tiling window of an empty workspace
    /// becomes its tiling focus without a focus request.
    pub(super) fn add_to_history(&mut self, window_id: WindowId) {
        if !self.focus_history.contains(&window_id) {
            self.focus_history.push(window_id);
        }
    }

    pub(super) fn drop_from_history(&mut self, window_id: WindowId) {
        if let Some(position) = self.focus_history.iter().position(|&w| w == window_id) {
            self.focus_history.remove(position);
        }
    }
}
