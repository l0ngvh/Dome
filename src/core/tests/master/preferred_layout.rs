use insta::assert_snapshot;

use crate::config::{Strategy, WindowMatcher};
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, default_dim, process_meta,
    snapshot, titled, titled_process,
};
use crate::core::{Direction, TilingAction, WindowRestrictions};

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
    let _w0 = hub
        .insert_window(
            titled_process("Term", "terminal.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("Filler", "other.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
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
    let _w0 = hub
        .insert_window(
            titled_process("Editor", "editor.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("Browser", "browser.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
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
fn master_full_pushes_unmatched_to_secondary() {
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
    let _w0 = hub
        .insert_window(
            titled_process("Filler", "other.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("Editor", "editor.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
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
fn master_full_continue_matching_in_secondary() {
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
                        process: Some("editor.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("code.exe".into()),
                        ..Default::default()
                    },
                ])
                .with_secondary(vec![
                    WindowMatcher {
                        title: Some("Code".into()),
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
    let _w0 = hub
        .insert_window(
            titled_process("Editor", "editor.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("Browser", "browser.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w2 = hub
        .insert_window(
            titled_process("Code", "code.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=15.00, highlighted)
        Window(id=WindowId(1), x=75.00, y=15.00, w=75.00, h=15.00)
      )

    +-------------------------------------------------------------------------+***************************************************************************
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
    |                                                                         |***************************************************************************
    |                                    W0                                   |+-------------------------------------------------------------------------+
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
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
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
    hub.insert_window(
        titled_process("Editor", "editor.exe"),
        default_dim(),
        WindowRestrictions::None,
    );
    hub.insert_window(
        titled_process("Other", "other.exe"),
        default_dim(),
        WindowRestrictions::None,
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
    let _w0 = hub
        .insert_window(
            titled_process("A", "A.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("B", "B.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w2 = hub
        .insert_window(
            titled_process("C", "C.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
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
    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
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
fn insert_master_full_evicts_unmatched() {
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
                .with_master_count(1)
                .build(),
        ])
        .build();
    let _filler = hub
        .insert_window(
            titled_process("Filler", "other.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _editor = hub
        .insert_window(
            titled_process("Editor", "editor.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();

    assert_snapshot!(snapshot(&hub), @"
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
    let _w0 = hub
        .insert_window(
            titled_process("C", "C.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("E", "E.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w2 = hub
        .insert_window(
            titled_process("A", "A.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w3 = hub
        .insert_window(
            titled_process("B", "B.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w4 = hub
        .insert_window(
            titled_process("D", "D.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
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
    let _w0 = hub
        .insert_window(
            titled_process("A", "A.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("B", "B.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w2 = hub
        .insert_window(
            titled_process("D", "D.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w3 = hub
        .insert_window(
            titled_process("C", "C.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
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
fn reloading_preferred_layout_puts_matched_windows_to_place() {
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

    let _w0 = hub
        .insert_window(
            titled_process("B", "B.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("C", "C.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w2 = hub
        .insert_window(
            titled_process("A", "A.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();

    hub.sync_preferred_layout(vec![
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
    ]);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=15.00, highlighted)
        Window(id=WindowId(0), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
      )

    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W2                                   *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************|                                                                         |
    +-------------------------------------------------------------------------+|                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
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
    let _w0 = hub
        .insert_window(
            titled_process("A", "A.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("C", "C.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.handle_tiling_action(TilingAction::MoveDirection {
        direction: Direction::Vertical,
        forward: false,
    });
    let _w2 = hub
        .insert_window(
            titled_process("B", "B.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
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
    let _w0 = hub
        .insert_window(
            titled_process("B", "B.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("C", "C.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.handle_tiling_action(TilingAction::MoveDirection {
        direction: Direction::Horizontal,
        forward: true,
    });
    let _w2 = hub
        .insert_window(
            titled_process("A", "A.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
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

#[test]
fn increase_master_count_without_matcher_change() {
    // When only the master_count increases in a preferred layout (matchers
    // stay the same), unmatched windows are promoted from stack to master.
    let mut hub = TestHubBuilder::new()
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_secondary(vec![WindowMatcher {
                    title: Some("w2".into()),
                    ..Default::default()
                }])
                .with_master_count(1)
                .build(),
        ])
        .build();
    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_dim(), WindowRestrictions::None);

    // Hot-reload preferred layout with count=2, same (empty) matchers.
    hub.sync_preferred_layout(vec![
        LayoutWorkspaceConfigBuilder::new("0")
            .with_strategy(Strategy::Master)
            .with_secondary(vec![WindowMatcher {
                title: Some("w2".into()),
                ..Default::default()
            }])
            .with_master_count(2)
            .build(),
    ]);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                    W2                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn insert_multiple_matched_windows_to_the_same_slot() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("browser.exe".into()),
                    ..Default::default()
                }])
                .with_secondary(vec![
                    WindowMatcher {
                        process: Some("editor.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("terminal.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("mail.exe".into()),
                        ..Default::default()
                    },
                ])
                .with_master_count(2)
                .build(),
        ])
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();
    let _w0 = hub
        .insert_window(
            titled_process("Terminal A", "terminal.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("Editor A", "editor.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w2 = hub
        .insert_window(
            titled_process("Browser A", "browser.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w3 = hub
        .insert_window(
            titled_process("Editor B", "editor.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w4 = hub
        .insert_window(
            titled_process("Mail A", "mail.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w5 = hub
        .insert_window(
            titled_process("Browser B", "browser.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w6 = hub
        .insert_window(
            titled_process("Terminal B", "terminal.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w7 = hub
        .insert_window(
            titled_process("Main B", "mail.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();

    let prev_snapshot = snapshot(&hub);

    assert_snapshot!(prev_snapshot, @"
    Hub(focused=WindowId(7))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(5), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=5.00)
        Window(id=WindowId(3), x=75.00, y=5.00, w=75.00, h=5.00)
        Window(id=WindowId(0), x=75.00, y=10.00, w=75.00, h=5.00)
        Window(id=WindowId(6), x=75.00, y=15.00, w=75.00, h=5.00)
        Window(id=WindowId(4), x=75.00, y=20.00, w=75.00, h=5.00)
        Window(id=WindowId(7), x=75.00, y=25.00, w=75.00, h=5.00, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W1                                   |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W2                                   ||                                    W3                                   |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W0                                   |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W6                                   |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W5                                   ||                                    W4                                   |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W7                                   *
    +-------------------------------------------------------------------------+***************************************************************************
    ");

    let exported = hub.export_workspace(ws_id);

    hub.sync_preferred_layout(vec![
        LayoutWorkspaceConfigBuilder::new("1")
            .with_strategy(Strategy::Master)
            .with_master(exported.master)
            .with_master_count(2)
            .build(),
    ]);
    assert_eq!(prev_snapshot, snapshot(&hub));
}

#[test]
fn insert_master_full_no_evictable_matches_against_secondary() {
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
                .with_secondary(vec![
                    WindowMatcher {
                        process: Some("terminal.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("editor.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("mail.exe".into()),
                        ..Default::default()
                    },
                ])
                .with_master_count(1)
                .build(),
        ])
        .build();
    let _w0 = hub
        .insert_window(
            titled_process("Editor A", "editor.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled_process("Terminal", "terminal.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w2 = hub
        .insert_window(
            titled_process("Mail", "mail.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w3 = hub
        .insert_window(
            titled_process("Editor B", "editor.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=10.00, w=75.00, h=10.00, highlighted)
        Window(id=WindowId(2), x=75.00, y=20.00, w=75.00, h=10.00)
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
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |***************************************************************************
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn matches_tiling_master_matcher() {
    let hub = TestHubBuilder::new()
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
    let ws = hub.current_workspace();
    let strategy = hub.strategies.for_workspace(ws);
    assert!(strategy.matches_tiling(ws, process_meta("editor.exe").as_ref()));
    assert!(!strategy.matches_tiling(ws, process_meta("other.exe").as_ref()));
}

#[test]
fn matches_tiling_secondary_matcher() {
    let hub = TestHubBuilder::new()
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
                .build(),
        ])
        .build();
    let ws = hub.current_workspace();
    let strategy = hub.strategies.for_workspace(ws);
    assert!(strategy.matches_tiling(ws, process_meta("terminal.exe").as_ref()));
    assert!(!strategy.matches_tiling(ws, process_meta("editor.exe").as_ref()));
}

#[test]
fn matches_tiling_no_preferred_layout() {
    let hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let ws = hub.current_workspace();
    let strategy = hub.strategies.for_workspace(ws);
    assert!(!strategy.matches_tiling(ws, process_meta("editor.exe").as_ref()));
}
