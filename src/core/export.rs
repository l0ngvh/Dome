use std::path::Path;

use super::node::WorkspaceId;
use super::strategy::WorkspaceExport;
use super::{Hub, WindowId};
use crate::config::{PaneConfig, SplitMode, TreeLayoutNode, WindowMatcher};
use crate::core::PaneDisplay;

impl Hub {
    /// Synthesises one matcher per window from its live metadata. A window a
    /// rule placed is re-synthesised too, so a general rule expands into one
    /// concrete matcher per matched window on export.
    pub(super) fn synthesize_display_matchers(
        &self,
        window_ids: &[WindowId],
    ) -> Vec<WindowMatcher> {
        window_ids
            .iter()
            .map(|&wid| self.access.windows.get(wid).metadata.to_window_matcher())
            .collect()
    }

    pub(crate) fn export_layout(&mut self, layout_path: &Path) -> anyhow::Result<()> {
        let ws_ids: Vec<(WorkspaceId, String)> = self
            .access
            .workspaces
            .sorted_ids()
            .into_iter()
            .map(|ws_id| (ws_id, self.access.workspaces.get(ws_id).name.clone()))
            .collect();

        let workspaces: Vec<(String, WorkspaceExport)> = ws_ids
            .into_iter()
            .map(|(ws_id, name)| (name, self.export_workspace(ws_id)))
            .collect();

        let rendered = render_layout(&workspaces);

        // Regenerate destroys any comments the user added, so keep the prior
        // file recoverable. A single fixed backup, overwritten each export.
        if layout_path.exists() {
            let backup = layout_path.with_extension("lua.bak");
            std::fs::copy(layout_path, &backup)?;
        }

        let tmp = layout_path.with_extension("lua.tmp");
        std::fs::write(&tmp, &rendered)?;
        std::fs::rename(&tmp, layout_path)?;

        Ok(())
    }
}

/// Regenerates the whole layout file from live state as a Lua table literal.
/// The leading annotation lets LuaLS check the file against dome.meta.lua.
pub(super) fn render_layout(workspaces: &[(String, WorkspaceExport)]) -> String {
    let mut out = String::new();
    out.push_str("---@type dome.Layout\n");
    out.push_str("return {\n");
    out.push_str("  workspace = {\n");
    for (name, ws) in workspaces {
        emit_workspace(&mut out, name, ws);
    }
    out.push_str("  },\n");
    out.push_str("}\n");
    out
}

/// Emits `s` as a Lua short-string literal, escaping the characters that would
/// otherwise break the literal or the file. Control characters use a decimal
/// escape so an odd window title cannot produce invalid Lua.
fn lua_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\{}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn indent(level: usize) -> String {
    "  ".repeat(level)
}

fn matcher_inline(matcher: &WindowMatcher) -> String {
    let mut fields: Vec<String> = Vec::new();
    let mut push = |key: &str, value: &Option<String>| {
        if let Some(v) = value {
            fields.push(format!("{key} = {}", lua_str(v)));
        }
    };
    push("app", &matcher.app);
    push("bundle_id", &matcher.bundle_id);
    push("title", &matcher.title);
    push("process", &matcher.process);
    push("class", &matcher.class);
    push("aumid", &matcher.aumid);
    if fields.is_empty() {
        "{}".to_string()
    } else {
        format!("{{ {} }}", fields.join(", "))
    }
}

/// `level` is the indent of the value's closing brace, so nested children sit
/// one level deeper.
fn emit_tree(node: &TreeLayoutNode, level: usize) -> String {
    match node {
        TreeLayoutNode::Leaf(matcher) => matcher_inline(matcher),
        TreeLayoutNode::Container { split, children } => {
            let child_pad = indent(level + 1);
            let close_pad = indent(level);
            let mut items = String::new();
            for child in children {
                items.push_str(&child_pad);
                items.push_str(&emit_tree(child, level + 1));
                items.push_str(",\n");
            }
            let array = format!("{{\n{items}{close_pad}}}");
            match split {
                None => array,
                Some(split) => format!(
                    "{{ split = {}, children = {array} }}",
                    lua_str(split_str(*split))
                ),
            }
        }
    }
}

fn emit_matcher_list(matchers: &[WindowMatcher], level: usize) -> String {
    let child_pad = indent(level + 1);
    let close_pad = indent(level);
    let mut items = String::new();
    for matcher in matchers {
        items.push_str(&child_pad);
        items.push_str(&matcher_inline(matcher));
        items.push_str(",\n");
    }
    format!("{{\n{items}{close_pad}}}")
}

fn emit_pane(pane: &PaneConfig, level: usize) -> String {
    let array = emit_matcher_list(&pane.children, level);
    match pane.display {
        PaneDisplay::Tiled => array,
        PaneDisplay::Tabbed => format!("{{ display = \"tabbed\", children = {array} }}"),
    }
}

fn split_str(split: SplitMode) -> &'static str {
    match split {
        SplitMode::Horizontal => "horizontal",
        SplitMode::Vertical => "vertical",
        SplitMode::Tabbed => "tabbed",
    }
}

fn emit_workspace(out: &mut String, name: &str, ws: &WorkspaceExport) {
    let element = indent(2);
    let field = indent(3);
    out.push_str(&element);
    out.push_str("{\n");
    out.push_str(&format!("{field}name = {},\n", lua_str(name)));
    out.push_str(&format!("{field}strategy = {},\n", lua_str(&ws.strategy)));
    match ws.strategy.as_str() {
        "partition_tree" => {
            if let Some(tree) = &ws.tree {
                out.push_str(&format!("{field}tree = {},\n", emit_tree(tree, 3)));
            }
        }
        "master" => {
            if let Some(ratio) = ws.master_ratio {
                out.push_str(&format!("{field}master_ratio = {ratio},\n"));
            }
            if let Some(count) = ws.master_count {
                out.push_str(&format!("{field}master_count = {count},\n"));
            }
            if !ws.master.children.is_empty() {
                out.push_str(&format!("{field}master = {},\n", emit_pane(&ws.master, 3)));
            }
            if !ws.secondary.children.is_empty() {
                out.push_str(&format!(
                    "{field}secondary = {},\n",
                    emit_pane(&ws.secondary, 3)
                ));
            }
        }
        _ => {}
    }
    if !ws.float.is_empty() {
        out.push_str(&format!(
            "{field}float = {},\n",
            emit_matcher_list(&ws.float, 3)
        ));
    }
    if !ws.fullscreen.is_empty() {
        out.push_str(&format!(
            "{field}fullscreen = {},\n",
            emit_matcher_list(&ws.fullscreen, 3)
        ));
    }
    out.push_str(&element);
    out.push_str("},\n");
}
