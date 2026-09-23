use crate::core::node::{Length, LimitObservation, LimitUpdate, WindowRestrictions};
use crate::core::tests::{default_rect, setup, snapshot, titled};

#[test]
fn min_width_does_not_change_partition_tree_split() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

    let before = snapshot(&hub);
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );

    assert_eq!(
        before,
        snapshot(&hub),
        "min_width must not reshape the even split"
    );
}

#[test]
fn max_width_does_not_change_partition_tree_split() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

    let before = snapshot(&hub);
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(50.0)),
            ..Default::default()
        },
    );

    assert_eq!(
        before,
        snapshot(&hub),
        "max_width must not reshape the even split"
    );
}

#[test]
fn min_height_does_not_change_partition_tree_split() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

    let before = snapshot(&hub);
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );

    assert_eq!(
        before,
        snapshot(&hub),
        "Partition Tree must ignore the minimum height Master respects"
    );
}

#[test]
fn clearing_a_constraint_leaves_the_even_split() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

    let even_split = snapshot(&hub);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );
    assert_eq!(
        even_split,
        snapshot(&hub),
        "setting min_width must not reshape the even split"
    );

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Cleared,
            ..Default::default()
        },
    );
    assert_eq!(
        even_split,
        snapshot(&hub),
        "clearing the constraint must leave the even split intact"
    );
}
