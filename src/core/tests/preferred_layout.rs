use crate::config::{LayoutConfig, LayoutWorkspaceConfig, WindowMatcher};
use crate::core::hub::Hub;
use crate::core::node::{Dimension, Length, WindowRestrictions};
use crate::core::tests::snapshot;
use insta::assert_snapshot;

const SCREEN: Dimension = Dimension::new(
    Length::new(0.0),
    Length::new(0.0),
    Length::new(800.0),
    Length::new(600.0),
);

#[test]
fn float_matcher_routes_to_float() {
    let mut hub = hub_with_matchers();
    hub.insert_window(process_meta("float.exe"), SCREEN, WindowRestrictions::None);
    hub.focus_workspace("3");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=800.00 h=600.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=800.00, h=600.00, float, highlighted)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *
    ");
}

#[test]
fn fullscreen_matcher_routes_to_fullscreen() {
    let mut hub = hub_with_matchers();
    hub.insert_window(
        process_meta("fullscreen.exe"),
        SCREEN,
        WindowRestrictions::None,
    );
    hub.focus_workspace("3");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=800.00 h=600.00),
        Fullscreen(id=WindowId(0))
      )

    +-----------------------------------------------------------------------------------------------------------------------------------------------------
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |
    ");
}

#[test]
fn fullscreen_beats_float_when_both_match() {
    let mut hub = hub_with_conflicting_matchers();
    hub.insert_window(titled_meta("matchme"), SCREEN, WindowRestrictions::None);
    hub.focus_workspace("3");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=800.00 h=600.00),
        Fullscreen(id=WindowId(0))
      )

    +-----------------------------------------------------------------------------------------------------------------------------------------------------
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |
    ");
}

#[test]
fn no_match_tiles_on_current_workspace() {
    let mut hub = hub_with_matchers();
    hub.insert_window(
        process_meta("unknown.exe"),
        SCREEN,
        WindowRestrictions::None,
    );
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=800.00 h=600.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=800.00, h=600.00, highlighted, spawn=right)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *
    ");
}

#[test]
fn matchers_on_partition_tree_variant() {
    let mut hub = Hub::new(
        SCREEN,
        1.0,
        LayoutConfig {
            strategy: crate::config::Strategy::PartitionTree,
            workspace: vec![LayoutWorkspaceConfig::PartitionTree {
                name: "ws2".into(),
                float: vec![WindowMatcher {
                    process: Some("float.exe".into()),
                    ..Default::default()
                }],
                fullscreen: vec![WindowMatcher {
                    process: Some("fullscreen.exe".into()),
                    ..Default::default()
                }],
            }],
            ..Default::default()
        },
    );
    hub.insert_window(process_meta("float.exe"), SCREEN, WindowRestrictions::None);
    hub.focus_workspace("ws2");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=800.00 h=600.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=800.00, h=600.00, float, highlighted)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *                                                                                                                                                     
    *
    ");
}

// ---------------------------------------------------------------------------
// Helpers

/// Hub with named workspace "3" that has float + fullscreen matchers
/// targeting different window processes.
fn hub_with_matchers() -> Hub {
    Hub::new(
        SCREEN,
        1.0,
        LayoutConfig {
            workspace: vec![LayoutWorkspaceConfig::Master {
                name: "3".into(),
                master_ratio: None,
                master_count: None,
                master: Vec::new(),
                secondary: Vec::new(),
                float: vec![WindowMatcher {
                    process: Some("float.exe".into()),
                    ..Default::default()
                }],
                fullscreen: vec![WindowMatcher {
                    process: Some("fullscreen.exe".into()),
                    ..Default::default()
                }],
            }],
            ..Default::default()
        },
    )
}

/// Hub where both float and fullscreen match on the same window title.
fn hub_with_conflicting_matchers() -> Hub {
    Hub::new(
        SCREEN,
        1.0,
        LayoutConfig {
            workspace: vec![LayoutWorkspaceConfig::Master {
                name: "3".into(),
                master_ratio: None,
                master_count: None,
                master: Vec::new(),
                secondary: Vec::new(),
                float: vec![WindowMatcher {
                    title: Some("matchme".into()),
                    ..Default::default()
                }],
                fullscreen: vec![WindowMatcher {
                    title: Some("matchme".into()),
                    ..Default::default()
                }],
            }],
            ..Default::default()
        },
    )
}

/// Build metadata with the given title.
fn titled_meta(t: &str) -> Box<dyn crate::core::WindowMetadata> {
    use crate::core::tests::TestMetadata;
    Box::new(TestMetadata {
        title: Some(t.into()),
        ..Default::default()
    })
}

/// Build metadata with the given process name.
fn process_meta(p: &str) -> Box<dyn crate::core::WindowMetadata> {
    use crate::core::tests::TestMetadata;
    Box::new(TestMetadata {
        process: Some(p.into()),
        ..Default::default()
    })
}
