use crate::config::PreferredLayouts;
use crate::core::{PaneDisplay, PreferredWorkspace, SplitMode, TreeLayoutNode};

// The struct-shape tests feed JSON, which deserializes through the same serde
// path the Lua loader routes into. layout_loads_from_lua_source covers the
// mlua front-end and its array-vs-map detection.
fn layout_from(src: &str) -> PreferredLayouts {
    let mut layout: PreferredLayouts = serde_json::from_str(src).expect("layout should load");
    layout.dedup_workspaces("");
    layout
}

fn workspace_from(src: &str) -> PreferredWorkspace {
    try_workspace(src).expect("workspace should deserialize")
}

fn try_workspace(src: &str) -> anyhow::Result<PreferredWorkspace> {
    Ok(serde_json::from_str(src)?)
}

#[test]
fn layout_loads_from_lua_source() {
    let src = r#"
---@type dome.Layout
return {
  workspace = {
    {
      name = "dev",
      strategy = "partition_tree",
      tree = {
        { app = "Ghostty" },
        { split = "vertical", children = {
          { app = "Firefox" },
          { app = "Slack" },
        } },
      },
      float = { { app = "System Settings" } },
    },
    {
      name = "work",
      strategy = "master",
      master_ratio = 0.6,
      master = { { app = "Ghostty" } },
      secondary = { display = "tabbed", children = { { app = "Firefox" } } },
    },
  },
}
"#;
    let layout = PreferredLayouts::from_lua("test layout", src).expect("lua layout should load");
    assert_eq!(layout.workspace.len(), 2);
    match &layout.workspace[0] {
        PreferredWorkspace::PartitionTree { tree, float, .. } => {
            let Some(TreeLayoutNode::Container { split, children }) = tree else {
                panic!("expected outer container");
            };
            assert!(split.is_none());
            assert_eq!(children.len(), 2);
            assert!(matches!(children[0], TreeLayoutNode::Leaf(..)));
            assert!(matches!(
                children[1],
                TreeLayoutNode::Container {
                    split: Some(SplitMode::Vertical),
                    ..
                }
            ));
            assert_eq!(float.len(), 1);
        }
        _ => panic!("expected PartitionTree"),
    }
    match &layout.workspace[1] {
        PreferredWorkspace::Master {
            master, secondary, ..
        } => {
            assert_eq!(master.display, PaneDisplay::Tiled);
            assert_eq!(master.children.len(), 1);
            assert_eq!(secondary.display, PaneDisplay::Tabbed);
            assert_eq!(secondary.children.len(), 1);
        }
        _ => panic!("expected Master"),
    }
}

#[test]
fn preferred_layout_default_empty() {
    assert!(layout_from("{}").workspace.is_empty());
}

#[test]
fn preferred_layout_parse_single_entry() {
    let layout = layout_from(r#"{ "workspace": [ { "name": "1", "strategy": "master" } ] }"#);
    assert_eq!(layout.workspace.len(), 1);
    assert_eq!(layout.workspace[0].name(), "1");
    assert!(matches!(
        layout.workspace[0],
        PreferredWorkspace::Master { .. }
    ));
}

#[test]
fn preferred_layout_parse_multiple_distinct() {
    let layout = layout_from(
        r#"{ "workspace": [
  { "name": "1", "strategy": "master" },
  { "name": "scratch", "strategy": "partition_tree" }
] }"#,
    );
    assert_eq!(layout.workspace.len(), 2);
    assert_eq!(layout.workspace[0].name(), "1");
    assert!(matches!(
        layout.workspace[0],
        PreferredWorkspace::Master { .. }
    ));
    assert_eq!(layout.workspace[1].name(), "scratch");
    assert!(matches!(
        layout.workspace[1],
        PreferredWorkspace::PartitionTree { .. }
    ));
}

#[test]
fn preferred_layout_rejects_unknown_strategy() {
    assert!(
        serde_json::from_str::<PreferredLayouts>(
            r#"{ "workspace": [ { "name": "bad", "strategy": "floating" } ] }"#,
        )
        .is_err()
    );
}

#[test]
fn preferred_layout_drop_empty_name() {
    let layout = layout_from(
        r#"{ "workspace": [
  { "name": "", "strategy": "master" },
  { "name": "valid", "strategy": "partition_tree" }
] }"#,
    );
    assert_eq!(layout.workspace.len(), 1);
    assert_eq!(layout.workspace[0].name(), "valid");
}

