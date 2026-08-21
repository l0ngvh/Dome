use super::*;
use crate::core::Pixels;

#[test]
fn single_window_placed_in_view() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
    assert_eq!(macos.window_frame(cg1), (4, 4, 1912, 1072));
}

#[test]
fn a_fractional_work_area_keeps_the_window_inside_it() {
    let mut macos = MacOS::new();
    // Zero border so the sole tile fills the work area exactly, leaving no inset to
    // absorb a sub-point rounding error.
    let mut dome = macos.setup_dome_with_config(Config {
        border_size: Pixels::ZERO,
        ..Config::default()
    });

    let mut monitor = default_monitor();
    monitor.work_area = PixelRect::from_dimension_inward(FRACTIONAL_WORK_AREA);
    dome.monitors_changed(vec![monitor]);

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert_inside_work_area(macos.window_frame(cg1), FRACTIONAL_WORK_AREA);
}

#[test]
fn degenerate_content_box_parks_window() {
    let mut macos = MacOS::new();
    // Each edge exceeds half of SCREEN_HEIGHT, so no content height remains.
    let mut dome = macos.setup_dome_with_config(Config {
        border_size: Pixels::new(600),
        ..Config::default()
    });

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert!(macos.is_offscreen(cg1));

    dome.config_changed(Config::default());
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
    assert_eq!(macos.window_frame(cg1), (4, 4, 1912, 1072));
}

#[test]
fn two_windows_split_horizontally() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    let (x1, _, w1, _) = macos.window_frame(cg1);
    let (x2, _, w2, _) = macos.window_frame(cg2);
    assert!(x1 < x2);
    assert!(w1 > 0 && w2 > 0);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn tile_past_work_area_is_trimmed() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    start_drag(&mut dome, 100);
    macos.window(cg1).position.set((500, 300));
    macos.window(cg1).size.set((400, 400));

    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg2)], &[], &[]);
    macos.settle(&mut dome, 10);

    end_drag(&mut dome, &macos, 100, cg1, 500, 300, 400, 400);
    macos.settle(&mut dome, 10);

    // The drop leaves the tree wider than the work area, so cg1 is scrolled off the
    // left edge and core's content_box for it starts at -92 with width 1912. macOS
    // must place the trimmed rect rather than that.
    let (x, _, w, _) = macos.window_frame(cg1);
    assert_eq!(x, 0, "left edge clamped to the work area");
    assert_eq!(w, 1820, "width trimmed down from the untrimmed 1912");
}

#[test]
fn workspace_switch_hides_and_restores() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    let placed = macos.window_frame(cg1);

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));
    assert!(macos.is_offscreen(cg2));

    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg1), placed);
}

#[test]
fn float_window_moved_by_user() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg2, 200, 150, 600, 400);
    macos.settle(&mut dome, 10);

    // Float should stay at the user-chosen position, not be corrected
    assert_eq!(macos.window_frame(cg2), (200, 150, 600, 400));

    let border = Length::from_pixels(Config::default().border_size).logical();
    let snap = macos
        .last_float_snapshot(cg2)
        .expect("float snapshot should be present for focused float");
    assert_eq!(
        snap.outer_frame,
        Dimension::new(
            Length::new(200.0 - border),
            Length::new(150.0 - border),
            Length::new(600.0 + 2.0 * border),
            Length::new(400.0 + 2.0 * border),
        )
    );

    let moves_before = macos.moves.borrow().len();
    macos.settle(&mut dome, 10);
    let moves_after = macos.moves.borrow();
    let new_moves: Vec<_> = moves_after[moves_before..]
        .iter()
        .filter(|(id, _, _, _, _)| *id == cg2)
        .collect();
    assert!(
        new_moves.is_empty(),
        "idempotence: expected no set_frame for cg2 after settle, got {new_moves:?}"
    );
}

