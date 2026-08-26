use std::time::{Duration, Instant};

use crate::platform::macos::dome::{DebounceBurst, WindowMove};

use super::*;

#[test]
fn drift_exhausts_retries_dome_gives_up() {
    let (macos, mut dome, cg1) = one_window();

    // Window resists placement — always snaps to (100, 100, 800, 600)
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
    macos.settle(&mut dome, 20);

    // After MAX_ENFORCEMENT_RETRIES (5), Dome gives up — window stays at its chosen position
    assert_eq!(macos.window_frame(cg1), (100, 100, 800, 600));
}

#[test]
fn drift_retries_reset_on_new_target() {
    let (mut macos, mut dome, cg1) = one_window();

    // Exhaust retries
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
    macos.settle(&mut dome, 20);
    assert_eq!(macos.window_frame(cg1), (100, 100, 800, 600));

    // Stop resisting and add a new window — target changes, retries reset
    macos.set_override_frame(cg1, None);
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg2)], &[], &[]);
    macos.settle(&mut dome, 10);

    // cg1 should now be placed at its new (half-screen) target
    assert!(!macos.is_offscreen(cg1));
    let (_, _, w1, _) = macos.window_frame(cg1);
    assert!(w1 < 1900, "cg1 should be half-screen, got width {w1}");
}

#[test]
fn drift_to_fullscreen_triggers_borderless_detection() {
    let (macos, mut dome, cg1) = one_window();

    // Window auto-zooms to fullscreen in response to set_frame
    macos.set_override_frame(cg1, Some((0, 0, 1920, 1080)));
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    // Should be detected as borderless fullscreen, not treated as drift
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn offscreen_window_fights_hide() {
    let (macos, mut dome, cg1, _cg2) = two_windows();

    // Hide both
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));

    // Window fights back — keeps snapping to visible position
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);

    // Dome retries hiding up to MAX_ENFORCEMENT_RETRIES times, then gives up.
    // report_move incremented retries to 1 and issued hide_at.
    // Each flush_moves round: window fights back → record_drift increments retries → hide_at.
    // After 4 flush rounds: retries = 5, should_retry() still true (5 <= 5).
    // 5th flush round: retries = 6, should_retry() false, just_gave_up() true → no hide_at.
    for _ in 0..4 {
        assert!(
            !macos.moves.borrow().is_empty(),
            "Dome should still be retrying hide"
        );
        macos.flush_moves(&mut dome);
    }
    // One more round: retries exceed limit, Dome gives up
    macos.flush_moves(&mut dome);
    assert!(
        macos.moves.borrow().is_empty(),
        "Dome should have stopped issuing hide_at"
    );
}

#[test]
fn hide_retries_reset_on_fresh_hide() {
    let (macos, mut dome, cg1, _cg2) = two_windows();

    // Hide both by switching workspace
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));

    // cg1 fights back — exhaust retries
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
    for _ in 0..5 {
        macos.flush_moves(&mut dome);
    }
    assert!(macos.moves.borrow().is_empty());

    // Switch back to workspace 0 — cg1 becomes Tiling
    macos.set_override_frame(cg1, None);
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));

    // Switch away again — fresh hide, retries reset to 0
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);

    // Now set override and start fighting again
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
    for _ in 0..4 {
        assert!(!macos.moves.borrow().is_empty(), "should still be retrying");
        macos.flush_moves(&mut dome);
    }
    macos.flush_moves(&mut dome);
    assert!(
        macos.moves.borrow().is_empty(),
        "should have given up again"
    );
}

#[test]
fn constraint_changes_over_time() {
    let (macos, mut dome, _cg1, cg2) = two_windows();

    // First constraint: cg2 reports min width 1000 (right-edge aligned)
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.simulate_external_move(&mut dome, cg2, x2, y2, 1000, h2);
    macos.settle(&mut dome, 10);
    let (_, _, w2, _) = macos.window_frame(cg2);
    assert!(w2 >= 1000, "First constraint: expected >= 1000, got {w2}");

    // Second constraint: cg2 now reports even larger min width
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.simulate_external_move(&mut dome, cg2, x2, y2, 1200, h2);
    macos.settle(&mut dome, 10);
    let (_, _, w2, _) = macos.window_frame(cg2);
    assert!(w2 >= 1200, "Updated constraint: expected >= 1200, got {w2}");
}

