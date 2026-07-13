use insta::assert_snapshot;

use crate::config::{Strategy, WindowMatcher};
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, TestMetadata, snapshot,
    titled,
};
use crate::core::{Direction, TilingAction, WindowMetadata};

#[test]
fn secondary_matched_goes_to_stack() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_secondary(vec![WindowMatcher {
                    process: Some("terminal.exe".into()),
                    ..Default::default()
                }])
                .with_master_count(2)
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Term", "terminal.exe"),
    );
    let _w1 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Filler", "other.exe"),
    );
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00, highlighted)
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=30.00)
      )

    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W1                                   *|                                    W0                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn master_matched_goes_to_master_pane() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("browser.exe".into()),
                    ..Default::default()
                }])
                .with_secondary(vec![WindowMatcher {
                    process: Some("editor.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Editor", "editor.exe"),
    );
    let _w1 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Browser", "browser.exe"),
    );
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00, highlighted)
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=30.00)
      )

    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W1                                   *|                                    W0                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn master_full_cascades_to_secondary() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("editor.exe".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Filler", "other.exe"),
    );
    let _w1 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Editor", "editor.exe"),
    );
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
fn slot_occupied_skip() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master(vec![
                    WindowMatcher {
                        process: Some("browser.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("browser.exe".into()),
                        ..Default::default()
                    },
                ])
                .build(),
        ])
        .build();
    hub.insert_tiling(hub.current_workspace(), titled_process("B1", "browser.exe"));
    hub.insert_tiling(hub.current_workspace(), titled_process("B2", "browser.exe"));
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
fn unmatched_fills_master_room() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("editor.exe".into()),
                    ..Default::default()
                }])
                .with_master_count(2)
                .build(),
        ])
        .build();
    hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Editor", "editor.exe"),
    );
    hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Other", "other.exe"),
    );
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=150.00, h=15.00, highlighted)
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
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
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn mixed_matched_and_unmatched_order() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master(vec![
                    WindowMatcher {
                        process: Some("C.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("A.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("B.exe".into()),
                        ..Default::default()
                    },
                ])
                .with_master_count(3)
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled_process("A", "A.exe"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled_process("B", "B.exe"));
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled_process("C", "C.exe"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=150.00, h=10.00, highlighted)
        Window(id=WindowId(0), x=0.00, y=10.00, w=150.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=20.00, w=150.00, h=10.00)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W2                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn unmatched_goes_to_stack_when_master_full() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .build(),
        ])
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.insert_tiling(hub.current_workspace(), titled("w1"));
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
fn matched_order_on_both_lanes() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master(vec![
                    WindowMatcher {
                        process: Some("A.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("B.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("C.exe".into()),
                        ..Default::default()
                    },
                ])
                .with_secondary(vec![
                    WindowMatcher {
                        process: Some("D.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("E.exe".into()),
                        ..Default::default()
                    },
                ])
                .with_master_count(3)
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled_process("C", "C.exe"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled_process("E", "E.exe"));
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled_process("A", "A.exe"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled_process("B", "B.exe"));
    let _w4 = hub.insert_tiling(hub.current_workspace(), titled_process("D", "D.exe"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=0.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=20.00, w=75.00, h=10.00)
        Window(id=WindowId(4), x=75.00, y=0.00, w=75.00, h=15.00, highlighted)
        Window(id=WindowId(1), x=75.00, y=15.00, w=75.00, h=15.00)
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W2                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W4                                   *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |***************************************************************************
    |                                    W3                                   |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn decrease_master_count_drop_matched_master() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master(vec![
                    WindowMatcher {
                        process: Some("A.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("B.exe".into()),
                        ..Default::default()
                    },
                ])
                .with_master_count(2)
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled_process("A", "A.exe"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled_process("B", "B.exe"));
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled_process("D", "D.exe"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled_process("C", "C.exe"));
    hub.handle_tiling_action(TilingAction::FewerMaster);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(2), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=20.00, w=75.00, h=10.00, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn reordering_matched_windows_doesnt_guarrantee_next_match() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master(vec![
                    WindowMatcher {
                        process: Some("A.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("B.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("C.exe".into()),
                        ..Default::default()
                    },
                ])
                .with_master_count(3)
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled_process("A", "A.exe"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled_process("C", "C.exe"));
    hub.handle_tiling_action(TilingAction::MoveDirection {
        direction: Direction::Vertical,
        forward: false,
    });
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled_process("B", "B.exe"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=150.00, h=10.00, highlighted)
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=20.00, w=150.00, h=10.00)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W2                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn swapping_secondary_window_doesnt_guarrantee_next_match() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_secondary(vec![
                    WindowMatcher {
                        process: Some("A.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("B.exe".into()),
                        ..Default::default()
                    },
                ])
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled_process("B", "B.exe"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled_process("C", "C.exe"));
    hub.handle_tiling_action(TilingAction::MoveDirection {
        direction: Direction::Horizontal,
        forward: true,
    });
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled_process("A", "A.exe"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=15.00, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W0                                   |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W2                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

fn titled_process(title: &str, process: &str) -> Box<dyn WindowMetadata> {
    Box::new(TestMetadata {
        title: Some(title.into()),
        process: Some(process.into()),
    })
}
