use std::collections::HashSet;

use super::LayoutWorkspaceConfigBuilder;
use super::scrolling::{border_boxes_by_window, process_matcher, scrolling_layout_hub, stack};
use crate::core::hub::{Hub, MonitorLayout};
use crate::core::master::PaneConfig;
use crate::core::node::{PixelRect, WindowId, WindowRestrictions, WorkspaceId};
use crate::core::tests::{
    PRIMARY_MONITOR, TestHubBuilder, TilingConfigBuilder, default_rect, master_entry,
    partition_tree_entry, preferred_layout, preferred_layout_on, process_meta, reported_monitor,
    save_then_apply, snapshot, titled, titled_matcher, validate_hub, work_area_at,
};
use crate::core::{
    ColumnConfig, MonitorSelector, Pixels, PreferredLayouts, PreferredMaster, PreferredWorkspace,
    ScrollingConfig, SizeConstraint, Strategy, TreeLayoutNode, WindowMatcher,
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

#[test]
fn apply_preferred_layouts_resets_a_parked_workspace_from_its_origin_entry() {
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
    let work = hub.current_workspace();
    hub.remove_monitor(second);

    hub.apply_preferred_layouts(preferred_layout_on(
        "monitor-1",
        [LayoutWorkspaceConfigBuilder::new("work")
            .with_strategy(Strategy::Master)
            .build()],
    ));

    assert_eq!(
        hub.export_workspace(work).tiling.strategy(),
        Strategy::Master
    );
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
fn apply_preferred_layouts_loads_a_new_float_matcher_onto_an_existing_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    let ws_id = hub.current_workspace();
    let float_exe = WindowMatcher {
        process: Some("float.exe".into()),
        ..Default::default()
    };

    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
        .with_float(vec![float_exe.clone()])
        .build()]));
    hub.insert_window(
        process_meta("float.exe"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .expect("window inserted");

    assert_eq!(
        hub.export_workspace(ws_id),
        PreferredWorkspace {
            float: vec![float_exe],
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
fn saved_slot_stays_held_when_its_window_moves_away() {
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
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
}

/// The window that each named workspace focuses once it has focus, leaving the last named
/// workspace focused.
fn focused_windows_on(hub: &mut Hub, names: &[&str]) -> HashSet<Option<WindowId>> {
    names
        .iter()
        .map(|name| {
            hub.focus_workspace(name, None);
            hub.get_visible_placements().focused_window
        })
        .collect()
}

#[test]
fn matching_windows_fill_the_master_slots_of_two_workspaces() {
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

    let mut insert_editor = || {
        hub.insert_window(
            process_meta("editor.exe"),
            default_rect(),
            WindowRestrictions::None,
        )
        .expect("routed window inserted")
    };
    let windows = [insert_editor(), insert_editor()];

    for name in ["a", "b"] {
        assert_eq!(
            hub.export_workspace(workspace_on(&hub, PRIMARY_MONITOR, name)),
            master_entry(PreferredMaster {
                master: PaneConfig::tiled(vec![editor()]),
                ..PreferredMaster::default()
            }),
            "{name} does not hold one editor in its master pane"
        );
    }
    assert_eq!(
        focused_windows_on(&mut hub, &["a", "b"]),
        HashSet::from(windows.map(Some))
    );
    hub.validate();
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

/// Leaves the hub focused on the named workspace.
fn layout_shown_on(hub: &mut Hub, name: &str) -> MonitorLayout {
    hub.focus_workspace(name, None);
    let placements = hub.get_visible_placements();
    let focused_monitor = placements.focused_monitor;
    placements
        .monitors
        .into_iter()
        .find(|monitor| monitor.monitor_id == focused_monitor)
        .expect("the focused monitor shows a workspace")
        .layout
}

#[test]
fn matching_windows_fill_a_fullscreen_slot_and_a_float_slot_on_two_workspaces() {
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

    let windows = [insert_chat(&mut hub), insert_chat(&mut hub)];

    for (name, fullscreen, float) in [("a", 1, 0), ("b", 0, 1)] {
        let entry = hub.export_workspace(workspace_on(&hub, PRIMARY_MONITOR, name));
        assert_eq!(
            (entry.fullscreen.len(), entry.float.len()),
            (fullscreen, float),
            "{name} holds the wrong fullscreen and float counts"
        );
    }
    // Either window can take either slot, so the placements are checked by kind rather than
    // pinned to window ids in a snapshot.
    let MonitorLayout::Fullscreen(on_a) = layout_shown_on(&mut hub, "a") else {
        panic!("a shows no fullscreen window");
    };
    let MonitorLayout::Normal {
        tiling_windows,
        float_windows,
        ..
    } = layout_shown_on(&mut hub, "b")
    else {
        panic!("b shows a fullscreen window");
    };
    assert!(tiling_windows.is_empty(), "b shows a tiling window");
    let [on_b] = float_windows.as_slice() else {
        panic!("b shows {} float windows", float_windows.len());
    };
    assert_eq!(
        on_b.border_box,
        default_rect(),
        "the float on b is not at the rectangle it opened at"
    );
    assert_eq!(HashSet::from([on_a, on_b.id]), HashSet::from(windows));
    hub.validate();
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
fn minimized_float_window_keeps_its_entry() {
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
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Minimized: [WindowId(0)]

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
fn float_window_keeps_its_entry_through_fullscreen() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![chat_float_on_dev()])
        .build();
    hub.focus_workspace("dev", None);
    insert_chat(&mut hub);
    hub.toggle_fullscreen();
    insert_chat(&mut hub);
    hub.toggle_fullscreen();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[chat, chat])
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
    |                                    W1                                   |*                                    W0                                   *
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
fn fullscreen_window_keeps_its_entry_after_it_tiles() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("dev")
                .with_fullscreen(vec![titled_matcher("chat")])
                .build(),
        ])
        .build();
    hub.focus_workspace("dev", None);
    insert_chat(&mut hub);
    hub.toggle_fullscreen();
    insert_chat(&mut hub);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[chat, chat])
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
fn float_window_on_another_workspace_keeps_its_entry_until_it_closes() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![chat_float_on_dev()])
        .build();
    hub.focus_workspace("dev", None);
    let a = insert_chat(&mut hub);
    hub.move_focused_to_workspace("0", None);
    insert_chat(&mut hub);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
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
fn matching_windows_fill_the_float_slots_of_two_workspaces() {
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

    let mut insert_chat_exe = || {
        hub.insert_window(
            process_meta("chat.exe"),
            PixelRect::new(10, 5, 30, 20),
            WindowRestrictions::None,
        )
        .expect("routed window inserted")
    };
    let windows = [insert_chat_exe(), insert_chat_exe()];

    for name in ["a", "b"] {
        let entry = hub.export_workspace(workspace_on(&hub, PRIMARY_MONITOR, name));
        assert_eq!(entry.float.len(), 1, "{name} holds no float");
    }
    assert_eq!(
        focused_windows_on(&mut hub, &["a", "b"]),
        HashSet::from(windows.map(Some))
    );
    hub.validate();
}

#[test]
fn restricted_window_holds_the_slot_it_matched() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![chat_float_on_dev()])
        .build();
    hub.insert_window(
        titled("chat"),
        default_rect(),
        WindowRestrictions::ProtectFullscreen,
    )
    .expect("restricted window inserted");
    hub.focus_workspace("0", None);

    insert_chat(&mut hub);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
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
fn reset_releases_the_slot_a_moved_window_holds_on_a_later_workspace() {
    let leaf_for_w = || {
        LayoutWorkspaceConfigBuilder::new("a")
            .with_tree(TreeLayoutNode::Leaf(titled_matcher("w")))
            .build()
    };
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master_count(1)
                .build(),
            leaf_for_w(),
        ])
        .build();
    hub.insert_window(titled("w"), default_rect(), WindowRestrictions::None)
        .expect("w inserted");
    hub.move_focused_to_workspace("0", None);
    hub.focus_workspace("0", None);
    hub.insert_window(titled("v"), default_rect(), WindowRestrictions::None)
        .expect("v inserted");

    hub.apply_preferred_layouts(preferred_layout([
        LayoutWorkspaceConfigBuilder::new("0")
            .with_strategy(Strategy::Master)
            .with_master(vec![titled_matcher("w"), titled_matcher("v")])
            .with_master_count(1)
            .build(),
        leaf_for_w(),
    ]));

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00, highlighted)
        Window(id=WindowId(1), x=0.00, y=15.00, w=150.00, h=15.00)
      )

    ******************************************************************************************************************************************************
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
    ******************************************************************************************************************************************************
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
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

