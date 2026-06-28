use super::setup_master;
use crate::config::{LayoutConfig, MasterConfig, MasterWorkspaceConfig, Strategy};
use crate::core::allocator::NodeId;
use crate::core::node::WindowId;
use crate::core::strategy::TilingAction;
use crate::core::tests::default_layout_for_tests;
use crate::core::tests::default_partition_tree_config_for_tests;
use crate::core::tests::{snapshot, titled, validate_hub};
use insta::assert_snapshot;

#[test]
fn single_window_layout() {
    let mut hub = setup_master();
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
    let mut hub = setup_master();
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
    let mut hub = setup_master();
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
    let mut hub = setup_master();
    hub.insert_tiling(hub.current_workspace(), titled("w6")); // W0 = master
    hub.insert_tiling(hub.current_workspace(), titled("w7")); // W1 = stack (focused)

    // Focus is on W1 (stack). Move left to master.
    hub.focus_left();
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(WindowId::new(0)));

    // Move right back to stack.
    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(WindowId::new(1)));

    // Right from stack is no-op.
    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(WindowId::new(1)));

    // Focus master, then left from master is no-op.
    hub.focus_left();
    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(WindowId::new(0)));
}

#[test]
fn focus_direction_up_down() {
    let mut hub = setup_master();
    hub.insert_tiling(hub.current_workspace(), titled("w8")); // W0 = master
    hub.insert_tiling(hub.current_workspace(), titled("w9")); // W1 = stack
    hub.insert_tiling(hub.current_workspace(), titled("w10")); // W2 = stack (focused)

    let ws = hub.current_workspace();

    // Focus is on W2 (stack index 2). Down wraps to W1 (stack index 1).
    hub.focus_down();
    assert_eq!(hub.focused_window(ws), Some(WindowId::new(1)));

    // Down from W1 wraps to W2.
    hub.focus_down();
    assert_eq!(hub.focused_window(ws), Some(WindowId::new(2)));

    // Up from W2 goes to W1.
    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(WindowId::new(1)));

    // Up from W1 wraps to W2.
    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(WindowId::new(2)));
}