#[test]
fn drift_correction_repositions_window() {
    let (macos, mut dome, cg1) = one_window();

    let expected = macos.window_frame(cg1);

    macos.simulate_external_move(&mut dome, cg1, 50, 50, 1916, 1076);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), expected);
}

#[test]
fn window_min_size_constraint() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Finder", "Home");
    macos.set_min_size(cg2, 1000, 400);

    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    let (_, _, w2_width, _) = macos.window_frame(cg2);
    assert!(
        w2_width >= 1000,
        "Finder should be at least 1000px wide, got {w2_width}"
    );
    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn window_max_size_constraint() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Finder", "Home");
    macos.set_max_size(cg2, 500, 2000);

    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    let (_, _, w2_width, _) = macos.window_frame(cg2);
    assert!(
        w2_width <= 500,
        "Finder should be at most 500px wide, got {w2_width}"
    );

    let (_, _, w1_width, _) = macos.window_frame(cg1);
    assert!(
        w1_width > 960,
        "Safari should get more than half the screen, got {w1_width}"
    );
}

#[test]
fn size_constraint_from_external_move() {
    let (macos, mut dome, _cg1, cg2) = two_windows();

    // App reports larger size than Dome requested (min width constraint)
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.simulate_external_move(&mut dome, cg2, x2, y2, 1000, h2);
    macos.settle(&mut dome, 10);

    let (_, _, w2_width, _) = macos.window_frame(cg2);
    assert!(w2_width >= 1000);
}

#[test]
fn stale_burst_discarded() {
    // A burst whose most recent timestamp is still before `placed_at` must
    // be discarded entirely. Use distinct timestamps (t1 != t2) so this
    // single case covers both the (t, t) and (t1, t2) tuple shapes.
    let (mut macos, mut dome, cg1) = one_window();
    let full_frame = macos.window_frame(cg1);

    // Add cg2 to trigger relayout -- cg1 shrinks to half-screen.
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg2)], &[], &[]);
    macos.settle(&mut dome, 10);
    let half_frame = macos.window_frame(cg1);
    macos.moves.borrow_mut().clear();

    let stale_first = Instant::now() - Duration::from_secs(10);
    let stale_last = Instant::now() - Duration::from_secs(5);
    dome.windows_moved(vec![WindowMove {
        cg_id: cg1,
        rect: PixelRect::new(full_frame.0, full_frame.1, full_frame.2, full_frame.3),
        observed_at: DebounceBurst {
            first: stale_first,
            last: stale_last,
        },
    }]);

    assert!(
        macos.moves.borrow().is_empty(),
        "stale event should not issue set_frame"
    );
    assert_eq!(
        macos.window_frame(cg1),
        half_frame,
        "window frame must not change"
    );
}

#[test]
fn stale_check_passes_when_any_timestamp_is_fresh() {
    let (macos, mut dome, cg1, _cg2) = two_windows();

    let target_frame = macos.window_frame(cg1);

    let stale_time = Instant::now() - Duration::from_secs(10);

    // Mixed timestamps: one stale, one fresh. Fresh one should prevent discard.
    let fresh_time = Instant::now() + Duration::from_secs(60);
    // Report cg1 at a position with no aligned edges vs its target. This
    // triggers drift detection (which issues a correction set_frame) if the
    // event passes the stale check.
    dome.windows_moved(vec![WindowMove {
        cg_id: cg1,
        rect: PixelRect::new(50, 50, target_frame.2 + 100, target_frame.3 + 100),
        observed_at: DebounceBurst {
            first: stale_time,
            last: fresh_time,
        },
    }]);

    // The event should NOT have been discarded. Dome saw drift (misaligned
    // edges vs target) and issued a correction set_frame.
    assert!(
        !macos.moves.borrow().is_empty(),
        "event with one fresh timestamp should not be discarded"
    );

    macos.settle(&mut dome, 10);
}

