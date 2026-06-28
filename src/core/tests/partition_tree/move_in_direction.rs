use crate::core::node::{Dimension, Length};
use crate::core::tests::{setup, snapshot, titled};

#[test]
fn move_right_from_vertical_container_to_horizontal_parent() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.insert_tiling(hub.current_workspace(), titled("w1"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.insert_tiling(hub.current_workspace(), titled("w3"));

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=50.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, Container, w3])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=50.00, h=30.00, titles=[w1, w2])
      )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                       W1                       |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                |+------------------------------------------------+*                                                *
    |                       W0                       |+------------------------------------------------+*                       W3                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                       W2                       |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}

#[test]
fn move_down_from_horizontal_container_to_vertical_parent() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w4"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w5"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w6"));
    hub.insert_tiling(hub.current_workspace(), titled("w7"));

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=0.00, y=20.00, w=150.00, h=10.00, highlighted, spawn=bottom)
        Window(id=WindowId(2), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w4, Container, w7])
        Container(id=ContainerId(1), x=0.00, y=10.00, w=150.00, h=10.00, titles=[w5, w6])
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
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W1                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W3                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn move_right_from_vertical_container_creates_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w8"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w9"));
    hub.insert_tiling(hub.current_workspace(), titled("w10"));

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w10])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00, titles=[w8, w9])
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
fn move_right_from_vertical_container_replaces_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w11"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w12"));

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w11, w12])
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
fn move_down_from_horizontal_container_creates_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w13"));
    hub.insert_tiling(hub.current_workspace(), titled("w14"));
    hub.insert_tiling(hub.current_workspace(), titled("w15"));

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w15])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=15.00, titles=[w13, w14])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
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
fn move_down_from_horizontal_container_replaces_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w16"));
    hub.insert_tiling(hub.current_workspace(), titled("w17"));

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w16, w17])
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
fn move_right_at_edge_goes_to_horizontal_grandparent() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w18"));
    hub.insert_tiling(hub.current_workspace(), titled("w19"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w20"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w21"));
    hub.insert_tiling(hub.current_workspace(), titled("w22"));

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=75.00, y=15.00, w=25.00, h=15.00)
        Window(id=WindowId(2), x=50.00, y=15.00, w=25.00, h=15.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w18, Container, w22])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=50.00, h=30.00, titles=[w19, Container])
        Container(id=ContainerId(2), x=50.00, y=15.00, w=50.00, h=15.00, titles=[w20, w21])
      )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                       W1                       |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                |+------------------------------------------------+*                                                *
    |                       W0                       |+-----------------------++-----------------------+*                       W4                       *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||           W2          ||          W3           |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    +------------------------------------------------++-----------------------++-----------------------+**************************************************
    ");
}

#[test]
fn move_left_at_edge_goes_to_horizontal_grandparent() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w23"));
    hub.insert_tiling(hub.current_workspace(), titled("w24"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w25"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w26"));
    hub.insert_tiling(hub.current_workspace(), titled("w27"));
    hub.focus_left();
    hub.focus_left();

    hub.move_left();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=125.00, y=15.00, w=25.00, h=15.00)
        Window(id=WindowId(3), x=100.00, y=15.00, w=25.00, h=15.00)
        Window(id=WindowId(1), x=100.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(2), x=50.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w23, w25, Container])
        Container(id=ContainerId(1), x=100.00, y=0.00, w=50.00, h=30.00, titles=[w24, Container])
        Container(id=ContainerId(2), x=100.00, y=15.00, w=50.00, h=15.00, titles=[w26, w27])
      )

    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W1                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *+------------------------------------------------+
    |                       W0                       |*                       W2                       *+-----------------------++-----------------------+
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|          W3           ||          W4           |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    +------------------------------------------------+**************************************************+-----------------------++-----------------------+
    ");
}

