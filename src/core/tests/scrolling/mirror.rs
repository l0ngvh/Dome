use super::{scrolling_hub, scrolling_hub_with};
use crate::core::hub::{Hub, MonitorLayout};
use crate::core::node::{Length, LimitObservation, LimitUpdate, WindowId};
use crate::core::tests::{default_rect, titled, validate_hub};
use crate::core::{ScrollingConfig, SizeConstraint, WindowRestrictions};

fn insert(hub: &mut Hub, title: &str) -> WindowId {
    hub.insert_window(titled(title), default_rect(), WindowRestrictions::None)
        .unwrap()
}

/// Whether each visible tiling window on the primary monitor is mirrored, sorted by window id.
fn mirrored_by_window(hub: &Hub) -> Vec<(WindowId, bool)> {
    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal { tiling_windows, .. } = &placements.monitors[0].layout else {
        panic!("expected a normal layout");
    };
    let mut mirrored: Vec<(WindowId, bool)> = tiling_windows
        .iter()
        .map(|w| (w.id, w.is_mirrored))
        .collect();
    mirrored.sort_by_key(|(id, _)| id.get());
    mirrored
}

#[test]
fn only_the_unfocused_column_the_left_edge_cuts_is_mirrored() {
    let mut hub = scrolling_hub_with(ScrollingConfig {
        default_column_width: SizeConstraint::Percent(40.0),
    });
    insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");
    let w2 = insert(&mut hub, "w2");
    let w3 = insert(&mut hub, "w3");

    assert_eq!(
        mirrored_by_window(&hub),
        vec![(w1, true), (w2, false), (w3, false)]
    );
    validate_hub(&hub);
}

#[test]
fn a_focused_column_wider_than_the_work_area_is_not_mirrored() {
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
    assert_eq!(mirrored_by_window(&hub), vec![(w0, false), (w1, false)]);

    hub.focus_right();
    assert_eq!(mirrored_by_window(&hub), vec![(w0, true), (w1, false)]);
    validate_hub(&hub);
}

#[test]
fn a_column_the_bottom_edge_cuts_is_mirrored_once_focus_leaves_it() {
    let mut hub = scrolling_hub();
    let w0 = insert(&mut hub, "w0");
    let tall = LimitObservation {
        min_height: LimitUpdate::Set(Length::new(40.0)),
        ..Default::default()
    };
    hub.set_window_constraint(w0, tall);
    let w1 = insert(&mut hub, "w1");
    assert_eq!(mirrored_by_window(&hub), vec![(w0, true), (w1, false)]);

    hub.focus_left();
    assert_eq!(mirrored_by_window(&hub), vec![(w0, false), (w1, false)]);
    validate_hub(&hub);
}
