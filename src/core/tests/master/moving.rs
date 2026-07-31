use crate::config::{Strategy, WindowMatcher};
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, TestMetadata,
    setup_logger_with_level, snapshot, titled,
};
use crate::core::{Hub, MonitorLayout, WindowId, WindowMetadata};
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
    hub.insert_tiling(hub.current_workspace(), titled("w11")); // W0 = master
    hub.insert_tiling(hub.current_workspace(), titled("w12")); // W1 = stack (focused)

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
    hub.insert_tiling(hub.current_workspace(), titled("w13")); // W0 = master
    hub.insert_tiling(hub.current_workspace(), titled("w14")); // W1 = stack
    hub.insert_tiling(hub.current_workspace(), titled("w15")); // W2 = stack (focused)

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
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("w1"));
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w2"));
    let w3 = hub.insert_tiling(hub.current_workspace(), titled("w3"));
    let w4 = hub.insert_tiling(hub.current_workspace(), titled("w4"));
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
fn single_window_focus_move_noop() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w16"));

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
        hub.insert_tiling(hub.current_workspace(), titled("w28")); // W0 = master
        hub.insert_tiling(hub.current_workspace(), titled("w29")); // W1 = stack (focused)
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
        hub.insert_tiling(hub.current_workspace(), titled("w30")); // W0 = master
        hub.insert_tiling(hub.current_workspace(), titled("w31")); // W1 = stack
        hub.insert_tiling(hub.current_workspace(), titled("w32")); // W2 = stack (focused)
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
    hub.insert_tiling(hub.current_workspace(), titled("w33")); // W0

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
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("A"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("B"));
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled("C"));
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

    let _w0 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Filler", "other.exe"),
    );
    let w1 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Editor", "editor.exe"),
    );
    let _w2 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Browser", "browser.exe"),
    );
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled_process("Code", "code.exe"));

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

    let _w0 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Filler", "other.exe"),
    );
    let w1 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Editor", "editor.exe"),
    );
    let _w2 = hub.insert_tiling(
        hub.current_workspace(),
        titled_process("Browser", "browser.exe"),
    );
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled_process("Code", "code.exe"));

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
    let mut found: Vec<(f32, WindowId)> = Vec::new();
    for monitor in &placements.monitors {
        let MonitorLayout::Normal { tiling_windows, .. } = &monitor.layout else {
            continue;
        };
        for placement in tiling_windows {
            if ids.contains(&placement.id) {
                found.push((placement.visible_frame.y.value(), placement.id));
            }
        }
    }
    assert_eq!(found.len(), ids.len(), "every id must have a placement");
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, id)| id).collect()
}

fn titled_process(title: &str, process: &str) -> Box<dyn WindowMetadata> {
    Box::new(TestMetadata {
        title: Some(title.into()),
        process: Some(process.into()),
    })
}
