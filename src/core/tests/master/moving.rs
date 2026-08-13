use crate::config::{Strategy, WindowMatcher};
use crate::core::node::{Dimension, Length};
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, default_dim,
    setup_logger_with_level, snapshot, titled, titled_matcher, titled_process,
};
use crate::core::{Hub, MonitorLayout, WindowId, WindowRestrictions};
use insta::assert_snapshot;

#[test]
fn swap_secondary_and_master() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_window(titled("w11"), default_dim(), WindowRestrictions::None); // W0 = master
    hub.insert_window(titled("w12"), default_dim(), WindowRestrictions::None); // W1 = stack (focused)

    // Move W1 left: swaps with last master (W0). W1 becomes master, W0 becomes stack.
    hub.move_left();
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
fn move_direction_up_down() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_window(titled("w13"), default_dim(), WindowRestrictions::None); // W0 = master
    hub.insert_window(titled("w14"), default_dim(), WindowRestrictions::None); // W1 = stack
    hub.insert_window(titled("w15"), default_dim(), WindowRestrictions::None); // W2 = stack (focused)

    // Move W2 up within stack: swap with W1.
    hub.move_up();
    assert_snapshot!(snapshot(&hub), @"
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
fn move_direction_up_down_wraps_within_three_window_pane() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master_count(2)
                .build(),
        ])
        .build();
    let _w0 = hub
        .insert_window(titled("w0"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("w1"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w2"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w3"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w4"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();

    // Three windows in the secondary pane. On a two-window pane both vertical directions resolve
    // to the same index, so the two moves could not be told apart.
    hub.set_focus(w3);
    assert_eq!(top_to_bottom(&hub, &[w2, w3, w4]), vec![w2, w3, w4]);

    hub.move_down();
    assert_eq!(hub.focused_window(ws), Some(w3));
    assert_eq!(top_to_bottom(&hub, &[w2, w3, w4]), vec![w2, w4, w3]);

    hub.move_up();
    assert_eq!(hub.focused_window(ws), Some(w3));
    assert_eq!(top_to_bottom(&hub, &[w2, w3, w4]), vec![w2, w3, w4]);

    hub.move_up();
    assert_eq!(hub.focused_window(ws), Some(w3));
    assert_eq!(top_to_bottom(&hub, &[w2, w3, w4]), vec![w3, w2, w4]);
}

#[test]
fn focus_and_move_noop() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_window(titled("w16"), default_dim(), WindowRestrictions::None);

    let before = snapshot(&hub);

    hub.focus_left();
    hub.focus_right();
    hub.focus_up();
    hub.focus_down();
    hub.move_left();
    hub.move_right();
    hub.move_up();
    hub.move_down();

    assert_eq!(snapshot(&hub), before);

    // Nothing renders while the float holds focus, though master does hoist
    // focus_history underneath.
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .with_float(vec![titled_matcher("w17")])
                .build(),
        )
        .build();
    hub.insert_window(titled("w18"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w19"), default_dim(), WindowRestrictions::None);
    let float_id = hub
        .insert_window(
            titled("w17"),
            Dimension::new(
                Length::new(50.0),
                Length::new(5.0),
                Length::new(40.0),
                Length::new(15.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
    let ws = hub.current_workspace();

    let before = snapshot(&hub);

    hub.focus_left();
    hub.focus_right();
    hub.focus_up();
    hub.focus_down();

    assert_eq!(snapshot(&hub), before);
    assert_eq!(hub.focused_window(ws), Some(float_id));
}

#[test]
fn move_window_to_workspace() {
    // Move master to another workspace
    {
        let mut hub = TestHubBuilder::new()
            .with_layout(
                LayoutConfigBuilder::new()
                    .with_strategy(Strategy::Master)
                    .build(),
            )
            .build();
        hub.insert_window(titled("w28"), default_dim(), WindowRestrictions::None); // W0 = master
        hub.insert_window(titled("w29"), default_dim(), WindowRestrictions::None); // W1 = stack (focused)
        hub.focus_left();
        hub.move_focused_to_workspace("1");
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

        hub.focus_workspace("1");
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

    // Move stack window to another workspace
    {
        let mut hub = TestHubBuilder::new()
            .with_layout(
                LayoutConfigBuilder::new()
                    .with_strategy(Strategy::Master)
                    .build(),
            )
            .build();
        hub.insert_window(titled("w30"), default_dim(), WindowRestrictions::None); // W0 = master
        hub.insert_window(titled("w31"), default_dim(), WindowRestrictions::None); // W1 = stack
        hub.insert_window(titled("w32"), default_dim(), WindowRestrictions::None); // W2 = stack (focused)
        hub.move_focused_to_workspace("1");
        assert_snapshot!(snapshot(&hub), @"
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

        hub.focus_workspace("1");
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
}

#[test]
fn move_only_window_to_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_window(titled("w33"), default_dim(), WindowRestrictions::None); // W0

    hub.move_focused_to_workspace("1");

    // Source workspace: empty
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");

    hub.focus_workspace("1");
    // Target workspace: W0 fills screen
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
fn promote_secondary_to_master_when_there_is_room() {
    setup_logger_with_level("trace");
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_secondary(vec![
                    WindowMatcher {
                        title: Some("B".to_string()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        title: Some("C".to_string()),
                        ..Default::default()
                    },
                ])
                .with_master_count(3)
                .build(),
        ])
        .build();
    let _w0 = hub
        .insert_window(titled("A"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("B"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("C"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.move_left();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=0.00, y=15.00, w=75.00, h=15.00, highlighted)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
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
    +-------------------------------------------------------------------------+|                                                                         |
    ***************************************************************************|                                    W1                                   |
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
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn move_matched_master_to_secondary_rematches() {
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
                        process: Some("browser.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("editor.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("code.exe".into()),
                        ..Default::default()
                    },
                ])
                .with_master_count(1)
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
    let w1 = hub
        .insert_window(
            titled_process("Editor", "editor.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w2 = hub
        .insert_window(
            titled_process("Browser", "browser.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w3 = hub
        .insert_window(
            titled_process("Code", "code.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();

    hub.set_focus(w1);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00, highlighted)
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(2), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=20.00, w=75.00, h=10.00)
      )

    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W0                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W1                                   *|                                    W2                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W3                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");

    hub.move_right();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=75.00, y=10.00, w=75.00, h=10.00, highlighted)
        Window(id=WindowId(3), x=75.00, y=20.00, w=75.00, h=10.00)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |***************************************************************************
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn move_matched_secondary_to_master_rematches() {
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
                        process: Some("editor.exe".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        process: Some("code.exe".into()),
                        ..Default::default()
                    },
                ])
                .with_secondary(vec![WindowMatcher {
                    process: Some("editor.exe".into()),
                    ..Default::default()
                }])
                .with_master_count(3)
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
    let w1 = hub
        .insert_window(
            titled_process("Editor", "editor.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w2 = hub
        .insert_window(
            titled_process("Browser", "browser.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w3 = hub
        .insert_window(
            titled_process("Code", "code.exe"),
            default_dim(),
            WindowRestrictions::None,
        )
        .unwrap();

    hub.set_focus(w1);
    hub.move_right();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=0.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=20.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W2                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W3                                   |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");

    hub.move_left();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=75.00, h=10.00, highlighted)
        Window(id=WindowId(3), x=0.00, y=20.00, w=75.00, h=10.00)
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=30.00)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W2                                   ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    ***************************************************************************|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W1                                   *|                                    W0                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************|                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W3                                   ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

/// `ids` sorted by on-screen y, which is the only observable that exposes pane order.
fn top_to_bottom(hub: &Hub, ids: &[WindowId]) -> Vec<WindowId> {
    let placements = hub.get_visible_placements();
    let mut found: Vec<(i32, WindowId)> = Vec::new();
    for monitor in &placements.monitors {
        let MonitorLayout::Normal { tiling_windows, .. } = &monitor.layout else {
            continue;
        };
        for placement in tiling_windows {
            if ids.contains(&placement.id) {
                found.push((placement.visible_border_box.y(), placement.id));
            }
        }
    }
    assert_eq!(found.len(), ids.len(), "every id must have a placement");
    found.sort_by_key(|&(y, _)| y);
    found.into_iter().map(|(_, id)| id).collect()
}
