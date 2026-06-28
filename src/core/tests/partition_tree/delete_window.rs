use crate::core::tests::{setup, snapshot, titled};
use insta::assert_snapshot;

#[test]
fn delete_window_removes_from_container() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w0"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w1"));
    hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.delete_window(w1);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w2])
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
    |                                    W0                                   |*                                    W2                                   *
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
fn delete_window_removes_parent_container() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w3"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w4"));
    hub.delete_window(w1);
    assert_snapshot!(snapshot(&hub), @"
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
fn delete_all_windows() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w5"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w6"));
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w7"));
    hub.delete_window(w0);
    hub.delete_window(w1);
    hub.delete_window(w2);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
}

#[test]
fn delete_all_windows_cleanup_unfocused_workspace() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w8"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w9"));
    hub.focus_workspace("1");
    hub.delete_window(w0);
    hub.delete_window(w1);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
}

#[test]
fn clean_up_parent_container_when_only_child_is_container() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w10"));
    hub.insert_tiling(hub.current_workspace(), titled("w11"));
    // Create new child container
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w12"));
    hub.focus_parent();
    hub.toggle_spawn_mode();
    // Should be inserted in the root container
    let w3 = hub.insert_tiling(hub.current_workspace(), titled("w13"));
    hub.delete_window(w0);
    hub.delete_window(w3);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w11, w12])
      )

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
    ******************************************************************************************************************************************************
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn delete_focused_window_change_focus_to_previous_window() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w14"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w15"));
    hub.insert_tiling(hub.current_workspace(), titled("w16"));
    hub.focus_left();
    hub.delete_window(w1);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w14, w16])
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
    *                                    W0                                   *|                                    W2                                   |
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
fn delete_focused_window_change_focus_to_next_window() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w17"));
    hub.insert_tiling(hub.current_workspace(), titled("w18"));
    hub.insert_tiling(hub.current_workspace(), titled("w19"));
    hub.focus_left();
    hub.focus_left();
    hub.delete_window(w0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w18, w19])
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
    *                                    W1                                   *|                                    W2                                   |
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
fn delete_focused_window_focus_last_window_of_preceding_container() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w20"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w21"));
    hub.focus_parent();
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w22"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w22])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00, titles=[w20, w21])
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
    hub.delete_window(w2);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w20, w21])
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
fn delete_focused_window_focus_following_container_focused_node() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w23"));
    hub.insert_tiling(hub.current_workspace(), titled("w24"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w25"));
    hub.focus_left();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w23, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w24, w25])
      )

    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W1                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *+-------------------------------------------------------------------------+
    *                                    W0                                   *+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W2                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
    hub.delete_window(w0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w24, w25])
      )

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
    ******************************************************************************************************************************************************
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn delete_window_when_parent_focused_gives_focus_to_last_child() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w26"));
    hub.insert_tiling(hub.current_workspace(), titled("w27"));
    hub.focus_parent();
    hub.delete_window(w0);
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
fn container_replaced_by_child_keeps_position_in_parent() {
    let mut hub = setup();
    // Create: [w0] [w1, w2] [w3]
    hub.insert_tiling(hub.current_workspace(), titled("w28"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w29"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w30"));
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w31"));
    hub.delete_window(w1);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w28, w30, w31])
      )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                       W0                       ||                       W2                       |*                       W3                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}

