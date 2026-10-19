use crate::core::hub::HubAccess;
use crate::core::node::{Direction, WorkspaceId};

use super::ScrollingStrategy;

impl ScrollingStrategy {
    /// A vertical move swaps the focused window with its neighbor in the column. A horizontal
    /// move takes a stacked window out into a new column beside its own, and swaps a column of
    /// one window with the neighboring column. A selected column moves only sideways, swapping
    /// with the neighboring column.
    pub(super) fn move_direction(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        direction: Direction,
        forward: bool,
    ) {
        let Some(column) = self.focused_column_index(hub, ws_id) else {
            return;
        };
        let selected = self.workspaces[&ws_id].selected_column.is_some();
        match direction {
            Direction::Vertical => {
                if !selected && self.reorder_in_column(hub, ws_id, column, forward) {
                    self.compute_placement(hub, ws_id);
                }
            }
            Direction::Horizontal => {
                let container = self.workspaces[&ws_id].columns[column].container;
                if !selected && Self::column_windows(hub, container).len() > 1 {
                    self.move_window_out(hub, ws_id, column, forward);
                } else {
                    let state = self.workspaces.get_mut(&ws_id).unwrap();
                    let target = match forward {
                        true if column + 1 < state.columns.len() => column + 1,
                        false if column > 0 => column - 1,
                        _ => return,
                    };
                    state.columns.swap(column, target);
                }
                self.compute_placement(hub, ws_id);
            }
        }
    }

    /// Flips the spawn direction of the focused window. Does nothing while a column is selected,
    /// because the column rather than a window has focus.
    pub(super) fn toggle_spawn_mode(&mut self, ws_id: WorkspaceId) {
        let state = &self.workspaces[&ws_id];
        let Some(focus) = state
            .focused_window()
            .filter(|_| state.selected_column.is_none())
        else {
            return;
        };
        let state = self.window_states.get_mut(&focus).unwrap();
        state.spawn_direction = match state.spawn_direction {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal,
        };
        tracing::debug!(window = %focus, direction = %state.spawn_direction, "Toggled spawn direction");
    }

    /// Swaps the focused window with its neighbor toward `forward` in the column. Returns false
    /// and changes nothing at the edge of the column.
    fn reorder_in_column(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        column: usize,
        forward: bool,
    ) -> bool {
        let Some(focus) = self.workspaces[&ws_id].focused_window() else {
            return false;
        };
        let container = self.workspaces[&ws_id].columns[column].container;
        let row = Self::row_of(hub, container, focus);
        let target = if forward {
            row + 1
        } else {
            let Some(above) = row.checked_sub(1) else {
                return false;
            };
            above
        };
        if target >= hub.containers.get(container).children().len() {
            return false;
        }
        hub.containers.get_mut(container).children.swap(row, target);
        true
    }

    /// Removes the focused window from its column and opens a new column for it directly beside
    /// the old one, on the `forward` side, at the old column's width.
    fn move_window_out(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        column: usize,
        forward: bool,
    ) {
        let state = &self.workspaces[&ws_id];
        let focus = state.focused_window().expect("a move has a focused window");
        let old = &state.columns[column];
        let (old_container, width) = (old.container, old.width);
        let row = Self::row_of(hub, old_container, focus);
        hub.containers.get_mut(old_container).children.remove(row);
        let index = if forward { column + 1 } else { column };
        self.insert_column(hub, ws_id, index, focus, width, None);
    }
}
