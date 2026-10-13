use std::time::Duration;

use crate::platform::macos::window_server::DebounceBurst;

use super::*;

#[test]
fn report_settle_ignores_stale_frames_learns_attributable_constraints_and_corrects_other_drift() {
    use crate::platform::macos::dome::events::HubEvent;
    use crate::platform::macos::window_server::PositionReadOrigin;

    for (first_offset, last_offset, learns, corrects) in [
        (-2i64, -1i64, false, false),
        (-1, 0, false, true),
        (0, 0, true, false),
        (1_000_000_000, 1_000_000_000, true, false),
        (1_000_000_001, 1_000_000_001, false, true),
    ] {
        let (mut env, key) = one_window();
        let placed = env.now();
        let offset = |value: i64| {
            if value < 0 {
                placed - Duration::from_nanos(value.unsigned_abs())
            } else {
                placed + Duration::from_nanos(value as u64)
            }
        };
        let origin = PositionReadOrigin::Settle(DebounceBurst {
            first: offset(first_offset),
            last: offset(last_offset),
        });
        let (x, y, width, height) = env.window_frame(key);
        env.advance(Duration::from_secs(2));
        env.queue_position(key, PixelRect::new(x, y, width - 100, height), origin);
        env.ui_pass();
        let reports = env.take_reports();
        assert_eq!(reports.iter().any(|report| matches!(report, HubEvent::SizeConstraint { key: reported, .. } if *reported == key)), learns, "first {first_offset}, last {last_offset}");
        assert_eq!(
            !env.recorded_moves().is_empty(),
            corrects,
            "first {first_offset}, last {last_offset}"
        );
    }
    let (mut env, key) = one_window();
    let (x, y, width, height) = env.window_frame(key);
    env.queue_position(
        key,
        PixelRect::new(x, y, width - 100, height),
        PositionReadOrigin::Periodic,
    );
    env.ui_pass();
    assert!(
        env.take_reports().is_empty(),
        "periodic facts teach no constraint"
    );
    assert_eq!(env.recorded_moves(), [(key, x, y, width, height)]);
}

#[test]
fn drift_exhausts_retries_dome_gives_up() {
    let (mut macos, cg1) = one_window();

    // Window resists placement -- always snaps to (100, 100, 800, 600)
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.simulate_external_move(cg1, 100, 100, 800, 600);
    macos.settle(20);

    assert_eq!(macos.window_frame(cg1), (100, 100, 800, 600));
}

#[test]
fn drift_retries_reset_on_new_target() {
    let (mut macos, cg1) = one_window();

    // Exhaust retries
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.simulate_external_move(cg1, 100, 100, 800, 600);
    macos.settle(20);
    assert_eq!(macos.window_frame(cg1), (100, 100, 800, 600));

    // Stop resisting and add a new window -- target changes, retries reset
    macos.set_override_frame(cg1, None);
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg2]);
    macos.settle(10);

    // cg1 should now be placed at its new (half-screen) target
    assert!(!macos.is_offscreen(cg1));
    let (_, _, w1, _) = macos.window_frame(cg1);
    assert!(w1 < 1900, "cg1 should be half-screen, got width {w1}");
}

#[test]
fn drift_to_fullscreen_triggers_borderless_detection() {
    let (mut macos, cg1) = one_window();

    // Window auto-zooms to fullscreen in response to set_frame
    macos.set_override_frame(cg1, Some((0, 0, 1920, 1080)));
    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);

    // Should be detected as borderless fullscreen, not treated as drift
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn offscreen_window_fights_hide() {
    let (mut macos, cg1, _cg2) = two_windows();

    // Hide both
    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_offscreen(cg1));

    // Window fights back -- keeps snapping to visible position
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.simulate_external_move(cg1, 100, 100, 800, 600);

    for _ in 0..4 {
        assert!(
            !macos.recorded_moves().is_empty(),
            "Dome should still be retrying hide"
        );
        macos.flush_moves();
    }
    // One more round: retries exceed limit, Dome gives up
    macos.flush_moves();
    assert!(
        macos.recorded_moves().is_empty(),
        "UI should have stopped issuing move_offscreen"
    );
}

