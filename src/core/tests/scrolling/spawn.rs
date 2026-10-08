use super::{border_boxes_by_window, dev_hub, insert, process_matcher, scrolling_hub_with};
use crate::core::hub::{Hub, MonitorLayout};
use crate::core::node::{Direction, WindowId};
use crate::core::tests::validate_hub;
use crate::core::{ColumnConfig, PixelRect, ScrollingConfig, SizeConstraint};

fn narrow_hub() -> Hub {
    scrolling_hub_with(ScrollingConfig {
        column_width: SizeConstraint::Percent(20.0),
    })
}

/// Tiling windows sorted by id, with the spawn direction each placement carries.
/// Asserts that no container placement carries one.
fn spawn_directions(hub: &Hub) -> Vec<(WindowId, Option<Direction>)> {
    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal {
        tiling_windows,
        containers,
        ..
    } = &placements.monitors[0].layout
    else {
        panic!("expected a normal layout");
    };
    assert!(
        containers.iter().all(|c| c.spawn_direction.is_none()),
        "a scrolling column placement carries a spawn direction"
    );
    let mut directions: Vec<(WindowId, Option<Direction>)> = tiling_windows
        .iter()
        .map(|w| (w.id, w.spawn_direction))
        .collect();
    directions.sort_by_key(|(id, _)| id.get());
    directions
}

#[test]
fn the_highlighted_window_shows_its_spawn_direction() {
    let mut hub = narrow_hub();
    let w0 = insert(&mut hub, "w0.exe");
    let w1 = insert(&mut hub, "w1.exe");

    assert_eq!(
        spawn_directions(&hub),
        vec![(w0, None), (w1, Some(Direction::Horizontal))]
    );

    hub.toggle_spawn_mode();
    assert_eq!(
        spawn_directions(&hub),
        vec![(w0, None), (w1, Some(Direction::Vertical))]
    );
    validate_hub(&hub);
}

#[test]
fn toggle_split_opens_the_next_window_below_the_focused_window() {
    let mut hub = narrow_hub();
    let w0 = insert(&mut hub, "w0.exe");
    hub.toggle_spawn_mode();
    let w1 = insert(&mut hub, "w1.exe");

    assert_eq!(hub.focused_window(hub.current_workspace()), Some(w1));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(60, 0, 30, 15)),
            (w1, PixelRect::new(60, 15, 30, 15)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_window_opened_below_keeps_stacking_the_next_window() {
    let mut hub = narrow_hub();
    let w0 = insert(&mut hub, "w0.exe");
    hub.toggle_spawn_mode();
    let w1 = insert(&mut hub, "w1.exe");
    let w2 = insert(&mut hub, "w2.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(60, 0, 30, 10)),
            (w1, PixelRect::new(60, 10, 30, 10)),
            (w2, PixelRect::new(60, 20, 30, 10)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_second_toggle_split_opens_the_next_window_as_a_column() {
    let mut hub = narrow_hub();
    let w0 = insert(&mut hub, "w0.exe");
    hub.toggle_spawn_mode();
    let w1 = insert(&mut hub, "w1.exe");
    hub.toggle_spawn_mode();
    let w2 = insert(&mut hub, "w2.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(45, 0, 30, 15)),
            (w1, PixelRect::new(45, 15, 30, 15)),
            (w2, PixelRect::new(75, 0, 30, 30)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn toggle_split_changes_only_the_focused_window() {
    let mut hub = narrow_hub();
    let w0 = insert(&mut hub, "w0.exe");
    let w1 = insert(&mut hub, "w1.exe");
    hub.set_focus(w0);
    hub.toggle_spawn_mode();

    hub.set_focus(w1);
    let w2 = insert(&mut hub, "w2.exe");
    hub.set_focus(w0);
    let w3 = insert(&mut hub, "w3.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(30, 0, 30, 15)),
            (w1, PixelRect::new(60, 0, 30, 30)),
            (w2, PixelRect::new(90, 0, 30, 30)),
            (w3, PixelRect::new(30, 15, 30, 15)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_window_moved_in_follows_the_spawn_direction() {
    let mut hub = narrow_hub();
    let w0 = insert(&mut hub, "w0.exe");
    hub.toggle_spawn_mode();

    hub.focus_workspace("1", None);
    let w1 = insert(&mut hub, "w1.exe");
    hub.move_focused_to_workspace("0", None);
    hub.focus_workspace("0", None);

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(60, 0, 30, 15)),
            (w1, PixelRect::new(60, 15, 30, 15)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_window_with_no_free_matcher_follows_the_spawn_direction() {
    let mut hub = dev_hub(vec![ColumnConfig::bare(process_matcher("t.exe"))]);
    let t0 = insert(&mut hub, "t.exe");
    hub.toggle_spawn_mode();
    let t1 = insert(&mut hub, "t.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (t0, PixelRect::new(60, 0, 30, 15)),
            (t1, PixelRect::new(60, 15, 30, 15)),
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_free_matcher_wins_over_the_spawn_direction() {
    let mut hub = dev_hub(vec![
        ColumnConfig::bare(process_matcher("a.exe")),
        ColumnConfig::bare(process_matcher("b.exe")),
    ]);
    let a = insert(&mut hub, "a.exe");
    hub.toggle_spawn_mode();
    let b = insert(&mut hub, "b.exe");
    let u = insert(&mut hub, "u.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(30, 0, 30, 30)),
            (b, PixelRect::new(60, 0, 30, 30)),
            (u, PixelRect::new(90, 0, 30, 30)),
        ],
        "a window that opens in a layout column starts horizontal"
    );
    validate_hub(&hub);
}