#[test]
fn move_direction_left_right() {
    let mut hub = setup_master();
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
    let mut hub = setup_master();
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
fn single_window_focus_move_noop() {
    let mut hub = setup_master();
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
fn increase_decrease_master_ratio() {
    let mut hub = setup_master();
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
    let mut hub = setup_master();
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
    let mut hub = setup_master();
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
    let mut hub = setup_master();
    hub.insert_tiling(hub.current_workspace(), titled("w25")); // W0 = master
    hub.insert_tiling(hub.current_workspace(), titled("w26")); // W1 = stack
    hub.insert_tiling(hub.current_workspace(), titled("w27")); // W2 = stack (focused)

    // Delete focused (last inserted)
    hub.delete_window(WindowId::new(2));
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
    hub.delete_window(WindowId::new(0));
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
fn move_window_to_workspace() {
    // Move master to another workspace
    {
        let mut hub = setup_master();
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
        let mut hub = setup_master();
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
    let mut hub = setup_master();
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
fn empty_workspace_persists_after_switch() {
    let mut hub = setup_master();
    hub.insert_tiling(hub.current_workspace(), titled("w34")); // W0
    hub.insert_tiling(hub.current_workspace(), titled("w35")); // W1

    // Move both windows to workspace "1"
    hub.move_focused_to_workspace("1");
    hub.focus_left(); // focus W0 (now the only window on ws "0")
    hub.move_focused_to_workspace("1");

    // ws "0" is empty but still active. Switch to "1".
    hub.focus_workspace("1");

    // Both workspaces remain because workspaces persist for the lifetime of the Hub.
    let workspaces = hub.query_workspaces();
    assert_eq!(workspaces.len(), 2);
    validate_hub(&hub);
}

#[test]
fn sync_config_preserves_workspace_master_count() {
    // Per-workspace master_count persists across config reload. The workspace
    // was seeded with master_count=1 and a reload with master_count=2 does
    // NOT push into the existing workspace's state.
    let mut hub = setup_master();
    hub.insert_tiling(hub.current_workspace(), titled("w36"));
    hub.insert_tiling(hub.current_workspace(), titled("w37"));
    hub.insert_tiling(hub.current_workspace(), titled("w38"));
    hub.insert_tiling(hub.current_workspace(), titled("w39"));
    hub.insert_tiling(hub.current_workspace(), titled("w40"));

    let ws = hub.current_workspace();
    let focus_before = hub.focused_window(ws);

    hub.sync_config(LayoutConfig {
        strategy: Strategy::Master,
        partition_tree: default_partition_tree_config_for_tests(),
        master: MasterConfig {
            master_ratio: 0.5,
            master_count: 2,
            workspace: vec![],
        },
        ..default_layout_for_tests()
    });

    // Window ordering and focus preserved (no rebuild, apply_config path).
    assert_eq!(hub.focused_window(ws), focus_before);
    // Layout still reflects original master_count=1 (preserve semantics).
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=7.50)
        Window(id=WindowId(2), x=75.00, y=7.50, w=75.00, h=7.50)
        Window(id=WindowId(3), x=75.00, y=15.00, w=75.00, h=7.50)
        Window(id=WindowId(4), x=75.00, y=22.50, w=75.00, h=7.50, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W0                                   |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W4                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn sync_config_seeds_new_workspace_with_master_ratio() {
    // Config values seed new workspaces via attach_window. After a reload with
    // master_ratio=0.3, a previously-untouched workspace gets that ratio on
    // its first attach.
    let mut hub = setup_master();

    hub.sync_config(LayoutConfig {
        strategy: Strategy::Master,
        partition_tree: default_partition_tree_config_for_tests(),
        master: MasterConfig {
            master_ratio: 0.3,
            master_count: 1,
            workspace: vec![],
        },
        ..default_layout_for_tests()
    });

    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w41"));
    hub.insert_tiling(hub.current_workspace(), titled("w42"));
    hub.insert_tiling(hub.current_workspace(), titled("w43"));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=45.00, h=30.00)
        Window(id=WindowId(1), x=45.00, y=0.00, w=105.00, h=15.00)
        Window(id=WindowId(2), x=45.00, y=15.00, w=105.00, h=15.00, highlighted)
      )

    +-------------------------------------------++-------------------------------------------------------------------------------------------------------+
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                   W1                                                  |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           |+-------------------------------------------------------------------------------------------------------+
    |                     W0                    |*********************************************************************************************************
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                   W2                                                  *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    +-------------------------------------------+*********************************************************************************************************
    ");
}

#[test]
fn sync_config_preserves_seeded_ratio_across_workspaces() {
    // Two workspaces (3+2 windows) seeded with ratio 0.5. A config reload
    // pushing ratio 0.4 does NOT override the seeded value (preserve
    // semantics). Ordering and focus survive on both workspaces.
    let mut hub = setup_master();

    // Workspace "0": insert 3 windows (ids allocated in order).
    hub.insert_tiling(hub.current_workspace(), titled("w44"));
    hub.insert_tiling(hub.current_workspace(), titled("w45"));
    hub.insert_tiling(hub.current_workspace(), titled("w46"));
    let ws0 = hub.current_workspace();
    let focus_ws0 = hub.focused_window(ws0);

    // Switch to workspace "1" and insert 2 windows.
    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w47"));
    hub.insert_tiling(hub.current_workspace(), titled("w48"));
    let ws1 = hub.current_workspace();
    let focus_ws1 = hub.focused_window(ws1);

    // Go back to workspace "0" for the snapshot.
    hub.focus_workspace("0");

    // Change master_ratio from 0.5 to 0.4 via hot-reload.
    hub.sync_config(LayoutConfig {
        strategy: Strategy::Master,
        partition_tree: default_partition_tree_config_for_tests(),
        master: MasterConfig {
            master_ratio: 0.4,
            master_count: 1,
            workspace: vec![],
        },
        ..default_layout_for_tests()
    });

    // Both workspaces: ordering and focus preserved.
    assert_eq!(hub.focused_window(ws0), focus_ws0);
    assert_eq!(hub.focused_window(ws1), focus_ws1);
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
fn sync_config_preserves_runtime_tuned_master_ratio() {
    // Runtime GrowMaster tuning persists across config reload. A hot-reload
    // does NOT reset the ratio back to the file value (preserve semantics).
    let mut hub = setup_master();
    hub.insert_tiling(hub.current_workspace(), titled("w49"));
    hub.insert_tiling(hub.current_workspace(), titled("w50"));

    // GrowMaster 3 times: 0.5 -> 0.55 -> 0.60 -> 0.65
    hub.handle_tiling_action(TilingAction::GrowMaster);
    hub.handle_tiling_action(TilingAction::GrowMaster);
    hub.handle_tiling_action(TilingAction::GrowMaster);

    // Hot-reload with a different file value does NOT override runtime tuning.
    hub.sync_config(LayoutConfig {
        strategy: Strategy::Master,
        partition_tree: default_partition_tree_config_for_tests(),
        master: MasterConfig {
            master_ratio: 0.4,
            master_count: 1,
            workspace: vec![],
        },
        ..default_layout_for_tests()
    });

    // Layout still shows runtime-tuned 0.65 ratio.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=97.00, h=30.00)
        Window(id=WindowId(1), x=97.00, y=0.00, w=53.00, h=30.00, highlighted)
      )

    +-----------------------------------------------------------------------------------------------+*****************************************************
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                               W0                                              |*                         W1                        *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    +-----------------------------------------------------------------------------------------------+*****************************************************
    ");
}

#[test]
fn sync_config_preserves_runtime_tuned_master_count() {
    // Runtime MoreMaster tuning persists across config reload. A hot-reload
    // does NOT reset the count back to the file value (preserve semantics).
    let mut hub = setup_master();
    hub.insert_tiling(hub.current_workspace(), titled("w51"));
    hub.insert_tiling(hub.current_workspace(), titled("w52"));
    hub.insert_tiling(hub.current_workspace(), titled("w53"));
    hub.insert_tiling(hub.current_workspace(), titled("w54"));

    // MoreMaster: master_count 1 -> 2
    hub.handle_tiling_action(TilingAction::MoreMaster);

    // Hot-reload with a different file value does NOT override runtime tuning.
    hub.sync_config(LayoutConfig {
        strategy: Strategy::Master,
        partition_tree: default_partition_tree_config_for_tests(),
        master: MasterConfig {
            master_ratio: 0.5,
            master_count: 3,
            workspace: vec![],
        },
        ..default_layout_for_tests()
    });

    // Layout still shows runtime-tuned master_count=2 (not config's 3).
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(3), x=75.00, y=15.00, w=75.00, h=15.00, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn more_master_only_affects_focused_workspace() {
    let mut hub = setup_master();
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
fn attach_window_seeds_master_count_from_per_workspace_override() {
    let mut hub = setup_master();
    hub.sync_config(LayoutConfig {
        strategy: Strategy::Master,
        partition_tree: default_partition_tree_config_for_tests(),
        master: MasterConfig {
            master_ratio: 0.5,
            master_count: 1,
            workspace: vec![MasterWorkspaceConfig {
                name: "1".to_string(),
                master_count: Some(3),
                master_ratio: None,
            }],
        },
        ..default_layout_for_tests()
    });
    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w59"));
    hub.insert_tiling(hub.current_workspace(), titled("w60"));
    hub.insert_tiling(hub.current_workspace(), titled("w61"));
    hub.insert_tiling(hub.current_workspace(), titled("w62"));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(2), x=0.00, y=20.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W2                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn attach_window_falls_back_to_global_when_no_per_workspace_override() {
    let mut hub = setup_master();
    hub.sync_config(LayoutConfig {
        strategy: Strategy::Master,
        partition_tree: default_partition_tree_config_for_tests(),
        master: MasterConfig {
            master_ratio: 0.5,
            master_count: 1,
            workspace: vec![MasterWorkspaceConfig {
                name: "1".to_string(),
                master_count: Some(3),
                master_ratio: None,
            }],
        },
        ..default_layout_for_tests()
    });
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
