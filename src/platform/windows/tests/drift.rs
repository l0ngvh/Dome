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
