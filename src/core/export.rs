use std::path::Path;

use serde::Serialize;
use toml_edit::ser::ValueSerializer;
use toml_edit::{ArrayOfTables, DocumentMut, Item};

use super::Hub;
use super::node::WorkspaceId;
use super::strategy::WorkspaceExport;

pub(super) fn write_layout(
    layout_path: &Path,
    exported: &[(String, WorkspaceExport)],
) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(layout_path).unwrap_or_default();
    let mut doc: DocumentMut = if content.is_empty() {
        DocumentMut::new()
    } else {
        content.parse()?
    };

    let arr = doc
        .entry("workspace")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| anyhow::anyhow!("workspace key exists but is not an array of tables"))?;

    for (name, ws) in exported {
        let mut found = false;
        for entry in arr.iter_mut() {
            if entry["name"].as_str() == Some(name) {
                fill_entry(entry, ws)?;
                found = true;
                break;
            }
        }

        if !found {
            let mut table = toml_edit::Table::new();
            table.insert("name", toml_edit::value(name));
            fill_entry(&mut table, ws)?;
            arr.push(table);
        }
    }

    Ok(doc.to_string())
}

fn fill_entry(table: &mut toml_edit::Table, ws: &WorkspaceExport) -> anyhow::Result<()> {
    table.insert("strategy", toml_edit::value(&ws.strategy));
    match ws.strategy.as_str() {
        "partition_tree" => {
            table.remove("master_ratio");
            table.remove("master_count");
            table.remove("master");
            table.remove("secondary");
            match &ws.tree {
                Some(t) => {
                    table.insert("tree", Item::Value(t.serialize(ValueSerializer::new())?));
                }
                None => {
                    table.remove("tree");
                }
            }
        }
        "master" => {
            table.remove("tree");
            if let Some(r) = ws.master_ratio {
                table.insert("master_ratio", toml_edit::value(r as f64));
            }
            if let Some(c) = ws.master_count {
                table.insert("master_count", toml_edit::value(c as i64));
            }
            if !ws.master.is_empty() {
                table.insert(
                    "master",
                    Item::Value(ws.master.serialize(ValueSerializer::new())?),
                );
            }
            if !ws.secondary.is_empty() {
                table.insert(
                    "secondary",
                    Item::Value(ws.secondary.serialize(ValueSerializer::new())?),
                );
            }
        }
        _ => {}
    }
    Ok(())
}

impl Hub {
    pub(crate) fn export_layout(&mut self, layout_path: &Path) -> anyhow::Result<()> {
        let ws_ids: Vec<(WorkspaceId, String)> = self
            .access
            .workspaces
            .all_active()
            .into_iter()
            .map(|(ws_id, ws)| (ws_id, ws.name.clone()))
            .collect();

        let workspaces: Vec<(String, WorkspaceExport)> = ws_ids
            .into_iter()
            .filter_map(|(ws_id, name)| self.export_workspace(ws_id).map(|export| (name, export)))
            .collect();

        let toml_string = write_layout(layout_path, &workspaces)?;

        let tmp = layout_path.with_extension("toml.tmp");
        std::fs::write(&tmp, &toml_string)?;
        std::fs::rename(&tmp, layout_path)?;

        Ok(())
    }
}
