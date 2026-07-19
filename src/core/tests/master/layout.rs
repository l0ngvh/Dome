use crate::config::{MasterConfig, Strategy, WindowMatcher};
use crate::core::strategy::TilingAction;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, snapshot, titled,
};
use insta::assert_snapshot;

#[test]
fn single_window_layout() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w0"));
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
fn two_windows_default_ratio() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w1"));
    hub.insert_tiling(hub.current_workspace(), titled("w2"));
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
}

#[test]
fn three_windows_layout() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w3"));
    hub.insert_tiling(hub.current_workspace(), titled("w4"));
    hub.insert_tiling(hub.current_workspace(), titled("w5"));
    assert_snapshot!(snapshot(&hub), @"
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
fn focus_direction_left_right() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w6")); // W0 = master
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w7")); // W1 = stack (focused)

    // Focus is on W1 (stack). Move left to master.
    hub.focus_left();
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(w0));

    // Move right back to stack.
    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w1));

    // Right from stack is no-op.
    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w1));

    // Focus master, then left from master is no-op.
    hub.focus_left();
    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w0));
}

#[test]
fn focus_direction_up_down() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w8")); // W0 = master
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w9")); // W1 = stack
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w10")); // W2 = stack (focused)

    let ws = hub.current_workspace();

    // Focus is on W2 (stack index 2). Down wraps to W1 (stack index 1).
    hub.focus_down();
    assert_eq!(hub.focused_window(ws), Some(w1));

    // Down from W1 wraps to W2.
    hub.focus_down();
    assert_eq!(hub.focused_window(ws), Some(w2));

    // Up from W2 goes to W1.
    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(w1));

    // Up from W1 wraps to W2.
    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(w2));
}

#[test]
fn increase_decrease_master_ratio() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w17"));
    hub.insert_tiling(hub.current_workspace(), titled("w18"));

    // Increase ratio: master gets wider
    hub.handle_tiling_action(TilingAction::GrowMaster);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=82.00, h=30.00)
        Window(id=WindowId(1), x=82.00, y=0.00, w=68.00, h=30.00, highlighted)
      )

    +--------------------------------------------------------------------------------+********************************************************************
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                       W0                                       |*                                W1                                *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    |                                                                                |*                                                                  *
    +--------------------------------------------------------------------------------+********************************************************************
    ");

    // Decrease twice to go below default
    hub.handle_tiling_action(TilingAction::ShrinkMaster);
    hub.handle_tiling_action(TilingAction::ShrinkMaster);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=67.00, h=30.00)
        Window(id=WindowId(1), x=67.00, y=0.00, w=83.00, h=30.00, highlighted)
      )

    +-----------------------------------------------------------------+***********************************************************************************
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                W0                               |*                                        W1                                       *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    |                                                                 |*                                                                                 *
    +-----------------------------------------------------------------+***********************************************************************************
    ");

    // Clamp at 0.1: decrease many times
    for _ in 0..20 {
        hub.handle_tiling_action(TilingAction::ShrinkMaster);
    }
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=15.00, h=30.00)
        Window(id=WindowId(1), x=15.00, y=0.00, w=135.00, h=30.00, highlighted)
      )

    +-------------+***************************************************************************************************************************************
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |      W0     |*                                                                  W1                                                                 *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    |             |*                                                                                                                                     *
    +-------------+***************************************************************************************************************************************
    ");

    // Clamp at 0.9: increase many times
    for _ in 0..20 {
        hub.handle_tiling_action(TilingAction::GrowMaster);
    }
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=135.00, h=30.00)
        Window(id=WindowId(1), x=135.00, y=0.00, w=15.00, h=30.00, highlighted)
      )

    +-------------------------------------------------------------------------------------------------------------------------------------+***************
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                  W0                                                                 |*      W1     *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    |                                                                                                                                     |*             *
    +-------------------------------------------------------------------------------------------------------------------------------------+***************
    ");
}

