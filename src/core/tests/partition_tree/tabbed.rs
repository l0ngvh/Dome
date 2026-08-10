use crate::core::ContainerId;
use crate::core::GlobalLayoutConfig;
use crate::core::allocator::NodeId;
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
fn toggle_tabbed_mode() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();

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
fn toggle_tabbed_mode_focus_currently_focused_node() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.focus_left();
    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, W1, W2])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W0                        |                     [W1]                       |                      W2                         |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn focus_prev_next_tab() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_prev_tab();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, W1, W2])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W0                        |                     [W1]                       |                      W2                         |
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
    ******************************************************************************************************************************************************
    ");
    hub.focus_next_tab();
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
fn focus_next_tab_wrapped() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_next_tab();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=0, titles=[W0, W1, W2])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                      [W0]                       |                      W1                        |                      W2                         |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn focus_prev_tab_wraps() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_prev_tab();
    hub.focus_prev_tab();
    hub.focus_prev_tab();

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
fn focus_tab_change_workspace_focus_to_active_tab_container_focused() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);
    hub.focus_up();
    hub.focus_left();
    hub.toggle_container_layout();
    hub.focus_next_tab();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=0.00, y=21.00, w=150.00, h=9.00)
        Window(id=WindowId(2), x=0.00, y=11.00, w=150.00, h=10.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=9.00)
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
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W3                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn focus_tab_change_workspace_focus_to_tabbed_container_active_tab_focused() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);
    hub.focus_up();
    hub.toggle_container_layout();
    hub.focus_left();
    hub.toggle_container_layout();
    hub.focus_next_tab();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=4.00, w=150.00, h=26.00, highlighted, spawn=bottom)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, Container])
        Container(id=ContainerId(1), x=0.00, y=2.00, w=150.00, h=28.00, tabbed, active_tab=1, titles=[W1, W2, W3])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                              [Container]                                |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W1                        |                     [W2]                       |                      W3                         |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn toggle_tabbed_off() {
    let mut hub = setup();

    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1])
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
fn tabbed_container_takes_one_slot() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);

    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=2.00, w=75.00, h=28.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=2, titles=[W1, W2, W3])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W2            |         [W3]           |
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
fn vertical_to_tabbed() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);
    hub.focus_parent();
    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=0.00, y=2.00, w=150.00, h=28.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=3, highlighted, spawn=bottom, titles=[W0, W1, W2, W3])
      )

    ******************************************************************************************************************************************************
    *                 W0                  |                W1                  |                W2                  |               [W3]                 *
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
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
    *                                                                         W3                                                                         *
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
fn container_in_tabbed_container() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);

    hub.toggle_spawn_mode();
    let w4 = hub
        .insert_window(titled("W4"), default_dim(), WindowRestrictions::None)
        .unwrap();

    hub.focus_parent();
    hub.focus_parent();
    hub.toggle_container_layout();
    hub.set_focus(w4);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=113.00, y=2.00, w=37.00, h=28.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=75.00, y=2.00, w=38.00, h=28.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=2, titles=[W1, W2, Container])
        Container(id=ContainerId(2), x=75.00, y=2.00, w=75.00, h=28.00, titles=[W3, W4])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W2            |      [Container]       |
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
    |                                                                         ||                 W3                 |*                 W4                *
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

    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=75.00, y=4.00, w=75.00, h=26.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=2, titles=[W1, W2, Container])
        Container(id=ContainerId(2), x=75.00, y=2.00, w=75.00, h=28.00, tabbed, active_tab=1, titles=[W3, W4])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W2            |      [Container]       |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                W3                  |               [W4]                 |
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
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
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
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn change_tab_shows_container_focus() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W4"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W5"), default_dim(), WindowRestrictions::None);

    hub.focus_left();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=125.00, y=2.00, w=25.00, h=28.00)
        Window(id=WindowId(4), x=100.00, y=2.00, w=25.00, h=28.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=75.00, y=2.00, w=25.00, h=28.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=2, titles=[W1, W2, Container])
        Container(id=ContainerId(2), x=75.00, y=2.00, w=75.00, h=28.00, titles=[W3, W4, W5])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W2            |      [Container]       |
    |                                                                         |+-----------------------+*************************+-----------------------+
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                    W0                                   ||                       |*                       *|                       |
    |                                                                         ||           W3          |*           W4          *|           W5          |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    +-------------------------------------------------------------------------++-----------------------+*************************+-----------------------+
    ");

    hub.focus_prev_tab();
    hub.focus_prev_tab();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=2.00, w=75.00, h=28.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=0, titles=[W1, W2, Container])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||         [W1]           |         W2            |       Container        |
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
    |                                                                         |*                                    W1                                   *
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

    hub.focus_next_tab();
    hub.focus_next_tab();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=125.00, y=2.00, w=25.00, h=28.00)
        Window(id=WindowId(4), x=100.00, y=2.00, w=25.00, h=28.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=75.00, y=2.00, w=25.00, h=28.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=2, titles=[W1, W2, Container])
        Container(id=ContainerId(2), x=75.00, y=2.00, w=75.00, h=28.00, titles=[W3, W4, W5])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W2            |      [Container]       |
    |                                                                         |+-----------------------+*************************+-----------------------+
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                    W0                                   ||                       |*                       *|                       |
    |                                                                         ||           W3          |*           W4          *|           W5          |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    +-------------------------------------------------------------------------++-----------------------+*************************+-----------------------+
    ");
}

