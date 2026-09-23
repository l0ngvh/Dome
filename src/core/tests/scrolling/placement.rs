use super::{border_boxes, scrolling_hub};
use crate::core::PixelRect;
use crate::core::WindowRestrictions;
use crate::core::node::{Length, LimitObservation, LimitUpdate};
use crate::core::tests::{default_rect, snapshot, titled, validate_hub};
use insta::assert_snapshot;

#[test]
fn single_window_takes_the_default_column_width() {
    let mut hub = scrolling_hub();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(0, 0, 75, 30)]);
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
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
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
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(0, 0, 102, 30)]);
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
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(0, 0, 75, 42)]);
    validate_hub(&hub);
}

#[test]
fn max_width_centers_the_window_in_the_column() {
    let mut hub = scrolling_hub();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(23.0)),
            ..Default::default()
        },
    );
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(25, 0, 25, 30)]);
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
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(0, 4, 75, 22)]);
    validate_hub(&hub);
}
