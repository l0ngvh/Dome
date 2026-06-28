use crate::core::tests::{setup_with_automatic_tiling, snapshot, titled};
use insta::assert_snapshot;

#[test]
fn auto_tile_sets_horizontal_spawn_mode_when_width_greater_than_height() {
    let mut hub = setup_with_automatic_tiling();
    hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.insert_tiling(hub.current_workspace(), titled("w1"));
    hub.insert_tiling(hub.current_workspace(), titled("w2"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1, w2])
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

#[test]
fn auto_tile_sets_vertical_spawn_mode_when_height_greater_than_width() {
    let mut hub = setup_with_automatic_tiling();
    // Going on a round trip to ensure that we can always create a horizontal container with 6
    // direct children, as the auto tile logic can get confused when width is approximately equal
    // to height, due to floating precision lost
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w3"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w4"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w5"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w6"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w7"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w8"));
    hub.toggle_direction();
    // Each window is 25x30, height > width, so spawn mode should be vertical
    hub.set_focus(w0);
    hub.insert_tiling(hub.current_workspace(), titled("w9"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(6))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=125.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(4), x=100.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(2), x=50.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(1), x=25.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(6), x=0.00, y=15.00, w=25.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=25.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w4, w5, w6, w7, w8])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=25.00, h=30.00, titles=[w3, w9])
      )

    +-----------------------++-----------------------++-----------------------++-----------------------++-----------------------++-----------------------+
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |           W0          ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    +-----------------------+|                       ||                       ||                       ||                       ||                       |
    *************************|           W1          ||           W2          ||          W3           ||          W4           ||          W5           |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *           W6          *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *************************+-----------------------++-----------------------++-----------------------++-----------------------++-----------------------+
    ");
}

#[test]
fn auto_tile_preserves_tab_spawn_mode() {
    let mut hub = setup_with_automatic_tiling();
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=2.00, w=75.00, h=28.00, highlighted, spawn=top)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=1, titles=[W1, W2])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                W1                  |               [W2]                 |
    |                                                                         |***************************************************************************
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
    |                                                                         |*                                    W2                                   *
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
fn auto_tile_adjusts_after_toggle_direction() {
    let mut hub = setup_with_automatic_tiling();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w10"));
    hub.insert_tiling(hub.current_workspace(), titled("w11"));
    hub.insert_tiling(hub.current_workspace(), titled("w12"));
    hub.toggle_direction();
    hub.set_focus(w0);
    hub.insert_tiling(hub.current_workspace(), titled("w13"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=20.00, w=150.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=10.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w11, w12])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=10.00, titles=[w10, w13])
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
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
    |                                                                         W2                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn auto_tile_with_tab_spawn_mode() {
    let mut hub = setup_with_automatic_tiling();
    hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=top)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=2, titles=[W0, W1, W2])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W0                        |                      W1                        |                     [W2]                        |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn auto_tile_preserves_tab_spawn_mode_on_nested_container_on_delete() {
    let mut hub = setup_with_automatic_tiling();
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    let w2 = hub.insert_tiling_titled();
    hub.focus_left();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();
    hub.toggle_container_layout();
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.delete_window(w2);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=2.00, w=75.00, h=28.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=1, highlighted, spawn=top, titles=[W1, W3])
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                W1                  |               [W3]                 *
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
    hub.insert_tiling(hub.current_workspace(), titled("w14"));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=75.00, y=2.00, w=75.00, h=28.00, highlighted, spawn=top)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=2, titles=[W1, W3, w14])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W3            |         [w14]          |
    |                                                                         |***************************************************************************
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
    |                                                                         |*                                    W4                                   *
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
