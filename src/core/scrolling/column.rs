use crate::core::SizeConstraint;
use crate::core::hub::HubAccess;
use crate::core::node::{Child, ContainerId, Dimension, Direction, Length, WindowId, WorkspaceId};
use crate::core::slot::SlotId;

use super::{Column, ScrollingStrategy, WindowState};

impl ScrollingStrategy {
    /// Places the window by its slot, or by the spawn rules without one, then lays out.
    pub(super) fn attach_tiling_window(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        id: WindowId,
        slot: Option<SlotId>,
    ) {
        hub.windows.get_mut(id).set_workspace(Some(ws_id));
        let spawn_direction = match slot {
            Some(slot) => {
                self.place_by_slot(hub, ws_id, id, slot);
                Direction::Horizontal
            }
            None => self.place_by_spawn_rules(hub, ws_id, id),
        };
        self.window_states.insert(
            id,
            WindowState {
                dimension: Dimension::default(),
                spawn_direction,
            },
        );
        self.workspaces.get_mut(&ws_id).unwrap().add_to_history(id);
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn detach_tiling_window(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        id: WindowId,
    ) {
        self.window_states.remove(&id).unwrap_or_else(|| {
            panic!("scrolling: detaching {id:?} but window_states has no entry")
        });
        self.remove_window(hub, ws_id, id);
        self.compute_placement(hub, ws_id);
    }

    /// Puts the window below the focused window when that window spawns vertically, otherwise
    /// in a new column right of the focused column. Returns the spawn direction the window
    /// starts with.
    fn place_by_spawn_rules(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        id: WindowId,
    ) -> Direction {
        let focus = self.workspaces[&ws_id].focused_window();
        if let Some(focus) = focus
            && self.window_states[&focus].spawn_direction == Direction::Vertical
        {
            let column = self
                .column_index_of(hub, ws_id, focus)
                .expect("the focused window sits in a column");
            let container = self.workspaces[&ws_id].columns[column].container;
            let row = Self::row_of(hub, container, focus);
            hub.containers
                .get_mut(container)
                .children
                .insert(row + 1, Child::Window(id));
            return Direction::Vertical;
        }
        let index = focus
            .and_then(|focus| self.column_index_of(hub, ws_id, focus))
            .map_or(self.workspaces[&ws_id].columns.len(), |column| column + 1);
        let width = self.column_width_for(ws_id);
        self.insert_column(hub, ws_id, index, id, width, None);
        Direction::Horizontal
    }

    pub(super) fn insert_column(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        index: usize,
        id: WindowId,
        width: SizeConstraint,
        occupy: Option<usize>,
    ) {
        let container = hub.allocate_container(vec![Child::Window(id)], ws_id);
        self.workspaces.get_mut(&ws_id).unwrap().columns.insert(
            index,
            Column {
                container,
                width,
                y_offset: Length::ZERO,
                occupy,
            },
        );
    }

    /// Removes the window from its column, and removes the column and frees its container once
    /// it is empty. Tiling focus on the window moves by position, to the window that takes its
    /// row, else to the window above it, else to the column that takes its column's place.
    fn remove_window(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, id: WindowId) {
        let column = self
            .column_index_of(hub, ws_id, id)
            .expect("a detached tiling window sits in a column");
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let container = state.columns[column].container;
        let was_focused = state.focused_window() == Some(id);
        state.drop_from_history(id);
        let row = Self::row_of(hub, container, id);
        hub.containers.get_mut(container).children.remove(row);
        let remaining = Self::column_windows(hub, container);
        if remaining.is_empty() {
            state.columns.remove(column);
            hub.free_container(container);
            if was_focused {
                self.focus_after_column_removal(hub, ws_id, column);
            }
        } else if was_focused {
            state.record_focus(remaining[row.min(remaining.len() - 1)]);
        }
    }

    /// Focuses the window used last in the column now at `index`, or in the last column when
    /// `index` is past the end.
    fn focus_after_column_removal(&mut self, hub: &HubAccess, ws_id: WorkspaceId, index: usize) {
        let Some(last) = self.workspaces[&ws_id].columns.len().checked_sub(1) else {
            return;
        };
        let target = self.last_focused_in_column(hub, ws_id, index.min(last));
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .record_focus(target);
    }

    /// Every window of the column, top to bottom. Panics on a nested container.
    pub(super) fn column_windows(hub: &HubAccess, container: ContainerId) -> Vec<WindowId> {
        hub.containers
            .get(container)
            .children()
            .iter()
            .map(|child| match child {
                Child::Window(window_id) => *window_id,
                Child::Container(_) => {
                    panic!("scrolling column {container:?} holds a nested container")
                }
            })
            .collect()
    }

    pub(super) fn column_index_of(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        window_id: WindowId,
    ) -> Option<usize> {
        self.workspaces
            .get(&ws_id)?
            .columns
            .iter()
            .position(|column| Self::column_windows(hub, column.container).contains(&window_id))
    }

    /// Panics when the column does not hold the window.
    pub(super) fn row_of(hub: &HubAccess, container: ContainerId, window_id: WindowId) -> usize {
        hub.containers
            .get(container)
            .children()
            .iter()
            .position(|child| matches!(child, Child::Window(w) if *w == window_id))
            .unwrap_or_else(|| panic!("scrolling column {container:?} does not hold {window_id:?}"))
    }

    /// The workspace's `column_width`, else `scrolling.column_width`.
    pub(super) fn column_width_for(&self, ws_id: WorkspaceId) -> SizeConstraint {
        self.workspaces[&ws_id]
            .column_width
            .unwrap_or(self.column_width)
    }
}