#[test]
fn user_moved_drift_handling() {
    #[derive(Copy, Clone)]
    enum Expect {
        Correct,
        Noop,
    }

    // User moving window to a different position must trigger a corrective set_frame.
    let cases: &[(&str, i32, i32, i32, i32, Expect)] = &[
        // Moving to different position triggers correction.
        ("position_shift", 200, 200, 0, 0, Expect::Correct),
        // Resizing triggers correction.
        ("only_resize", 0, 0, -50, -50, Expect::Correct),
        // Moving to the same position is no-op.
        ("same_position", 0, 0, 0, 0, Expect::Noop),
    ];

    for (label, dx, dy, dw, dh, expect) in cases {
        let (macos, mut dome, cg) = one_window();
        let target = macos.window_frame(cg);
        let late = Instant::now() + Duration::from_secs(60);
        dome.windows_moved(vec![WindowMove {
            cg_id: cg,
            rect: PixelRect::new(target.0 + dx, target.1 + dy, target.2 + dw, target.3 + dh),
            observed_at: DebounceBurst {
                first: late,
                last: late,
            },
        }]);
        match expect {
            Expect::Correct => {
                assert_eq!(
                    macos.moves.borrow().len(),
                    1,
                    "[{label}] expected exactly one corrective set_frame"
                );
                let (out_cg, x, y, w, h) = macos.moves.borrow()[0];
                assert_eq!(out_cg, cg, "[{label}] correction targets wrong window");
                assert_eq!(
                    (x, y, w, h),
                    target,
                    "[{label}] correction should restore target frame"
                );
            }
            Expect::Noop => {
                assert!(
                    macos.moves.borrow().is_empty(),
                    "[{label}] should not issue set_frame"
                );
            }
        }
    }
}

#[test]
fn late_event_consumes_retry_budget() {
    let (macos, mut dome, cg1) = one_window();
    let target = macos.window_frame(cg1);

    // Window resists set_frame by always snapping to a drifted position.
    let drifted = (target.0 + 200, target.1 + 200, target.2, target.3);
    macos.set_override_frame(cg1, Some(drifted));

    // Fire 6 late events. The first 5 consume the retry budget and each
    // issues a correcting set_frame (the window snaps back each time).
    // The 6th hits just_gave_up and issues nothing.
    for i in 0..6u64 {
        macos.moves.borrow_mut().clear();
        let late = Instant::now() + Duration::from_secs(60) + Duration::from_millis(i);
        let frame = drifted;
        dome.windows_moved(vec![WindowMove {
            cg_id: cg1,
            rect: PixelRect::new(frame.0, frame.1, frame.2, frame.3),
            observed_at: DebounceBurst {
                first: late,
                last: late,
            },
        }]);
        if i < 5 {
            assert_eq!(
                macos.moves.borrow().len(),
                1,
                "iteration {i}: expected a correction"
            );
        } else {
            assert!(
                macos.moves.borrow().is_empty(),
                "iteration {i}: Dome should have given up"
            );
        }
    }
}

#[test]
fn mixed_freshness_burst_runs_constraint_detection() {
    let (macos, mut dome, cg1, cg2) = two_windows();

    // Report cg2 at a larger width than its target, with observed_at.first just
    // before placed_at and observed_at.last after. Under the new predicate,
    // observed_at.first <= placed_at + 1s holds, so constraint detection runs.
    // The larger reported width becomes a constraint, so cg1 shrinks.
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    let (_, _, w1_before, _) = macos.window_frame(cg1);
    let before_placed = Instant::now() - Duration::from_secs(5);
    let now = Instant::now();
    dome.windows_moved(vec![WindowMove {
        cg_id: cg2,
        rect: PixelRect::new(x2, y2, 1200, h2),
        observed_at: DebounceBurst {
            first: before_placed,
            last: now,
        },
    }]);
    macos.settle(&mut dome, 10);

    let (_, _, w2_after, _) = macos.window_frame(cg2);
    assert!(
        w2_after >= 1200,
        "constraint should have been recorded, got {w2_after}"
    );
    let (_, _, w1_after, _) = macos.window_frame(cg1);
    assert!(
        w1_after < w1_before,
        "cg1 should shrink after cg2's constraint is recorded: before {w1_before}, after {w1_after}"
    );
}

#[test]
fn borderless_minimized_resurface_loop_caps() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    let cg1 = macos.spawn_window(100, "Game", "game.exe");
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

    // Park cg1 in BorderlessMinimized via zoom + workspace switch.
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    // Resurface MAX_ENFORCEMENT_RETRIES + 2 times. Each simulate_external_move
    // clears is_minimized. Dome re-issues minimize until the cap is crossed.
    for _ in 0..7 {
        macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
        macos.settle(&mut dome, 10);
    }
    // Past the cap, Dome stops re-minimizing: the last simulate's
    // is_minimized=false survives.
    assert!(!macos.is_minimized(cg1));
}

