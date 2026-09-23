use crate::config::PreferredLayouts;
use crate::core::{
    ColumnConfig, PaneDisplay, Pixels, PreferredWorkspace, SizeConstraint, SplitMode,
    TreeLayoutNode, WindowMatcher,
};

fn layout_from(src: &str) -> PreferredLayouts {
    PreferredLayouts::from_lua("test layout", src).expect("layout should load")
}

/// Wraps a bare workspace table in the surrounding monitor keys, so the read
/// still goes through the loader rather than a shortcut into the type.
fn workspace_from(src: &str) -> PreferredWorkspace {
    let wrapped = format!("return {{ desk = {{ w = {src} }} }}");
    layout_from(&wrapped)
        .workspace("desk", "w")
        .expect("workspace entry")
        .clone()
}

#[test]
fn layout_loads_from_lua_source() {
    let src = r#"
---@type dome.Layout
return {
  ["PC Monitor"] = {
    ["dev"] = {
      layout = "partition_tree",
      tree = {
        { app = "Ghostty" },
        { split = "vertical", children = {
          { app = "Firefox" },
          { app = "Slack" },
        } },
      },
      float = { { app = "System Settings" } },
    },
    ["work"] = {
      layout = "master",
      master_ratio = 0.6,
      master = { { app = "Ghostty" } },
      secondary = { display = "tabbed", children = { { app = "Firefox" } } },
    },
  },
}
"#;
    let layout = PreferredLayouts::from_lua("test layout", src).expect("lua layout should load");
    assert_eq!(layout.entries().count(), 2);
    match layout.workspace("PC Monitor", "dev").expect("dev entry") {
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
    match layout.workspace("PC Monitor", "work").expect("work entry") {
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
fn fullscreen_rules_load_on_either_strategy() {
    let steam = vec![WindowMatcher {
        app: Some("Steam".to_string()),
        ..Default::default()
    }];

    let tree =
        workspace_from(r#"{ layout = "partition_tree", fullscreen = { { app = "Steam" } } }"#);
    match tree {
        PreferredWorkspace::PartitionTree { fullscreen, .. } => assert_eq!(fullscreen, steam),
        _ => panic!("expected PartitionTree variant"),
    }

    let master = workspace_from(r#"{ layout = "master", fullscreen = { { app = "Steam" } } }"#);
    match master {
        PreferredWorkspace::Master { fullscreen, .. } => assert_eq!(fullscreen, steam),
        _ => panic!("expected Master variant"),
    }
}

#[test]
fn out_of_range_master_override_is_dropped() {
    // A dropped override leaves the workspace following config.lua. Pinning it
    // to the type default would override with a value this file never named.
    let src = r#"
---@type dome.Layout
return {
  ["PC Monitor"] = {
    ["bad"] = { layout = "master", master_ratio = 1.5, master_count = 0 },
    ["good"] = { layout = "master", master_ratio = 0.6, master_count = 2 },
  },
}
"#;
    let layout = PreferredLayouts::from_lua("test layout", src).expect("lua layout should load");
    match layout.workspace("PC Monitor", "bad").expect("bad entry") {
        PreferredWorkspace::Master {
            master_ratio,
            master_count,
            ..
        } => {
            assert_eq!(*master_ratio, None);
            assert_eq!(*master_count, None);
        }
        _ => panic!("expected Master"),
    }
    match layout.workspace("PC Monitor", "good").expect("good entry") {
        PreferredWorkspace::Master {
            master_ratio,
            master_count,
            ..
        } => {
            assert_eq!(*master_ratio, Some(0.6));
            assert_eq!(*master_count, Some(2));
        }
        _ => panic!("expected Master"),
    }
}

#[test]
fn layout_shared_table_applies_to_both_monitor_keys() {
    // One table assigned to several keys is how one file serves several
    // workstations, so both keys must resolve to it.
    let src = r#"
---@type dome.MonitorLayout
local shared = {
  ["1"] = { layout = "master" },
}

---@type dome.Layout
return {
  ["Built-in Retina Display"] = shared,
  ["PC Monitor"] = shared,
}
"#;
    let layout = PreferredLayouts::from_lua("test layout", src).expect("lua layout should load");
    assert_eq!(layout.entries().count(), 2);
    assert!(matches!(
        layout.workspace("Built-in Retina Display", "1"),
        Some(PreferredWorkspace::Master { .. })
    ));
    assert!(matches!(
        layout.workspace("PC Monitor", "1"),
        Some(PreferredWorkspace::Master { .. })
    ));
}

#[test]
fn layout_repeated_key_keeps_the_last_entry() {
    // Lua itself resolves the repeat, so no dedup pass has to.
    let src = r#"
---@type dome.Layout
return {
  ["PC Monitor"] = {
    ["1"] = { layout = "partition_tree" },
    ["1"] = { layout = "master" },
  },
}
"#;
    let layout = PreferredLayouts::from_lua("test layout", src).expect("lua layout should load");
    assert_eq!(layout.entries().count(), 1);
    assert!(matches!(
        layout.workspace("PC Monitor", "1"),
        Some(PreferredWorkspace::Master { .. })
    ));
}

#[test]
fn preferred_layout_default_empty() {
    assert_eq!(layout_from("return {}"), PreferredLayouts::default());
}

#[test]
fn preferred_layout_parse_single_entry() {
    let layout = layout_from(r#"return { desk = { ["1"] = { layout = "master" } } }"#);
    assert_eq!(layout.entries().count(), 1);
    assert!(matches!(
        layout.workspace("desk", "1"),
        Some(PreferredWorkspace::Master { .. })
    ));
}

#[test]
fn preferred_layout_parses_scrolling_workspace() {
    let layout = layout_from(
        r#"return { desk = { ["1"] = { layout = "scrolling", float = { { app = "mpv" } } } } }"#,
    );
    match layout.workspace("desk", "1") {
        Some(PreferredWorkspace::Scrolling { float, .. }) => assert_eq!(float.len(), 1),
        other => panic!("expected a scrolling workspace, got {other:?}"),
    }
}

fn scrolling_columns(src: &str) -> Vec<ColumnConfig> {
    match workspace_from(src) {
        PreferredWorkspace::Scrolling { columns, .. } => columns,
        other => panic!("expected a scrolling workspace, got {other:?}"),
    }
}

fn process(name: &str) -> WindowMatcher {
    WindowMatcher {
        process: Some(name.into()),
        ..Default::default()
    }
}

#[test]
fn scrolling_columns_parse_bare_and_keyed_entries() {
    let columns = scrolling_columns(
        r#"{ layout = "scrolling", columns = {
  { process = "editor.exe" },
  { width = "40%", children = { { process = "terminal.exe" } } },
  { width = 800, children = { { process = "logs.exe" } } },
} }"#,
    );
    assert_eq!(
        columns,
        vec![
            ColumnConfig::bare(process("editor.exe")),
            ColumnConfig {
                width: Some(SizeConstraint::Percent(40.0)),
                children: vec![process("terminal.exe")],
            },
            ColumnConfig {
                width: Some(SizeConstraint::Pixels(Pixels::new(800))),
                children: vec![process("logs.exe")],
            },
        ]
    );
}

#[test]
fn scrolling_column_keeps_only_its_first_child() {
    let columns = scrolling_columns(
        r#"{ layout = "scrolling", columns = {
  { width = "40%", children = { { process = "a.exe" }, { process = "b.exe" } } },
} }"#,
    );
    assert_eq!(
        columns,
        vec![ColumnConfig {
            width: Some(SizeConstraint::Percent(40.0)),
            children: vec![process("a.exe")],
        }]
    );
}

#[test]
fn scrolling_columns_written_as_one_matcher_yield_no_column() {
    let columns =
        scrolling_columns(r#"{ layout = "scrolling", columns = { process = "editor.exe" } }"#);
    assert!(columns.is_empty());
}

#[test]
fn scrolling_columns_default_to_empty() {
    let columns = scrolling_columns(r#"{ layout = "scrolling" }"#);
    assert!(columns.is_empty());
}

#[test]
fn scrolling_column_with_width_but_no_children_is_dropped() {
    let columns = scrolling_columns(
        r#"{ layout = "scrolling", columns = {
  { width = "40%", process = "a.exe" },
  { process = "b.exe" },
} }"#,
    );
    assert_eq!(columns, vec![ColumnConfig::bare(process("b.exe"))]);
}

#[test]
fn scrolling_column_recovers_from_an_invalid_width() {
    let columns = scrolling_columns(
        r#"{ layout = "scrolling", columns = {
  { width = "wide", children = { { process = "a.exe" } } },
} }"#,
    );
    assert_eq!(columns, vec![ColumnConfig::bare(process("a.exe"))]);
}

