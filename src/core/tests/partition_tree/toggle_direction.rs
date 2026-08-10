use crate::core::GlobalLayoutConfig;
use crate::core::node::{Dimension, Length, WindowRestrictions};
use crate::core::tests::{
    LayoutConfigBuilder, default_dim, setup, setup_with_layout, snapshot, titled, titled_matcher,
};
use insta::assert_snapshot;

/// Float matchers by exact title, since this file also inserts tiling windows named `wN`.
fn layout_floating(titles: &[&str]) -> GlobalLayoutConfig {
    LayoutConfigBuilder::new()
        .with_float(titles.iter().map(|t| titled_matcher(t)).collect())
        .build()
}

#[test]
fn toggle_direction_on_focused_container() {
    let mut hub = setup();

    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    hub.focus_parent();
    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=15.00, w=150.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right, titles=[w0, w1])
      )

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
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
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
fn toggle_direction_on_window() {
    let mut hub = setup();

    hub.insert_window(titled("w2"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w3"), default_dim(), WindowRestrictions::None);
    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w2, w3])
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
fn toggle_direction_on_window_nested() {
    let mut hub = setup();

    hub.insert_window(titled("w4"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w5"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w6"), default_dim(), WindowRestrictions::None);
    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w4, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w5, w6])
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
    |                                    W1                                   |*                                    W2                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn toggle_direction_inside_tabbed_only_affects_tabbed_subtree() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W4"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W5"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W6"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W7"), default_dim(), WindowRestrictions::None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(7))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(7), x=137.00, y=2.00, w=13.00, h=28.00, highlighted, spawn=right)
        Window(id=WindowId(6), x=125.00, y=2.00, w=12.00, h=28.00)
        Window(id=WindowId(5), x=112.00, y=2.00, w=13.00, h=28.00)
        Window(id=WindowId(4), x=100.00, y=2.00, w=12.00, h=28.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, W1, Container])
        Container(id=ContainerId(1), x=100.00, y=0.00, w=50.00, h=30.00, tabbed, active_tab=2, titles=[W2, W3, Container])
        Container(id=ContainerId(2), x=100.00, y=2.00, w=50.00, h=28.00, titles=[W4, W5, W6, W7])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||      W2        |     W3        | [Container]   |
    |                                                ||                                                |+----------++-----------++----------+*************
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                       W0                       ||                       W1                       ||          ||           ||          |*           *
    |                                                ||                                                ||    W4    ||     W5    ||    W6    |*     W7    *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    +------------------------------------------------++------------------------------------------------++----------++-----------++----------+*************
    ");

    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(7))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(7), x=100.00, y=23.00, w=50.00, h=7.00, highlighted, spawn=right)
        Window(id=WindowId(6), x=100.00, y=16.00, w=50.00, h=7.00)
        Window(id=WindowId(5), x=100.00, y=9.00, w=50.00, h=7.00)
        Window(id=WindowId(4), x=100.00, y=2.00, w=50.00, h=7.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, W1, Container])
        Container(id=ContainerId(1), x=100.00, y=0.00, w=50.00, h=30.00, tabbed, active_tab=2, titles=[W2, W3, Container])
        Container(id=ContainerId(2), x=100.00, y=2.00, w=50.00, h=28.00, titles=[W4, W5, W6, W7])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||      W2        |     W3        | [Container]   |
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                       W4                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                       W5                       |
    |                                                ||                                                ||                                                |
    |                       W0                       ||                       W1                       |+------------------------------------------------+
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                       W6                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                |**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                       W7                       *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}

