use super::LayoutWorkspaceConfigBuilder;
use super::scrolling::{border_boxes_by_window, process_matcher, scrolling_layout_hub};
use crate::core::node::{PixelRect, WindowId, WindowRestrictions, WorkspaceId};
use crate::core::strategy::WorkspaceExport;
use crate::core::tests::{
    PRIMARY_MONITOR, TestHubBuilder, TilingConfigBuilder, default_rect, preferred_layout,
    process_meta, reported_monitor, snapshot, titled, validate_hub, work_area_at,
};
use crate::core::{
    ColumnConfig, Hub, Pixels, PreferredLayouts, ScrollingConfig, SizeConstraint, Strategy,
    WindowMatcher,
};
use insta::assert_snapshot;

#[test]
fn preferred_layout_indexing_visits_workspace_names_sorted() {
    let hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("e").build(),
            LayoutWorkspaceConfigBuilder::new("d").build(),
            LayoutWorkspaceConfigBuilder::new("c").build(),
            LayoutWorkspaceConfigBuilder::new("b").build(),
            LayoutWorkspaceConfigBuilder::new("a").build(),
        ])
        .build();

    let names: Vec<String> = hub
        .access
        .workspaces
        .sorted_ids()
        .into_iter()
        .map(|ws_id| hub.access.workspaces.get(ws_id).name.clone())
        .collect();

    assert_eq!(names, vec!["0", "a", "b", "c", "d", "e"]);
}

