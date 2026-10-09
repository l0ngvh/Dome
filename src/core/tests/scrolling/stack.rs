use super::{border_boxes_by_window, dev_hub, insert, process_matcher, stack};
use crate::core::node::{Length, LimitObservation, LimitUpdate};
use crate::core::tests::{float_border_box, validate_hub};
use crate::core::{ColumnConfig, PixelRect, SizeConstraint};

#[test]
fn a_layout_column_stacks_its_windows_in_matcher_order() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe"])]);
    let b = insert(&mut hub, "b.exe");
    let a = insert(&mut hub, "a.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (b, PixelRect::new(60, 15, 30, 15)),
            (a, PixelRect::new(60, 0, 30, 15)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn stacked_windows_share_the_column_height() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe", "c.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    let c = insert(&mut hub, "c.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 0, 30, 10)),
            (b, PixelRect::new(60, 10, 30, 10)),
            (c, PixelRect::new(60, 20, 30, 10)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_larger_minimum_height_keeps_its_minimum_in_a_stack() {
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

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 0, 30, 22)),
            (b, PixelRect::new(60, 22, 30, 8)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_capped_window_is_centered_in_its_share() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    hub.set_window_constraint(
        a,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(9.0)),
            ..Default::default()
        },
    );

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 2, 30, 11)),
            (b, PixelRect::new(60, 15, 30, 15)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn stacked_minimums_past_the_work_area_scroll_the_focused_window_into_view() {
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

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, -14, 30, 22)),
            (b, PixelRect::new(60, 8, 30, 22)),
        ]
    );

    hub.set_focus(a);
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
fn toggling_float_in_a_scrolled_stack_keeps_the_on_screen_rect() {
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

    hub.toggle_float();
    assert_eq!(
        float_border_box(&hub, b),
        Some(PixelRect::new(60, 8, 30, 22))
    );
    validate_hub(&hub);
}

#[test]
fn a_stacked_column_takes_the_largest_minimum_width() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    hub.set_window_constraint(
        b,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(54, 0, 42, 15)),
            (b, PixelRect::new(54, 15, 42, 15)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_window_with_no_free_matcher_opens_as_an_unmatched_column() {
    let mut hub = dev_hub(vec![ColumnConfig {
        width: Some(SizeConstraint::Percent(40.0)),
        children: vec![process_matcher("t.exe"), process_matcher("t.exe")],
    }]);
    let t0 = insert(&mut hub, "t.exe");
    let t1 = insert(&mut hub, "t.exe");
    let t2 = insert(&mut hub, "t.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (t0, PixelRect::new(30, 0, 60, 15)),
            (t1, PixelRect::new(30, 15, 60, 15)),
            (t2, PixelRect::new(90, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_repeated_layout_column_takes_the_next_matching_window() {
    let mut hub = dev_hub(vec![
        ColumnConfig::bare(process_matcher("t.exe")),
        ColumnConfig {
            width: Some(SizeConstraint::Percent(40.0)),
            children: vec![process_matcher("t.exe")],
        },
    ]);
    let t0 = insert(&mut hub, "t.exe");
    let t1 = insert(&mut hub, "t.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (t0, PixelRect::new(30, 0, 30, 30)),
            (t1, PixelRect::new(60, 0, 60, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn closing_a_focused_stacked_window_focuses_below_then_above() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe", "c.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    let c = insert(&mut hub, "c.exe");
    let ws = hub.current_workspace();
    hub.set_focus(a);
    hub.set_focus(b);

    hub.delete_window(b);
    assert_eq!(
        hub.focused_window(ws),
        Some(c),
        "focus follows the row the removed window vacated, not the history head {a:?}"
    );

    hub.delete_window(c);
    assert_eq!(hub.focused_window(ws), Some(a));
    validate_hub(&hub);
}

#[test]
fn closing_a_stacked_window_frees_its_matcher() {
    let mut hub = dev_hub(vec![stack(&["t.exe", "t.exe"])]);
    let t0 = insert(&mut hub, "t.exe");
    let t1 = insert(&mut hub, "t.exe");

    hub.delete_window(t0);
    let t2 = insert(&mut hub, "t.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (t1, PixelRect::new(60, 15, 30, 15)),
            (t2, PixelRect::new(60, 0, 30, 15)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn focus_into_a_stacked_column_takes_its_last_focused_window() {
    let mut hub = dev_hub(vec![
        stack(&["a.exe", "b.exe"]),
        ColumnConfig::bare(process_matcher("c.exe")),
    ]);
    let _a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    let c = insert(&mut hub, "c.exe");
    let ws = hub.current_workspace();
    hub.set_focus(b);
    hub.set_focus(c);

    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(b));
    validate_hub(&hub);
}
