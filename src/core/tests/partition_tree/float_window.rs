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
fn focus_falls_back_to_container_focus_after_float_delete() {
    let mut hub = setup_with_layout(layout_floating(&["w3"]));
    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_dim(), WindowRestrictions::None);

    // Focus W1 (middle window)
    hub.focus_left();

    let f0 = hub
        .insert_window(
            titled("w3"),
            Dimension::new(
                Length::new(50.0),
                Length::new(5.0),
                Length::new(40.0),
                Length::new(15.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();

    hub.delete_window(f0);

    // Focus should fall back to W1 (container's focus), not W2 (last window)
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1, w2])
      )

    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                       W0                       |*                       W1                       *|                       W2                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn toggle_float_to_tiling_with_nested_containers() {
    let mut hub = setup_with_layout(layout_floating(&["w7"]));
    hub.insert_window(titled("w4"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w5"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w6"), default_dim(), WindowRestrictions::None);
    hub.insert_window(
        titled("w7"),
        Dimension::new(
            Length::new(50.0),
            Length::new(5.0),
            Length::new(40.0),
            Length::new(15.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.toggle_float();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=15.00, w=50.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=50.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w4, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w5, w6, w7])
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
    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                       W1                       ||                       W2                       |*                       W3                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}

#[test]
fn toggle_float_with_container_focused() {
    let mut hub = setup();

    hub.insert_window(titled("w8"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w9"), default_dim(), WindowRestrictions::None);
    hub.focus_parent();
    // After focus_parent, focused_tiling_window() returns None (container highlighted).
    // toggle_float is a no-op: both windows stay tiling, container stays highlighted.
    hub.toggle_float();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right, titles=[w8, w9])
      )

    ******************************************************************************************************************************************************
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
    *                                                                         ||                                                                         *
    *                                    W0                                   ||                                    W1                                   *
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn toggle_float_with_scrolled_viewport() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w10"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let w1 = hub
        .insert_window(titled("w11"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    let w2 = hub
        .insert_window(titled("w12"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w2, Some(100.0), None, None, None);

    // Focus w2 scrolls viewport right (offset = 150, since total 300px, screen 150px)
    hub.set_focus(w2);
    hub.toggle_float();

    // Layout x=200, offset=150, screen.x=0 => screen-absolute x = 200 - 150 + 0 = 50
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=50.00, y=0.00, w=100.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=50.00, y=0.00, w=100.00, h=30.00, float, highlighted)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w10, w11])
      )

    -------------------------------------------------+****************************************************************************************************
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                            W0                       |*                                                F2                                                *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    -------------------------------------------------+****************************************************************************************************
    ");
}

#[test]
fn toggle_float_to_tiling_with_scrolled_viewport() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w13"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let w1 = hub
        .insert_window(titled("w14"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w1, Some(100.0), None, None, None);

    // Make w1 a float
    hub.set_focus(w1);
    hub.toggle_float();

    // Focus w0 (the only tiling window, viewport resets)
    hub.set_focus(w0);

    // Focus the float and toggle back to tiling
    hub.set_focus(w1);
    hub.toggle_float();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=50.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w13, w14])
      )

    -------------------------------------------------+****************************************************************************************************
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                            W0                       |*                                                W1                                                *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    -------------------------------------------------+****************************************************************************************************
    ");
}

#[test]
fn focus_direction_keeps_float_focus() {
    let mut hub = setup_with_layout(layout_floating(&["w2"]));
    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    let float_id = hub
        .insert_window(
            titled("w2"),
            Dimension::new(
                Length::new(50.0),
                Length::new(5.0),
                Length::new(40.0),
                Length::new(15.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
    let ws = hub.current_workspace();

    // The tiling focus moves underneath, but nothing renders under a float.
    let before = snapshot(&hub);
    hub.focus_left();
    assert_eq!(before, snapshot(&hub));
    hub.focus_right();
    assert_eq!(before, snapshot(&hub));
    hub.focus_up();
    assert_eq!(before, snapshot(&hub));
    hub.focus_down();
    assert_eq!(before, snapshot(&hub));
    assert_eq!(hub.focused_window(ws), Some(float_id));
}

#[test]
fn focus_parent_keeps_float_focus() {
    let mut hub = setup_with_layout(layout_floating(&["w2"]));
    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    let float_id = hub
        .insert_window(
            titled("w2"),
            Dimension::new(
                Length::new(50.0),
                Length::new(5.0),
                Length::new(40.0),
                Length::new(15.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
    let ws = hub.current_workspace();

    // Focus moves up to the container underneath, but nothing renders under a float.
    let before = snapshot(&hub);
    hub.focus_parent();
    assert_eq!(before, snapshot(&hub));
    assert_eq!(hub.focused_window(ws), Some(float_id));
}

#[test]
fn move_direction_keeps_float_focus() {
    let mut hub = setup_with_layout(layout_floating(&["w2"]));
    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    let float_id = hub
        .insert_window(
            titled("w2"),
            Dimension::new(
                Length::new(50.0),
                Length::new(5.0),
                Length::new(40.0),
                Length::new(15.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
    let ws = hub.current_workspace();

    // Vertical out of a horizontal root, which is the branch that re-focuses the
    // moved window. W1 is the tiling focus under the float, so it is the one that
    // relocates.
    hub.move_up();

    assert_eq!(hub.focused_window(ws), Some(float_id));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=15.00, w=150.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=15.00)
        Window(id=WindowId(2), x=50.00, y=5.00, w=40.00, h=15.00, float, highlighted)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w1, w0])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                 ****************************************                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                  F2                  *                                                           |
    +-------------------------------------------------*                                      *-----------------------------------------------------------+
    +-------------------------------------------------*                                      *-----------------------------------------------------------+
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 ****************************************                                                           |
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
    ");
}

#[test]
fn tab_switch_keeps_float_focus() {
    let mut hub = setup_with_layout(layout_floating(&["w2"]));
    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    hub.toggle_container_layout();
    let float_id = hub
        .insert_window(
            titled("w2"),
            Dimension::new(
                Length::new(50.0),
                Length::new(5.0),
                Length::new(40.0),
                Length::new(15.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
    let ws = hub.current_workspace();

    let w1_front_snapshot = snapshot(&hub);
    assert_snapshot!(w1_front_snapshot, @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=28.00)
        Window(id=WindowId(2), x=50.00, y=5.00, w=40.00, h=15.00, float, highlighted)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[w0, w1])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   w0                                     |                                 [w1]                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                 ****************************************                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                  F2                  *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 ****************************************                                                           |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");

    hub.focus_next_tab();
    assert_eq!(hub.focused_window(ws), Some(float_id));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=2.00, w=150.00, h=28.00)
        Window(id=WindowId(2), x=50.00, y=5.00, w=40.00, h=15.00, float, highlighted)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=0, titles=[w0, w1])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                  [w0]                                    |                                  w1                                     |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                 ****************************************                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                  F2                  *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 ****************************************                                                           |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");

    hub.focus_tab_index(ContainerId::new(0), 1);
    assert_eq!(hub.focused_window(ws), Some(float_id));
    assert_eq!(snapshot(&hub), w1_front_snapshot);
}