#[test]
fn each_monitor_takes_its_own_entry_for_a_shared_workspace_name() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(Strategy::Master)
                .build(),
        ])
        .with_preferred_layout_on(
            "monitor-1",
            vec![
                LayoutWorkspaceConfigBuilder::new("1")
                    .with_strategy(Strategy::PartitionTree)
                    .build(),
            ],
        )
        .build();

    hub.add_monitor(reported_monitor(
        "monitor-1".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    let on_primary = workspace_on(&hub, PRIMARY_MONITOR, "1");
    let on_second = workspace_on(&hub, "monitor-1", "1");
    assert_eq!(hub.export_workspace(on_primary).strategy, "master");
    assert_eq!(hub.export_workspace(on_second).strategy, "partition_tree");
}

#[test]
fn entry_under_an_absent_monitor_applies_nothing() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout_on(
            "desktop",
            vec![
                LayoutWorkspaceConfigBuilder::new("desk")
                    .with_float(vec![WindowMatcher {
                        process: Some("float.exe".into()),
                        ..Default::default()
                    }])
                    .build(),
            ],
        )
        .build();

    let names: Vec<String> = hub
        .access
        .workspaces
        .sorted_ids()
        .into_iter()
        .map(|ws_id| hub.access.workspaces.get(ws_id).name.clone())
        .collect();
    assert_eq!(names, vec!["0"]);

    let ws_id = hub.access.workspaces.sorted_ids()[0];
    hub.insert_window(
        process_meta("float.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .expect("window inserted");
    assert!(hub.export_workspace(ws_id).float.is_empty());
}

fn workspace_on(hub: &crate::core::Hub, monitor: &str, name: &str) -> WorkspaceId {
    hub.access
        .workspaces
        .sorted_ids()
        .into_iter()
        .find(|&ws_id| {
            let ws = hub.access.workspaces.get(ws_id);
            ws.name == name && hub.access.origin_monitor_name(ws_id) == monitor
        })
        .expect("workspace present on that monitor")
}

#[test]
fn sync_preferred_layout_creates_new_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();

    hub.sync_preferred_layout(preferred_layout([LayoutWorkspaceConfigBuilder::new("dev")
        .with_strategy(Strategy::Master)
        .with_float(vec![WindowMatcher {
            process: Some("float.exe".into()),
            ..Default::default()
        }])
        .build()]));

    hub.focus_workspace("dev", None);
    hub.insert_window(
        process_meta("float.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F0             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}

#[test]
fn float_matcher_routes_to_float() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("3")
                .with_strategy(Strategy::Master)
                .with_float(vec![WindowMatcher {
                    process: Some("float.exe".into()),
                    ..Default::default()
                }])
                .with_fullscreen(vec![WindowMatcher {
                    process: Some("fullscreen.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.insert_window(
        process_meta("float.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );
    hub.focus_workspace("3", None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F0             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}

#[test]
fn fullscreen_matcher_routes_to_fullscreen() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("3")
                .with_strategy(Strategy::Master)
                .with_float(vec![WindowMatcher {
                    process: Some("float.exe".into()),
                    ..Default::default()
                }])
                .with_fullscreen(vec![WindowMatcher {
                    process: Some("fullscreen.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.insert_window(
        process_meta("fullscreen.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );
    hub.focus_workspace("3", None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Fullscreen(id=WindowId(0))
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn fullscreen_beats_float_when_both_match() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("3")
                .with_strategy(Strategy::Master)
                .with_float(vec![WindowMatcher {
                    title: Some("matchme".into()),
                    ..Default::default()
                }])
                .with_fullscreen(vec![WindowMatcher {
                    title: Some("matchme".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.insert_window(
        titled("matchme"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );
    hub.focus_workspace("3", None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Fullscreen(id=WindowId(0))
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn no_match_tiles_on_current_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("3")
                .with_strategy(Strategy::Master)
                .with_float(vec![WindowMatcher {
                    process: Some("float.exe".into()),
                    ..Default::default()
                }])
                .with_fullscreen(vec![WindowMatcher {
                    process: Some("fullscreen.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.insert_window(
        process_meta("unknown.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn matchers_on_partition_tree_variant() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::PartitionTree)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("ws2")
                .with_float(vec![WindowMatcher {
                    process: Some("float.exe".into()),
                    ..Default::default()
                }])
                .with_fullscreen(vec![WindowMatcher {
                    process: Some("fullscreen.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.insert_window(
        process_meta("float.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );
    hub.focus_workspace("ws2", None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F0             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}

#[test]
fn global_float_matcher_floats_on_current_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_float(vec![WindowMatcher {
                    process: Some("calc.exe".into()),
                    ..Default::default()
                }])
                .build(),
        )
        .build();
    hub.insert_window(
        process_meta("calc.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );
    // Window stays on workspace "0" (current), not routed anywhere.
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F0             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}

#[test]
fn global_fullscreen_matcher_fullscreens_on_current_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_fullscreen(vec![WindowMatcher {
                    process: Some("slides.exe".into()),
                    ..Default::default()
                }])
                .build(),
        )
        .build();
    hub.insert_window(
        process_meta("slides.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Fullscreen(id=WindowId(0))
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn per_workspace_override_beats_global() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_float(vec![WindowMatcher {
                    process: Some("calc.exe".into()),
                    ..Default::default()
                }])
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("3")
                .with_strategy(Strategy::Master)
                .with_float(vec![WindowMatcher {
                    process: Some("calc.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    // "calc.exe" matches both per-workspace float on ws "3" and global float.
    // Per-workspace wins. Routes to workspace "3" as float.
    hub.insert_window(
        process_meta("calc.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );
    hub.focus_workspace("3", None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F0             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}

#[test]
fn no_match_uses_global_matcher() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_float(vec![WindowMatcher {
                    process: Some("calc.exe".into()),
                    ..Default::default()
                }])
                .build(),
        )
        .build();
    // "unknown.exe" matches nothing, so it tiles on the current workspace.
    hub.insert_window(
        process_meta("unknown.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn tiling_matcher_routes_to_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("dev")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("editor.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.insert_window(
        process_meta("editor.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.focus_workspace("dev", None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn float_beats_tiling() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("dev")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("popup.exe".into()),
                    ..Default::default()
                }])
                .with_float(vec![WindowMatcher {
                    process: Some("popup.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.insert_window(
        process_meta("popup.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.focus_workspace("dev", None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F0             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}

#[test]
fn name_order_first_match_wins() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("code")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("editor.exe".into()),
                    ..Default::default()
                }])
                .build(),
            LayoutWorkspaceConfigBuilder::new("chat")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("editor.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.insert_window(
        process_meta("editor.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.focus_workspace("chat", None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn no_tiling_match_falls_back_to_current() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("dev")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("editor.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.insert_window(
        process_meta("unknown.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.insert_window(
        titled("Unknown1"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .unwrap();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn sync_preferred_layout_synthesises_float_when_matcher_survives() {
    let float_matcher = WindowMatcher {
        process: Some("/float.*/".into()),
        ..Default::default()
    };

    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("dev")
                .with_float(vec![float_matcher.clone()])
                .build(),
        ])
        .build();
    hub.focus_workspace("dev", None);
    let ws_id = hub.current_workspace();

    hub.insert_window(
        process_meta("float-live-window"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .expect("the window should insert");

    hub.sync_preferred_layout(preferred_layout([LayoutWorkspaceConfigBuilder::new("dev")
        .with_float(vec![float_matcher.clone()])
        .build()]));

    // Export synthesises from live metadata rather than re-emitting the rule.
    assert_eq!(
        hub.export_workspace(ws_id),
        WorkspaceExport {
            strategy: "partition_tree".into(),
            float: vec![WindowMatcher {
                process: Some("float-live-window".into()),
                ..Default::default()
            }],
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn sync_preferred_layout_synthesises_float_when_matcher_removed() {
    let float_matcher = WindowMatcher {
        process: Some("/float.*/".into()),
        ..Default::default()
    };

    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("dev")
                .with_float(vec![float_matcher.clone()])
                .build(),
        ])
        .build();
    hub.focus_workspace("dev", None);
    let ws_id = hub.current_workspace();

    hub.insert_window(
        process_meta("float-live-window"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    );

    hub.sync_preferred_layout(preferred_layout([
        LayoutWorkspaceConfigBuilder::new("dev").build()
    ]));

    assert_eq!(
        hub.export_workspace(ws_id),
        WorkspaceExport {
            strategy: "partition_tree".into(),
            float: vec![WindowMatcher {
                process: Some("float-live-window".into()),
                ..Default::default()
            }],
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn sync_preferred_layout_adopts_manual_float_when_matcher_added() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![LayoutWorkspaceConfigBuilder::new("dev").build()])
        .build();
    hub.focus_workspace("dev", None);
    let ws_id = hub.current_workspace();

    let window_id = hub
        .insert_window(
            process_meta("float-live-window"),
            PixelRect::new(10, 5, 30, 20),
            WindowRestrictions::None,
        )
        .expect("window inserted");
    hub.set_focus(window_id);
    hub.toggle_float();

    let float_matcher = WindowMatcher {
        process: Some("/float.*/".into()),
        ..Default::default()
    };
    hub.sync_preferred_layout(preferred_layout([LayoutWorkspaceConfigBuilder::new("dev")
        .with_float(vec![float_matcher.clone()])
        .build()]));

    // Export synthesises from live metadata rather than re-emitting the rule.
    assert_eq!(
        hub.export_workspace(ws_id),
        WorkspaceExport {
            strategy: "partition_tree".into(),
            float: vec![WindowMatcher {
                process: Some("float-live-window".into()),
                ..Default::default()
            }],
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn tiling_insert_routes_against_post_export_state() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("dev")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("editor.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();

    hub.focus_workspace("dev", None);
    let dev = hub
        .access
        .workspaces
        .find(|w| w.name == "dev")
        .expect("workspace exists");

    hub.insert_window(
        process_meta("other.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .expect("foreign tiling window inserted");

    assert!(
        !hub.strategies
            .for_workspace(dev)
            .matches_tiling(dev, process_meta("other.exe").as_ref())
    );

    hub.export_workspace(dev);

    hub.focus_workspace("0", None);
    let new_window = hub
        .insert_window(
            process_meta("other.exe"),
            PixelRect::new(10, 5, 30, 20),
            WindowRestrictions::None,
        )
        .expect("routed window inserted");

    assert_eq!(hub.access.windows.get(new_window).workspace(), Some(dev));
}

#[test]
fn scrolling_insert_routes_against_post_export_state() {
    let mut hub = scrolling_layout_hub(vec![ColumnConfig::bare(process_matcher("a.exe"))]);
    hub.focus_workspace("dev", None);
    let dev = dev_workspace(&hub);
    insert_process(&mut hub, "u.exe");
    assert!(
        !hub.strategies
            .for_workspace(dev)
            .matches_tiling(dev, process_meta("u.exe").as_ref())
    );

    hub.export_workspace(dev);

    hub.focus_workspace("0", None);
    let routed = insert_process(&mut hub, "u.exe");
    assert_eq!(hub.access.windows.get(routed).workspace(), Some(dev));
    validate_hub(&hub);
}

#[test]
fn tiling_routes_to_current_workspace_when_it_can_house() {
    let editor = || WindowMatcher {
        process: Some("editor.exe".into()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("a")
                .with_strategy(Strategy::Master)
                .with_master(vec![editor()])
                .build(),
            LayoutWorkspaceConfigBuilder::new("b")
                .with_strategy(Strategy::Master)
                .with_master(vec![editor()])
                .build(),
        ])
        .build();

    // "a" has the smaller id, so sorted order alone would pick it. Visit it,
    // then move to "b": the current workspace must win.
    hub.focus_workspace("a", None);
    hub.focus_workspace("b", None);

    hub.insert_window(
        process_meta("editor.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .expect("routed window inserted");

    // The snapshot renders the current workspace "b", where the tiled window
    // now shows, so it routed to the current workspace rather than "a".
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn tiling_falls_back_to_first_workspace_when_current_cannot_house() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("a")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("editor.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();

    // The current workspace "0" has no preferred layout, so routing falls back
    // to "a". Focus "a" to observe: the window shows there, not on "0".
    hub.insert_window(
        process_meta("editor.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .expect("routed window inserted");

    hub.focus_workspace("a", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn float_routes_to_current_workspace_when_it_can_house() {
    let chat = || WindowMatcher {
        process: Some("chat.exe".into()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("a")
                .with_float(vec![chat()])
                .build(),
            LayoutWorkspaceConfigBuilder::new("b")
                .with_float(vec![chat()])
                .build(),
        ])
        .build();

    // "a" has the smaller id, so sorted order alone would pick it. Visit it,
    // then move to "b": the current workspace must win.
    hub.focus_workspace("a", None);
    hub.focus_workspace("b", None);

    hub.insert_window(
        process_meta("chat.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .expect("routed window inserted");

    // The snapshot renders the current workspace "b". The float window shows
    // there, so it both routed to the current workspace and stayed a float.
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F0             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}

fn insert_process(hub: &mut Hub, process: &str) -> WindowId {
    hub.insert_window(
        process_meta(process),
        default_rect(),
        WindowRestrictions::None,
    )
    .expect("tiling window inserted")
}

fn dev_workspace(hub: &Hub) -> WorkspaceId {
    workspace_on(hub, PRIMARY_MONITOR, "dev")
}

fn keyed_column(width: SizeConstraint, process: &str) -> ColumnConfig {
    ColumnConfig {
        width: Some(width),
        children: vec![process_matcher(process)],
    }
}

fn scrolling_dev_layout(columns: Vec<ColumnConfig>) -> PreferredLayouts {
    preferred_layout(vec![
        LayoutWorkspaceConfigBuilder::new("dev")
            .with_strategy(Strategy::Scrolling)
            .with_columns(columns)
            .build(),
    ])
}

#[test]
fn scrolling_column_routes_to_its_workspace() {
    let mut hub = scrolling_layout_hub(vec![ColumnConfig::bare(process_matcher("a.exe"))]);
    let dev = dev_workspace(&hub);

    let w0 = insert_process(&mut hub, "a.exe");

    assert_eq!(hub.access.windows.get(w0).workspace(), Some(dev));
    validate_hub(&hub);
}

#[test]
fn scrolling_windows_in_layout_order_take_their_widths() {
    let mut hub = scrolling_layout_hub(vec![
        ColumnConfig::bare(process_matcher("a.exe")),
        keyed_column(SizeConstraint::Percent(40.0), "b.exe"),
        keyed_column(SizeConstraint::Pixels(Pixels::new(45)), "c.exe"),
    ]);
    hub.focus_workspace("dev", None);

    let w0 = insert_process(&mut hub, "a.exe");
    let w1 = insert_process(&mut hub, "b.exe");
    let w2 = insert_process(&mut hub, "c.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(0, 0, 30, 30)),
            (w1, PixelRect::new(30, 0, 60, 30)),
            (w2, PixelRect::new(90, 0, 45, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn scrolling_windows_out_of_order_fill_their_layout_places() {
    let mut hub = scrolling_layout_hub(vec![
        ColumnConfig::bare(process_matcher("a.exe")),
        keyed_column(SizeConstraint::Percent(40.0), "b.exe"),
        keyed_column(SizeConstraint::Pixels(Pixels::new(45)), "c.exe"),
    ]);
    hub.focus_workspace("dev", None);

    let w0 = insert_process(&mut hub, "c.exe");
    let w1 = insert_process(&mut hub, "b.exe");
    let w2 = insert_process(&mut hub, "a.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(90, 0, 45, 30)),
            (w1, PixelRect::new(30, 0, 60, 30)),
            (w2, PixelRect::new(0, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn unmatched_window_opens_right_of_focus_between_layout_columns() {
    let mut hub = scrolling_layout_hub(vec![
        ColumnConfig::bare(process_matcher("a.exe")),
        ColumnConfig::bare(process_matcher("b.exe")),
    ]);
    hub.focus_workspace("dev", None);
    let w0 = insert_process(&mut hub, "a.exe");
    let w1 = insert_process(&mut hub, "b.exe");
    hub.set_focus(w0);

    let w2 = insert_process(&mut hub, "u.exe");
    let w3 = insert_process(&mut hub, "b.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(0, 0, 30, 30)),
            (w1, PixelRect::new(60, 0, 30, 30)),
            (w2, PixelRect::new(30, 0, 30, 30)),
            (w3, PixelRect::new(90, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn switching_a_workspace_to_scrolling_fills_its_layout_columns() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_scrolling_config(ScrollingConfig {
                    default_column_width: SizeConstraint::Percent(20.0),
                })
                .build(),
        )
        .build();
    hub.focus_workspace("dev", None);
    let w0 = insert_process(&mut hub, "a.exe");
    let w1 = insert_process(&mut hub, "b.exe");
    let w2 = insert_process(&mut hub, "c.exe");

    hub.sync_preferred_layout(scrolling_dev_layout(vec![
        ColumnConfig::bare(process_matcher("c.exe")),
        ColumnConfig::bare(process_matcher("b.exe")),
        ColumnConfig::bare(process_matcher("a.exe")),
    ]));

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(60, 0, 30, 30)),
            (w1, PixelRect::new(30, 0, 30, 30)),
            (w2, PixelRect::new(0, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn editing_the_columns_reorders_open_windows() {
    let mut hub = scrolling_layout_hub(vec![
        ColumnConfig::bare(process_matcher("a.exe")),
        ColumnConfig::bare(process_matcher("b.exe")),
    ]);
    hub.focus_workspace("dev", None);
    let w0 = insert_process(&mut hub, "a.exe");
    let w1 = insert_process(&mut hub, "b.exe");

    hub.sync_preferred_layout(scrolling_dev_layout(vec![
        ColumnConfig::bare(process_matcher("b.exe")),
        keyed_column(SizeConstraint::Percent(40.0), "a.exe"),
    ]));

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(30, 0, 60, 30)),
            (w1, PixelRect::new(0, 0, 30, 30)),
        ]
    );
    assert_eq!(hub.focused_window(dev_workspace(&hub)), Some(w1));
    validate_hub(&hub);
}

#[test]
fn rebuilding_the_columns_keeps_unmatched_windows_in_order() {
    let mut hub = scrolling_layout_hub(vec![ColumnConfig::bare(process_matcher("a.exe"))]);
    hub.focus_workspace("dev", None);
    let w0 = insert_process(&mut hub, "x.exe");
    let w1 = insert_process(&mut hub, "y.exe");
    let w2 = insert_process(&mut hub, "z.exe");

    hub.sync_preferred_layout(scrolling_dev_layout(vec![ColumnConfig::bare(
        process_matcher("b.exe"),
    )]));

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(0, 0, 30, 30)),
            (w1, PixelRect::new(30, 0, 30, 30)),
            (w2, PixelRect::new(60, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn dropping_the_layout_entry_keeps_every_column_width() {
    let mut hub = scrolling_layout_hub(vec![keyed_column(SizeConstraint::Percent(40.0), "a.exe")]);
    hub.focus_workspace("dev", None);
    let w0 = insert_process(&mut hub, "a.exe");
    let w1 = insert_process(&mut hub, "u.exe");

    hub.sync_preferred_layout(PreferredLayouts::default());

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(0, 0, 60, 30)),
            (w1, PixelRect::new(60, 0, 30, 30)),
        ]
    );
    hub.focus_workspace("0", None);
    let w2 = insert_process(&mut hub, "a.exe");
    assert_eq!(
        hub.access.windows.get(w2).workspace(),
        Some(workspace_on(&hub, PRIMARY_MONITOR, "0"))
    );
    validate_hub(&hub);
}
