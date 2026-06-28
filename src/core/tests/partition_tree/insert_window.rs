use crate::core::tests::{setup, snapshot, titled};
use insta::assert_snapshot;

#[test]
fn initial_window_cover_full_screen() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w0"));
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
fn split_window_evenly() {
    let mut hub = setup();
    for i in 0..4 {
        hub.insert_tiling(hub.current_workspace(), titled(format!("w{i}").as_str()));
    }
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=112.50, y=0.00, w=37.50, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=0.00, w=37.50, h=30.00)
        Window(id=WindowId(1), x=37.50, y=0.00, w=37.50, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=37.50, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1, w2, w3])
      )

    +------------------------------------++-----------------------------------++------------------------------------+*************************************
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                 W0                 ||                W1                 ||                 W2                 |*                W3                 *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    +------------------------------------++-----------------------------------++------------------------------------+*************************************
    ");
}

#[test]
fn new_container_preserves_wrapped_window_position() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.insert_tiling(hub.current_workspace(), titled("w3"));
    hub.insert_tiling(hub.current_workspace(), titled("w4"));
    // Focus w1 (middle)
    hub.focus_left();
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w5"));
    // New container wrapping w1 should be in the middle position
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(3), x=50.00, y=15.00, w=50.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w2, Container, w4])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=50.00, h=30.00, titles=[w3, w5])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                       W1                       ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                       W0                       |**************************************************|                       W2                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                       W3                       *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn insert_window_after_focused_window() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w6"));
    hub.insert_tiling(hub.current_workspace(), titled("w7"));
    hub.insert_tiling(hub.current_workspace(), titled("w8"));
    // Focus w1 (middle)
    hub.focus_left();
    hub.insert_tiling(hub.current_workspace(), titled("w9"));
    // w3 should be inserted right after w1, not at the end
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=112.50, y=0.00, w=37.50, h=30.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=37.50, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=37.50, y=0.00, w=37.50, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=37.50, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w6, w7, w9, w8])
      )

    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                 W0                 ||                W1                 |*                 W3                 *|                W2                 |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    ");
}

#[test]
fn insert_window_after_focused_container_with_same_new_window_direction() {
    let mut hub = setup();
    // Create: [w0] [w1, w2] [w3]
    hub.insert_tiling(hub.current_workspace(), titled("w10"));
    hub.insert_tiling(hub.current_workspace(), titled("w11"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w12"));
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w13"));
    // Focus the middle container and toggle back spawn direction
    hub.focus_left();
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w14"));
    // w4 should be inserted right after the focused container, not at the end
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(4), x=50.00, y=20.00, w=50.00, h=10.00, highlighted, spawn=bottom)
        Window(id=WindowId(2), x=50.00, y=10.00, w=50.00, h=10.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w10, Container, w13])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=50.00, h=30.00, titles=[w11, w12, w14])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                       W1                       ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W0                       ||                       W2                       ||                       W3                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                |**************************************************|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                       W4                       *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn insert_to_new_container_when_focused_container_window_insert_direction_differ_and_no_parent() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w15"));
    hub.insert_tiling(hub.current_workspace(), titled("w16"));
    hub.insert_tiling(hub.current_workspace(), titled("w17"));
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w18"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w18])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=15.00, titles=[w15, w16, w17])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W0                       ||                       W1                       ||                       W2                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W3                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn insert_to_parent_when_focused_container_window_insert_direction_differ_but_has_parent() {
    let mut hub = setup();
    // Creating [w0, [w1, w2], w3]
    hub.insert_tiling(hub.current_workspace(), titled("w19"));
    hub.insert_tiling(hub.current_workspace(), titled("w20"));
    hub.insert_tiling(hub.current_workspace(), titled("w21"));
    hub.focus_left();
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w22"));
    hub.focus_parent();
    hub.toggle_spawn_mode();
    // Should be inserted in the root container
    hub.insert_tiling(hub.current_workspace(), titled("w23"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=112.50, y=0.00, w=37.50, h=30.00)
        Window(id=WindowId(4), x=75.00, y=0.00, w=37.50, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=37.50, y=15.00, w=37.50, h=15.00)
        Window(id=WindowId(1), x=37.50, y=0.00, w=37.50, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=37.50, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w19, Container, w23, w21])
        Container(id=ContainerId(1), x=37.50, y=0.00, w=37.50, h=30.00, titles=[w20, w22])
      )

    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                W1                 |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    |+-----------------------------------+*                                    *|                                   |
    |                 W0                 |+-----------------------------------+*                 W4                 *|                W2                 |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                W3                 |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    ");
}

// TODO: test unfocus then insert new window

#[test]
fn insert_window_after_focusing_parent() {
    let mut hub = setup();

    hub.insert_tiling(hub.current_workspace(), titled("w24"));
    hub.insert_tiling(hub.current_workspace(), titled("w25"));
    hub.focus_parent();
    hub.insert_tiling(hub.current_workspace(), titled("w26"));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w24, w25, w26])
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
    |                       W0                       ||                       W1                       |*                       W2                       *
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
