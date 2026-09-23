use crate::core::master::PaneConfig;
use crate::core::node::WindowRestrictions;
use crate::core::tests::{
    LayoutWorkspaceConfigBuilder, PRIMARY_MONITOR, TestHubBuilder, TilingConfigBuilder,
    default_rect, master_entry, parse_exported_layout, partition_tree_entry, process_meta,
    reported_monitor, titled, work_area_at,
};
use crate::core::{
    MonitorSelector, PaneDisplay, PreferredLayouts, PreferredMaster, PreferredTiling,
    PreferredWorkspace, SplitMode, Strategy, TreeLayoutNode, WindowMatcher,
};

struct CleanupFile(std::path::PathBuf);
impl Drop for CleanupFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

#[test]
fn export_synthesises_from_live_even_for_rule_placed_windows() {
    let float_matcher = WindowMatcher {
        process: Some("/float.*/".into()),
        ..Default::default()
    };
    let fullscreen_matcher = WindowMatcher {
        process: Some("/fs.*/".into()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_float(vec![WindowMatcher {
                    process: Some("orphan.exe".into()),
                    ..Default::default()
                }])
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_float(vec![float_matcher.clone(), float_matcher.clone()])
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
        PreferredWorkspace {
            float: vec![
                WindowMatcher {
                    process: Some("float-window-alpha".into()),
                    ..Default::default()
                },
                WindowMatcher {
                    process: Some("float-window-beta".into()),
                    ..Default::default()
                },
                WindowMatcher {
                    process: Some("orphan.exe".into()),
                    ..Default::default()
                },
            ],
            fullscreen: vec![WindowMatcher {
                process: Some("fs-window-alpha".into()),
                ..Default::default()
            }],
            ..partition_tree_entry(None)
        }
    );
}

#[test]
fn export_synthesises_from_live_across_float_and_fullscreen() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
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
        PreferredWorkspace {
            float: vec![WindowMatcher {
                title: Some("float-live-alpha".into()),
                ..Default::default()
            }],
            fullscreen: vec![WindowMatcher {
                title: Some("fs-live-beta".into()),
                ..Default::default()
            }],
            ..partition_tree_entry(None)
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
        .with_tiling(TilingConfigBuilder::new().build())
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
    assert_eq!(result, partition_tree_entry(None));
}

#[test]
fn save_layout_writes_workspace_keys_in_name_order() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    // Focus order is the allocation order, so a walk over workspace ids would
    // write these keys reversed.
    hub.focus_workspace("c", None);
    hub.focus_workspace("b", None);
    hub.focus_workspace("a", None);

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dome_export_key_order_{nanos}.lua"));
    let _cleanup = CleanupFile(path.clone());

    hub.save_layout(&path).unwrap();

    let rendered = std::fs::read_to_string(&path).unwrap();
    let keys: Vec<&str> = rendered
        .lines()
        .filter_map(|line| line.trim().strip_prefix("["))
        .filter_map(|line| line.split('"').nth(1))
        .collect();
    assert_eq!(keys, vec![PRIMARY_MONITOR, "0", "a", "b", "c"]);
}

#[test]
fn save_layout_writes_entry_for_empty_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
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
    let path = std::env::temp_dir().join(format!("dome_export_empty_entry_{nanos}.lua"));
    let _cleanup = CleanupFile(path.clone());

    hub.save_layout(&path).unwrap();

    let parsed = parse_exported_layout(path.to_str().unwrap());

    let empty = parsed
        .workspace(PRIMARY_MONITOR, "1")
        .expect("workspace 1 present");
    match empty {
        PreferredWorkspace {
            tiling: PreferredTiling::PartitionTree { tree },
            float,
            fullscreen,
        } => {
            assert!(tree.is_none());
            assert!(float.is_empty());
            assert!(fullscreen.is_empty());
        }
        _ => panic!("workspace 1 should be partition_tree"),
    }

    let filled = parsed
        .workspace(PRIMARY_MONITOR, "2")
        .expect("workspace 2 present");
    match filled {
        PreferredWorkspace {
            tiling: PreferredTiling::PartitionTree { tree },
            ..
        } => {
            assert!(tree.is_some());
        }
        _ => panic!("workspace 2 should be partition_tree"),
    }
}

#[test]
fn save_layout_backs_up_the_previous_file() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1", None);
    hub.insert_window(titled("alpha"), default_rect(), WindowRestrictions::None);

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dome_export_backup_{nanos}.lua"));
    let backup = path.with_extension("lua.bak");
    let _cleanup_path = CleanupFile(path.clone());
    let _cleanup_backup = CleanupFile(backup.clone());

    let prior = "-- prior hand-written file\nreturn { desk = {} }\n";
    std::fs::write(&path, prior).unwrap();

    hub.save_layout(&path).unwrap();

    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        prior,
        "the prior file is preserved in the .bak"
    );
    let rewritten = std::fs::read_to_string(&path).unwrap();
    assert!(rewritten.starts_with("---@type dome.Layout\n"));
    assert_ne!(rewritten, prior);
}