#[test]
fn toggle_direction_skips_nested_tabbed_container() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    let w1 = hub
        .insert_window(titled("W1"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W4"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W5"), default_dim(), WindowRestrictions::None);
    hub.set_focus(w1);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=125.00, y=2.00, w=25.00, h=28.00)
        Window(id=WindowId(4), x=100.00, y=2.00, w=25.00, h=28.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, W1, Container])
        Container(id=ContainerId(1), x=100.00, y=0.00, w=50.00, h=30.00, tabbed, active_tab=2, titles=[W2, W3, Container])
        Container(id=ContainerId(2), x=100.00, y=2.00, w=50.00, h=28.00, titles=[W4, W5])
      )

    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|      W2        |     W3        | [Container]   |
    |                                                |*                                                *+-----------------------++-----------------------+
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                       W0                       |*                       W1                       *|                       ||                       |
    |                                                |*                                                *|           W4          ||           W5          |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    +------------------------------------------------+**************************************************+-----------------------++-----------------------+
    ");

    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=75.00, y=22.00, w=75.00, h=8.00)
        Window(id=WindowId(4), x=0.00, y=22.00, w=75.00, h=8.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=10.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, W1, Container])
        Container(id=ContainerId(1), x=0.00, y=20.00, w=150.00, h=10.00, tabbed, active_tab=2, titles=[W2, W3, Container])
        Container(id=ContainerId(2), x=0.00, y=22.00, w=150.00, h=8.00, titles=[W4, W5])
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
    *                                                                         W1                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W2                        |                      W3                        |                  [Container]                    |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W4                                   ||                                    W5                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn toggle_direction_inside_tabbed_skips_nested_tabbed() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    let w1 = hub
        .insert_window(titled("W1"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.set_focus(w1);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W4"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W5"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    let w6 = hub
        .insert_window(titled("W6"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("W7"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W8"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W9"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W10"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W11"), default_dim(), WindowRestrictions::None);
    hub.set_focus(w6);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(6))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(11), x=94.00, y=4.00, w=6.00, h=26.00)
        Window(id=WindowId(10), x=89.00, y=4.00, w=5.00, h=26.00)
        Window(id=WindowId(9), x=83.00, y=4.00, w=6.00, h=26.00)
        Window(id=WindowId(6), x=67.00, y=2.00, w=16.00, h=28.00, highlighted, spawn=right)
        Window(id=WindowId(5), x=50.00, y=2.00, w=17.00, h=28.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container, W2])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=50.00, h=30.00, tabbed, active_tab=3, titles=[W1, W3, W4, Container])
        Container(id=ContainerId(2), x=50.00, y=2.00, w=50.00, h=28.00, titles=[W5, W6, Container])
        Container(id=ContainerId(3), x=83.00, y=2.00, w=17.00, h=28.00, tabbed, active_tab=2, titles=[W7, W8, Container])
        Container(id=ContainerId(4), x=83.00, y=4.00, w=17.00, h=26.00, titles=[W9, W10, W11])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||    W1      |   W3      |   W4      [Container] ||                                                |
    |                                                |+---------------+****************+---------------+|                                                |
    |                                                ||               |*              *| W7  |W[Contain||                                                |
    |                                                ||               |*              *+----++---++----+|                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                       W0                       ||               |*              *|    ||   ||    ||                       W2                       |
    |                                                ||       W5      |*      W6      *|    ||   ||    ||                                                |
    |                                                ||               |*              *| W9 || W1|| W11||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    +------------------------------------------------++---------------+****************+----++---++----++------------------------------------------------+
    ");

    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(6))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(11), x=83.00, y=23.00, w=17.00, h=7.00)
        Window(id=WindowId(10), x=67.00, y=23.00, w=16.00, h=7.00)
        Window(id=WindowId(9), x=50.00, y=23.00, w=17.00, h=7.00)
        Window(id=WindowId(6), x=50.00, y=11.00, w=50.00, h=10.00, highlighted, spawn=right)
        Window(id=WindowId(5), x=50.00, y=2.00, w=50.00, h=9.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container, W2])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=50.00, h=30.00, tabbed, active_tab=3, titles=[W1, W3, W4, Container])
        Container(id=ContainerId(2), x=50.00, y=2.00, w=50.00, h=28.00, titles=[W5, W6, Container])
        Container(id=ContainerId(3), x=50.00, y=21.00, w=50.00, h=9.00, tabbed, active_tab=2, titles=[W7, W8, Container])
        Container(id=ContainerId(4), x=50.00, y=23.00, w=50.00, h=7.00, titles=[W9, W10, W11])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||    W1      |   W3      |   W4      [Container] ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                       W5                       ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                |**************************************************|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                       W0                       |*                                                *|                       W2                       |
    |                                                |*                       W6                       *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |**************************************************|                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                ||      W7        |     W8        | [Container]   ||                                                |
    |                                                |+---------------++--------------++---------------+|                                                |
    |                                                ||               ||              ||               ||                                                |
    |                                                ||               ||              ||               ||                                                |
    |                                                ||               ||              ||               ||                                                |
    |                                                ||       W9      ||      W10     ||       W11     ||                                                |
    |                                                ||               ||              ||               ||                                                |
    +------------------------------------------------++---------------++--------------++---------------++------------------------------------------------+
    ");
}

#[test]
fn toggle_direction_noop() {
    let mut hub = setup();
    let before = snapshot(&hub);
    hub.toggle_direction();
    assert_eq!(before, snapshot(&hub));

    hub.insert_window(titled("w7"), default_dim(), WindowRestrictions::None);
    let before = snapshot(&hub);
    hub.toggle_direction();
    assert_eq!(before, snapshot(&hub));

    let mut hub = setup_with_layout(layout_floating(&["w8"]));
    hub.insert_window(
        titled("w8"),
        Dimension::new(
            Length::new(10.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    let before = snapshot(&hub);
    hub.toggle_direction();
    assert_eq!(before, snapshot(&hub));
}
