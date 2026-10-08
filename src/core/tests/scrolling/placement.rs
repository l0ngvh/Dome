use super::{border_boxes, border_boxes_by_window, insert, scrolling_hub, scrolling_hub_with};
use crate::core::hub::{Hub, MonitorLayout};
use crate::core::node::{Length, LimitObservation, LimitUpdate, WindowId};
use crate::core::tests::{
    PRIMARY_MONITOR, default_rect, reported_monitor, snapshot, titled, validate_hub,
};
use crate::core::{PixelRect, ScrollingConfig, SizeConstraint, WindowRestrictions};
use insta::assert_snapshot;

/// A tile is partially off screen when the work area clips its border box.
fn partially_off_screen_by_window(hub: &Hub) -> Vec<(WindowId, bool)> {
    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal { tiling_windows, .. } = &placements.monitors[0].layout else {
        panic!("expected a normal layout");
    };
    let mut off_screen: Vec<(WindowId, bool)> = tiling_windows
        .iter()
        .map(|w| (w.id, w.visible_border_box != w.border_box))
        .collect();
    off_screen.sort_by_key(|(id, _)| id.get());
    off_screen
}

#[test]
fn a_row_narrower_than_the_work_area_is_centered() {
    let mut hub = scrolling_hub_with(ScrollingConfig {
        column_width: SizeConstraint::Percent(20.0),
    });
    let w0 = insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(45, 0, 30, 30)),
            (w1, PixelRect::new(75, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn the_row_centers_again_when_a_column_closes() {
    let mut hub = scrolling_hub_with(ScrollingConfig {
        column_width: SizeConstraint::Percent(20.0),
    });
    let w0 = insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");
    let w2 = insert(&mut hub, "w2");
    hub.delete_window(w1);
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(45, 0, 30, 30)),
            (w2, PixelRect::new(75, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn single_window_takes_the_default_column_width() {
    let mut hub = scrolling_hub();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(38, 0, 75, 30)]);
    validate_hub(&hub);
}

#[test]
fn three_columns_overflow_and_the_first_scrolls_offscreen() {
    let mut hub = scrolling_hub();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=75.00, h=30.00, titles=[w1])
        Container(id=ContainerId(2), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w2])
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
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn only_the_column_the_left_edge_cuts_is_partially_off_screen() {
    let mut hub = scrolling_hub_with(ScrollingConfig {
        column_width: SizeConstraint::Percent(40.0),
    });
    let [_, w1, w2, w3] = ["w0", "w1", "w2", "w3"].map(|title| {
        hub.insert_window(titled(title), default_rect(), WindowRestrictions::None)
            .unwrap()
    });

    assert_eq!(
        partially_off_screen_by_window(&hub),
        vec![(w1, true), (w2, false), (w3, false)]
    );
    validate_hub(&hub);
}

#[test]
fn detaching_a_column_shifts_the_survivors_left() {
    let mut hub = scrolling_hub();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let middle = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);

    hub.delete_window(middle);

    assert_eq!(
        border_boxes(&hub),
        vec![PixelRect::new(0, 0, 75, 30), PixelRect::new(75, 0, 75, 30)]
    );
    validate_hub(&hub);
}

#[test]
fn min_width_widens_the_column() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(24, 0, 102, 30)]);
    validate_hub(&hub);
}

#[test]
fn a_focused_column_wider_than_the_work_area_is_partially_off_screen() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(200.0)),
            ..Default::default()
        },
    );
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.focus_left();

    assert_eq!(
        partially_off_screen_by_window(&hub),
        vec![(w0, true), (w1, false)]
    );
    validate_hub(&hub);
}

#[test]
fn min_height_overflows_below_the_screen() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(38, 0, 75, 42)]);
    assert_eq!(partially_off_screen_by_window(&hub), vec![(w0, true)]);
    validate_hub(&hub);
}

#[test]
fn max_width_narrows_the_column() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(23.0)),
            ..Default::default()
        },
    );
    assert_eq!(
        border_boxes(&hub),
        vec![PixelRect::new(25, 0, 25, 30), PixelRect::new(50, 0, 75, 30)]
    );
    validate_hub(&hub);
}

#[test]
fn a_stack_takes_the_smallest_maximum_width() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(23.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w1,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(38.0)),
            ..Default::default()
        },
    );
    assert_eq!(
        border_boxes(&hub),
        vec![
            PixelRect::new(63, 0, 25, 15),
            PixelRect::new(63, 15, 25, 15)
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_minimum_beats_a_smaller_maximum() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w1,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(22.0)),
            ..Default::default()
        },
    );
    assert_eq!(
        border_boxes(&hub),
        vec![
            PixelRect::new(54, 0, 42, 15),
            PixelRect::new(63, 15, 24, 15)
        ]
    );
    validate_hub(&hub);
}

#[test]
fn max_height_centers_the_window_vertically() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(38, 4, 75, 22)]);
    validate_hub(&hub);
}

#[test]
fn a_percentage_column_width_follows_the_work_area() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");

    hub.update_monitor(
        hub.primary_monitor(),
        reported_monitor(
            PRIMARY_MONITOR.to_string(),
            PixelRect::new(0, 0, 200, 30),
            1.0,
        ),
        None,
    );

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(0, 0, 100, 30)),
            (w1, PixelRect::new(100, 0, 100, 30)),
        ]
    );
    validate_hub(&hub);
}