#[test]
fn hide_retries_reset_on_fresh_hide() {
    let (mut macos, cg1, _cg2) = two_windows();

    // Hide both by switching workspace
    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_offscreen(cg1));

    // cg1 fights back -- exhaust retries
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.simulate_external_move(cg1, 100, 100, 800, 600);
    for _ in 0..5 {
        macos.flush_moves();
    }
    assert!(macos.recorded_moves().is_empty());

    // Switch back to workspace 0 -- cg1 becomes Tiling
    macos.set_override_frame(cg1, None);
    macos.run_actions("focus workspace 0");
    macos.settle(10);
    assert!(!macos.is_offscreen(cg1));

    // Switch away again -- fresh hide, retries reset to 0
    macos.run_actions("focus workspace 1");
    macos.settle(10);

    // Now set override and start fighting again
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.simulate_external_move(cg1, 100, 100, 800, 600);
    for _ in 0..4 {
        assert!(
            !macos.recorded_moves().is_empty(),
            "should still be retrying"
        );
        macos.flush_moves();
    }
    macos.flush_moves();
    assert!(
        macos.recorded_moves().is_empty(),
        "should have given up again"
    );
}

#[test]
fn constraint_changes_over_time() {
    let (mut macos, _cg1, cg2) = two_windows();

    // First constraint: cg2 reports min width 1000 (right-edge aligned)
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.simulate_external_move(cg2, x2, y2, 1000, h2);
    macos.settle(10);
    let (_, _, w2, _) = macos.window_frame(cg2);
    assert!(w2 >= 1000, "First constraint: expected >= 1000, got {w2}");

    // Second constraint: cg2 now reports even larger min width
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.simulate_external_move(cg2, x2, y2, 1200, h2);
    macos.settle(10);
    let (_, _, w2, _) = macos.window_frame(cg2);
    assert!(w2 >= 1200, "Updated constraint: expected >= 1200, got {w2}");
}

#[test]
fn drift_correction_repositions_window() {
    let (mut macos, cg1) = one_window();

    let expected = macos.window_frame(cg1);

    macos.simulate_external_move(cg1, 50, 50, 1916, 1076);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg1), expected);
}

#[test]
fn window_min_size_constraint() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Finder", "Home");
    macos.set_min_size(cg2, 1000, 400);

    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    let (_, _, w2_width, _) = macos.window_frame(cg2);
    assert!(
        w2_width >= 1000,
        "Finder should be at least 1000px wide, got {w2_width}"
    );
    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn window_max_size_constraint() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Finder", "Home");
    macos.set_max_size(cg2, 500, 2000);

    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

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
    let (mut macos, _cg1, cg2) = two_windows();

    // App reports larger size than Dome requested (min width constraint)
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.simulate_external_move(cg2, x2, y2, 1000, h2);
    macos.settle(10);

    let (_, _, w2_width, _) = macos.window_frame(cg2);
    assert!(w2_width >= 1000);
}

