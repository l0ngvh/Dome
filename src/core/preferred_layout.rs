use std::collections::BTreeMap;

use crate::config::lua::deserializer::{FromLuaValue, LoadContext, as_table};
use crate::core::master::{PaneConfig, read_master_count_override, read_master_ratio_override};
use crate::core::matcher::WindowMatcher;
use crate::core::partition_tree::TreeLayoutNode;
use crate::core::scrolling::ColumnConfig;

/// The `layout.lua` file root. Every workspace entry sits under a monitor key,
/// which is that monitor's `unique_name`. Both levels are sorted maps, so
/// repeated exports of one state produce the same bytes.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PreferredLayouts {
    monitor: BTreeMap<String, PreferredMonitor>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PreferredMonitor {
    workspace: BTreeMap<String, PreferredWorkspace>,
}

impl FromLuaValue for PreferredLayouts {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        Ok(PreferredLayouts {
            monitor: BTreeMap::from_lua_value(value, cx)?,
        })
    }
}

impl FromLuaValue for PreferredMonitor {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        Ok(PreferredMonitor {
            workspace: BTreeMap::from_lua_value(value, cx)?,
        })
    }
}

impl PreferredLayouts {
    pub(crate) fn workspace(&self, monitor: &str, workspace: &str) -> Option<&PreferredWorkspace> {
        self.monitor.get(monitor)?.workspace.get(workspace)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (&str, &str, &PreferredWorkspace)> {
        self.monitor.iter().flat_map(|(monitor, entries)| {
            entries
                .workspace
                .iter()
                .map(move |(name, entry)| (monitor.as_str(), name.as_str(), entry))
        })
    }

    pub(crate) fn monitors(&self) -> impl Iterator<Item = (&str, &PreferredMonitor)> {
        self.monitor
            .iter()
            .map(|(name, entries)| (name.as_str(), entries))
    }

    pub(crate) fn insert(&mut self, monitor: &str, workspace: &str, entry: PreferredWorkspace) {
        self.monitor
            .entry(monitor.to_string())
            .or_default()
            .workspace
            .insert(workspace.to_string(), entry);
    }

    pub(crate) fn clear_monitor(&mut self, monitor: &str) {
        if let Some(entries) = self.monitor.get_mut(monitor) {
            entries.workspace.clear();
        }
    }
}

impl PreferredMonitor {
    pub(crate) fn workspaces(&self) -> impl Iterator<Item = (&str, &PreferredWorkspace)> {
        self.workspace
            .iter()
            .map(|(name, entry)| (name.as_str(), entry))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PreferredWorkspace {
    PartitionTree {
        tree: Option<TreeLayoutNode>,
        float: Vec<WindowMatcher>,
        fullscreen: Vec<WindowMatcher>,
    },
    Master {
        master_ratio: Option<f32>,
        master_count: Option<usize>,
        master: PaneConfig,
        secondary: PaneConfig,
        float: Vec<WindowMatcher>,
        fullscreen: Vec<WindowMatcher>,
    },
    Scrolling {
        columns: Vec<ColumnConfig>,
        float: Vec<WindowMatcher>,
        fullscreen: Vec<WindowMatcher>,
    },
}

impl FromLuaValue for PreferredWorkspace {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a workspace table")?;
        let layout: String = cx.field_or_else(table, "layout", String::new);
        match layout.as_str() {
            "" => Err(mlua::Error::runtime(
                "layout is required, and must be \"partition_tree\", \"master\" or \"scrolling\"",
            )),
            "partition_tree" => Ok(PreferredWorkspace::PartitionTree {
                tree: cx.field(table, "tree"),
                float: cx.field(table, "float"),
                fullscreen: cx.field(table, "fullscreen"),
            }),
            "master" => Ok(PreferredWorkspace::Master {
                master_ratio: read_master_ratio_override(table, cx),
                master_count: read_master_count_override(table, cx),
                master: cx.field(table, "master"),
                secondary: cx.field(table, "secondary"),
                float: cx.field(table, "float"),
                fullscreen: cx.field(table, "fullscreen"),
            }),
            "scrolling" => Ok(PreferredWorkspace::Scrolling {
                columns: cx.field(table, "columns"),
                float: cx.field(table, "float"),
                fullscreen: cx.field(table, "fullscreen"),
            }),
            other => Err(mlua::Error::runtime(format!(
                "layout must be \"partition_tree\", \"master\" or \"scrolling\", got \"{other}\""
            ))),
        }
    }
}
