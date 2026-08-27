use crate::config::{
    LayoutConfig, LayoutWorkspaceConfig, PaneConfig, SplitMode, TreeLayoutNode, WindowMatcher,
};
use crate::core::node::WindowRestrictions;
use crate::core::strategy::WorkspaceExport;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, default_rect, process_meta,
    titled, titled_process,
};

struct CleanupFile(std::path::PathBuf);
impl Drop for CleanupFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

#[test]
fn export_reemits_config_matchers_across_float_and_fullscreen() {
    let float_matcher = WindowMatcher {
        process: Some("/float.*/".into()),
        ..Default::default()
    };
    let fullscreen_matcher = WindowMatcher {
        process: Some("/fs.*/".into()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                // Global, so the hit carries `matcher_id: None` and export
                // synthesises this window's matcher from live metadata.
                .with_float(vec![WindowMatcher {
                    process: Some("orphan.exe".into()),
                    ..Default::default()
                }])
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_float(vec![float_matcher.clone()])
                .with_fullscreen(vec![fullscreen_matcher.clone()])
                .build(),
        ])
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();

    hub.insert_window(
        process_meta("float-window-alpha"),
        default_rect(),
        WindowRestrictions::None,
    );
    hub.insert_window(
        process_meta("float-window-beta"),
        default_rect(),
        WindowRestrictions::None,
    );
    hub.insert_window(
        process_meta("orphan.exe"),
        default_rect(),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.insert_window(
        process_meta("fs-window-alpha"),
        default_rect(),
        WindowRestrictions::None,
    );

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            float: vec![
                float_matcher,
                WindowMatcher {
                    process: Some("orphan.exe".into()),
                    ..Default::default()
                }
            ],
            fullscreen: vec![fullscreen_matcher],
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn export_synthesises_from_live_across_float_and_fullscreen() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();

    let float_wid = hub
        .insert_window(
            titled("float-live-alpha"),
            default_rect(),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.set_focus(float_wid);
    hub.toggle_float();

    let fullscreen_wid = hub
        .insert_window(
            titled("fs-live-beta"),
            default_rect(),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.set_focus(fullscreen_wid);
    hub.set_fullscreen(fullscreen_wid, WindowRestrictions::None);

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            float: vec![WindowMatcher {
                title: Some("float-live-alpha".into()),
                ..Default::default()
            }],
            fullscreen: vec![WindowMatcher {
                title: Some("fs-live-beta".into()),
                ..Default::default()
            }],
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn export_synthesises_from_live_for_global_matched_float() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_float(vec![WindowMatcher {
                    process: Some("/float.*/".into()),
                    ..Default::default()
                }])
                .build(),
        )
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();

    hub.insert_window(
        process_meta("float-window-alpha"),
        default_rect(),
        WindowRestrictions::None,
    );

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            float: vec![WindowMatcher {
                process: Some("float-window-alpha".into()),
                ..Default::default()
            }],
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn export_drops_matcher_on_cross_workspace_move() {
    let m = WindowMatcher {
        process: Some("float.exe".into()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_float(vec![m.clone()])
                .build(),
        ])
        .build();
    hub.focus_workspace("1", None);

    let wid = hub
        .insert_window(
            titled_process("distinct-alpha", "float.exe"),
            default_rect(),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.set_focus(wid);
    hub.move_focused_to_workspace("2", None);

    hub.focus_workspace("2", None);
    let ws2_id = hub.current_workspace();
    let result = hub.export_workspace(ws2_id);
    assert_eq!(
        result,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            float: vec![WindowMatcher {
                title: Some("distinct-alpha".into()),
                process: Some("float.exe".into()),
                ..Default::default()
            }],
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn export_drops_matcher_on_unminimize() {
    let m = WindowMatcher {
        process: Some("float.exe".into()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_float(vec![m.clone()])
                .build(),
        ])
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();

    let wid = hub
        .insert_window(
            titled_process("distinct-alpha", "float.exe"),
            default_rect(),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.minimize_window(wid);
    hub.unminimize_window(wid);

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            float: vec![WindowMatcher {
                title: Some("distinct-alpha".into()),
                process: Some("float.exe".into()),
                ..Default::default()
            }],
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn export_returns_empty_export_for_empty_workspace() {
    let m = WindowMatcher {
        process: Some("float.exe".into()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_float(vec![m])
                .build(),
        ])
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();

    let wid = hub
        .insert_window(
            process_meta("float.exe"),
            default_rect(),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.delete_window(wid);

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn export_layout_writes_entry_for_empty_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1", None);
    hub.focus_workspace("2", None);
    hub.insert_window(
        titled("tiled-alpha"),
        default_rect(),
        WindowRestrictions::None,
    );

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dome_export_empty_entry_{nanos}.jsonc"));
    let _cleanup = CleanupFile(path.clone());

    hub.export_layout(&path).unwrap();

    let parsed = LayoutConfig::load(path.to_str().unwrap())
        .expect("exported layout.jsonc parses through the JSONC loader");

    let empty = parsed
        .workspace
        .iter()
        .find(|w| w.name() == "1")
        .expect("workspace 1 present");
    match empty {
        LayoutWorkspaceConfig::PartitionTree {
            tree,
            float,
            fullscreen,
            ..
        } => {
            assert!(tree.is_none());
            assert!(float.is_empty());
            assert!(fullscreen.is_empty());
        }
        _ => panic!("workspace 1 should be partition_tree"),
    }

    let filled = parsed
        .workspace
        .iter()
        .find(|w| w.name() == "2")
        .expect("workspace 2 present");
    match filled {
        LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
            assert!(tree.is_some());
        }
        _ => panic!("workspace 2 should be partition_tree"),
    }
}

#[test]
fn export_float_toggled_to_tiling_returns_to_tree() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_float(vec![WindowMatcher {
                    process: Some("float.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();

    let wid = hub
        .insert_window(
            process_meta("float.exe"),
            default_rect(),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.set_focus(wid);
    hub.toggle_float();

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Leaf(WindowMatcher {
                process: Some("float.exe".into()),
                ..Default::default()
            })),
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn render_layout_round_trips_master_and_nested_tree() {
    // A quote and a backslash exercise JSONC string escaping.
    let quoted = WindowMatcher {
        title: Some("a\"b\\c".into()),
        ..Default::default()
    };
    let master_ws = WorkspaceExport {
        strategy: "master".into(),
        master_ratio: Some(0.5),
        master_count: Some(2),
        master: PaneConfig::tiled(vec![WindowMatcher {
            app: Some("Editor".into()),
            title: Some("main".into()),
            ..Default::default()
        }]),
        secondary: PaneConfig::tiled(vec![WindowMatcher {
            process: Some("term".into()),
            ..Default::default()
        }]),
        float: vec![quoted.clone()],
        ..WorkspaceExport::default()
    };
    let tree = TreeLayoutNode::Container {
        split: Some(SplitMode::Horizontal),
        children: vec![
            TreeLayoutNode::Leaf(WindowMatcher {
                process: Some("editor".into()),
                ..Default::default()
            }),
            TreeLayoutNode::Container {
                split: None,
                children: vec![
                    TreeLayoutNode::Leaf(WindowMatcher {
                        process: Some("terminal".into()),
                        ..Default::default()
                    }),
                    TreeLayoutNode::Leaf(WindowMatcher {
                        process: Some("logs".into()),
                        ..Default::default()
                    }),
                ],
            },
        ],
    };
    let tree_ws = WorkspaceExport {
        strategy: "partition_tree".into(),
        tree: Some(tree.clone()),
        ..WorkspaceExport::default()
    };

    let rendered =
        crate::core::export::render_layout("", &[("m".into(), master_ws), ("t".into(), tree_ws)])
            .unwrap();

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dome_export_roundtrip_{nanos}.jsonc"));
    let _cleanup = CleanupFile(path.clone());
    std::fs::write(&path, &rendered).unwrap();

    let parsed = LayoutConfig::load(path.to_str().unwrap())
        .expect("rendered layout.jsonc parses through the JSONC loader");

    let m = parsed
        .workspace
        .iter()
        .find(|w| w.name() == "m")
        .expect("workspace m present");
    match m {
        LayoutWorkspaceConfig::Master {
            master_ratio,
            master_count,
            master,
            secondary,
            float,
            fullscreen,
            ..
        } => {
            assert_eq!(*master_ratio, Some(0.5));
            assert_eq!(*master_count, Some(2));
            assert_eq!(
                master,
                &PaneConfig::tiled(vec![WindowMatcher {
                    app: Some("Editor".into()),
                    title: Some("main".into()),
                    ..Default::default()
                }])
            );
            assert_eq!(
                secondary,
                &PaneConfig::tiled(vec![WindowMatcher {
                    process: Some("term".into()),
                    ..Default::default()
                }])
            );
            assert_eq!(float, &vec![quoted]);
            assert!(fullscreen.is_empty());
        }
        _ => panic!("workspace m should be master"),
    }

    let t = parsed
        .workspace
        .iter()
        .find(|w| w.name() == "t")
        .expect("workspace t present");
    match t {
        LayoutWorkspaceConfig::PartitionTree {
            tree: parsed_tree, ..
        } => {
            assert_eq!(parsed_tree.as_ref(), Some(&tree));
        }
        _ => panic!("workspace t should be partition_tree"),
    }
}

#[test]
fn render_layout_preserves_comments_and_reconciles_in_place() {
    let existing = r#"{
  // Dome layout. Hand-written comments survive export.
  "workspace": [
    { "name": "1", "strategy": "master", "master_count": 1 }
  ]
}
"#;
    let updated = WorkspaceExport {
        strategy: "master".into(),
        master_count: Some(2),
        ..WorkspaceExport::default()
    };
    let appended = WorkspaceExport {
        strategy: "partition_tree".into(),
        ..WorkspaceExport::default()
    };
    let rendered = crate::core::export::render_layout(
        existing,
        &[("1".into(), updated), ("2".into(), appended)],
    )
    .unwrap();

    // The file header comment survives the rewrite.
    assert!(rendered.contains("Dome layout. Hand-written comments survive export."));

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dome_export_comments_{nanos}.jsonc"));
    let _cleanup = CleanupFile(path.clone());
    std::fs::write(&path, &rendered).unwrap();
    let parsed = LayoutConfig::load(path.to_str().unwrap())
        .expect("rendered layout.jsonc parses through the JSONC loader");

    // Workspace 1 was updated in place, workspace 2 appended.
    let ws1 = parsed
        .workspace
        .iter()
        .find(|w| w.name() == "1")
        .expect("workspace 1 present");
    match ws1 {
        LayoutWorkspaceConfig::Master { master_count, .. } => assert_eq!(*master_count, Some(2)),
        _ => panic!("workspace 1 should be master"),
    }
    assert!(parsed.workspace.iter().any(|w| w.name() == "2"));
}
