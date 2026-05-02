use super::{snapshot, validate_hub};
use crate::core::allocator::NodeId;
use crate::core::hub::{Hub, HubConfig};
use crate::core::master_stack::MasterStackStrategy;
use crate::core::node::{Dimension, WindowId};
use crate::core::strategy::TilingAction;
use insta::assert_snapshot;

fn setup_master_stack() -> Hub {
    Hub::new_with_strategy(
        Dimension {
            x: 0.0,
            y: 0.0,
            width: 150.0,
            height: 30.0,
        },
        1.0,
        HubConfig::default(),
        Box::new(MasterStackStrategy::new()),
    )
}

#[test]
fn single_window_layout() {
    let mut hub = setup_master_stack();
    hub.insert_tiling();
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
    let mut hub = setup_master_stack();
    hub.insert_tiling();
    hub.insert_tiling();
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
    let mut hub = setup_master_stack();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
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
    let mut hub = setup_master_stack();
    hub.insert_tiling(); // W0 = master
    hub.insert_tiling(); // W1 = stack (focused)

    // Focus is on W1 (stack). Move left to master.
    hub.focus_left();
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_tiling_window(ws), Some(WindowId::new(0)));

    // Move right back to stack.
    hub.focus_right();
    assert_eq!(hub.focused_tiling_window(ws), Some(WindowId::new(1)));

    // Right from stack is no-op.
    hub.focus_right();
    assert_eq!(hub.focused_tiling_window(ws), Some(WindowId::new(1)));

    // Focus master, then left from master is no-op.
    hub.focus_left();
    hub.focus_left();
    assert_eq!(hub.focused_tiling_window(ws), Some(WindowId::new(0)));
}

#[test]
fn focus_direction_up_down() {
    let mut hub = setup_master_stack();
    hub.insert_tiling(); // W0 = master
    hub.insert_tiling(); // W1 = stack
    hub.insert_tiling(); // W2 = stack (focused)

    let ws = hub.current_workspace();

    // Focus is on W2 (stack index 2). Down wraps to W1 (stack index 1).
    hub.focus_down();
    assert_eq!(hub.focused_tiling_window(ws), Some(WindowId::new(1)));

    // Down from W1 wraps to W2.
    hub.focus_down();
    assert_eq!(hub.focused_tiling_window(ws), Some(WindowId::new(2)));

    // Up from W2 goes to W1.
    hub.focus_up();
    assert_eq!(hub.focused_tiling_window(ws), Some(WindowId::new(1)));

    // Up from W1 wraps to W2.
    hub.focus_up();
    assert_eq!(hub.focused_tiling_window(ws), Some(WindowId::new(2)));
}

#[test]
fn move_direction_left_right() {
    let mut hub = setup_master_stack();
    hub.insert_tiling(); // W0 = master
    hub.insert_tiling(); // W1 = stack (focused)

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
    let mut hub = setup_master_stack();
    hub.insert_tiling(); // W0 = master
    hub.insert_tiling(); // W1 = stack
    hub.insert_tiling(); // W2 = stack (focused)

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
    let mut hub = setup_master_stack();
    hub.insert_tiling();

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
    let mut hub = setup_master_stack();
    hub.insert_tiling();
    hub.insert_tiling();

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
    let mut hub = setup_master_stack();
    hub.insert_tiling(); // W0
    hub.insert_tiling(); // W1
    hub.insert_tiling(); // W2

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

    // Decrement below 1 is no-op
    hub.handle_tiling_action(TilingAction::FewerMaster);
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
fn master_count_exceeds_window_count() {
    let mut hub = setup_master_stack();
    hub.insert_tiling(); // W0
    hub.insert_tiling(); // W1
    hub.insert_tiling(); // W2

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
    let mut hub = setup_master_stack();
    hub.insert_tiling(); // W0 = master
    hub.insert_tiling(); // W1 = stack
    hub.insert_tiling(); // W2 = stack (focused)

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
        let mut hub = setup_master_stack();
        hub.insert_tiling(); // W0 = master
        hub.insert_tiling(); // W1 = stack (focused)
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
        let mut hub = setup_master_stack();
        hub.insert_tiling(); // W0 = master
        hub.insert_tiling(); // W1 = stack
        hub.insert_tiling(); // W2 = stack (focused)
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
    let mut hub = setup_master_stack();
    hub.insert_tiling(); // W0

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
fn prune_workspace() {
    let mut hub = setup_master_stack();
    hub.insert_tiling(); // W0
    hub.insert_tiling(); // W1

    // Move both windows to workspace "1"
    hub.move_focused_to_workspace("1");
    hub.focus_left(); // focus W0 (now the only window on ws "0")
    hub.move_focused_to_workspace("1");

    // ws "0" is empty but still active. Switch to "1" to trigger prune of "0".
    hub.focus_workspace("1");

    // ws "0" should have been pruned (empty and no longer active)
    let workspaces = hub.all_workspaces();
    assert_eq!(workspaces.len(), 1);
    validate_hub(&hub);
}