#[test]
fn delete_window_focus_sibling_containers_last_window() {
    let mut hub = setup();

    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w32"));
    hub.insert_tiling(hub.current_workspace(), titled("w33"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w34"));

    // Delete W0, container collapses, should focus W2 (last window of sibling container)
    hub.delete_window(w0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w33, w34])
      )

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
    ******************************************************************************************************************************************************
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn delete_window_focus_sibling_container_if_last_focused_container() {
    let mut hub = setup();

    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w35"));
    hub.insert_tiling(hub.current_workspace(), titled("w36"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w37"));
    hub.focus_parent();

    hub.delete_window(w0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=15.00, w=150.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=bottom, titles=[w36, w37])
      )

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
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn promoted_container_toggles_direction_to_differ_from_grandparent() {
    let mut hub = setup();

    hub.insert_tiling(hub.current_workspace(), titled("w38"));
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w39"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w40"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w41"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=22.50, w=75.00, h=7.50, highlighted, spawn=bottom)
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=7.50)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w38, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w39, Container])
        Container(id=ContainerId(2), x=75.00, y=15.00, w=75.00, h=15.00, titles=[w40, w41])
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
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W1                                   |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");

    hub.delete_window(w1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=15.00, w=75.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(2), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w38, Container])
        Container(id=ContainerId(2), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w40, w41])
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
    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W2                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn promote_child_to_tabbed_grandparent() {
    let mut hub = setup();

    // Create 3 nested containers: tabbed grandparent > split parent > split child
    // tabbed: [W0] [[W1] [W2 W3]]
    hub.insert_tiling_titled();
    let w1 = hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();

    // Make grandparent tabbed
    hub.focus_parent();
    hub.focus_parent();
    hub.focus_parent();
    hub.toggle_container_layout();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=16.00, w=75.00, h=14.00)
        Window(id=WindowId(2), x=0.00, y=16.00, w=75.00, h=14.00)
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=14.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, highlighted, spawn=right, titles=[W0, Container])
        Container(id=ContainerId(1), x=0.00, y=2.00, w=150.00, h=28.00, titles=[W1, Container])
        Container(id=ContainerId(2), x=0.00, y=16.00, w=150.00, h=14.00, titles=[W2, W3])
      )

    ******************************************************************************************************************************************************
    *                                   W0                                     |                              [Container]                                *
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
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
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *-------------------------------------------------------------------------++-------------------------------------------------------------------------*
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                    W2                                   ||                                    W3                                   *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    ******************************************************************************************************************************************************
    ");

    // Delete W1 - parent container cleaned up, child container promoted to tabbed grandparent
    hub.delete_window(w1);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=2.00, w=75.00, h=28.00)
        Window(id=WindowId(2), x=0.00, y=2.00, w=75.00, h=28.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, highlighted, spawn=right, titles=[W0, Container])
        Container(id=ContainerId(2), x=0.00, y=2.00, w=150.00, h=28.00, titles=[W2, W3])
      )

    ******************************************************************************************************************************************************
    *                                   W0                                     |                              [Container]                                *
    *-------------------------------------------------------------------------++-------------------------------------------------------------------------*
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                    W2                                   ||                                    W3                                   *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn promote_child_from_tabbed_parent_to_split_grandparent_does_nothing_when_direction_differ() {
    let mut hub = setup();

    // Create 3 nested containers: split grandparent > tabbed parent > split child
    // [W0] [tabbed: [W1] [W2 W3]]
    hub.insert_tiling_titled();
    let w1 = hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();

    // Make parent tabbed
    hub.focus_parent();
    hub.focus_parent();
    hub.toggle_container_layout();

    // Toggle child direction
    hub.set_focus(w2);
    hub.toggle_direction();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=16.00, w=75.00, h=14.00)
        Window(id=WindowId(2), x=75.00, y=2.00, w=75.00, h=14.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=1, titles=[W1, Container])
        Container(id=ContainerId(2), x=75.00, y=2.00, w=75.00, h=28.00, titles=[W2, W3])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                W1                  |            [Container]             |
    |                                                                         |***************************************************************************
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
    |                                    W0                                   |***************************************************************************
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");

    // Delete W1 - tabbed parent cleaned up, child container promoted to split grandparent
    hub.delete_window(w1);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(2), x=75.00, y=0.00, w=75.00, h=30.00, titles=[W2, W3])
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
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn promote_tabbed_child_to_split_grandparent() {
    let mut hub = setup();

    // Create 3 nested containers: split grandparent > split parent > tabbed child
    // [W0] [[W1] [tabbed: W2 W3]]
    hub.insert_tiling_titled();
    let w1 = hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();

    // Make child tabbed
    hub.focus_parent();
    hub.toggle_container_layout();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=17.00, w=75.00, h=13.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, titles=[W1, Container])
        Container(id=ContainerId(2), x=75.00, y=15.00, w=75.00, h=15.00, tabbed, active_tab=1, highlighted, spawn=right, titles=[W2, W3])
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
    |                                                                         |*                W2                  |               [W3]                 *
    |                                                                         |*-------------------------------------------------------------------------*
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");

    // Delete W1 - parent cleaned up, tabbed child promoted to split grandparent
    hub.delete_window(w1);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=2.00, w=75.00, h=28.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(2), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=1, highlighted, spawn=right, titles=[W2, W3])
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                W2                  |               [W3]                 *
    |                                                                         |*-------------------------------------------------------------------------*
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
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                    W3                                   *
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
fn promote_child_of_tabbed_to_workspace_root_expand_child_to_full_screen() {
    let mut hub = setup();

    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w42"));
    hub.insert_tiling(hub.current_workspace(), titled("w43"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w44"));

    hub.set_focus(w0);
    hub.toggle_container_layout();
    hub.delete_window(w0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w43, w44])
      )

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
    ******************************************************************************************************************************************************
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn delete_window_after_orientation_change() {
    let mut hub = setup();

    hub.insert_tiling(hub.current_workspace(), titled("w45"));
    hub.insert_tiling(hub.current_workspace(), titled("w46"));
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w47"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w45, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w46, w47])
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
    hub.delete_window(w2);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w45, w46])
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
