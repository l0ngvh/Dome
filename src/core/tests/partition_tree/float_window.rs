use crate::core::node::Dimension;
use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn focus_falls_back_to_container_focus_after_float_delete() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    // Focus W1 (middle window)
    hub.focus_left();

    let f0 = hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 40.0,
        height: 15.0,
    });

    hub.delete_window(f0);

    // Focus should fall back to W1 (container's focus), not W2 (last window)
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[, , ])
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
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 40.0,
        height: 15.0,
    });
    hub.toggle_float();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=15.00, w=50.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=50.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[, , ])
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

    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_parent();
    // After focus_parent, focused_tiling_window() returns None (container highlighted).
    // toggle_float is a no-op: both windows stay tiling, container stays highlighted.
    hub.toggle_float();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right, titles=[, ])
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
    let w0 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    let w2 = hub.insert_tiling();
    hub.set_window_constraint(w2, Some(100.0), None, None, None);

    // Focus w2 scrolls viewport right (offset = 150, since total 300px, screen 150px)
    hub.set_focus(w2);
    hub.toggle_float();

    assert!(hub.get_window(w2).is_float());
    // Layout x=200, offset=150, screen.x=0 => screen-absolute x = 200 - 150 + 0 = 50
    let ws_id = hub.current_workspace();
    let float_dim = hub
        .access
        .workspaces
        .get(ws_id)
        .float_windows()
        .iter()
        .find(|&&(id, _)| id == w2)
        .unwrap()
        .1;
    assert_eq!(float_dim.x, 50.0);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=50.00, y=0.00, w=100.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=50.00, y=0.00, w=100.00, h=30.00, float, highlighted)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[, ])
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
    let w0 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, Some(100.0), None, None, None);

    // Make w1 a float
    hub.set_focus(w1);
    hub.toggle_float();

    // Focus w0 (the only tiling window, viewport resets)
    hub.set_focus(w0);

    // Focus the float and toggle back to tiling
    hub.set_focus(w1);
    hub.toggle_float();

    assert!(!hub.get_window(w1).is_float());
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=50.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[, ])
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
