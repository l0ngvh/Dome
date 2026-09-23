use crate::config::lua::deserializer::{FromLuaValue, LoadContext, as_table, string_enum};

use super::hub::Hub;
use super::node::{MonitorId, WindowId, WindowMetadata, WorkspaceId};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct WindowMatcher {
    pub(crate) app: Option<String>,
    pub(crate) bundle_id: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) process: Option<String>,
    pub(crate) class: Option<String>,
    pub(crate) aumid: Option<String>,
}

impl FromLuaValue for WindowMatcher {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a window matcher table")?;
        Ok(WindowMatcher {
            app: read_pattern(table, "app", cx),
            bundle_id: read_pattern(table, "bundle_id", cx),
            title: read_pattern(table, "title", cx),
            process: read_pattern(table, "process", cx),
            class: read_pattern(table, "class", cx),
            aumid: read_pattern(table, "aumid", cx),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum WindowMode {
    Tiling,
    Float,
    Fullscreen,
}

string_enum!(
    WindowMode,
    "\"tiling\", \"float\" or \"fullscreen\"",
    "tiling" => WindowMode::Tiling,
    "float" => WindowMode::Float,
    "fullscreen" => WindowMode::Fullscreen,
);

pub(crate) fn pattern_matches(pattern: &str, text: &str) -> bool {
    if let Some(regex) = pattern.strip_prefix('/').and_then(|p| p.strip_suffix('/')) {
        regex::Regex::new(regex)
            .map(|r| r.is_match(text))
            .unwrap_or(false)
    } else {
        pattern == text
    }
}

fn read_pattern(table: &mlua::Table, key: &str, cx: &mut LoadContext) -> Option<String> {
    let pattern: Option<String> = cx.field(table, key);
    let pattern = pattern?;
    if let Some(regex) = pattern.strip_prefix('/').and_then(|p| p.strip_suffix('/'))
        && let Err(e) = regex::Regex::new(regex)
    {
        cx.push(key);
        cx.warn_value(&format!("is not a valid regex: {e}"));
        cx.pop();
        return None;
    }
    Some(pattern)
}

/// A per-workspace float or fullscreen entry. The list it lives in decides its
/// mode. `window` is the window currently holding the entry, or `None` when the
/// entry is free.
#[derive(Debug, Clone)]
pub(super) struct FloatFullscreenEntry {
    pub(super) matcher: WindowMatcher,
    pub(super) window: Option<WindowId>,
}

/// Result of routing a new window through the matcher lists.
pub(super) struct MatcherHit {
    /// Workspace to place the window on. `None` means the current workspace, used by global matchers.
    pub(super) ws_id: Option<WorkspaceId>,
    pub(super) mode: WindowMode,
    /// Index of the free entry the window will hold, within the target workspace's list for
    /// `mode`.
    ///
    /// - `None` for a tiling hit, because a tiling window holds a slot of its strategy.
    /// - `None` for a global hit, because a global matcher may route any number of windows.
    pub(super) entry_index: Option<usize>,
}

impl Hub {
    /// Routes a window's metadata to a placement, or `None` if nothing matches.
    pub(super) fn resolve_matcher(&self, metadata: &dyn WindowMetadata) -> Option<MatcherHit> {
        let current_ws = self.current_workspace();
        let search_order: Vec<WorkspaceId> = std::iter::once(current_ws)
            .chain(
                self.access
                    .workspaces
                    .sorted_ids()
                    .into_iter()
                    .filter(|&id| id != current_ws),
            )
            .collect();

        for mode in [WindowMode::Fullscreen, WindowMode::Float] {
            for &ws_id in &search_order {
                if let Some(entry_index) = self.find_free_entry(ws_id, mode, metadata) {
                    return Some(MatcherHit {
                        ws_id: Some(ws_id),
                        mode,
                        entry_index: Some(entry_index),
                    });
                }
            }
        }
        for &ws_id in &search_order {
            if self
                .strategies
                .for_workspace(ws_id)
                .matches_tiling(ws_id, metadata)
            {
                return Some(MatcherHit {
                    ws_id: Some(ws_id),
                    mode: WindowMode::Tiling,
                    entry_index: None,
                });
            }
        }
        let tiling = &self.access.tiling;
        for (matchers, mode) in [
            (&tiling.fullscreen, WindowMode::Fullscreen),
            (&tiling.float, WindowMode::Float),
        ] {
            if matchers.iter().any(|m| metadata.matches_window_matcher(m)) {
                return Some(MatcherHit {
                    ws_id: None,
                    mode,
                    entry_index: None,
                });
            }
        }
        None
    }

    /// Returns the index of the first free entry in `ws_id`'s list for `mode`
    /// whose matcher accepts `metadata`.
    fn find_free_entry(
        &self,
        ws_id: WorkspaceId,
        mode: WindowMode,
        metadata: &dyn WindowMetadata,
    ) -> Option<usize> {
        let ws = self.access.workspaces.get(ws_id);
        let entries = match mode {
            WindowMode::Fullscreen => &ws.fullscreen_entries,
            WindowMode::Float => &ws.float_entries,
            WindowMode::Tiling => return None,
        };
        entries
            .iter()
            .position(|e| e.window.is_none() && metadata.matches_window_matcher(&e.matcher))
    }

    /// Rebuilds the float and fullscreen entry lists of `ws_id` from its entry in the
    /// layout file, with every entry free.
    pub(super) fn load_entries(&mut self, ws_id: WorkspaceId) {
        let monitor = self.access.origin_monitor_name(ws_id);
        let workspace = self.access.workspaces.get_mut(ws_id);
        let layout = self
            .access
            .preferred_layouts
            .workspace(&monitor, &workspace.name);
        let free = |matchers: &[WindowMatcher]| {
            matchers
                .iter()
                .map(|m| FloatFullscreenEntry {
                    matcher: m.clone(),
                    window: None,
                })
                .collect()
        };
        workspace.float_entries = layout.map_or_else(Vec::new, |l| free(&l.float));
        workspace.fullscreen_entries = layout.map_or_else(Vec::new, |l| free(&l.fullscreen));
    }

    /// Creates each workspace that the layout file names under a connected monitor.
    pub(super) fn create_named_workspaces(&mut self) {
        let named: Vec<(MonitorId, String)> = self
            .access
            .preferred_layouts
            .entries()
            .filter_map(|(monitor, name, _)| {
                let monitor_id = self.monitor_id_by_disambiguated_name(monitor)?;
                Some((monitor_id, name.to_string()))
            })
            .collect();
        for (monitor_id, name) in named {
            self.get_or_create_workspace_on(&name, Some(monitor_id));
        }
    }
}
