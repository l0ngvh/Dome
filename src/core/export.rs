use std::path::Path;

use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};

use super::matcher::FloatFullscreenMatcherId;
use super::node::{DisplayMode, WorkspaceId};
use super::strategy::WorkspaceExport;
use super::{Hub, WindowId};
use crate::config::{PaneConfig, SplitMode, TreeLayoutNode, WindowMatcher};
use crate::core::PaneDisplay;

fn matcher_to_cst(matcher: &WindowMatcher) -> CstInputValue {
    let mut fields: Vec<(String, CstInputValue)> = Vec::new();
    let mut push = |key: &str, value: &Option<String>| {
        if let Some(v) = value {
            fields.push((key.to_string(), v.clone().into()));
        }
    };
    push("app", &matcher.app);
    push("bundle_id", &matcher.bundle_id);
    push("title", &matcher.title);
    push("process", &matcher.process);
    push("class", &matcher.class);
    push("aumid", &matcher.aumid);
    CstInputValue::Object(fields)
}

fn push_matcher_list(fields: &mut Vec<(String, CstInputValue)>, key: &str, list: &[WindowMatcher]) {
    if list.is_empty() {
        return;
    }
    let matchers = list.iter().map(matcher_to_cst).collect::<Vec<_>>();
    fields.push((key.to_string(), CstInputValue::Array(matchers)));
}

/// A tiled pane exports as a plain matcher array. A tabbed pane exports as
/// `{ display: "tabbed", children: [...] }`, the object shape `PaneConfig`
/// reads back. An empty pane exports nothing.
fn push_pane(fields: &mut Vec<(String, CstInputValue)>, key: &str, pane: &PaneConfig) {
    if pane.children.is_empty() {
        return;
    }
    let matchers = pane.children.iter().map(matcher_to_cst).collect::<Vec<_>>();
    let value = match pane.display {
        PaneDisplay::Tiled => CstInputValue::Array(matchers),
        PaneDisplay::Tabbed => CstInputValue::Object(vec![
            ("display".to_string(), "tabbed".into()),
            ("children".to_string(), CstInputValue::Array(matchers)),
        ]),
    };
    fields.push((key.to_string(), value));
}

/// Work-stack frame for `tree_to_cst`, which builds the tree bottom-up without
/// recursion per the no-recursion rule.
enum TreeFrame<'a> {
    Enter(&'a TreeLayoutNode),
    ExitContainer {
        split: Option<SplitMode>,
        children: usize,
    },
}

fn tree_to_cst(root: &TreeLayoutNode) -> CstInputValue {
    let mut work: Vec<TreeFrame> = vec![TreeFrame::Enter(root)];
    let mut built: Vec<CstInputValue> = Vec::new();
    for _ in super::bounded_loop() {
        let Some(frame) = work.pop() else {
            break;
        };
        match frame {
            TreeFrame::Enter(TreeLayoutNode::Leaf(matcher)) => {
                built.push(matcher_to_cst(matcher));
            }
            TreeFrame::Enter(TreeLayoutNode::Container { split, children }) => {
                work.push(TreeFrame::ExitContainer {
                    split: *split,
                    children: children.len(),
                });
                for child in children.iter().rev() {
                    work.push(TreeFrame::Enter(child));
                }
            }
            TreeFrame::ExitContainer { split, children } => {
                let kids = built.split_off(built.len() - children);
                match split {
                    None => built.push(CstInputValue::Array(kids)),
                    Some(split) => built.push(CstInputValue::Object(vec![
                        ("split".to_string(), split_str(split).into()),
                        ("children".to_string(), CstInputValue::Array(kids)),
                    ])),
                }
            }
        }
    }
    built
        .pop()
        .expect("tree_to_cst leaves exactly one built node")
}