#[test]
fn increment_decrement_master_count() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w19")); // W0
    hub.insert_tiling(hub.current_workspace(), titled("w20")); // W1
    hub.insert_tiling(hub.current_workspace(), titled("w21")); // W2

    // Increment master_count to 2: two masters on left, one stack on right
    hub.handle_tiling_action(TilingAction::MoreMaster);
    assert_snapshot!(snapshot(&hub), @"
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

    // Decrement back to 1
    hub.handle_tiling_action(TilingAction::FewerMaster);
    let after_decrement = snapshot(&hub);
    assert_snapshot!(after_decrement, @"
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

    // Decrement below 1 is no-op
    hub.handle_tiling_action(TilingAction::FewerMaster);
    assert_eq!(snapshot(&hub), after_decrement);
}

#[test]
fn master_count_exceeds_window_count() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w22")); // W0
    hub.insert_tiling(hub.current_workspace(), titled("w23")); // W1
    hub.insert_tiling(hub.current_workspace(), titled("w24")); // W2

    // Set master_count to 5 (exceeds 3 windows): all windows fill screen
    for _ in 0..4 {
        hub.handle_tiling_action(TilingAction::MoreMaster);
    }
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=10.00)
        Window(id=WindowId(2), x=0.00, y=20.00, w=150.00, h=10.00, highlighted)
      )

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
    ");
}

#[test]
fn delete_window() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w25")); // W0 = master
    hub.insert_tiling(hub.current_workspace(), titled("w26")); // W1 = stack
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w27")); // W2 = stack (focused)

    hub.delete_window(w2);
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

    // Delete unfocused from the remaining two
    hub.delete_window(w0);
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
fn more_master_only_affects_focused_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    // Workspace "0": 2 windows.
    hub.insert_tiling(hub.current_workspace(), titled("w55"));
    hub.insert_tiling(hub.current_workspace(), titled("w56"));
    // Switch to workspace "1": 2 windows.
    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w57"));
    hub.insert_tiling(hub.current_workspace(), titled("w58"));
    // MoreMaster on workspace "1".
    hub.handle_tiling_action(TilingAction::MoreMaster);

    // Switch back to workspace "0". Its layout reflects original master_count=1.
    hub.focus_workspace("0");
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
}

#[test]
fn attach_window_falls_back_to_global_when_no_per_workspace_override() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let l = LayoutConfigBuilder::new()
        .with_strategy(Strategy::Master)
        .with_master_config(MasterConfig {
            master_ratio: 0.5,
            master_count: 1,
        })
        .build();
    hub.sync_configuration(l);
    hub.focus_workspace("2");
    hub.insert_tiling(hub.current_workspace(), titled("w63"));
    hub.insert_tiling(hub.current_workspace(), titled("w64"));
    hub.insert_tiling(hub.current_workspace(), titled("w65"));
    assert_snapshot!(snapshot(&hub), @"
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
fn more_master_promotes_unmatched_over_matched() {
    // MoreMaster promotes an unmatched window from stack before touching
    // matched secondary windows.
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
                    title: Some("B".into()),
                    ..Default::default()
                }])
                .build(),
        ])
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("A")); // W0 = master (unmatched)
    hub.insert_tiling(hub.current_workspace(), titled("B")); // W1 = stack (matched secondary)
    hub.insert_tiling(hub.current_workspace(), titled("C")); // W2 = stack (unmatched, focused)

    hub.handle_tiling_action(TilingAction::MoreMaster);

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
fn more_master_noop_when_no_unmatched_in_stack() {
    // MoreMaster does not move windows when all stack windows are matched.
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
                        title: Some("B".into()),
                        ..Default::default()
                    },
                    WindowMatcher {
                        title: Some("C".into()),
                        ..Default::default()
                    },
                ])
                .build(),
        ])
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("A")); // W0 = master
    hub.insert_tiling(hub.current_workspace(), titled("B")); // W1 = stack (matched)
    hub.insert_tiling(hub.current_workspace(), titled("C")); // W2 = stack (matched, focused)

    hub.handle_tiling_action(TilingAction::MoreMaster);

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
fn fewer_master_demotes_last_unmatched() {
    // FewerMaster demotes the last master window (which is unmatched) to stack.
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
                    title: Some("A".into()),
                    ..Default::default()
                }])
                .with_master_count(2)
                .build(),
        ])
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("B")); // W0 = master (unmatched)
    hub.insert_tiling(hub.current_workspace(), titled("A")); // W1 = master (matched, focused)
    hub.insert_tiling(hub.current_workspace(), titled("C")); // W2 = stack

    hub.handle_tiling_action(TilingAction::FewerMaster);

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
