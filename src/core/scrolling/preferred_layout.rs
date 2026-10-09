use crate::config::lua::deserializer::{
    FromLuaValue, LoadContext, Shape, as_table, warn_if_shape_mismatched,
};
use crate::core::hub::HubAccess;
use crate::core::matcher::WindowMode;
use crate::core::node::{Child, WindowId, WorkspaceId};
use crate::core::slot::{Slot, SlotId, held_tiling_slot};
use crate::core::{PreferredScrolling, SizeConstraint, WindowMatcher};

use super::ScrollingStrategy;

/// One entry of a scrolling workspace's `columns` list, left to right.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnConfig {
    /// `None` takes the workspace `column_width`, else `scrolling.column_width`.
    pub(crate) width: Option<SizeConstraint>,
    /// Top to bottom.
    pub(crate) children: Vec<WindowMatcher>,
}

impl ColumnConfig {
    #[cfg(test)]
    pub(crate) fn bare(matcher: WindowMatcher) -> Self {
        Self {
            width: None,
            children: vec![matcher],
        }
    }
}

/// Accepts either a window matcher or a table carrying `width` and `children`.
impl FromLuaValue for ColumnConfig {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a window matcher, or a column table")?;
        warn_if_shape_mismatched(table, Shape::Map, cx);
        let has_width = table.contains_key("width")?;
        let has_children = table.contains_key("children")?;
        if !has_width && !has_children {
            let matcher = WindowMatcher::from_lua_value(value, cx)?;
            return Ok(ColumnConfig {
                width: None,
                children: vec![matcher],
            });
        }
        if !has_children {
            return Err(mlua::Error::runtime(
                "a column with width must also have children",
            ));
        }
        let children: Vec<WindowMatcher> = cx.field(table, "children");
        Ok(ColumnConfig {
            width: cx.field(table, "width"),
            children,
        })
    }
}

/// One entry of a workspace's `columns` list, kept as the slot ids of its matchers.
#[derive(Debug)]
pub(super) struct PreferredColumnSlot {
    /// `None` takes the workspace `column_width`, else `scrolling.column_width`.
    pub(super) width: Option<SizeConstraint>,
    /// Top to bottom.
    pub(super) slots: Vec<SlotId>,
}

impl ScrollingStrategy {
    /// Allocates one tiling slot per matcher, column slot by column slot and top to bottom
    /// inside each.
    pub(super) fn allocate_column_slots(
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        columns: &[ColumnConfig],
    ) -> Vec<PreferredColumnSlot> {
        columns
            .iter()
            .map(|column| PreferredColumnSlot {
                width: column.width,
                slots: column
                    .children
                    .iter()
                    .map(|matcher| {
                        hub.slots
                            .allocate(Slot::new(matcher.clone(), ws_id, WindowMode::Tiling))
                    })
                    .collect(),
            })
            .collect()
    }

    /// Inserts window `id` into the live column that the column slot listing `slot` opened. The
    /// window goes above the first window there that holds a later slot of the same column
    /// slot, otherwise at the bottom. When no live column remembers that column slot, the window
    /// opens a new column before the first live column that remembers a later one, otherwise at
    /// the end of the row.
    pub(super) fn place_by_slot(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        id: WindowId,
        slot: SlotId,
    ) {
        let state = &self.workspaces[&ws_id];
        let (column_slot, row) = state
            .column_slots
            .iter()
            .enumerate()
            .find_map(|(index, column_slot)| {
                let row = column_slot.slots.iter().position(|&s| s == slot)?;
                Some((index, row))
            })
            .unwrap_or_else(|| {
                panic!("{slot:?} is a slot of no column of scrolling workspace {ws_id}")
            });
        let Some(column) = state
            .columns
            .iter()
            .position(|column| column.occupy == Some(column_slot))
        else {
            let index = state
                .columns
                .iter()
                .position(|column| column.occupy.is_some_and(|other| other > column_slot))
                .unwrap_or(state.columns.len());
            let width = state.column_slots[column_slot]
                .width
                .unwrap_or_else(|| self.column_width_for(ws_id));
            self.insert_column(hub, ws_id, index, id, width, Some(column_slot));
            return;
        };
        let container = state.columns[column].container;
        let slots = &state.column_slots[column_slot].slots;
        let children = hub.containers.get(container).children();
        let position = children
            .iter()
            .position(|child| {
                let Child::Window(window_id) = child else {
                    return false;
                };
                held_tiling_slot(&hub.slots, *window_id, ws_id)
                    .and_then(|held| slots.iter().position(|&s| s == held))
                    .is_some_and(|held_row| held_row > row)
            })
            .unwrap_or(children.len());
        hub.containers
            .get_mut(container)
            .children
            .insert(position, Child::Window(id));
    }

    /// Writes every live column with its stored width and one matcher per window.
    pub(super) fn export_columns(&self, hub: &HubAccess, ws_id: WorkspaceId) -> PreferredScrolling {
        let state = self.workspaces.get(&ws_id).unwrap();
        PreferredScrolling {
            column_width: state.column_width,
            columns: state
                .columns
                .iter()
                .map(|column| ColumnConfig {
                    width: Some(column.width),
                    children: Self::column_windows(hub, column.container)
                        .into_iter()
                        .map(|window_id| hub.windows.get(window_id).metadata.to_window_matcher())
                        .collect(),
                })
                .collect(),
        }
    }
}