#[test]
fn preferred_layout_parse_multiple_distinct() {
    let layout = layout_from(
        r#"return { desk = {
  ["1"] = { layout = "master" },
  scratch = { layout = "partition_tree" },
} }"#,
    );
    assert_eq!(layout.entries().count(), 2);
    assert!(matches!(
        layout.workspace("desk", "1"),
        Some(PreferredWorkspace::Master { .. })
    ));
    assert!(matches!(
        layout.workspace("desk", "scratch"),
        Some(PreferredWorkspace::PartitionTree { .. })
    ));
}

#[test]
fn preferred_layout_drops_a_workspace_with_an_unknown_strategy() {
    let layout = layout_from(
        r#"return { desk = {
  bad = { layout = "floating" },
  good = { layout = "master" },
} }"#,
    );
    assert!(layout.workspace("desk", "bad").is_none());
    assert!(matches!(
        layout.workspace("desk", "good"),
        Some(PreferredWorkspace::Master { .. })
    ));
}

#[test]
fn tree_leaf_parses() {
    let ws = workspace_from(r#"{ layout = "partition_tree", tree = { process = "editor.exe" } }"#);
    match ws {
        PreferredWorkspace::PartitionTree { tree, .. } => {
            let Some(TreeLayoutNode::Leaf(matcher)) = tree else {
                panic!("expected Leaf");
            };
            assert_eq!(
                matcher,
                WindowMatcher {
                    process: Some("editor.exe".to_string()),
                    ..Default::default()
                }
            );
        }
        _ => panic!("expected PartitionTree variant"),
    }
}

#[test]
fn tree_array_container_parses() {
    let ws = workspace_from(
        r#"{ layout = "partition_tree", tree = {
  { process = "editor.exe" },
  { process = "terminal.exe" },
} }"#,
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
        r#"{ layout = "partition_tree", tree = {
  split = "horizontal",
  children = { { process = "a.exe" }, { process = "b.exe" } },
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
        r#"{ layout = "partition_tree", tree = {
  split = "tabbed",
  children = { { process = "browser.exe" }, { process = "editor.exe" } },
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
        r#"{ layout = "partition_tree", tree = {
  split = "horizontal",
  children = {
    { process = "editor.exe" },
    { split = "vertical", children = {
      { process = "terminal.exe" },
      { process = "logs.exe" },
    } },
  },
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
    let ws = workspace_from(r#"{ layout = "partition_tree" }"#);
    match ws {
        PreferredWorkspace::PartitionTree { tree, .. } => {
            assert!(tree.is_none());
        }
        _ => panic!("expected PartitionTree variant"),
    }
}

#[test]
fn tree_recovers_from_an_invalid_split() {
    let ws = workspace_from(
        r#"{ layout = "partition_tree", tree = { split = "diagonal", children = {
  { process = "editor.exe" },
  { process = "terminal.exe" },
} } }"#,
    );
    match ws {
        PreferredWorkspace::PartitionTree { tree, .. } => {
            let Some(TreeLayoutNode::Container { split, children }) = tree else {
                panic!("expected Container");
            };
            assert!(split.is_none());
            assert_eq!(children.len(), 2);
        }
        _ => panic!("expected PartitionTree variant"),
    }
}