#[test]
fn borderless_minimized_resurface_loop_caps_with_other_workspace_unaffected() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Game1", "game1.exe");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    // reconcile assigns to the active workspace, so switch to ws1 before reconciling cg2.
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    let cg2 = macos.spawn_window(101, "Game2", "game2.exe");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg2)], &[], &[]);
    macos.settle(&mut dome, 10);
    macos.simulate_external_move(&mut dome, cg2, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    // On ws0: cg1 visible, cg2 parked BorderlessMinimized.
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_minimized(cg1));
    assert!(macos.is_minimized(cg2));

    // cg2 is uncooperative. Past MAX_ENFORCEMENT_RETRIES + 2 attempts, Dome stops re-minimizing.
    for _ in 0..7 {
        macos.simulate_external_move(&mut dome, cg2, 0, 0, 1920, 1080);
        macos.settle(&mut dome, 10);
    }
    assert!(!macos.is_minimized(cg2));

    assert!(!macos.is_minimized(cg1));
}

#[test]
fn borderless_minimized_retries_reset_on_workspace_return() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    let cg1 = macos.spawn_window(100, "Game", "game.exe");
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

    // Park cg1 in BorderlessMinimized via zoom + workspace switch.
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    // Exhaust the retry cap.
    for _ in 0..7 {
        macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
        macos.settle(&mut dome, 10);
    }
    assert!(!macos.is_minimized(cg1));

    // Switch back to cg1's workspace then away again. The variant is rebuilt
    // with retries: 0, so the next resurface is re-minimized.
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));
}

#[test]
fn genuine_limit_survives_display_change() {
    let (macos, mut dome, _cg1, cg2) = two_windows();

    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.simulate_external_move(&mut dome, cg2, x2, y2, 1200, h2);
    macos.settle(&mut dome, 10);
    let (_, _, w2, _) = macos.window_frame(cg2);
    assert!(w2 >= 1200, "precondition: min-width recorded, got {w2}");

    dome.monitors_changed(vec![default_monitor(), taller_right_monitor()]);
    dome.finish_monitor_settle();
    macos.settle(&mut dome, 10);

    let (_, _, w2_after, _) = macos.window_frame(cg2);
    assert!(
        w2_after >= 1200,
        "genuine min-width must survive the display change, got {w2_after}"
    );
}

#[test]
fn monitor_resolution_change_does_not_record_constraint() {
    let (macos, mut dome, cg1, cg2) = two_windows();

    // Shrinking the primary re-tiles both windows onto the smaller monitor and
    // arms the settle.
    let shrunk = MonitorInfo {
        bounds: Dimension::new(
            Length::ZERO,
            Length::ZERO,
            Length::new(1400.0),
            Length::new(900.0),
        ),
        work_area: PixelRect::from_dimension_inward(Dimension::new(
            Length::ZERO,
            Length::ZERO,
            Length::new(1400.0),
            Length::new(900.0),
        )),
        full_height: 900.0,
        ..default_monitor()
    };
    dome.monitors_changed(vec![shrunk]);
    macos.settle(&mut dome, 10);
    let (_, _, w1_before, _) = macos.window_frame(cg1);

    // While macOS finishes the relayout, cg2 lingers wider than its new target.
    // The armed settle must not read that transient as a min-width and starve cg1.
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.simulate_external_move(&mut dome, cg2, x2, y2, 1300, h2);
    macos.settle(&mut dome, 10);

    let (_, _, w1_after, _) = macos.window_frame(cg1);
    assert_eq!(
        w1_before, w1_after,
        "a relayout's transient width must not become a min-width"
    );
}

#[test]
fn cross_monitor_move_does_not_record_clamp() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    dome.monitors_changed(vec![default_monitor(), taller_right_monitor()]);
    dome.finish_monitor_settle();

    let cg = macos.spawn_window(100, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg)], &[], &[]);
    macos.settle(&mut dome, 10);

    // macOS clamps the window to the source-display height during the
    // cross-monitor set_frame.
    send(&mut dome, "move monitor right");
    macos.settle(&mut dome, 10);
    let (tx, ty, tw, _) = macos.window_frame(cg);
    macos.moves.borrow_mut().clear();

    // The arrival echo carries the source clamp: aligned origin, shorter height.
    macos.simulate_external_move(&mut dome, cg, tx, ty, tw, 1072);

    // A recorded max-height would re-clamp the window and log a set_frame, so
    // empty moves prove the clamp was suppressed.
    let clamped: Vec<_> = macos
        .moves
        .borrow()
        .iter()
        .filter(|m| m.0 == cg)
        .copied()
        .collect();
    assert!(
        clamped.is_empty(),
        "a cross-monitor clamp must not be recorded, got {clamped:?}"
    );
}

