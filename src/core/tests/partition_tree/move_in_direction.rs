use crate::core::GlobalLayoutConfig;
use crate::core::node::{PixelRect, WindowRestrictions};
use crate::core::tests::{
    LayoutConfigBuilder, default_rect, setup, setup_with_layout, snapshot, titled, titled_matcher,
};

/// Float matchers by exact title, since this file also inserts tiling windows named `wN`.
fn layout_floating(titles: &[&str]) -> GlobalLayoutConfig {
    LayoutConfigBuilder::new()
        .with_float(titles.iter().map(|t| titled_matcher(t)).collect())
        .build()
}

#[test]
fn move_right_from_vertical_container_to_horizontal_parent() {
    let mut hub = setup();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);

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
    hub.insert_window(titled("w4"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w5"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w6"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w7"), default_rect(), WindowRestrictions::None);

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
    hub.insert_window(titled("w8"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w9"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w10"), default_rect(), WindowRestrictions::None);

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
    hub.insert_window(titled("w11"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w12"), default_rect(), WindowRestrictions::None);

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
    hub.insert_window(titled("w13"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w14"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w15"), default_rect(), WindowRestrictions::None);

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
    hub.insert_window(titled("w16"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w17"), default_rect(), WindowRestrictions::None);

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
    hub.insert_window(titled("w18"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w19"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w20"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w21"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w22"), default_rect(), WindowRestrictions::None);

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @"
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
    |                                                ||           W2          ||           W3          |*                                                *
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
    hub.insert_window(titled("w23"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w24"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w25"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w26"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w27"), default_rect(), WindowRestrictions::None);
    hub.focus_left();
    hub.focus_left();

    hub.move_left();
    insta::assert_snapshot!(snapshot(&hub), @"
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
    |                                                |*                                                *|           W3          ||           W4          |
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
    hub.insert_window(titled("w28"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w29"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w30"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w31"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w32"), default_rect(), WindowRestrictions::None);

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @"
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
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
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
    hub.insert_window(titled("w33"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w34"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w35"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w36"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w37"), default_rect(), WindowRestrictions::None);
    hub.focus_up();
    hub.focus_up();

    hub.move_up();
    insta::assert_snapshot!(snapshot(&hub), @"
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
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W1                                   |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W4                                   |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn swap_right_in_horizontal_container() {
    let mut hub = setup();
    hub.insert_window(titled("w38"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w39"), default_rect(), WindowRestrictions::None);
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
    hub.insert_window(titled("w40"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w41"), default_rect(), WindowRestrictions::None);
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
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W3"), default_rect(), WindowRestrictions::None);
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
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W3"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_prev_tab();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W4"), default_rect(), WindowRestrictions::None);
    insta::assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=113.00, y=2.00, w=37.00, h=28.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=2.00, w=38.00, h=28.00)
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
    |                                                                         ||                 W2                 |*                 W4                *
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
    hub.insert_window(titled("w42"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w43"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w44"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w45"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w46"), default_rect(), WindowRestrictions::None);
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
    hub.insert_window(titled("w47"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w48"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w49"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w50"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w51"), default_rect(), WindowRestrictions::None);
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

    hub.insert_window(titled("w52"), default_rect(), WindowRestrictions::None);
    let before = snapshot(&hub);
    hub.move_left();
    assert_eq!(before, snapshot(&hub));
    hub.move_right();
    assert_eq!(before, snapshot(&hub));
    hub.move_up();
    assert_eq!(before, snapshot(&hub));
    hub.move_down();
    assert_eq!(before, snapshot(&hub));

    let mut hub = setup_with_layout(layout_floating(&["w53"]));
    hub.insert_window(
        titled("w53"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .unwrap();
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
