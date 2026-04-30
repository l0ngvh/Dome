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
    let ws_id = dome.hub.current_workspace();
    let ws = dome.hub.get_workspace(ws_id);
    // The most recently inserted float is cg2. Its WindowId is the last float in the workspace.
    let (_, stored_dim) = ws
        .float_windows()
        .last()
        .expect("float should be in workspace");
    assert_eq!(
        *stored_dim,
        Dimension {
            x: 200.0 - border,
            y: 150.0 - border,
            width: 600.0 + 2.0 * border,
            height: 400.0 + 2.0 * border,
        }
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
fn float_window_replaced_after_drag_issues_set_frame() {
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

    // Toggle cg2 (focused) to float and settle
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    // Record cg2's initial float frame
    let initial_frame = macos.window_frame(cg2);

    // User drags the float to a new position
    macos.simulate_external_move(&mut dome, cg2, 200, 150, 600, 400);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg2), (200, 150, 600, 400));

    // Clear move log before the programmatic re-place
    macos.moves.borrow_mut().clear();

    // Update core's float dimension to the ORIGINAL float frame (differs from
    // drag position), then flush layout. This triggers place_window with a
    // content rect different from fp.target, so FloatPlacement::set_target
    // returns true and issues set_frame.
    let ws_id = dome.hub.current_workspace();
    let ws = dome.hub.get_workspace(ws_id);
    let (float_wid, _) = *ws.float_windows().last().expect("float should exist");
    let border = Config::default().border_size;
    // Use a clearly different outer frame
    dome.hub.update_float_dimension(
        float_wid,
        Dimension {
            x: initial_frame.0 as f32 - border,
            y: initial_frame.1 as f32 - border,
            width: initial_frame.2 as f32 + 2.0 * border,
            height: initial_frame.3 as f32 + 2.0 * border,
        },
    );
    dome.flush_layout();

    // Check moves before settle -- settle drains the move log via std::mem::take
    // to feed entries back as AX observations, so the log is empty afterward.
    let moves: Vec<_> = macos
        .moves
        .borrow()
        .iter()
        .filter(|(id, _, _, _, _)| *id == cg2)
        .copied()
        .collect();
    assert!(
        !moves.is_empty(),
        "expected at least one set_frame for cg2 after programmatic float move, initial_frame={initial_frame:?}"
    );

    // Settle and verify the window converges to the initial float position
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg2), initial_frame);
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
