use super::*;

#[test]
fn single_window_placed_in_view() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)]);
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
    );
    macos.settle(&mut dome, 10);

    // Toggle cg2 (focused) to float
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    // User drags the float to a new position
    macos.simulate_external_move(&mut dome, cg2, 200, 150, 600, 400);
    macos.settle(&mut dome, 10);

    // Float should stay at the user-chosen position, not be corrected
    assert_eq!(macos.window_frame(cg2), (200, 150, 600, 400));

    // Core should reflect the outer frame (reverse-inset of the observed content rect)
    let border = Config::default().border_size;
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

    // Idempotence: a follow-up settle should issue no new set_frame calls for cg2
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
    );
    macos.settle(&mut dome, 10);

    // Toggle focused cg2 to float and settle.
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    // Snapshot core's stored outer dim before the border change. Hub-dim does
    // not change across config_changed; only the inset applied on flush_layout.
    let snap_before = macos
        .last_float_snapshot(cg2)
        .expect("float snapshot must exist once cg2 is floated and visible");

    // Clear the move log so we can assert on set_frame calls caused strictly
    // by the config change.
    macos.moves.borrow_mut().clear();

    // Bump border_size from the default (4.0) to 12.0, giving an 8 px delta
    // well beyond rounding noise.
    let mut new_config = Config::default();
    new_config.border_size = 12.0;
    dome.config_changed(new_config);

    // config_changed calls flush_layout, which passes the new content_dim to
    // FloatPlacement::set_target. The new content differs from fp.target
    // (computed with the old border), so set_target returns true and set_frame
    // runs. Check before settle because settle drains the move log.
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

    // After settle: OS window converged to apply_inset(outer_stored, 12.0).
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

    // Core's stored outer dim must be unchanged: sync_config does not touch
    // float dims (see src/core/hub.rs sync_config).
    let snap_after = macos
        .last_float_snapshot(cg2)
        .expect("float snapshot must exist after re-flush");
    assert_eq!(
        snap_after.outer_frame, snap_before.outer_frame,
        "border-size change must not alter the hub-stored outer dim"
    );
    // The RenderFrame's content_dim must reflect the new 12px border inset.
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
    );
    macos.settle(&mut dome, 10);

    // Toggle cg2 (focused) to float
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    // Clear move log
    macos.moves.borrow_mut().clear();

    // Issue another layout flush with no dimension change.
    // FloatPlacement::set_target should return false, so no set_frame.
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