#[test]
fn stale_burst_discarded() {
    // A burst whose most recent timestamp is still before `placed_at` must
    // be discarded entirely. Use distinct timestamps (t1 != t2) so this
    // single case covers both the (t, t) and (t1, t2) tuple shapes.
    let (mut macos, cg1) = one_window();
    let full_frame = macos.window_frame(cg1);

    // Add cg2 to trigger relayout -- cg1 shrinks to half-screen.
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg2]);
    macos.settle(10);
    let half_frame = macos.window_frame(cg1);
    macos.clear_moves();

    let stale_first = macos.now() - Duration::from_secs(10);
    let stale_last = macos.now() - Duration::from_secs(5);
    macos.report_move(
        cg1,
        PixelRect::new(full_frame.0, full_frame.1, full_frame.2, full_frame.3),
        DebounceBurst {
            first: stale_first,
            last: stale_last,
        },
    );

    assert!(
        macos.recorded_moves().is_empty(),
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
    let (mut macos, cg1, _cg2) = two_windows();

    let target_frame = macos.window_frame(cg1);

    let stale_time = macos.now() - Duration::from_secs(10);

    // Mixed timestamps: one stale, one fresh. Fresh one should prevent discard.
    macos.advance(Duration::from_secs(60));
    let fresh_time = macos.now();
    // Report cg1 at a position with no aligned edges vs its target. This
    // triggers drift detection (which issues a correction set_frame) if the
    // event passes the stale check.
    macos.report_move(
        cg1,
        PixelRect::new(50, 50, target_frame.2 + 100, target_frame.3 + 100),
        DebounceBurst {
            first: stale_time,
            last: fresh_time,
        },
    );

    // The event should NOT have been discarded. Dome saw drift (misaligned
    // edges vs target) and issued a correction set_frame.
    assert!(
        !macos.recorded_moves().is_empty(),
        "event with one fresh timestamp should not be discarded"
    );

    macos.settle(10);
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
        let (mut macos, cg) = one_window();
        let target = macos.window_frame(cg);
        macos.advance(Duration::from_secs(60));
        let late = macos.now();
        macos.report_move(
            cg,
            PixelRect::new(target.0 + dx, target.1 + dy, target.2 + dw, target.3 + dh),
            DebounceBurst {
                first: late,
                last: late,
            },
        );
        match expect {
            Expect::Correct => {
                assert_eq!(
                    macos.recorded_moves().len(),
                    1,
                    "[{label}] expected exactly one corrective set_frame"
                );
                let (out_cg, x, y, w, h) = macos.recorded_moves()[0];
                assert_eq!(out_cg, cg, "[{label}] correction targets wrong window");
                assert_eq!(
                    (x, y, w, h),
                    target,
                    "[{label}] correction should restore target frame"
                );
            }
            Expect::Noop => {
                assert!(
                    macos.recorded_moves().is_empty(),
                    "[{label}] should not issue set_frame"
                );
            }
        }
    }
}

#[test]
fn late_event_consumes_retry_budget() {
    let (mut macos, cg1) = one_window();
    let target = macos.window_frame(cg1);

    // Window resists set_frame by always snapping to a drifted position.
    let drifted = (target.0 + 200, target.1 + 200, target.2, target.3);
    macos.set_override_frame(cg1, Some(drifted));

    // Fire 6 late events. The first 5 consume the retry budget and each
    // issues a correcting set_frame (the window snaps back each time).
    // The 6th hits just_gave_up and issues nothing.
    macos.advance(Duration::from_secs(60));
    for i in 0..6u64 {
        macos.clear_moves();
        macos.advance(Duration::from_millis(1));
        let late = macos.now();
        let frame = drifted;
        macos.report_move(
            cg1,
            PixelRect::new(frame.0, frame.1, frame.2, frame.3),
            DebounceBurst {
                first: late,
                last: late,
            },
        );
        if i < 5 {
            assert_eq!(
                macos.recorded_moves().len(),
                1,
                "iteration {i}: expected a correction"
            );
        } else {
            assert!(
                macos.recorded_moves().is_empty(),
                "iteration {i}: Dome should have given up"
            );
        }
    }
}

#[test]
fn report_mixed_freshness_burst_does_not_record_constraint() {
    let (mut macos, cg1, cg2) = two_windows();

    let (x2, y2, w2_before, h2) = macos.window_frame(cg2);
    let (_, _, w1_before, _) = macos.window_frame(cg1);
    let before_placed = macos.now() - Duration::from_secs(5);
    let now = macos.now();
    macos.report_move(
        cg2,
        PixelRect::new(x2, y2, 1200, h2),
        DebounceBurst {
            first: before_placed,
            last: now,
        },
    );
    macos.settle(10);

    let (_, _, w2_after, _) = macos.window_frame(cg2);
    assert_eq!(
        w2_after, w2_before,
        "pre-placement movement teaches no limit"
    );
    let (_, _, w1_after, _) = macos.window_frame(cg1);
    assert_eq!(w1_after, w1_before, "sibling must not shrink");
}

