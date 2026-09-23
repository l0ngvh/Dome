use super::{border_boxes, scrolling_hub};
use crate::core::PixelRect;
use crate::core::WindowRestrictions;
use crate::core::tests::{default_rect, snapshot, titled};
use insta::assert_snapshot;

#[test]
fn new_window_opens_right_of_focus_and_takes_it() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    let ws = hub.current_workspace();

    hub.focus_left();
    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w0));

    let w3 = hub
        .insert_window(titled("w3"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_eq!(hub.focused_window(ws), Some(w3));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00, titles=[w0])
        Container(id=ContainerId(3), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w3])
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
    |                                    W0                                   |*                                    W3                                   *
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
fn detach_focused_column_moves_focus_to_the_positional_neighbor() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();

    hub.focus_left();
    hub.focus_left();
    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w1));

    hub.delete_window(w1);
    assert_eq!(
        hub.focused_window(ws),
        Some(w2),
        "focus follows the departing index positionally, not the history (which would pick {w0:?})"
    );
}

#[test]
fn focus_direction_horizontal_moves_between_columns() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(w2));

    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w1));
    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w0));
    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w0));

    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w1));
    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w2));
    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w2));
}

#[test]
fn focus_direction_vertical_leaves_focus_unchanged() {
    let mut hub = scrolling_hub();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(w1));

    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(w1));
    hub.focus_down();
    assert_eq!(hub.focused_window(ws), Some(w1));
}

#[test]
fn move_direction_horizontal_swaps_columns() {
    let mut hub = scrolling_hub();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(w1));

    hub.move_left();
    assert_eq!(hub.focused_window(ws), Some(w1));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00, highlighted)
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=75.00, h=30.00, titles=[w1])
        Container(id=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w0])
      )

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
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W1                                   *|                                    W0                                   |
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
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");

    hub.move_left();
    assert_eq!(hub.focused_window(ws), Some(w1));
    assert_eq!(
        border_boxes(&hub),
        vec![PixelRect::new(75, 0, 75, 30), PixelRect::new(0, 0, 75, 30)]
    );

    hub.move_right();
    assert_eq!(hub.focused_window(ws), Some(w1));
    assert_eq!(
        border_boxes(&hub),
        vec![PixelRect::new(0, 0, 75, 30), PixelRect::new(75, 0, 75, 30)]
    );
}

#[test]
fn move_direction_vertical_does_nothing() {
    let mut hub = scrolling_hub();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

    let before = snapshot(&hub);
    hub.move_up();
    assert_eq!(snapshot(&hub), before);
    hub.move_down();
    assert_eq!(snapshot(&hub), before);
}

#[test]
fn bindings_without_meaning_are_ignored() {
    let mut hub = scrolling_hub();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

    let before = snapshot(&hub);
    hub.toggle_container_layout();
    assert_eq!(snapshot(&hub), before);
    hub.focus_next_tab();
    assert_eq!(snapshot(&hub), before);
    hub.focus_prev_tab();
    assert_eq!(snapshot(&hub), before);
}