#[test]
fn cross_monitor_move_settle_is_per_window() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    dome.monitors_changed(vec![default_monitor(), taller_right_monitor()]);
    dome.finish_monitor_settle();

    // Moving A to the taller monitor arms A's per-window settle.
    let a = macos.spawn_window(100, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, a)], &[], &[]);
    macos.settle(&mut dome, 10);
    send(&mut dome, "move monitor right");
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus monitor left");
    let b = macos.spawn_window(101, "Finder", "Home");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, b)], &[], &[]);
    macos.settle(&mut dome, 10);
    let (bx, by, bw, bh) = macos.window_frame(b);
    macos.moves.borrow_mut().clear();

    // B reports a narrower width (a genuine max-width) right after placement.
    macos.simulate_external_move(&mut dome, b, bx, by, bw - 200, bh);
    let b_moves: Vec<_> = macos
        .moves
        .borrow()
        .iter()
        .filter(|m| m.0 == b)
        .copied()
        .collect();
    assert!(
        !b_moves.is_empty(),
        "A's settle must not suppress detection for B"
    );
}

#[test]
fn cross_monitor_move_relearns_on_destination() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    dome.monitors_changed(vec![default_monitor(), taller_right_monitor()]);
    dome.finish_monitor_settle();

    let w = macos.spawn_window(100, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, w)], &[], &[]);
    macos.settle(&mut dome, 10);
    send(&mut dome, "move monitor right");
    macos.settle(&mut dome, 10);

    // A second window re-places W with a new, non-crossing target, clearing W's
    // stamp so detection resumes.
    send(&mut dome, "focus monitor right");
    let v = macos.spawn_window(101, "Finder", "Home");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, v)], &[], &[]);
    macos.settle(&mut dome, 10);
    let (wx, wy, ww, wh) = macos.window_frame(w);
    macos.moves.borrow_mut().clear();

    // W reports a narrower width (a genuine max-width) after the re-place.
    macos.simulate_external_move(&mut dome, w, wx, wy, ww - 200, wh);
    let w_moves: Vec<_> = macos
        .moves
        .borrow()
        .iter()
        .filter(|m| m.0 == w)
        .copied()
        .collect();
    assert!(
        !w_moves.is_empty(),
        "a non-crossing re-place must clear the stamp and resume detection"
    );
}

#[test]
fn genuine_limit_survives_cross_monitor_move() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    dome.monitors_changed(vec![default_monitor(), taller_right_monitor()]);
    dome.finish_monitor_settle();

    let w = macos.spawn_window(100, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, w)], &[], &[]);
    macos.settle(&mut dome, 10);

    let (wx, wy, ww, _) = macos.window_frame(w);
    macos.simulate_external_move(&mut dome, w, wx, wy, ww, 800);
    macos.settle(&mut dome, 10);
    let (_, _, _, h_primary) = macos.window_frame(w);
    assert!(
        h_primary <= 800,
        "precondition: max-height recorded, got {h_primary}"
    );

    send(&mut dome, "move monitor right");
    macos.settle(&mut dome, 10);
    let (_, _, _, h_external) = macos.window_frame(w);
    assert!(
        h_external <= 800,
        "genuine max-height must survive the cross-monitor move, got {h_external}"
    );
}

/// Set up a MacOS harness with a single Safari window, settled.
fn one_window() -> (MacOS, Dome, CGWindowID) {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);
    (macos, dome, cg1)
}

/// Set up a MacOS harness with Safari + Finder, both placed and settled.
fn two_windows() -> (MacOS, Dome, CGWindowID, CGWindowID) {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Finder", "Home");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);
    (macos, dome, cg1, cg2)
}

/// Taller than the primary (1440 vs 1080) so a window moved onto it has room to
/// grow and a source-display clamp shows up.
fn taller_right_monitor() -> MonitorInfo {
    MonitorInfo {
        display_id: 2,
        name: "External".to_string(),
        work_area: PixelRect::new(1920, 0, 2560, 1440),
        bounds: Dimension::new(
            Length::new(1920.0),
            Length::ZERO,
            Length::new(2560.0),
            Length::new(1440.0),
        ),
        full_height: 1440.0,
        is_primary: false,
        scale: 2.0,
    }
}
