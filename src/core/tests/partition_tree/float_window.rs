use crate::core::ContainerId;
use crate::core::allocator::NodeId;
use crate::core::node::{PixelRect, WindowRestrictions};
use crate::core::tests::{default_rect, setup, setup_with_modes, snapshot, titled};
use insta::assert_snapshot;

#[test]
fn focus_falls_back_to_last_focused_window_after_float_delete() {
    let mut hub = setup_with_modes("0", &["w3"], &[]);
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);

    // Focus W1 (middle window)
    hub.focus_left();

    let f0 = hub
        .insert_window(
            titled("w3"),
            PixelRect::new(50, 5, 40, 15),
            WindowRestrictions::None,
        )
        .unwrap();

    hub.delete_window(f0);

    // Focus should fall back to W1 (last focused), not W2 (last window)
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
    let mut hub = setup_with_modes("0", &["w7"], &[]);
    hub.insert_window(titled("w4"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w5"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w6"), default_rect(), WindowRestrictions::None);
    hub.insert_window(
        titled("w7"),
        PixelRect::new(50, 5, 40, 15),
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

    hub.insert_window(titled("w8"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w9"), default_rect(), WindowRestrictions::None);
    hub.focus_parent();
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
fn focus_direction_keeps_float_focus() {
    let mut hub = setup_with_modes("0", &["w2"], &[]);
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    let float_id = hub
        .insert_window(
            titled("w2"),
            PixelRect::new(50, 5, 40, 15),
            WindowRestrictions::None,
        )
        .unwrap();
    let ws = hub.current_workspace();

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
    let mut hub = setup_with_modes("0", &["w2"], &[]);
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    let float_id = hub
        .insert_window(
            titled("w2"),
            PixelRect::new(50, 5, 40, 15),
            WindowRestrictions::None,
        )
        .unwrap();
    let ws = hub.current_workspace();

    let before = snapshot(&hub);
    hub.focus_parent();
    assert_eq!(before, snapshot(&hub));
    assert_eq!(hub.focused_window(ws), Some(float_id));
}

#[test]
fn move_direction_keeps_float_focus() {
    let mut hub = setup_with_modes("0", &["w2"], &[]);
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    let float_id = hub
        .insert_window(
            titled("w2"),
            PixelRect::new(50, 5, 40, 15),
            WindowRestrictions::None,
        )
        .unwrap();
    let ws = hub.current_workspace();

    let before = snapshot(&hub);
    hub.move_up();

    assert_eq!(hub.focused_window(ws), Some(float_id));
    assert_eq!(
        snapshot(&hub),
        before,
        "a move while the float has focus leaves the tiling layout as it was"
    );
}

#[test]
fn focus_next_tab_keeps_float_focus_and_a_tab_click_takes_it() {
    let mut hub = setup_with_modes("0", &["w2"], &[]);
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    let float_id = hub
        .insert_window(
            titled("w2"),
            PixelRect::new(50, 5, 40, 15),
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
    assert_eq!(snapshot(&hub), w1_front_snapshot);

    hub.focus_tab_index(ContainerId::new(0), 0);
    assert_eq!(hub.focused_window(ws), Some(w0));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=50.00, y=5.00, w=40.00, h=15.00, float)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=0, titles=[w0, w1])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                  [w0]                                    |                                  w1                                     |
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                 +--------------------------------------+                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                  F2                  |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 |                                      |                                                           *
    *                                                 +--------------------------------------+                                                           *
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