fn dev_entry_with_width(columns: Vec<ColumnConfig>, percent: f32) -> (String, PreferredWorkspace) {
    LayoutWorkspaceConfigBuilder::new("dev")
        .with_strategy(Strategy::Scrolling)
        .with_columns(columns)
        .with_column_width(SizeConstraint::Percent(percent))
        .build()
}

fn dev_hub_with_width(columns: Vec<ColumnConfig>, percent: f32) -> Hub {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Scrolling)
                .with_scrolling_config(ScrollingConfig {
                    column_width: SizeConstraint::Percent(20.0),
                })
                .build(),
        )
        .with_preferred_layout(vec![dev_entry_with_width(columns, percent)])
        .build();
    hub.focus_workspace("dev", None);
    hub
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
            (w0, PixelRect::new(8, 0, 30, 30)),
            (w1, PixelRect::new(38, 0, 60, 30)),
            (w2, PixelRect::new(98, 0, 45, 30)),
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
            (w0, PixelRect::new(98, 0, 45, 30)),
            (w1, PixelRect::new(38, 0, 60, 30)),
            (w2, PixelRect::new(8, 0, 30, 30)),
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
            (w0, PixelRect::new(15, 0, 30, 30)),
            (w1, PixelRect::new(105, 0, 30, 30)),
            (w2, PixelRect::new(45, 0, 30, 30)),
            (w3, PixelRect::new(75, 0, 30, 30)),
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
                    column_width: SizeConstraint::Percent(20.0),
                })
                .build(),
        )
        .build();
    hub.focus_workspace("dev", None);
    let w0 = insert_process(&mut hub, "a.exe");
    let w1 = insert_process(&mut hub, "b.exe");
    let w2 = insert_process(&mut hub, "c.exe");

    hub.apply_preferred_layouts(scrolling_dev_layout(vec![
        ColumnConfig::bare(process_matcher("c.exe")),
        ColumnConfig::bare(process_matcher("b.exe")),
        ColumnConfig::bare(process_matcher("a.exe")),
    ]));

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(90, 0, 30, 30)),
            (w1, PixelRect::new(60, 0, 30, 30)),
            (w2, PixelRect::new(30, 0, 30, 30)),
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

    hub.apply_preferred_layouts(scrolling_dev_layout(vec![
        ColumnConfig::bare(process_matcher("b.exe")),
        keyed_column(SizeConstraint::Percent(40.0), "a.exe"),
    ]));

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(60, 0, 60, 30)),
            (w1, PixelRect::new(30, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_workspace_column_width_sizes_each_column_without_its_own_width() {
    let mut hub = dev_hub_with_width(
        vec![
            ColumnConfig::bare(process_matcher("a.exe")),
            keyed_column(SizeConstraint::Percent(10.0), "b.exe"),
        ],
        40.0,
    );
    let a = insert_process(&mut hub, "a.exe");
    let b = insert_process(&mut hub, "b.exe");
    let u = insert_process(&mut hub, "u.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(8, 0, 60, 30)),
            (b, PixelRect::new(68, 0, 15, 30)),
            (u, PixelRect::new(83, 0, 60, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn applying_a_layout_sets_the_workspace_column_width_for_every_column() {
    let mut hub = dev_hub_with_width(vec![], 40.0);
    let u0 = insert_process(&mut hub, "u0.exe");

    hub.apply_preferred_layouts(preferred_layout(vec![dev_entry_with_width(vec![], 10.0)]));
    let u1 = insert_process(&mut hub, "u1.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (u0, PixelRect::new(60, 0, 15, 30)),
            (u1, PixelRect::new(75, 0, 15, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn editing_the_columns_stacks_open_windows() {
    let mut hub = scrolling_layout_hub(vec![
        ColumnConfig::bare(process_matcher("a.exe")),
        ColumnConfig::bare(process_matcher("b.exe")),
    ]);
    hub.focus_workspace("dev", None);
    let w0 = insert_process(&mut hub, "a.exe");
    let w1 = insert_process(&mut hub, "b.exe");

    hub.apply_preferred_layouts(scrolling_dev_layout(vec![stack(&["a.exe", "b.exe"])]));

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(60, 0, 30, 15)),
            (w1, PixelRect::new(60, 15, 30, 15)),
        ]
    );
    validate_hub(&hub);
}
