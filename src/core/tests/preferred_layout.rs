use super::LayoutWorkspaceConfigBuilder;
use crate::core::hub::Hub;
use crate::core::node::{PixelRect, WindowId, WindowRestrictions, WorkspaceId};
use crate::core::tests::{
    PRIMARY_MONITOR, TestHubBuilder, TilingConfigBuilder, default_rect, partition_tree_entry,
    preferred_layout, process_meta, reported_monitor, save_then_apply, snapshot, titled,
    titled_matcher, work_area_at,
};
use crate::core::{PreferredWorkspace, Strategy, WindowMatcher};
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
    assert_eq!(
        hub.export_workspace(on_primary).tiling.strategy(),
        Strategy::Master
    );
    assert_eq!(
        hub.export_workspace(on_second).tiling.strategy(),
        Strategy::PartitionTree
    );
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
fn apply_preferred_layouts_creates_new_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();

    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("dev")
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
fn apply_preferred_layouts_synthesises_float_when_matcher_survives() {
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

    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("dev")
        .with_float(vec![float_matcher.clone()])
        .build()]));

    // Export synthesises from live metadata rather than re-emitting the rule.
    assert_eq!(
        hub.export_workspace(ws_id),
        PreferredWorkspace {
            float: vec![WindowMatcher {
                process: Some("float-live-window".into()),
                ..Default::default()
            }],
            ..partition_tree_entry(None)
        }
    );
}

#[test]
fn apply_preferred_layouts_synthesises_float_when_matcher_removed() {
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

    hub.apply_preferred_layouts(preferred_layout([
        LayoutWorkspaceConfigBuilder::new("dev").build()
    ]));

    assert_eq!(
        hub.export_workspace(ws_id),
        PreferredWorkspace {
            float: vec![WindowMatcher {
                process: Some("float-live-window".into()),
                ..Default::default()
            }],
            ..partition_tree_entry(None)
        }
    );
}

#[test]
fn apply_preferred_layouts_adopts_manual_float_when_matcher_added() {
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
    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("dev")
        .with_float(vec![float_matcher.clone()])
        .build()]));

    assert_eq!(
        hub.export_workspace(ws_id),
        PreferredWorkspace {
            float: vec![WindowMatcher {
                process: Some("float-live-window".into()),
                ..Default::default()
            }],
            ..partition_tree_entry(None)
        }
    );
}

#[test]
fn saved_slot_frees_when_its_window_closes() {
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

    let first = hub
        .insert_window(
            process_meta("other.exe"),
            PixelRect::new(10, 5, 30, 20),
            WindowRestrictions::None,
        )
        .expect("foreign tiling window inserted");

    save_then_apply(&mut hub);

    hub.focus_workspace("0", None);
    hub.insert_window(
        process_meta("other.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .expect("second window inserted");
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted)
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
    *                                                                         W1                                                                         *
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

    hub.delete_window(first);
    hub.insert_window(
        process_meta("other.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .expect("routed window inserted");
    hub.focus_workspace("dev", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=150.00, h=30.00, highlighted)
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
    *                                                                         W2                                                                         *
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
fn saved_slot_frees_when_its_window_moves_away() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("dev")
                .with_strategy(Strategy::Master)
                .build(),
        ])
        .build();

    hub.focus_workspace("dev", None);
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .expect("first window inserted");
    save_then_apply(&mut hub);
    hub.move_focused_to_workspace("2", None);

    hub.focus_workspace("0", None);
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .expect("routed window inserted");
    hub.focus_workspace("dev", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted)
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
    *                                                                         W1                                                                         *
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

fn chat_float_on_dev() -> (String, PreferredWorkspace) {
    LayoutWorkspaceConfigBuilder::new("dev")
        .with_float(vec![titled_matcher("chat")])
        .build()
}

fn insert_chat(hub: &mut Hub) -> WindowId {
    hub.insert_window(titled("chat"), default_rect(), WindowRestrictions::None)
        .expect("chat window inserted")
}

#[test]
fn held_fullscreen_entry_sends_next_window_to_float_entry() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("a")
                .with_fullscreen(vec![titled_matcher("chat")])
                .build(),
            LayoutWorkspaceConfigBuilder::new("b")
                .with_float(vec![titled_matcher("chat")])
                .build(),
        ])
        .build();
    insert_chat(&mut hub);
    insert_chat(&mut hub);

    hub.focus_workspace("a", None);
    assert_snapshot!(snapshot(&hub), @"
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
    hub.focus_workspace("b", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=100.00, h=30.00, float, highlighted)
      )

    ****************************************************************************************************                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                F1                                                *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *
    ");
}

#[test]
fn closed_float_window_frees_its_entry() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![chat_float_on_dev()])
        .build();
    hub.focus_workspace("dev", None);

    let a = insert_chat(&mut hub);
    insert_chat(&mut hub);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=30.00, float)
      )

    +--------------------------------------------------------------------------------------------------+**************************************************
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                F0                                                |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |**************************************************************************************************|**************************************************
    ");

    hub.delete_window(a);
    insert_chat(&mut hub);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(2), x=0.00, y=0.00, w=100.00, h=30.00, float, highlighted)
      )

    ****************************************************************************************************-------------------------------------------------+
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                F2                                                *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *                                                                                                  *                                                 |
    *--------------------------------------------------------------------------------------------------*-------------------------------------------------+
    ");
}

#[test]
fn config_reload_keeps_a_held_float_entry() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![chat_float_on_dev()])
        .build();
    hub.focus_workspace("dev", None);
    insert_chat(&mut hub);

    hub.sync_configuration(TilingConfigBuilder::new().build());

    insert_chat(&mut hub);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=30.00, float)
      )

    +--------------------------------------------------------------------------------------------------+**************************************************
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                F0                                                |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |**************************************************************************************************|**************************************************
    ");
}

#[test]
fn adding_a_monitor_keeps_a_held_float_entry() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![chat_float_on_dev()])
        .build();
    hub.focus_workspace("dev", None);
    insert_chat(&mut hub);

    hub.add_monitor(reported_monitor(
        "monitor-1".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    ));

    insert_chat(&mut hub);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=30.00, float)
      )
      Monitor(id=MonitorId(1), name="monitor-1", screen=(x=150.00 y=0.00 w=100.00 h=30.00))

    +--------------------------------------------------------------------------------------------------+**************************************************
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                F0                                                |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |                                                                                                  |                                                 *
    |**************************************************************************************************|**************************************************
    "#);
}

#[test]
fn minimized_float_window_frees_its_entry() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![chat_float_on_dev()])
        .build();
    hub.focus_workspace("dev", None);

    let a = insert_chat(&mut hub);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=30.00, float, highlighted)
      )

    ****************************************************************************************************                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                F0                                                *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *
    ");
    hub.minimize_window(a);
    insert_chat(&mut hub);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=100.00, h=30.00, float, highlighted)
      )
      Minimized: [WindowId(0)]

    ****************************************************************************************************                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                F1                                                *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *
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
