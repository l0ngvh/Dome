use std::collections::BTreeSet;
use std::path::Path;

use super::node::WorkspaceId;
use super::{Hub, WindowId};
use crate::core::PaneDisplay;
use crate::core::master::PaneConfig;
use crate::core::preferred_layout::{PreferredLayouts, PreferredWorkspace};
use crate::core::{SplitMode, TreeLayoutNode, WindowMatcher};

impl Hub {
    /// Synthesises one matcher per window from its live metadata. A window that a
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
        let ws_ids: Vec<WorkspaceId> = self.access.workspaces.sorted_ids();

        // Clearing first is what drops a workspace that no longer exists. A
        // monitor with no live workspace keeps its entries, so a file written on
        // the laptop still carries the desktop's layout.
        let live_monitors: BTreeSet<String> = ws_ids
            .iter()
            .map(|&ws_id| self.access.origin_monitor_name(ws_id))
            .collect();
        for monitor in &live_monitors {
            self.access.preferred_layouts.clear_monitor(monitor);
        }
        for ws_id in ws_ids {
            self.export_workspace(ws_id);
        }

        let rendered = render_layout(&self.access.preferred_layouts);

        // Regenerate destroys any comments the user added, so keep the prior
        // file recoverable.
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

/// Regenerates the whole layout file from the in-memory layout as a Lua table
/// literal. The leading annotation lets LuaLS check the file against
/// dome.meta.lua.
pub(super) fn render_layout(layouts: &PreferredLayouts) -> String {
    let mut out = String::new();
    out.push_str("---@type dome.Layout\n");
    out.push_str("return {\n");
    for (monitor, entries) in layouts.monitors() {
        out.push_str(&format!("{}[{}] = {{\n", indent(1), lua_str(monitor)));
        for (name, ws) in entries.workspaces() {
            emit_workspace(&mut out, name, ws, 2);
        }
        out.push_str(&format!("{}}},\n", indent(1)));
    }
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
                // Lua reads up to three digits after a decimal escape, so an
                // unpadded one merges with a digit that follows it in the title.
                out.push_str(&format!("\\{:03}", c as u32));
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

/// `level` is the indent of the entry's own key, so its fields sit one level
/// deeper.
fn emit_workspace(out: &mut String, name: &str, ws: &PreferredWorkspace, level: usize) {
    let element = indent(level);
    let field = indent(level + 1);
    let nested = level + 1;
    out.push_str(&element);
    out.push_str(&format!("[{}] = {{\n", lua_str(name)));
    match ws {
        PreferredWorkspace::PartitionTree {
            tree,
            float,
            fullscreen,
        } => {
            out.push_str(&format!("{field}layout = \"partition_tree\",\n"));
            if let Some(tree) = tree {
                out.push_str(&format!("{field}tree = {},\n", emit_tree(tree, nested)));
            }
            emit_display_lists(out, &field, nested, float, fullscreen);
        }
        PreferredWorkspace::Master {
            master_ratio,
            master_count,
            master,
            secondary,
            float,
            fullscreen,
        } => {
            out.push_str(&format!("{field}layout = \"master\",\n"));
            if let Some(ratio) = master_ratio {
                out.push_str(&format!("{field}master_ratio = {ratio},\n"));
            }
            if let Some(count) = master_count {
                out.push_str(&format!("{field}master_count = {count},\n"));
            }
            if !master.children.is_empty() {
                out.push_str(&format!("{field}master = {},\n", emit_pane(master, nested)));
            }
            if !secondary.children.is_empty() {
                out.push_str(&format!(
                    "{field}secondary = {},\n",
                    emit_pane(secondary, nested)
                ));
            }
            emit_display_lists(out, &field, nested, float, fullscreen);
        }
    }
    out.push_str(&element);
    out.push_str("},\n");
}

fn emit_display_lists(
    out: &mut String,
    field: &str,
    level: usize,
    float: &[WindowMatcher],
    fullscreen: &[WindowMatcher],
) {
    if !float.is_empty() {
        out.push_str(&format!(
            "{field}float = {},\n",
            emit_matcher_list(float, level)
        ));
    }
    if !fullscreen.is_empty() {
        out.push_str(&format!(
            "{field}fullscreen = {},\n",
            emit_matcher_list(fullscreen, level)
        ));
    }
}