#[test]
fn export_float_toggled_to_tiling_returns_to_tree() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
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
        partition_tree_entry(Some(TreeLayoutNode::Leaf(WindowMatcher {
            process: Some("float.exe".into()),
            ..Default::default()
        })))
    );
}

#[test]
fn render_layout_round_trips_master_and_nested_tree() {
    let quoted = WindowMatcher {
        title: Some("a\"b\\c".into()),
        ..Default::default()
    };
    let master_ws = PreferredWorkspace {
        float: vec![quoted.clone()],
        ..master_entry(PreferredMaster {
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
        })
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
    let tree_ws = partition_tree_entry(Some(tree.clone()));

    let tabbed_master = PaneConfig {
        display: PaneDisplay::Tabbed,
        children: vec![
            WindowMatcher {
                app: Some("Slack".into()),
                ..Default::default()
            },
            WindowMatcher {
                app: Some("Discord".into()),
                ..Default::default()
            },
        ],
    };
    let tabbed_secondary = PaneConfig {
        display: PaneDisplay::Tabbed,
        children: vec![WindowMatcher {
            process: Some("notes.exe".into()),
            ..Default::default()
        }],
    };
    let tabbed_ws = master_entry(PreferredMaster {
        master: tabbed_master.clone(),
        secondary: tabbed_secondary.clone(),
        ..PreferredMaster::default()
    });

    let mut layouts = PreferredLayouts::default();
    layouts.insert(PRIMARY_MONITOR, "m", master_ws);
    layouts.insert(PRIMARY_MONITOR, "t", tree_ws);
    layouts.insert(PRIMARY_MONITOR, "tab", tabbed_ws);
    let rendered = crate::core::export::render_layout(&layouts);

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dome_export_roundtrip_{nanos}.lua"));
    let _cleanup = CleanupFile(path.clone());
    std::fs::write(&path, &rendered).unwrap();

    let parsed = parse_exported_layout(path.to_str().unwrap());

    let m = parsed
        .workspace(PRIMARY_MONITOR, "m")
        .expect("workspace m present");
    match m {
        PreferredWorkspace {
            tiling:
                PreferredTiling::Master(PreferredMaster {
                    master_ratio,
                    master_count,
                    master,
                    secondary,
                }),
            float,
            fullscreen,
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
        .workspace(PRIMARY_MONITOR, "t")
        .expect("workspace t present");
    match t {
        PreferredWorkspace {
            tiling: PreferredTiling::PartitionTree { tree: parsed_tree },
            ..
        } => {
            assert_eq!(parsed_tree.as_ref(), Some(&tree));
        }
        _ => panic!("workspace t should be partition_tree"),
    }

    let tab = parsed
        .workspace(PRIMARY_MONITOR, "tab")
        .expect("workspace tab present");
    match tab {
        PreferredWorkspace {
            tiling:
                PreferredTiling::Master(PreferredMaster {
                    master, secondary, ..
                }),
            ..
        } => {
            assert_eq!(master, &tabbed_master);
            assert_eq!(secondary, &tabbed_secondary);
        }
        _ => panic!("workspace tab should be master"),
    }
}

/// Lua reads up to three digits after a decimal escape, so an unpadded escape
/// merges with a digit that follows it in the title. `\1` before `2` would read
/// back as `\12`, and `\31` before `5` would read back as `\315`, which
/// exceeds 255 and makes the file fail to parse at all.
#[test]
fn render_layout_round_trips_a_control_character_before_a_digit() {
    let hazard = WindowMatcher {
        title: Some("a\u{1}2b\u{1f}5c".into()),
        ..Default::default()
    };
    let ws = PreferredWorkspace {
        float: vec![hazard.clone()],
        ..master_entry(PreferredMaster::default())
    };

    let mut layouts = PreferredLayouts::default();
    layouts.insert(PRIMARY_MONITOR, "m", ws);
    let rendered = crate::core::export::render_layout(&layouts);

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dome_export_control_{nanos}.lua"));
    let _cleanup = CleanupFile(path.clone());
    std::fs::write(&path, &rendered).unwrap();

    let parsed = parse_exported_layout(path.to_str().unwrap());
    let m = parsed
        .workspace(PRIMARY_MONITOR, "m")
        .expect("workspace m present");
    match m {
        PreferredWorkspace {
            tiling: PreferredTiling::Master(_),
            float,
            ..
        } => assert_eq!(float, &vec![hazard]),
        _ => panic!("workspace m should be master"),
    }
}

#[test]
fn render_layout_roundtrips_master_workspace() {
    let mut layouts = PreferredLayouts::default();
    layouts.insert(
        PRIMARY_MONITOR,
        "1",
        master_entry(PreferredMaster {
            master_count: Some(2),
            ..PreferredMaster::default()
        }),
    );
    let rendered = crate::core::export::render_layout(&layouts);

    assert!(rendered.starts_with("---@type dome.Layout\n"));

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dome_export_regen_{nanos}.lua"));
    let _cleanup = CleanupFile(path.clone());
    std::fs::write(&path, &rendered).unwrap();
    let parsed = parse_exported_layout(path.to_str().unwrap());

    let ws1 = parsed
        .workspace(PRIMARY_MONITOR, "1")
        .expect("workspace 1 present");
    match ws1 {
        PreferredWorkspace {
            tiling: PreferredTiling::Master(PreferredMaster { master_count, .. }),
            ..
        } => assert_eq!(*master_count, Some(2)),
        _ => panic!("workspace 1 should be master"),
    }
}

#[test]
fn render_layout_writes_the_same_text() {
    let src = r#"
return {
  ["desk"] = {
    ["dev"] = {
      layout = "partition_tree",
      tree = {
        split = "horizontal",
        children = {
          { app = "Ghostty" },
          {
            { app = "Firefox", title = "docs" },
            { process = "slack.exe" },
          },
        },
      },
      float = { { app = "System Settings" } },
      fullscreen = { { app = "Steam" } },
    },
    ["empty"] = { layout = "partition_tree" },
  },
  ["laptop"] = {
    ["work"] = {
      layout = "master",
      master_ratio = 0.6,
      master_count = 2,
      master = { { app = "Editor" } },
      secondary = { display = "tabbed", children = { { app = "Mail" }, { class = "Notes" } } },
      float = { { title = "Picture in Picture" } },
    },
  },
}
"#;
    let layouts = PreferredLayouts::from_lua("layout.lua", src).expect("layout should load");

    insta::assert_snapshot!(crate::core::export::render_layout(&layouts), @r#"
    ---@type dome.Layout
    return {
      ["desk"] = {
        ["dev"] = {
          layout = "partition_tree",
          tree = { split = "horizontal", children = {
            { app = "Ghostty" },
            {
              { app = "Firefox", title = "docs" },
              { process = "slack.exe" },
            },
          } },
          float = {
            { app = "System Settings" },
          },
          fullscreen = {
            { app = "Steam" },
          },
        },
        ["empty"] = {
          layout = "partition_tree",
        },
      },
      ["laptop"] = {
        ["work"] = {
          layout = "master",
          master_ratio = 0.6,
          master_count = 2,
          master = {
            { app = "Editor" },
          },
          secondary = { display = "tabbed", children = {
            { app = "Mail" },
            { class = "Notes" },
          } },
          float = {
            { title = "Picture in Picture" },
          },
        },
      },
    }
    "#);
}

#[test]
fn save_layout_keeps_the_entries_of_a_monitor_that_is_not_connected() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout_on(
            "desktop",
            vec![
                LayoutWorkspaceConfigBuilder::new("9")
                    .with_strategy(Strategy::Master)
                    .build(),
            ],
        )
        .build();
    hub.focus_workspace("1", None);
    hub.insert_window(titled("alpha"), default_rect(), WindowRestrictions::None);

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dome_export_absent_monitor_{nanos}.lua"));
    let _cleanup = CleanupFile(path.clone());

    hub.save_layout(&path).unwrap();
    let parsed = parse_exported_layout(path.to_str().unwrap());

    assert!(
        matches!(
            parsed.workspace("desktop", "9"),
            Some(PreferredWorkspace {
                tiling: PreferredTiling::Master(_),
                ..
            })
        ),
        "an export on the laptop must not drop the desktop's entries"
    );
    assert!(parsed.workspace(PRIMARY_MONITOR, "1").is_some());
}

#[test]
fn save_layout_puts_a_parked_workspace_under_its_origin_monitor() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    hub.add_monitor(reported_monitor(
        "monitor-1".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    let second = hub
        .monitor_id_by_disambiguated_name("monitor-1")
        .expect("the added monitor resolves");
    hub.focus_monitor(&MonitorSelector::Name("monitor-1".to_string()));
    hub.focus_workspace("work", None);
    hub.insert_window(titled("alpha"), default_rect(), WindowRestrictions::None);

    // Parking points the workspace's `monitor` at the primary, so an export
    // reading that field would file the entry under the wrong key.
    hub.remove_monitor(second);

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dome_export_parked_{nanos}.lua"));
    let _cleanup = CleanupFile(path.clone());

    hub.save_layout(&path).unwrap();
    let parsed = parse_exported_layout(path.to_str().unwrap());

    assert!(parsed.workspace("monitor-1", "work").is_some());
    assert!(parsed.workspace(PRIMARY_MONITOR, "work").is_none());
}
