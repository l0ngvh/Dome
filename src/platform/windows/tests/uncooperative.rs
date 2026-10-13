use super::*;

use crate::platform::windows::ui::window::MAX_DRIFT_RETRIES;

#[test]
fn compliant_window_no_redundant_set_position() {
    let mut env = TestEnv::new();
    let _w1 = env.open();
    let _w2 = env.open();
    env.settle(10);

    env.clear_moves();
    env.layout();
    env.assert_settled("apply_layout should not re-issue set_position for settled windows");
}

#[test]
fn drift_exhausts_retries() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    env.settle(10);

    // Window resists placement -- always snaps to (100, 100, 800, 600)
    env.simulate_resist(w1, (100, 100, 800, 600));
    env.settle(20);

    // After MAX_DRIFT_RETRIES, Dome gives up -- window stays at its chosen position
    let d = env.dim(w1);
    assert_eq!((d.x.value() as i32, d.y.value() as i32), (100, 100));
}

#[test]
fn drift_retries_reset_on_new_target() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    env.settle(10);

    // Exhaust retries
    env.simulate_resist(w1, (100, 100, 800, 600));
    env.settle(20);

    // Stop resisting and add a new window -- target changes, retries reset
    env.clear_override_position(w1);
    let _w2 = env.open();
    env.settle(10);

    let d = env.dim(w1);
    assert!(
        (d.width.value() as i32) < 1900,
        "w1 should be half-screen, got width {}",
        d.width.value() as i32
    );
}

#[test]
fn drift_correction_repositions_window() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    env.settle(10);
    let expected = env.dim(w1);

    env.move_window_to(
        w1,
        Dimension::new(
            Length::new(50.0),
            Length::new(50.0),
            Length::new(800.0),
            Length::new(600.0),
        ),
    );
    env.settle(10);

    assert_eq!(env.dim(w1), expected);
}

#[test]
fn stale_tiling_observation_ignored() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    env.settle(10);

    let before = env.now();
    env.advance_time(std::time::Duration::from_millis(1));

    let _w2 = env.open();
    env.settle(10);

    let dim_after_retile = env.dim(w1);
    env.clear_moves();

    env.report_move(w1, PixelRect::new(100, 100, 400, 300), 1, before);

    env.assert_settled("stale observation must not trigger drift correction");
    assert_eq!(
        env.dim(w1),
        dim_after_retile,
        "window dimension must not change from a stale observation"
    );
}

#[test]
fn stale_tiling_observation_in_fullscreen_arm_ignored() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    env.settle(10);

    let before = env.now();
    env.advance_time(std::time::Duration::from_millis(1));

    let _w2 = env.open();
    env.settle(10);

    let dim_after_retile = env.dim(w1);
    env.clear_moves();

    env.report_move(w1, PixelRect::from_dimension(fullscreen_dim()), 1, before);

    assert_eq!(
        env.dim(w1),
        dim_after_retile,
        "stale fullscreen-shaped observation must not change window state"
    );
}

#[test]
fn stale_float_observation_does_not_write_target() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    env.run_actions("toggle float");
    env.settle(10);

    let target_before = env.dim(w1);
    let before = env.now();
    env.advance_time(std::time::Duration::from_millis(1));

    // A changed target starts a new placement timestamp.
    env.run_actions("toggle float");
    env.run_actions("toggle float");
    env.settle(10);

    env.clear_moves();

    let drag_target = PixelRect::new(300, 200, 500, 400);
    env.report_move(w1, drag_target, 1, before);

    env.assert_settled("stale float observation must not trigger any set_position");
    assert_eq!(
        env.dim(w1),
        target_before,
        "float target must not change from a stale observation"
    );
}

#[test]
fn offscreen_window_fights_hide() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let _w2 = env.open();
    env.settle(10);

    env.run_actions("focus workspace 1");
    env.settle(10);
    assert!(env.is_offscreen(w1));

    env.simulate_resist(w1, (100, 100, 800, 600));
    env.flush_moves();

    let mut rehides = 0;
    for _ in 0..=MAX_DRIFT_RETRIES {
        if !env.moved(w1) {
            break;
        }
        rehides += 1;
        env.flush_moves();
    }

    assert_eq!(
        rehides,
        MAX_DRIFT_RETRIES - 1,
        "a parked window that keeps resurfacing must be re-hidden until its retry budget is spent, then left alone"
    );
}

