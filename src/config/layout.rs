//! The `layout.lua` file format and its deserialization.

use anyhow::anyhow;
use serde::Deserialize;
use std::collections::HashMap;

use crate::core::PreferredWorkspace;

use super::lua;

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub(crate) struct PreferredLayouts {
    #[serde(default)]
    pub(crate) workspace: Vec<PreferredWorkspace>,
}

impl PreferredLayouts {
    pub(crate) fn load(path: &str) -> anyhow::Result<Self> {
        let src = std::fs::read_to_string(path)?;
        let mut layout = Self::from_lua(path, &src)?;
        layout.dedup_workspaces("");
        Ok(layout)
    }

    // No field of a layout file holds a function, so no callback is ever
    // registered. The VM needs no `dome` global either.
    pub(super) fn from_lua(path: &str, src: &str) -> anyhow::Result<Self> {
        let vm = mlua::Lua::new();
        let mut callbacks = Vec::new();
        lua::evaluate(&vm, path, src, &mut callbacks).map_err(|e| anyhow!("{path}: {e}"))
    }

    /// Drop workspaces with an empty name and, on a duplicate name, keep the
    /// last entry. `prefix` names the parent field for warning messages.
    pub(super) fn dedup_workspaces(&mut self, prefix: &str) {
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut out: Vec<PreferredWorkspace> = Vec::with_capacity(self.workspace.len());
        for entry in std::mem::take(&mut self.workspace) {
            let ws_name = entry.name().to_string();
            if ws_name.is_empty() {
                tracing::warn!(
                    field = %field_path(prefix, "workspace"),
                    "Empty workspace name, dropping",
                );
                continue;
            }
            if let Some(&idx) = seen.get(&ws_name) {
                tracing::warn!(
                    field = %field_path(prefix, "workspace"),
                    name = ws_name,
                    "Duplicate workspace, replacing earlier entry",
                );
                out[idx] = entry;
            } else {
                seen.insert(ws_name, out.len());
                out.push(entry);
            }
        }
        self.workspace = out;
    }
}

fn field_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}