#[test]
fn preferred_layout_dedup_last_wins() {
    let layout = layout_from(
        r#"{ "workspace": [
  { "name": "1", "strategy": "partition_tree" },
  { "name": "1", "strategy": "master" }
] }"#,
    );
    assert_eq!(layout.workspace.len(), 1);
    assert_eq!(layout.workspace[0].name(), "1");
    assert!(matches!(
        layout.workspace[0],
        PreferredWorkspace::Master { .. }
    ));
}

#[test]
fn tree_leaf_parses() {
    let ws = workspace_from(
        r#"{ "name": "dev", "strategy": "partition_tree", "tree": { "process": "editor.exe" } }"#,
    );
    assert_eq!(ws.name(), "dev");
    match ws {
        PreferredWorkspace::PartitionTree { tree, .. } => {
            assert!(matches!(tree, Some(TreeLayoutNode::Leaf(..))));
        }
        _ => panic!("expected PartitionTree variant"),
    }
}

#[test]
fn tree_array_container_parses() {
    let ws = workspace_from(
        r#"{ "name": "dev", "strategy": "partition_tree", "tree": [
  { "process": "editor.exe" },
  { "process": "terminal.exe" }
] }"#,
    );
    match ws {
        PreferredWorkspace::PartitionTree { tree, .. } => {
            let Some(TreeLayoutNode::Container { split, children }) = tree else {
                panic!("expected Container");
            };
            assert!(split.is_none());
            assert_eq!(children.len(), 2);
            assert!(matches!(children[0], TreeLayoutNode::Leaf(..)));
            assert!(matches!(children[1], TreeLayoutNode::Leaf(..)));
        }
        _ => panic!("expected PartitionTree variant"),
    }
}

#[test]
fn tree_split_container_parses() {
    let ws = workspace_from(
        r#"{ "name": "dev", "strategy": "partition_tree", "tree": {
  "split": "horizontal",
  "children": [ { "process": "a.exe" }, { "process": "b.exe" } ]
} }"#,
    );
    match ws {
        PreferredWorkspace::PartitionTree { tree, .. } => {
            let Some(TreeLayoutNode::Container { split, children }) = tree else {
                panic!("expected Container");
            };
            assert_eq!(split, Some(SplitMode::Horizontal));
            assert_eq!(children.len(), 2);
        }
        _ => panic!("expected PartitionTree variant"),
    }
}

#[test]
fn tree_tabbed_parses() {
    let ws = workspace_from(
        r#"{ "name": "dev", "strategy": "partition_tree", "tree": {
  "split": "tabbed",
  "children": [ { "process": "browser.exe" }, { "process": "editor.exe" } ]
} }"#,
    );
    match ws {
        PreferredWorkspace::PartitionTree { tree, .. } => {
            let Some(TreeLayoutNode::Container { split, children }) = tree else {
                panic!("expected Container");
            };
            assert_eq!(split, Some(SplitMode::Tabbed));
            assert_eq!(children.len(), 2);
        }
        _ => panic!("expected PartitionTree variant"),
    }
}

#[test]
fn tree_nested_parses() {
    let ws = workspace_from(
        r#"{ "name": "dev", "strategy": "partition_tree", "tree": {
  "split": "horizontal",
  "children": [
    { "process": "editor.exe" },
    { "split": "vertical", "children": [
      { "process": "terminal.exe" },
      { "process": "logs.exe" }
    ] }
  ]
} }"#,
    );
    match ws {
        PreferredWorkspace::PartitionTree { tree, .. } => {
            let Some(TreeLayoutNode::Container { split, children }) = tree else {
                panic!("expected outer Container");
            };
            assert_eq!(split, Some(SplitMode::Horizontal));
            assert_eq!(children.len(), 2);
            assert!(matches!(children[0], TreeLayoutNode::Leaf(..)));
            assert!(matches!(
                children[1],
                TreeLayoutNode::Container {
                    split: Some(SplitMode::Vertical),
                    ..
                }
            ));
        }
        _ => panic!("expected PartitionTree variant"),
    }
}

#[test]
fn tree_default_none() {
    let ws = workspace_from(r#"{ "name": "dev", "strategy": "partition_tree" }"#);
    match ws {
        PreferredWorkspace::PartitionTree { tree, .. } => {
            assert!(tree.is_none());
        }
        _ => panic!("expected PartitionTree variant"),
    }
}

#[test]
fn tree_invalid_split_rejected() {
    assert!(
        try_workspace(
            r#"{ "name": "dev", "strategy": "partition_tree", "tree": { "split": "diagonal", "children": [] } }"#,
        )
        .is_err()
    );
}