#[test]
fn move_down_at_edge_goes_to_vertical_grandparent() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w28"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w29"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w30"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w31"));
    hub.insert_tiling(hub.current_workspace(), titled("w32"));

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=0.00, y=20.00, w=150.00, h=10.00, highlighted, spawn=bottom)
        Window(id=WindowId(3), x=75.00, y=15.00, w=75.00, h=5.00)
        Window(id=WindowId(2), x=75.00, y=10.00, w=75.00, h=5.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w28, Container, w32])
        Container(id=ContainerId(1), x=0.00, y=10.00, w=150.00, h=10.00, titles=[w29, Container])
        Container(id=ContainerId(2), x=75.00, y=10.00, w=75.00, h=10.00, titles=[w30, w31])
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
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W2                                   |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W1                                   |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W4                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn move_up_at_edge_goes_to_vertical_grandparent() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w33"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w34"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w35"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w36"));
    hub.insert_tiling(hub.current_workspace(), titled("w37"));
    hub.focus_up();
    hub.focus_up();

    hub.move_up();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=75.00, y=25.00, w=75.00, h=5.00)
        Window(id=WindowId(3), x=75.00, y=20.00, w=75.00, h=5.00)
        Window(id=WindowId(1), x=0.00, y=20.00, w=75.00, h=10.00)
        Window(id=WindowId(2), x=0.00, y=10.00, w=150.00, h=10.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w33, w35, Container])
        Container(id=ContainerId(1), x=0.00, y=20.00, w=150.00, h=10.00, titles=[w34, Container])
        Container(id=ContainerId(2), x=75.00, y=20.00, w=75.00, h=10.00, titles=[w36, w37])
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
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W1                                   |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W4                                   |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn swap_right_in_horizontal_container() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w38"));
    hub.insert_tiling(hub.current_workspace(), titled("w39"));
    hub.focus_left();

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w39, w38])
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
    |                                    W1                                   |*                                    W0                                   *
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
fn swap_down_in_vertical_container() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w40"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w41"));
    hub.focus_up();

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w41, w40])
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
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn move_from_tabbed_parent_goes_to_grandparent() {
    let mut hub = setup();
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_container_layout();
    hub.focus_prev_tab();

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=50.00, y=2.00, w=50.00, h=28.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container, W2])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=50.00, h=30.00, tabbed, active_tab=0, titles=[W1, W3])
      )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||         [W1]           |         W3            |*                                                *
    |                                                |+------------------------------------------------+*                                                *
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
    |                       W0                       ||                                                |*                       W2                       *
    |                                                ||                       W1                       |*                                                *
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
fn move_from_nested_container_skip_tabbed_grandparent() {
    let mut hub = setup();
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_container_layout();
    hub.focus_prev_tab();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();
    insta::assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=112.50, y=2.00, w=37.50, h=28.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=2.00, w=37.50, h=28.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=1, titles=[W1, Container, W3])
        Container(id=ContainerId(2), x=75.00, y=2.00, w=75.00, h=28.00, titles=[W2, W4])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |     [Container]       |          W3            |
    |                                                                         |+------------------------------------+*************************************
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                    W0                                   ||                                    |*                                   *
    |                                                                         ||                 W2                 |*                W4                 *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    +-------------------------------------------------------------------------++------------------------------------+*************************************
    ");

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=50.00, y=2.00, w=50.00, h=28.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container, W4])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=50.00, h=30.00, tabbed, active_tab=1, titles=[W1, W2, W3])
      )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||      W1        |    [W2]       |     W3        |*                                                *
    |                                                |+------------------------------------------------+*                                                *
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
    |                       W0                       ||                                                |*                       W4                       *
    |                                                ||                       W2                       |*                                                *
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
fn move_container_up_toggles_direction_when_matching_parent() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w42"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w43"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w44"));
    hub.insert_tiling(hub.current_workspace(), titled("w45"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w46"));
    hub.focus_parent();

    hub.move_up();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=20.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=20.00, w=75.00, h=10.00)
        Window(id=WindowId(4), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=0.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w42, Container, Container])
        Container(id=ContainerId(1), x=0.00, y=20.00, w=150.00, h=10.00, titles=[w43, w44])
        Container(id=ContainerId(2), x=0.00, y=10.00, w=150.00, h=10.00, highlighted, spawn=bottom, titles=[w45, w46])
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
    ******************************************************************************************************************************************************
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                    W3                                   ||                                    W4                                   *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    ******************************************************************************************************************************************************
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W1                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn move_container_left_toggles_direction_when_matching_parent() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w47"));
    hub.insert_tiling(hub.current_workspace(), titled("w48"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w49"));
    hub.insert_tiling(hub.current_workspace(), titled("w50"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w51"));
    hub.focus_parent();

    hub.move_left();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=100.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(4), x=50.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(3), x=50.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w47, Container, Container])
        Container(id=ContainerId(1), x=100.00, y=0.00, w=50.00, h=30.00, titles=[w48, w49])
        Container(id=ContainerId(2), x=50.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right, titles=[w50, w51])
      )

    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                       W3                       *|                       W1                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*------------------------------------------------*+------------------------------------------------+
    |                       W0                       |*------------------------------------------------*+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                       W4                       *|                       W2                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn move_in_direction_noop() {
    let mut hub = setup();
    let before = snapshot(&hub);
    hub.move_left();
    assert_eq!(before, snapshot(&hub));
    hub.move_right();
    assert_eq!(before, snapshot(&hub));
    hub.move_up();
    assert_eq!(before, snapshot(&hub));
    hub.move_down();
    assert_eq!(before, snapshot(&hub));

    hub.insert_tiling(hub.current_workspace(), titled("w52"));
    let before = snapshot(&hub);
    hub.move_left();
    assert_eq!(before, snapshot(&hub));
    hub.move_right();
    assert_eq!(before, snapshot(&hub));
    hub.move_up();
    assert_eq!(before, snapshot(&hub));
    hub.move_down();
    assert_eq!(before, snapshot(&hub));

    let mut hub = setup();
    hub.insert_float(
        hub.current_workspace(),
        Dimension::new(
            Length::new(10.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        titled("w53"),
    );
    let before = snapshot(&hub);
    hub.move_left();
    assert_eq!(before, snapshot(&hub));
    hub.move_right();
    assert_eq!(before, snapshot(&hub));
    hub.move_up();
    assert_eq!(before, snapshot(&hub));
    hub.move_down();
    assert_eq!(before, snapshot(&hub));
}
