use super::*;

#[test]
fn compliant_window_no_redundant_set_position() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.settle(10);

    env.moves.lock().unwrap().clear();
    env.dome.apply_layout();
    assert!(
        env.moves.lock().unwrap().is_empty(),
        "apply_layout should not re-issue set_position for settled windows"
    );
}

#[test]
fn drift_exhausts_retries() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
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
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.settle(10);

    // Exhaust retries
    env.simulate_resist(w1, (100, 100, 800, 600));
    env.settle(20);

    // Stop resisting and add a new window -- target changes, retries reset
    env.clear_override_position(w1);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
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
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.settle(10);
    let expected = env.dim(w1);

    env.set_dim(
        w1,
        Dimension::new(
            Length::new(50.0),
            Length::new(50.0),
            Length::new(800.0),
            Length::new(600.0),
        ),
    );
    env.simulate_external_move(w1);
    env.settle(10);

    assert_eq!(env.dim(w1), expected);
}

#[test]
fn stale_tiling_observation_ignored() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.settle(10);

    let before = Instant::now();

    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.settle(10);

    let dim_after_retile = env.dim(w1);
    env.moves.lock().unwrap().clear();

    env.window_moved_at(w1, dim(100, 100, 400, 300), 1, before);

    assert!(
        env.moves.lock().unwrap().is_empty(),
        "stale observation must not trigger drift correction"
    );
    assert_eq!(
        env.dim(w1),
        dim_after_retile,
        "window dimension must not change from a stale observation"
    );
}

#[test]
fn fresh_tiling_observation_drift_corrects() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.settle(10);

    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.settle(10);

    let target = env.dim(w1);
    env.moves.lock().unwrap().clear();

    let off_target = dim(
        target.x.value() as i32 + 5,
        target.y.value() as i32,
        target.width.value() as i32 - 10,
        target.height.value() as i32,
    );

    env.window_moved(w1, off_target, 1);

    let moves = env.moves.lock().unwrap();
    assert!(
        !moves.is_empty(),
        "fresh off-target observation must trigger drift correction"
    );
    let corrected = moves.iter().find(|(id, _)| *id == w1);
    assert!(
        corrected.is_some(),
        "drift correction must issue set_position for w1"
    );
    assert_eq!(
        corrected.unwrap().1,
        target,
        "drift correction must re-apply the target dimension"
    );
}

#[test]
fn stale_tiling_observation_in_fullscreen_arm_ignored() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.settle(10);

    let before = Instant::now();

    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.settle(10);

    let dim_after_retile = env.dim(w1);
    env.moves.lock().unwrap().clear();

    env.window_moved_at(w1, fullscreen_dim(), 1, before);

    assert_eq!(
        env.dim(w1),
        dim_after_retile,
        "stale fullscreen-shaped observation must not change window state"
    );
}

#[test]
fn stale_float_observation_does_not_write_target() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    let target_before = env.dim(w1);
    let before = Instant::now();

    // Trigger a fresh show_float that bumps fp.placed_at past `before`.
    env.run_actions("toggle float");
    env.run_actions("toggle float");
    env.settle(10);

    env.moves.lock().unwrap().clear();

    let drag_target = dim(300, 200, 500, 400);
    env.window_moved_at(w1, drag_target, 1, before);

    assert!(
        env.moves.lock().unwrap().is_empty(),
        "stale float observation must not trigger any set_position"
    );
    assert_eq!(
        env.dim(w1),
        target_before,
        "float target must not change from a stale observation"
    );
}

#[test]
fn offscreen_window_fights_hide() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.settle(10);

    env.run_actions("focus workspace 1");
    env.settle(10);
    assert!(env.is_offscreen(w1));

    env.simulate_resist(w1, (100, 100, 800, 600));

    for _ in 0..4 {
        assert!(
            !env.moves.lock().unwrap().is_empty(),
            "should still be retrying"
        );
        env.flush_moves();
    }
    env.flush_moves();
    assert!(env.moves.lock().unwrap().is_empty(), "should have given up");
}

#[test]
fn offscreen_retries_reset_on_fresh_hide() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
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
        assert!(
            !env.moves.lock().unwrap().is_empty(),
            "should still be retrying"
        );
        env.flush_moves();
    }
    env.flush_moves();
    assert!(
        env.moves.lock().unwrap().is_empty(),
        "should have given up again"
    );
}

#[test]
fn borderless_minimized_resurface_loop_caps() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", fullscreen_dim());
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

    let w1 = env.open(1, "Game1", "game1.exe", fullscreen_dim());
    env.run_actions("focus workspace 1");
    env.settle(20);
    let w2 = env.open(2, "Game2", "game2.exe", fullscreen_dim());
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
    let w1 = env.open(1, "Game", "game.exe", fullscreen_dim());
    env.run_actions("focus workspace 1");
    env.settle(20);
    assert!(env.is_minimized(w1));

    // Exhaust the retry cap.
    for _ in 0..7 {
        env.unminimize_window(w1);
    }
    assert!(!env.is_minimized(w1));

    // Switch back then away: variant rebuilt with retries: 0.
    env.run_actions("focus workspace 0");
    env.run_actions("focus workspace 1");
    env.settle(20);
    assert!(env.is_minimized(w1));

    env.unminimize_window(w1);
    assert!(env.is_minimized(w1));
}
