use crate::config::{MasterConfig, Strategy, WindowMatcher};
use crate::core::WindowRestrictions;
use crate::core::strategy::TilingAction;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, default_rect, snapshot,
    titled,
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
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
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
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
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
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w4"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w5"), default_rect(), WindowRestrictions::None);
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
fn focus_direction_up_down() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_window(titled("w8"), default_rect(), WindowRestrictions::None); // W0 = master
    let w1 = hub
        .insert_window(titled("w9"), default_rect(), WindowRestrictions::None)
        .unwrap(); // W1 = stack
    let w2 = hub
        .insert_window(titled("w10"), default_rect(), WindowRestrictions::None)
        .unwrap(); // W2 = stack (focused)

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
    hub.insert_window(titled("w17"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w18"), default_rect(), WindowRestrictions::None);

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
    hub.insert_window(titled("w19"), default_rect(), WindowRestrictions::None); // W0
    hub.insert_window(titled("w20"), default_rect(), WindowRestrictions::None); // W1
    hub.insert_window(titled("w21"), default_rect(), WindowRestrictions::None); // W2

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
    hub.insert_window(titled("w22"), default_rect(), WindowRestrictions::None); // W0
    hub.insert_window(titled("w23"), default_rect(), WindowRestrictions::None); // W1
    hub.insert_window(titled("w24"), default_rect(), WindowRestrictions::None); // W2

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
fn more_master_only_affects_focused_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    // Workspace "0": 2 windows.
    hub.insert_window(titled("w55"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w56"), default_rect(), WindowRestrictions::None);
    // Switch to workspace "1": 2 windows.
    hub.focus_workspace("1", None);
    hub.insert_window(titled("w57"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w58"), default_rect(), WindowRestrictions::None);
    // MoreMaster on workspace "1".
    hub.handle_tiling_action(TilingAction::MoreMaster);

    // Switch back to workspace "0". Its layout reflects original master_count=1.
    hub.focus_workspace("0", None);
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
    hub.focus_workspace("2", None);
    hub.insert_window(titled("w63"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w64"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w65"), default_rect(), WindowRestrictions::None);
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
    hub.insert_window(titled("A"), default_rect(), WindowRestrictions::None); // W0 = master (unmatched)
    hub.insert_window(titled("B"), default_rect(), WindowRestrictions::None); // W1 = stack (matched secondary)
    hub.insert_window(titled("C"), default_rect(), WindowRestrictions::None); // W2 = stack (unmatched, focused)

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
    hub.insert_window(titled("A"), default_rect(), WindowRestrictions::None); // W0 = master
    hub.insert_window(titled("B"), default_rect(), WindowRestrictions::None); // W1 = stack (matched)
    hub.insert_window(titled("C"), default_rect(), WindowRestrictions::None); // W2 = stack (matched, focused)

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
    hub.insert_window(titled("B"), default_rect(), WindowRestrictions::None); // W0 = master (unmatched)
    hub.insert_window(titled("A"), default_rect(), WindowRestrictions::None); // W1 = master (matched, focused)
    hub.insert_window(titled("C"), default_rect(), WindowRestrictions::None); // W2 = stack

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