#[test]
fn float_window_reshaped_on_border_size_change() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    let snap_before = macos
        .last_float_snapshot(cg2)
        .expect("float snapshot must exist once cg2 is floated and visible");

    // Clear the move log so we can assert on set_frame calls caused strictly
    // by the config change.
    macos.moves.borrow_mut().clear();

    // A border several points above the default, so the delta cannot be
    // mistaken for rounding noise.
    let new_config = Config {
        border_size: Pixels::new(12),
        ..Default::default()
    };
    dome.config_changed(new_config);

    // Check before settle because settle drains the move log.
    let reshape_moves: Vec<_> = macos
        .moves
        .borrow()
        .iter()
        .filter(|(id, _, _, _, _)| *id == cg2)
        .copied()
        .collect();
    assert!(
        !reshape_moves.is_empty(),
        "expected at least one set_frame for cg2 after border_size change, got none"
    );

    macos.settle(&mut dome, 10);

    // Outer-frame values are exact integers by construction (default float
    // placement rounds to whole pixels).
    let expected_x = snap_before.outer_frame.x.value() as i32 + 12;
    let expected_y = snap_before.outer_frame.y.value() as i32 + 12;
    let expected_w = snap_before.outer_frame.width.value() as i32 - 24;
    let expected_h = snap_before.outer_frame.height.value() as i32 - 24;
    assert_eq!(
        macos.window_frame(cg2),
        (expected_x, expected_y, expected_w, expected_h)
    );

    let snap_after = macos
        .last_float_snapshot(cg2)
        .expect("float snapshot must exist after re-flush");
    assert_eq!(
        snap_after.outer_frame, snap_before.outer_frame,
        "border-size change must not alter the hub-stored outer dim"
    );
    assert_eq!(
        snap_after.content_dim,
        Dimension::new(
            Length::new(snap_before.outer_frame.x.value() + 12.0),
            Length::new(snap_before.outer_frame.y.value() + 12.0),
            Length::new(snap_before.outer_frame.width.value() - 24.0),
            Length::new(snap_before.outer_frame.height.value() - 24.0),
        )
    );
}

#[test]
fn float_place_with_same_target_is_noop() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    macos.moves.borrow_mut().clear();

    dome.flush_layout();
    macos.settle(&mut dome, 10);

    let moves: Vec<_> = macos
        .moves
        .borrow()
        .iter()
        .filter(|(id, _, _, _, _)| *id == cg2)
        .copied()
        .collect();
    assert!(
        moves.is_empty(),
        "expected zero set_frame calls for cg2 on same-target re-place, got {moves:?}"
    );
}

#[test]
fn multi_monitor_per_display() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    dome.monitors_changed(vec![default_monitor(), second_monitor()]);

    let win1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win1)], &[], &[]);
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus monitor right");
    let win2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win2)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(win1), (4, 4, 1912, 1072));
    assert_eq!(macos.window_frame(win2), (1924, 4, 2552, 1432));
}

#[test]
fn set_reserved_bar_shrinks_and_restores_work_area() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let win = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win)], &[], &[]);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(win), (4, 4, 1912, 1072));

    dome.set_reserved_bar(Ok(BarGeometry::new(30.0, Some("top".into()), 0.0, 0.0)));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(win), (4, 34, 1912, 1042));

    dome.set_reserved_bar(Ok(BarGeometry::new(0.0, None, 0.0, 0.0)));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(win), (4, 4, 1912, 1072));
}

/// The only production path that can be handed a fractional work area from a test.
/// `get_all_monitors` builds its rect from `NSScreen`, so the snap it performs is
/// unreachable here.
#[test]
fn a_fractional_reserved_bar_keeps_the_window_inside_the_reserved_area() {
    const BAR_HEIGHT: f64 = 30.4;

    let mut macos = MacOS::new();
    // Zero border so the sole tile fills the work area exactly, leaving no inset to
    // absorb a sub-point rounding error.
    let mut dome = macos.setup_dome_with_config(Config {
        border_size: Pixels::ZERO,
        ..Config::default()
    });

    let win = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win)], &[], &[]);
    macos.settle(&mut dome, 10);

    dome.set_reserved_bar(Ok(BarGeometry::new(
        BAR_HEIGHT,
        Some("top".into()),
        0.0,
        0.0,
    )));
    macos.settle(&mut dome, 10);

    let bar_height = Length::new(BAR_HEIGHT as f32);
    let reserved = Dimension::new(
        Length::ZERO,
        bar_height,
        SCREEN_WIDTH,
        SCREEN_HEIGHT - bar_height,
    );
    assert_inside_work_area(macos.window_frame(win), reserved);
}

#[test]
fn display_added_after_probe_is_inset() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    dome.set_reserved_bar(Ok(BarGeometry::new(30.0, Some("top".into()), 0.0, 0.0)));
    dome.monitors_changed(vec![default_monitor(), second_monitor()]);

    let win1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win1)], &[], &[]);
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus monitor right");
    let win2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win2)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(win1), (4, 34, 1912, 1042));
    assert_eq!(macos.window_frame(win2), (1924, 34, 2552, 1402));
}

#[test]
fn failed_probe_keeps_previous_reservation() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let win = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win)], &[], &[]);
    macos.settle(&mut dome, 10);

    dome.set_reserved_bar(Ok(BarGeometry::new(30.0, Some("top".into()), 0.0, 0.0)));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(win), (4, 34, 1912, 1042));

    dome.set_reserved_bar(Err(anyhow::anyhow!("probe failed")));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(win), (4, 34, 1912, 1042));
}
