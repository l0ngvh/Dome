use super::{border_boxes_by_window, scrolling_hub, scrolling_hub_with};
use crate::core::hub::Hub;
use crate::core::node::{Length, LimitObservation, LimitUpdate, WindowId};
use crate::core::tests::{default_rect, float_border_box, titled, validate_hub};
use crate::core::{PixelRect, ScrollingConfig, SizeConstraint, WindowRestrictions};

fn sixty_wide() -> ScrollingConfig {
    ScrollingConfig {
        column_width: SizeConstraint::Percent(40.0),
    }
}

fn insert(hub: &mut Hub, title: &str) -> WindowId {
    hub.insert_window(titled(title), default_rect(), WindowRestrictions::None)
        .unwrap()
}

fn over_wide_beside_normal() -> (Hub, WindowId, WindowId) {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(200.0)),
            ..Default::default()
        },
    );
    let w1 = insert(&mut hub, "w1");
    hub.focus_left();
    (hub, w0, w1)
}

#[test]
fn scrolls_the_focused_column_against_the_right_edge() {
    let mut hub = scrolling_hub_with(sixty_wide());
    let w0 = insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");
    let w2 = insert(&mut hub, "w2");
    let w3 = insert(&mut hub, "w3");
    let ws = hub.current_workspace();
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w1, PixelRect::new(-30, 0, 60, 30)),
            (w2, PixelRect::new(30, 0, 60, 30)),
            (w3, PixelRect::new(90, 0, 60, 30)),
        ]
    );

    hub.set_focus(w0);
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(0, 0, 60, 30)),
            (w1, PixelRect::new(60, 0, 60, 30)),
            (w2, PixelRect::new(120, 0, 60, 30)),
        ]
    );

    hub.focus_right();
    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w2));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(-30, 0, 60, 30)),
            (w1, PixelRect::new(30, 0, 60, 30)),
            (w2, PixelRect::new(90, 0, 60, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn focus_into_a_hidden_over_wide_column_aligns_the_edge_it_came_from() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(200.0)),
            ..Default::default()
        },
    );
    let w1 = insert(&mut hub, "w1");
    hub.set_window_constraint(
        w1,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(148.0)),
            ..Default::default()
        },
    );
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(w1));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w1, PixelRect::new(0, 0, 150, 30))]
    );

    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w0));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w0, PixelRect::new(-52, 0, 202, 30))]
    );
    validate_hub(&hub);
}

#[test]
fn focus_into_a_partly_shown_over_wide_column_keeps_the_offset() {
    let (hub, w0, w1) = over_wide_beside_normal();
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(w0));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(-127, 0, 202, 30)),
            (w1, PixelRect::new(75, 0, 75, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn detach_clamps_the_horizontal_offset() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");
    let w2 = insert(&mut hub, "w2");
    let ws = hub.current_workspace();
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w1, PixelRect::new(0, 0, 75, 30)),
            (w2, PixelRect::new(75, 0, 75, 30)),
        ]
    );

    hub.delete_window(w2);
    assert_eq!(hub.focused_window(ws), Some(w1));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(0, 0, 75, 30)),
            (w1, PixelRect::new(75, 0, 75, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn narrower_column_clamps_the_horizontal_offset() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(200.0)),
            ..Default::default()
        },
    );
    let w1 = insert(&mut hub, "w1");
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(w1));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(-127, 0, 202, 30)),
            (w1, PixelRect::new(75, 0, 75, 30)),
        ]
    );

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Cleared,
            ..Default::default()
        },
    );
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(0, 0, 75, 30)),
            (w1, PixelRect::new(75, 0, 75, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn detaching_the_last_column_clamps_the_horizontal_offset() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(200.0)),
            ..Default::default()
        },
    );
    hub.focus_right();
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w0, PixelRect::new(-52, 0, 202, 30))]
    );

    hub.delete_window(w0);
    assert_eq!(border_boxes_by_window(&hub), vec![]);
    validate_hub(&hub);
}

#[test]
fn toggling_float_keeps_the_scrolled_on_screen_rect() {
    let mut hub = scrolling_hub();
    insert(&mut hub, "w0");
    insert(&mut hub, "w1");
    let w2 = insert(&mut hub, "w2");

    hub.toggle_float();
    assert_eq!(
        float_border_box(&hub, w2),
        Some(PixelRect::new(75, 0, 75, 30))
    );
    validate_hub(&hub);
}

#[test]
fn tall_window_reveals_its_hidden_bottom_then_stops() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );
    let ws = hub.current_workspace();

    hub.focus_down();
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w0, PixelRect::new(38, -12, 75, 42))]
    );
    hub.focus_down();
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w0, PixelRect::new(38, -12, 75, 42))]
    );
    hub.focus_up();
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w0, PixelRect::new(38, 0, 75, 42))]
    );
    assert_eq!(hub.focused_window(ws), Some(w0));
    validate_hub(&hub);
}

#[test]
fn tall_window_scrolls_one_viewport_length_per_press() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(70.0)),
            ..Default::default()
        },
    );

    for y in [-30, -42, -42] {
        hub.focus_down();
        assert_eq!(
            border_boxes_by_window(&hub),
            vec![(w0, PixelRect::new(38, y, 75, 72))]
        );
    }
    for y in [-12, 0] {
        hub.focus_up();
        assert_eq!(
            border_boxes_by_window(&hub),
            vec![(w0, PixelRect::new(38, y, 75, 72))]
        );
    }
    validate_hub(&hub);
}

#[test]
fn over_wide_column_reveals_before_focus_moves() {
    let (mut hub, w0, w1) = over_wide_beside_normal();
    let ws = hub.current_workspace();

    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w0));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w0, PixelRect::new(0, 0, 202, 30))]
    );

    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w0));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w0, PixelRect::new(-52, 0, 202, 30))]
    );

    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w1));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(-127, 0, 202, 30)),
            (w1, PixelRect::new(75, 0, 75, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn relayout_after_a_reveal_keeps_the_offset() {
    let (mut hub, w0, w1) = over_wide_beside_normal();
    hub.focus_left();
    hub.focus_right();

    hub.set_window_constraint(w1, LimitObservation::default());
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w0, PixelRect::new(-52, 0, 202, 30))]
    );
    validate_hub(&hub);
}

#[test]
fn unfocused_column_keeps_its_vertical_offset() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w1,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(w1));

    hub.focus_down();
    let scrolled = vec![
        (w0, PixelRect::new(0, 0, 75, 42)),
        (w1, PixelRect::new(75, -12, 75, 42)),
    ];
    assert_eq!(border_boxes_by_window(&hub), scrolled);

    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w0));
    assert_eq!(border_boxes_by_window(&hub), scrolled);
    validate_hub(&hub);
}

#[test]
fn shorter_window_clamps_the_vertical_offset() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );
    hub.focus_down();

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Cleared,
            ..Default::default()
        },
    );
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w0, PixelRect::new(38, 0, 75, 30))]
    );
    validate_hub(&hub);
}

#[test]
fn toggling_float_keeps_the_vertically_scrolled_rect() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );
    hub.focus_down();

    hub.toggle_float();
    assert_eq!(
        float_border_box(&hub, w0),
        Some(PixelRect::new(38, -12, 75, 42))
    );
    validate_hub(&hub);
}