fn workspace_to_cst(name: &str, ws: &WorkspaceExport) -> CstInputValue {
    let mut fields: Vec<(String, CstInputValue)> = vec![
        ("name".to_string(), name.into()),
        ("strategy".to_string(), ws.strategy.clone().into()),
    ];
    match ws.strategy.as_str() {
        "partition_tree" => {
            if let Some(tree) = &ws.tree {
                fields.push(("tree".to_string(), tree_to_cst(tree)));
            }
        }
        "master" => {
            if let Some(ratio) = ws.master_ratio {
                fields.push(("master_ratio".to_string(), f64::from(ratio).into()));
            }
            if let Some(count) = ws.master_count {
                fields.push(("master_count".to_string(), count.into()));
            }
            push_pane(&mut fields, "master", &ws.master);
            push_pane(&mut fields, "secondary", &ws.secondary);
        }
        _ => {}
    }
    push_matcher_list(&mut fields, "float", &ws.float);
    push_matcher_list(&mut fields, "fullscreen", &ws.fullscreen);
    CstInputValue::Object(fields)
}

fn split_str(split: SplitMode) -> &'static str {
    match split {
        SplitMode::Horizontal => "horizontal",
        SplitMode::Vertical => "vertical",
        SplitMode::Tabbed => "tabbed",
    }
}

fn object_name(obj: &CstObject) -> Option<String> {
    obj.to_serde_value()?
        .get("name")?
        .as_str()
        .map(str::to_string)
}

/// Reconciles live workspaces into the existing `layout.jsonc` text, preserving
/// comments and formatting. Matches each workspace to a document entry by name,
/// updates it in place, appends a new one, and drops entries no longer live. A
/// missing or empty file starts from an empty object. The root node stays alive
/// until `to_string`, because dropping it early can panic.
pub(super) fn render_layout(
    existing: &str,
    workspaces: &[(String, WorkspaceExport)],
) -> anyhow::Result<String> {
    let source = if existing.trim().is_empty() {
        "{}\n"
    } else {
        existing
    };
    let root =
        CstRootNode::parse(source, &ParseOptions::default()).map_err(|e| anyhow::anyhow!("{e}"))?;
    let root_obj = root.object_value_or_set();
    let ws_arr = root_obj.array_value_or_set("workspace");

    let live: Vec<(String, CstInputValue)> = workspaces
        .iter()
        .map(|(name, ws)| (name.clone(), workspace_to_cst(name, ws)))
        .collect();
    let live_names: std::collections::HashSet<&str> =
        live.iter().map(|(name, _)| name.as_str()).collect();

    for element in ws_arr.elements() {
        let Some(obj) = element.as_object() else {
            continue;
        };
        if object_name(&obj).is_some_and(|name| !live_names.contains(name.as_str())) {
            obj.remove();
        }
    }

    for (name, value) in live {
        let existing = ws_arr
            .elements()
            .into_iter()
            .filter_map(|element| element.as_object())
            .find(|obj| object_name(obj).as_deref() == Some(name.as_str()));
        match existing {
            Some(obj) => {
                obj.replace_with(value);
            }
            None => {
                ws_arr.append(value);
            }
        }
    }

    Ok(root.to_string())
}

impl Hub {
    /// Re-emits a deduped clone of the occupying matcher for matcher-placed windows and
    /// synthesises one from live window metadata otherwise, in window-id order.
    pub(super) fn collect_display_matchers(
        &self,
        window_ids: &[WindowId],
        occupy_of: impl Fn(&DisplayMode) -> Option<FloatFullscreenMatcherId>,
    ) -> Vec<WindowMatcher> {
        let mut out: Vec<WindowMatcher> = Vec::new();
        let mut seen: Vec<FloatFullscreenMatcherId> = Vec::new();
        for &wid in window_ids {
            let window = self.access.windows.get(wid);
            match occupy_of(&window.mode) {
                Some(mid) => {
                    if seen.contains(&mid) {
                        continue;
                    }
                    seen.push(mid);
                    out.push(self.float_fullscreen_matchers.get(mid).clone());
                }
                None => out.push(window.metadata.to_window_matcher()),
            }
        }
        out
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

        let existing = std::fs::read_to_string(layout_path).unwrap_or_default();
        let rendered = render_layout(&existing, &workspaces)?;

        let tmp = layout_path.with_extension("jsonc.tmp");
        std::fs::write(&tmp, &rendered)?;
        std::fs::rename(&tmp, layout_path)?;

        Ok(())
    }
}