#[test]
fn set_focus_updates_active_tab() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("W0"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();

    // Focus W0 should update active_tab to 0
    hub.set_focus(w0);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=0, titles=[W0, W1, W2])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                      [W0]                       |                      W1                        |                      W2                         |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn delete_active_tab_updates_active_tab() {
    let mut hub = setup();
    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    let w2 = hub
        .insert_window(titled("W2"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_container_layout();

    // W2 is active (index 2), delete it
    hub.delete_window(w2);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, W1])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                                 [W1]                                    |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn toggle_tabbed_off_fixes_direction_conflict_with_parent_and_children() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    let w1 = hub
        .insert_window(titled("W1"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    let w3 = hub
        .insert_window(titled("W3"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("W4"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W5"), default_dim(), WindowRestrictions::None);
    hub.set_focus(w1);

    hub.toggle_direction();
    hub.set_focus(w3);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=0.00, y=22.00, w=150.00, h=8.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, W1, Container])
        Container(id=ContainerId(1), x=0.00, y=20.00, w=150.00, h=10.00, tabbed, active_tab=1, titles=[W2, W3, Container])
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
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W2                        |                     [W3]                       |                   Container                     |
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W3                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");

    hub.toggle_container_layout();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=100.00, y=25.00, w=50.00, h=5.00)
        Window(id=WindowId(4), x=100.00, y=20.00, w=50.00, h=5.00)
        Window(id=WindowId(3), x=50.00, y=20.00, w=50.00, h=10.00, highlighted, spawn=bottom)
        Window(id=WindowId(2), x=0.00, y=20.00, w=50.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, W1, Container])
        Container(id=ContainerId(1), x=0.00, y=20.00, w=150.00, h=10.00, titles=[W2, W3, Container])
        Container(id=ContainerId(2), x=100.00, y=20.00, w=50.00, h=10.00, titles=[W4, W5])
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
    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W4                       |
    |                                                |*                                                *+------------------------------------------------+
    |                       W2                       |*                       W3                       *+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W5                       |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn toggle_tabbed_off_dont_rotate_child_when_its_already_correct() {
    let mut hub = setup();

    // Create horizontal container with 3 windows
    hub.insert_window(titled("w2"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w3"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w4"), default_dim(), WindowRestrictions::None);

    // Make it tabbed
    hub.toggle_container_layout();

    // Create a vertical nested container in the middle tab
    hub.focus_prev_tab();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w5"), default_dim(), WindowRestrictions::None);

    // Focus parent and toggle back to split (horizontal)
    hub.focus_parent();
    hub.focus_parent();
    hub.toggle_container_layout();

    // The nested container should stay vertical (not rotated) since it differs from parent
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(3), x=50.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right, titles=[w2, Container, w4])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=50.00, h=30.00, titles=[w3, w5])
      )

    ******************************************************************************************************************************************************
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                       W1                       ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                |+------------------------------------------------+|                                                *
    *                       W0                       |+------------------------------------------------+|                       W2                       *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                       W3                       ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn delete_unfocused_child_keeps_active_tab() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_prev_tab();
    hub.toggle_spawn_mode();
    let w3 = hub
        .insert_window(titled("W3"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.focus_prev_tab();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=0, titles=[W0, Container, W2])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                      [W0]                       |                   Container                    |                      W2                         |
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
    ******************************************************************************************************************************************************
    ");

    hub.delete_window(w3);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=0, titles=[W0, W1, W2])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                      [W0]                       |                      W1                        |                      W2                         |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn toggle_container_layout_in_nested_tabbed_maintain_direction_invariant() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    let w1 = hub
        .insert_window(titled("W1"), default_dim(), WindowRestrictions::None)
        .unwrap();

    hub.toggle_container_layout();
    let w2 = hub
        .insert_window(titled("W2"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W3"), default_dim(), WindowRestrictions::None);
    hub.set_focus(w1);
    hub.toggle_container_layout();
    // [w2, w3] was still vertical as toggle_container_layout doesn't change child orientation, nor
    // should it do
    hub.set_focus(w2);
    hub.toggle_direction();
    // Now we have 3 containers where original orientation were horizontal, 2 of which got turned
    // into tabbed
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=4.00, w=75.00, h=26.00)
        Window(id=WindowId(2), x=0.00, y=4.00, w=75.00, h=26.00, highlighted, spawn=bottom)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, Container])
        Container(id=ContainerId(1), x=0.00, y=2.00, w=150.00, h=28.00, tabbed, active_tab=1, titles=[W1, Container])
        Container(id=ContainerId(2), x=0.00, y=4.00, w=150.00, h=26.00, titles=[W2, W3])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                              [Container]                                |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W1                                     |                              [Container]                                |
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
    *                                    W2                                   *|                                    W3                                   |
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

    hub.set_focus(w1);
    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=16.00, w=75.00, h=14.00)
        Window(id=WindowId(2), x=75.00, y=2.00, w=75.00, h=14.00)
        Window(id=WindowId(1), x=0.00, y=2.00, w=75.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, Container])
        Container(id=ContainerId(1), x=0.00, y=2.00, w=150.00, h=28.00, titles=[W1, Container])
        Container(id=ContainerId(2), x=75.00, y=2.00, w=75.00, h=28.00, titles=[W2, W3])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                              [Container]                                |
    ***************************************************************************+-------------------------------------------------------------------------+
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
    *                                                                         *+-------------------------------------------------------------------------+
    *                                    W1                                   *+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W3                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn toggle_tabbed_when_focused_is_inside_child_container() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    let w1 = hub
        .insert_window(titled("W1"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("W2"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("W3"), default_dim(), WindowRestrictions::None)
        .unwrap();
    // Creating multiple nested container to cover non focused container branch
    hub.set_focus(w1);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W4"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W5"), default_dim(), WindowRestrictions::None);
    hub.set_focus(w2);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W6"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W7"), default_dim(), WindowRestrictions::None);

    hub.set_focus(w3);
    hub.focus_parent();

    // After this focus will be on w7
    hub.delete_window(w3);

    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(7), x=0.00, y=21.00, w=150.00, h=9.00)
        Window(id=WindowId(6), x=0.00, y=11.00, w=150.00, h=10.00)
        Window(id=WindowId(2), x=0.00, y=2.00, w=150.00, h=9.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=2, highlighted, spawn=right, titles=[W0, Container, Container])
        Container(id=ContainerId(2), x=0.00, y=2.00, w=150.00, h=28.00, titles=[W2, W6, W7])
      )

    ******************************************************************************************************************************************************
    *                       W0                        |                   Container                    |                  [Container]                    *
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W2                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W6                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W7                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn focus_tab_index() {
    let mut hub = setup();

    hub.insert_window(titled("W0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();

    hub.focus_tab_index(ContainerId::new(0), 0);
    let pre = snapshot(&hub);
    assert_snapshot!(pre, @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=0, titles=[W0, W1, W2])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                      [W0]                       |                      W1                        |                      W2                         |
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
    ******************************************************************************************************************************************************
    ");

    hub.focus_tab_index(ContainerId::new(0), 99);
    assert_eq!(snapshot(&hub), pre);
}

#[test]
fn focus_tab_noop() {
    let mut hub = setup();
    let before = snapshot(&hub);
    hub.focus_next_tab();
    assert_eq!(before, snapshot(&hub));
    hub.focus_prev_tab();
    assert_eq!(before, snapshot(&hub));

    let mut hub = setup_with_layout(layout_floating(&["w6"]));
    hub.insert_window(
        titled("w6"),
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
    hub.focus_next_tab();
    assert_eq!(before, snapshot(&hub));
    hub.focus_prev_tab();
    assert_eq!(before, snapshot(&hub));

    // Two tiling windows with no tabbed ancestor
    let mut hub = setup();
    hub.insert_window(titled("w7"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w8"), default_dim(), WindowRestrictions::None);
    let before = snapshot(&hub);
    hub.focus_next_tab();
    assert_eq!(before, snapshot(&hub));
    hub.focus_prev_tab();
    assert_eq!(before, snapshot(&hub));
}

#[test]
fn toggle_container_layout_noop() {
    let mut hub = setup();
    let before = snapshot(&hub);
    hub.toggle_container_layout();
    assert_eq!(before, snapshot(&hub));

    hub.insert_window(titled("w9"), default_dim(), WindowRestrictions::None);
    let before = snapshot(&hub);
    hub.toggle_container_layout();
    assert_eq!(before, snapshot(&hub));

    let mut hub = setup_with_layout(layout_floating(&["w10"]));
    hub.insert_window(
        titled("w10"),
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
    hub.toggle_container_layout();
    assert_eq!(before, snapshot(&hub));
}

#[test]
fn tab_bar_visible_when_min_height_exceeds_screen() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("W0"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("W1"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.set_window_constraint(w0, None, Some(60.0), None, None);
    hub.set_focus(w0);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=0, titles=[W0, W1])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                  [W0]                                    |                                  W1                                     |
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
    ");
}
