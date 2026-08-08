use crate::config::{TreeLayoutNode, WindowMatcher};
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
    let path = std::env::temp_dir().join(format!("dome_export_empty_entry_{nanos}.toml"));
    let _cleanup = CleanupFile(path.clone());

    hub.export_layout(&path).unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    let doc: toml::Value = toml::from_str(&written).unwrap();
    let entries = doc["workspace"].as_array().unwrap();

    let empty = entries
        .iter()
        .find(|s| s["name"].as_str() == Some("1"))
        .unwrap();
    assert_eq!(empty["strategy"].as_str(), Some("partition_tree"));
    assert!(empty.get("tree").is_none());
    assert!(empty.get("float").is_none());
    assert!(empty.get("fullscreen").is_none());

    let filled = entries
        .iter()
        .find(|s| s["name"].as_str() == Some("2"))
        .unwrap();
    assert_eq!(filled["strategy"].as_str(), Some("partition_tree"));
    assert!(filled.get("tree").is_some());
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
