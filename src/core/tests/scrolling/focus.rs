use super::{border_boxes, border_boxes_by_window, dev_hub, insert, scrolling_hub, stack};
use crate::core::PixelRect;
use crate::core::WindowRestrictions;
use crate::core::node::{Length, LimitObservation, LimitUpdate};
use crate::core::tests::{default_rect, snapshot, titled, validate_hub};
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
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
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
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
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
    hub.toggle_direction();
    assert_eq!(snapshot(&hub), before);
}

#[test]
fn focus_up_and_down_move_within_a_stack() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe", "c.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    let c = insert(&mut hub, "c.exe");
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(c));

    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(b));
    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(a));
    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(a));

    hub.focus_down();
    assert_eq!(hub.focused_window(ws), Some(b));
    hub.focus_down();
    assert_eq!(hub.focused_window(ws), Some(c));
    hub.focus_down();
    assert_eq!(hub.focused_window(ws), Some(c));
    validate_hub(&hub);
}

#[test]
fn vertical_focus_scrolls_the_new_window_into_view() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    hub.set_window_constraint(
        a,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        b,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    let ws = hub.current_workspace();

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, -14, 30, 22)),
            (b, PixelRect::new(60, 8, 30, 22)),
        ]
    );
    assert_eq!(hub.focused_window(ws), Some(b));

    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(a));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 0, 30, 22)),
            (b, PixelRect::new(60, 22, 30, 22)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn move_up_and_down_reorder_a_stack() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe", "c.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    let c = insert(&mut hub, "c.exe");
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(c));

    hub.move_up();
    // a, c, b
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 0, 30, 10)),
            (b, PixelRect::new(60, 20, 30, 10)),
            (c, PixelRect::new(60, 10, 30, 10)),
        ]
    );

    hub.move_up();
    // c, a, b
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 10, 30, 10)),
            (b, PixelRect::new(60, 20, 30, 10)),
            (c, PixelRect::new(60, 0, 30, 10)),
        ]
    );

    hub.move_up();
    // c, a, b unchanged at the top edge
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 10, 30, 10)),
            (b, PixelRect::new(60, 20, 30, 10)),
            (c, PixelRect::new(60, 0, 30, 10)),
        ]
    );

    hub.move_down();
    // a, c, b
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 0, 30, 10)),
            (b, PixelRect::new(60, 20, 30, 10)),
            (c, PixelRect::new(60, 10, 30, 10)),
        ]
    );

    hub.move_down();
    // a, b, c
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 0, 30, 10)),
            (b, PixelRect::new(60, 10, 30, 10)),
            (c, PixelRect::new(60, 20, 30, 10)),
        ]
    );
    assert_eq!(hub.focused_window(ws), Some(c));
    validate_hub(&hub);
}

#[test]
fn move_right_takes_a_stacked_window_out_to_its_right() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(b));

    hub.move_right();
    assert_eq!(hub.focused_window(ws), Some(b));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(45, 0, 30, 30)),
            (b, PixelRect::new(75, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn move_left_takes_a_stacked_window_out_to_its_left() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    let ws = hub.current_workspace();
    hub.set_focus(a);

    hub.move_left();
    assert_eq!(hub.focused_window(ws), Some(a));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(45, 0, 30, 30)),
            (b, PixelRect::new(75, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_stacked_window_moves_out_at_the_row_edge() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(b));

    hub.move_left();
    assert_eq!(hub.focused_window(ws), Some(b));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(75, 0, 30, 30)),
            (b, PixelRect::new(45, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}
