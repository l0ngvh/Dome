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
fn toggle_spawn_mode_creates_new_container() {
    let mut hub = setup();

    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w2"), default_dim(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w1, w2])
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
fn toggle_spawn_mode_in_single_window_workspace_creates_vertical_container() {
    let mut hub = setup();

    hub.insert_window(titled("w3"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w4"), default_dim(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w3, w4])
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
fn toggle_spawn_mode_in_vertical_container() {
    let mut hub = setup();

    hub.insert_window(titled("w5"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w6"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w7"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w8"), default_dim(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=112.50, y=15.00, w=37.50, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=15.00, w=37.50, h=15.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w5, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w6, Container])
        Container(id=ContainerId(2), x=75.00, y=15.00, w=75.00, h=15.00, titles=[w7, w8])
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
    |                                    W0                                   |+------------------------------------+*************************************
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                 W2                 |*                W3                 *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    +-------------------------------------------------------------------------++------------------------------------+*************************************
    ");
}

#[test]
fn toggle_spawn_mode_to_tab_inserts_to_parent_tabbed_container() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
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
fn toggle_spawn_mode_to_tab_inserts_to_ancestor_tabbed_container() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W4"), default_dim(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=2, titles=[W0, Container, W4])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W0                        |                   Container                    |                     [W4]                        |
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
    *                                                                         W4                                                                         *
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
fn toggle_spawn_mode_to_tab_creates_tabbed_container_if_no_ancestor() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);

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
fn toggle_spawn_mode_horizontal_when_focused_is_tabbed_container() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_parent();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=2.00, w=75.00, h=28.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, W2])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=1, titles=[W0, W1])
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                W0                  |               [W1]                 |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
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
    |                                                                         |*                                    W2                                   *
    |                                    W1                                   |*                                                                         *
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
fn toggle_spawn_mode_vertical_when_focused_is_tabbed_container() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=13.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, W2])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=15.00, tabbed, active_tab=1, titles=[W0, W1])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                                 [W1]                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
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
fn toggle_spawn_mode_tab_when_focused_is_tabbed_container() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);

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
fn toggle_spawn_mode_tab_when_focused_is_split_container_creates_parent() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=2.00, w=75.00, h=28.00, highlighted, spawn=top)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(2), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=1, titles=[Container, W3])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||             Container              |               [W3]                 |
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
fn spawn_non_tab_in_tabbed_parent_creates_child_container() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=2.00, w=75.00, h=28.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=2.00, w=75.00, h=28.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, Container])
        Container(id=ContainerId(1), x=0.00, y=2.00, w=150.00, h=28.00, titles=[W1, W2])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                              [Container]                                |
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
    |                                    W1                                   |*                                    W2                                   *
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
fn spawn_non_tab_in_tabbed_parent_with_focused_container() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.focus_parent();
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=0.00, y=20.67, w=150.00, h=9.33, highlighted, spawn=bottom)
        Window(id=WindowId(2), x=0.00, y=11.33, w=150.00, h=9.33)
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=9.33)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, Container])
        Container(id=ContainerId(1), x=0.00, y=2.00, w=150.00, h=28.00, titles=[W1, W2, W3])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                              [Container]                                |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
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
    ******************************************************************************************************************************************************
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
fn toggle_spawn_mode_noop() {
    let mut hub = setup();
    let before = snapshot(&hub);
    hub.toggle_spawn_mode();
    assert_eq!(before, snapshot(&hub));

    let mut hub = setup_with_layout(layout_floating(&["w9"]));
    hub.insert_window(
        titled("w9"),
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
    hub.toggle_spawn_mode();
    assert_eq!(before, snapshot(&hub));
}