#[test]
fn borderless_minimized_resurface_loop_caps() {
    let mut macos = TestEnv::new();
    macos.setup();
    let cg1 = macos.spawn_window(100, "Game", "game.exe");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    // Park cg1 in BorderlessMinimized via zoom + workspace switch.
    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));

    for _ in 0..7 {
        macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
        macos.settle(10);
    }
    // Past the cap, Dome stops re-minimizing: the last simulate's
    // is_minimized=false survives.
    assert!(!macos.is_minimized(cg1));
}

#[test]
fn borderless_minimized_resurface_loop_caps_with_other_workspace_unaffected() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Game1", "game1.exe");
    macos.discover_windows(&[cg1]);
    macos.settle(10);
    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);

    // reconcile assigns to the active workspace, so switch to ws1 before reconciling cg2.
    macos.run_actions("focus workspace 1");
    macos.settle(10);
    let cg2 = macos.spawn_window(101, "Game2", "game2.exe");
    macos.discover_windows(&[cg2]);
    macos.settle(10);
    macos.simulate_external_move(cg2, 0, 0, 1920, 1080);
    macos.settle(10);

    // On ws0: cg1 visible, cg2 parked BorderlessMinimized.
    macos.run_actions("focus workspace 0");
    macos.settle(10);
    assert!(!macos.is_minimized(cg1));
    assert!(macos.is_minimized(cg2));

    for _ in 0..7 {
        macos.simulate_external_move(cg2, 0, 0, 1920, 1080);
        macos.settle(10);
    }
    assert!(!macos.is_minimized(cg2));

    assert!(!macos.is_minimized(cg1));
}

#[test]
fn borderless_minimized_retries_reset_on_workspace_return() {
    let mut macos = TestEnv::new();
    macos.setup();
    let cg1 = macos.spawn_window(100, "Game", "game.exe");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    // Park cg1 in BorderlessMinimized via zoom + workspace switch.
    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));

    // Exhaust the retry cap.
    for _ in 0..7 {
        macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
        macos.settle(10);
    }
    assert!(!macos.is_minimized(cg1));

    // Switch back to cg1's workspace then away again. The variant is rebuilt
    // with retries: 0, so the next resurface is re-minimized.
    macos.run_actions("focus workspace 0");
    macos.settle(10);
    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    assert!(macos.is_minimized(cg1));
}

#[test]
fn genuine_limit_survives_display_change() {
    let (mut macos, _cg1, cg2) = two_windows();

    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.simulate_external_move(cg2, x2, y2, 1200, h2);
    macos.settle(10);
    let (_, _, w2, _) = macos.window_frame(cg2);
    assert!(w2 >= 1200, "precondition: min-width recorded, got {w2}");

    macos.change_monitors(vec![default_monitor(), taller_right_monitor()]);
    macos.finish_monitor_settle();
    macos.settle(10);

    let (_, _, w2_after, _) = macos.window_frame(cg2);
    assert!(
        w2_after >= 1200,
        "genuine min-width must survive the display change, got {w2_after}"
    );
}

#[test]
fn monitor_resolution_change_does_not_record_constraint() {
    let (mut macos, cg1, cg2) = two_windows();

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
    macos.change_monitors(vec![shrunk]);
    macos.settle(10);
    let (_, _, w1_before, _) = macos.window_frame(cg1);

    // While macOS finishes the relayout, cg2 lingers wider than its new target.
    // The armed settle must not read that transient as a min-width and starve cg1.
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.simulate_external_move(cg2, x2, y2, 1300, h2);
    macos.settle(10);

    let (_, _, w1_after, _) = macos.window_frame(cg1);
    assert_eq!(
        w1_before, w1_after,
        "a relayout's transient width must not become a min-width"
    );
}

