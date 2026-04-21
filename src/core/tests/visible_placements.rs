use super::setup;

use crate::core::hub::{Hub, MonitorLayout, MonitorPlacements};

fn placements(hub: &Hub) -> MonitorPlacements {
    let mut all = hub.get_visible_placements();
    assert_eq!(all.len(), 1);
    all.remove(0)
}

fn normal_windows(p: &MonitorPlacements) -> &[crate::core::hub::WindowPlacement] {
    match &p.layout {
        MonitorLayout::Normal { windows, .. } => windows,
        MonitorLayout::Fullscreen(_) => panic!("expected Normal layout"),
    }
}

#[test]
fn partially_visible_window_has_clipped_visible_frame() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    // Total 200px, screen 150px — focus w0 so w1 is partially visible
    hub.set_focus(w0);

    let p = placements(&hub);
    let w1p = normal_windows(&p).iter().find(|wp| wp.id == w1).unwrap();

    assert_eq!(w1p.frame.width, 100.0);
    assert!(w1p.visible_frame.width < w1p.frame.width);
}

#[test]
fn focused_window_tagged() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.set_focus(w0);

    let p = placements(&hub);

    assert!(
        normal_windows(&p)
            .iter()
            .find(|wp| wp.id == w0)
            .unwrap()
            .is_focused
    );
    assert!(
        !normal_windows(&p)
            .iter()
            .find(|wp| wp.id == w1)
            .unwrap()
            .is_focused
    );
}

#[test]
fn viewport_offset_survives_relayout() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    let w2 = hub.insert_tiling();
    hub.set_window_constraint(w2, Some(100.0), None, None, None);

    hub.set_focus(w2); // scrolls to show w2
    let x_before = normal_windows(&placements(&hub))
        .iter()
        .find(|wp| wp.id == w2)
        .unwrap()
        .frame
        .x;

    // Trigger relayout — offset should be preserved
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let x_after = normal_windows(&placements(&hub))
        .iter()
        .find(|wp| wp.id == w2)
        .unwrap()
        .frame
        .x;

    assert_eq!(x_before, x_after);
}

#[test]
fn viewport_offset_clamped_on_layout_shrink() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    let w2 = hub.insert_tiling();
    hub.set_window_constraint(w2, Some(100.0), None, None, None);

    hub.set_focus(w2); // scroll to end
    hub.delete_window(w2); // layout shrinks, offset must clamp

    let p = placements(&hub);
    assert!(!normal_windows(&p).is_empty());
}

#[test]
fn oversized_window_shows_left_edge() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(200.0), None, None, None);
    // Window wider than screen (150px)

    let p = placements(&hub);
    assert_eq!(normal_windows(&p)[0].frame.x, 0.0);
}

#[test]
fn scroll_right_on_focus_past_viewport() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    let w2 = hub.insert_tiling();
    hub.set_window_constraint(w2, Some(100.0), None, None, None);
    // Total 300px, screen 150px. Focus on w0, no scroll.

    hub.set_focus(w0);
    let p = placements(&hub);
    assert!(normal_windows(&p).iter().any(|wp| wp.id == w0));
    assert!(!normal_windows(&p).iter().any(|wp| wp.id == w2));

    // Focus w2 — should scroll right to reveal it
    hub.set_focus(w2);
    let p = placements(&hub);
    assert!(normal_windows(&p).iter().any(|wp| wp.id == w2));
    assert!(!normal_windows(&p).iter().any(|wp| wp.id == w0));
}

#[test]
fn scroll_left_on_focus_before_viewport() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    let w2 = hub.insert_tiling();
    hub.set_window_constraint(w2, Some(100.0), None, None, None);

    // Scroll to end
    hub.set_focus(w2);
    assert!(
        !normal_windows(&placements(&hub))
            .iter()
            .any(|wp| wp.id == w0)
    );

    // Focus w0 — should scroll left to reveal it
    hub.set_focus(w0);
    let p = placements(&hub);
    assert!(normal_windows(&p).iter().any(|wp| wp.id == w0));
    assert_eq!(
        normal_windows(&p)
            .iter()
            .find(|wp| wp.id == w0)
            .unwrap()
            .frame
            .x,
        0.0
    );
}

#[test]
fn no_scroll_when_focus_already_in_view() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling();
    hub.set_window_constraint(_w0, Some(50.0), None, None, None);
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, Some(50.0), None, None, None);
    let w2 = hub.insert_tiling();
    hub.set_window_constraint(w2, Some(50.0), None, None, None);
    let w3 = hub.insert_tiling();
    hub.set_window_constraint(w3, Some(50.0), None, None, None);

    hub.set_focus(w3); // offset=50. w1(0-50), w2(50-100), w3(100-150) all fully visible
    let w2_x_before = normal_windows(&placements(&hub))
        .iter()
        .find(|wp| wp.id == w2)
        .unwrap()
        .frame
        .x;

    // w2 already fully in view — focusing it shouldn't change scroll
    hub.set_focus(w2);
    let w2_x_after = normal_windows(&placements(&hub))
        .iter()
        .find(|wp| wp.id == w2)
        .unwrap()
        .frame
        .x;

    assert_eq!(w2_x_before, w2_x_after);
}

#[test]
fn float_does_not_scroll_with_viewport() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    let w2 = hub.insert_tiling();
    hub.set_window_constraint(w2, Some(100.0), None, None, None);

    // Focus w1 (viewport offset becomes 50), then toggle to float.
    // Layout x=100, offset=50, screen.x=0 => screen-absolute x = 100 - 50 + 0 = 50.
    hub.set_focus(w1);
    hub.toggle_float();

    let get_float_x = |hub: &Hub| {
        normal_windows(&placements(hub))
            .iter()
            .find(|wp| wp.id == w1)
            .map(|wp| wp.frame.x)
    };

    let x_after_toggle = get_float_x(&hub);

    // Scroll viewport left by focusing w0
    hub.set_focus(w0);
    let x_after_focus_w0 = get_float_x(&hub);

    // Scroll viewport right by focusing w2
    hub.set_focus(w2);
    let x_after_focus_w2 = get_float_x(&hub);

    // Float stays at the same screen position regardless of viewport scroll
    assert_eq!(x_after_toggle, x_after_focus_w0);
    assert_eq!(x_after_toggle, x_after_focus_w2);
    assert!(
        x_after_toggle.is_some(),
        "float should be visible in placements"
    );
}

#[test]
fn partially_offscreen_float_still_in_placements() {
    let mut hub = setup();
    hub.insert_tiling();
    // Float extends 20px past the right edge of the 150px screen
    let f0 = hub.insert_float(crate::core::Dimension {
        x: 130.0,
        y: 5.0,
        width: 40.0,
        height: 20.0,
    });

    let p = placements(&hub);
    let fp = normal_windows(&p)
        .iter()
        .find(|wp| wp.id == f0)
        .expect("partially offscreen float should be in placements");

    assert_eq!(fp.frame.x, 130.0);
    assert_eq!(fp.frame.width, 40.0);
    // visible_frame is clipped to screen bounds (0..150)
    assert_eq!(fp.visible_frame.width, 20.0);
}