#[test]
fn offscreen_retries_reset_on_fresh_hide() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let _w2 = env.open();
    env.settle(10);

    env.run_actions("focus workspace 1");
    env.settle(10);

    // Exhaust retries
    env.simulate_resist(w1, (100, 100, 800, 600));
    env.settle(20);

    // Switch back, stop resisting, switch away again -- retries reset
    env.clear_override_position(w1);
    env.run_actions("focus workspace 0");
    env.settle(10);
    assert!(!env.is_offscreen(w1));

    env.run_actions("focus workspace 1");
    env.settle(10);
    assert!(env.is_offscreen(w1));

    // Fight again -- should get fresh retries
    env.simulate_resist(w1, (100, 100, 800, 600));
    for _ in 0..4 {
        env.assert_placement_pending("should still be retrying");
        env.flush_moves();
    }
    env.flush_moves();
    env.assert_settled("should have given up again");
}

#[test]
fn borderless_minimized_resurface_loop_caps() {
    let mut env = TestEnv::new();
    let w1 = env.window().spawned_at(fullscreen_dim()).open();
    env.run_actions("focus workspace 1");
    env.settle(20);
    assert!(env.is_minimized(w1));

    for _ in 0..7 {
        env.unminimize_window(w1);
    }
    assert!(!env.is_minimized(w1));
}

#[test]
fn borderless_minimized_resurface_loop_caps_with_other_workspace_unaffected() {
    let mut env = TestEnv::new();

    let w1 = env.window().spawned_at(fullscreen_dim()).open();
    env.run_actions("focus workspace 1");
    env.settle(20);
    let w2 = env.window().spawned_at(fullscreen_dim()).open();
    env.settle(20);

    // w1 stays BorderlessFullscreen on ws0. w2 parks BorderlessMinimized.
    env.run_actions("focus workspace 0");
    env.settle(20);
    assert!(!env.is_minimized(w1));
    assert!(env.is_minimized(w2));

    // w2 is uncooperative: it resurfaces every iteration. After
    // MAX_DRIFT_RETRIES + 2 attempts Dome gives up on it.
    for _ in 0..7 {
        env.unminimize_window(w2);
    }
    assert!(!env.is_minimized(w2));

    assert!(!env.is_minimized(w1));
    assert_eq!(env.dim(w1), fullscreen_dim());
}

#[test]
fn borderless_minimized_retries_reset_on_workspace_return() {
    let mut env = TestEnv::new();
    let w1 = env.window().spawned_at(fullscreen_dim()).open();
    env.run_actions("focus workspace 1");
    env.settle(20);
    assert!(env.is_minimized(w1));

    // Exhaust the retry cap.
    for _ in 0..7 {
        env.unminimize_window(w1);
    }
    assert!(!env.is_minimized(w1));

    // A new hide target restores the correction allowance.
    env.run_actions("focus workspace 0");
    env.run_actions("focus workspace 1");
    env.settle(20);
    assert!(env.is_minimized(w1));

    env.unminimize_window(w1);
    assert!(env.is_minimized(w1));
}

#[test]
fn accept_equal_timestamps_preserves_tiling_anchor_and_correction_cap() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.settle(10);
    let anchor = env.now();
    let target = PixelRect::from_dimension(env.dim(window));
    let drift = PixelRect::new(100, 100, 500, 300);
    env.hold_reports(true);
    let before = env.frame_submissions().len();
    env.report_move(
        window,
        drift,
        1,
        anchor - std::time::Duration::from_nanos(1),
    );
    assert_eq!(env.frame_submissions().len(), before);
    env.advance_time(std::time::Duration::from_secs(1));
    for correction in 1..=MAX_DRIFT_RETRIES + 2 {
        env.report_move(window, drift, 1, anchor);
        assert_eq!(
            env.frame_submissions().len() - before,
            usize::from(correction.min(MAX_DRIFT_RETRIES))
        );
        env.layout();
    }
    assert!(
        env.frame_submissions()[before..]
            .iter()
            .all(|write| *write == (window, target))
    );
    env.hold_reports(false);
    env.open();
    let changed = env.frame_submissions().len();
    env.report_move(window, drift, 1, env.now());
    assert_eq!(env.frame_submissions().len(), changed + 1);
}