#[test]
fn cross_monitor_move_does_not_record_clamp() {
    let mut macos = TestEnv::new();
    macos.setup();
    macos.change_monitors(vec![default_monitor(), taller_right_monitor()]);
    macos.finish_monitor_settle();

    let cg = macos.spawn_window(100, "Terminal", "zsh");
    macos.discover_windows(&[cg]);
    macos.settle(10);

    // macOS clamps the window to the source-display height during the
    // cross-monitor set_frame.
    macos.run_actions("move monitor right");
    macos.settle(10);
    let (tx, ty, tw, _) = macos.window_frame(cg);
    macos.clear_moves();

    // The arrival echo carries the source clamp: aligned origin, shorter height.
    macos.simulate_external_move(cg, tx, ty, tw, 1072);

    // A recorded max-height would re-clamp the window and log a set_frame, so
    // empty moves prove the clamp was suppressed.
    let clamped: Vec<_> = macos
        .recorded_moves()
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
    let mut macos = TestEnv::new();
    macos.setup();
    macos.change_monitors(vec![default_monitor(), taller_right_monitor()]);
    macos.finish_monitor_settle();

    // Moving A to the taller monitor arms A's per-window settle.
    let a = macos.spawn_window(100, "Terminal", "zsh");
    macos.discover_windows(&[a]);
    macos.settle(10);
    macos.run_actions("move monitor right");
    macos.settle(10);

    macos.run_actions("focus monitor left");
    let b = macos.spawn_window(101, "Finder", "Home");
    macos.discover_windows(&[b]);
    macos.settle(10);
    let (bx, by, bw, bh) = macos.window_frame(b);
    macos.clear_moves();

    // B reports a narrower width (a genuine max-width) right after placement.
    macos.simulate_external_move(b, bx, by, bw - 200, bh);
    let b_moves: Vec<_> = macos
        .recorded_moves()
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
    let mut macos = TestEnv::new();
    macos.setup();
    macos.change_monitors(vec![default_monitor(), taller_right_monitor()]);
    macos.finish_monitor_settle();

    let w = macos.spawn_window(100, "Terminal", "zsh");
    macos.discover_windows(&[w]);
    macos.settle(10);
    macos.run_actions("move monitor right");
    macos.settle(10);

    // A second window re-places W with a new, non-crossing target, clearing W's
    // stamp so detection resumes.
    macos.run_actions("focus monitor right");
    let v = macos.spawn_window(101, "Finder", "Home");
    macos.discover_windows(&[v]);
    macos.settle(10);
    let (wx, wy, ww, wh) = macos.window_frame(w);
    macos.clear_moves();

    // W reports a narrower width (a genuine max-width) after the re-place.
    macos.simulate_external_move(w, wx, wy, ww - 200, wh);
    let w_moves: Vec<_> = macos
        .recorded_moves()
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
    let mut macos = TestEnv::new();
    macos.setup();
    macos.change_monitors(vec![default_monitor(), taller_right_monitor()]);
    macos.finish_monitor_settle();

    let w = macos.spawn_window(100, "Terminal", "zsh");
    macos.discover_windows(&[w]);
    macos.settle(10);

    let (wx, wy, ww, _) = macos.window_frame(w);
    macos.simulate_external_move(w, wx, wy, ww, 800);
    macos.settle(10);
    let (_, _, _, h_primary) = macos.window_frame(w);
    assert!(
        h_primary <= 800,
        "precondition: max-height recorded, got {h_primary}"
    );

    macos.run_actions("move monitor right");
    macos.settle(10);
    let (_, _, _, h_external) = macos.window_frame(w);
    assert!(
        h_external <= 800,
        "genuine max-height must survive the cross-monitor move, got {h_external}"
    );
}

fn one_window() -> (TestEnv, CGWindowID) {
    let mut macos = TestEnv::new();
    macos.setup();
    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);
    (macos, cg1)
}

fn two_windows() -> (TestEnv, CGWindowID, CGWindowID) {
    let mut macos = TestEnv::new();
    macos.setup();
    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Finder", "Home");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);
    (macos, cg1, cg2)
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
        backing_scale: 2.0,
    }
}
