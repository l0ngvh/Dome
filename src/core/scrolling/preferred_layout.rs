use crate::config::lua::deserializer::{
    FromLuaValue, LoadContext, Shape, as_table, warn_if_shape_mismatched,
};
use crate::core::hub::HubAccess;
use crate::core::node::WorkspaceId;
use crate::core::scrolling::ScrollingStrategy;
use crate::core::strategy::TilingStrategy;
use crate::core::{PreferredWorkspace, SizeConstraint, WindowMatcher};

/// One entry of a scrolling workspace's `columns` list, left to right.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnConfig {
    /// `None` takes `default_column_width`.
    pub(crate) width: Option<SizeConstraint>,
    /// At most one matcher, because a column holds one window.
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
        let mut children: Vec<WindowMatcher> = cx.field(table, "children");
        if children.len() > 1 {
            cx.push("children");
            cx.warn_dropped(&format!(
                "a column holds one window, so {} matcher(s) after the first are dropped",
                children.len() - 1
            ));
            cx.pop();
            children.truncate(1);
        }
        Ok(ColumnConfig {
            width: cx.field(table, "width"),
            children,
        })
    }
}

impl ScrollingStrategy {
    pub(super) fn sync_preferred_layout(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        incoming: Option<&PreferredWorkspace>,
    ) {
        let incoming: &[ColumnConfig] = match incoming {
            Some(PreferredWorkspace::Scrolling { columns, .. }) => columns,
            _ => &[],
        };
        let Some(state) = self.workspaces.get_mut(&ws_id) else {
            return;
        };
        if state.slots == incoming {
            self.compute_placement(hub, ws_id);
            return;
        }
        tracing::debug!(%ws_id, "Scrolling preferred layout changed, reloading");
        state.slots = incoming.to_vec();
        let focused = state.focused_window();
        let previous_history = std::mem::take(&mut state.focus_history);
        let old_columns = std::mem::take(&mut state.columns);
        for column in old_columns {
            let wid = Self::column_window(hub, column.container);
            hub.free_container(column.container);
            self.insert_column(hub, wid, ws_id, column.width);
            self.workspaces.get_mut(&ws_id).unwrap().record_focus(wid);
        }
        self.workspaces.get_mut(&ws_id).unwrap().focus_history = previous_history;
        self.compute_placement(hub, ws_id);
        if let Some(window) = focused {
            self.set_focus(hub, window);
        }
    }
}