#[test]
fn cap_hidden_correction_requires_both_coordinates_visible() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.run_actions("focus workspace 1");
    env.settle(10);
    let anchor = env.now();
    let before = env.park_submissions().len();
    for frame in [
        PixelRect::new(-32000, 100, 600, 400),
        PixelRect::new(100, -32000, 600, 400),
    ] {
        env.report_move(window, frame, 1, anchor);
    }
    assert_eq!(env.park_submissions().len(), before);
    for correction in 1..=MAX_DRIFT_RETRIES + 2 {
        env.report_move(window, PixelRect::new(100, 100, 600, 400), 1, anchor);
        assert_eq!(
            env.park_submissions().len() - before,
            usize::from(correction.min(MAX_DRIFT_RETRIES - 1))
        );
        env.layout();
    }
    env.run_actions("focus workspace 0");
    env.run_actions("focus workspace 1");
    let reset = env.park_submissions().len();
    env.report_move(window, PixelRect::new(100, 100, 600, 400), 1, env.now());
    assert_eq!(env.park_submissions().len(), reset + 1);
}

#[test]
fn cap_borderless_resurfacing_preserves_equal_scene_allowance() {
    let mut env = TestEnv::new();
    let window = env.window().spawned_at(fullscreen_dim()).open();
    env.run_actions("focus workspace 1");
    env.settle(10);
    let before = env.minimize_submissions().len();
    for correction in 1..=MAX_DRIFT_RETRIES + 2 {
        env.report_move(
            window,
            PixelRect::from_dimension(fullscreen_dim()),
            1,
            env.now(),
        );
        assert_eq!(
            env.minimize_submissions().len() - before,
            usize::from(correction.min(MAX_DRIFT_RETRIES))
        );
        env.layout();
    }
    env.run_actions("focus workspace 0");
    env.run_actions("focus workspace 1");
    let reset = env.minimize_submissions().len();
    env.report_move(
        window,
        PixelRect::from_dimension(fullscreen_dim()),
        1,
        env.now(),
    );
    assert_eq!(env.minimize_submissions().len(), reset + 1);
}

#[test]
fn reset_hidden_branch_allowance_across_native_fullscreen_transitions() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.run_actions("focus workspace 1");
    env.settle(10);
    env.hold_reports(true);
    let visible = PixelRect::new(100, 100, 600, 400);
    for _ in 0..MAX_DRIFT_RETRIES {
        env.report_move(window, visible, 1, env.now());
    }
    let fullscreen = PixelRect::from_dimension(fullscreen_dim());
    env.report_move(window, fullscreen, 1, env.now());
    let before = env.minimize_submissions().len();
    for _ in 0..MAX_DRIFT_RETRIES + 1 {
        env.report_move(window, fullscreen, 1, env.now());
    }
    assert_eq!(
        env.minimize_submissions().len() - before,
        usize::from(MAX_DRIFT_RETRIES)
    );
    env.clear_trace();
    env.report_move(window, visible, 1, env.now());
    let writes: Vec<_> = env
        .trace()
        .into_iter()
        .filter(|step| matches!(*step, "restore" | "park" | "frame"))
        .collect();
    assert_eq!(writes, vec!["restore", "park"]);
    let before = env.park_submissions().len();
    for _ in 0..MAX_DRIFT_RETRIES + 1 {
        env.report_move(window, visible, 1, env.now());
    }
    assert_eq!(
        env.park_submissions().len() - before,
        usize::from(MAX_DRIFT_RETRIES - 1)
    );
}

#[test]
fn accept_equal_float_timestamp_without_adopting_target() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.run_actions("toggle float");
    env.settle(10);
    env.hold_reports(true);
    let anchor = env.now();
    let frame = PixelRect::new(200, 100, 500, 400);
    let before = env.frame_submissions().len();
    env.report_move(
        window,
        frame,
        1,
        anchor - std::time::Duration::from_nanos(1),
    );
    assert!(env.take_reports().is_empty());
    env.report_move(window, frame, 1, anchor);
    assert!(matches!(env.take_reports().as_slice(),
        [crate::platform::windows::dome::HubEvent::FloatMoved { frame: reported, .. }] if *reported == frame));
    assert_eq!(env.frame_submissions().len(), before);
    env.layout();
    assert_eq!(env.frame_submissions().len(), before);
}
