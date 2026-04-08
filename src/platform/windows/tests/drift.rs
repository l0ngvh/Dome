use super::*;

#[test]
fn compliant_window_no_redundant_set_position() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
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
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.settle(10);

    // Window resists placement — always snaps to (100, 100, 800, 600)
    env.simulate_resist(&w1, (100, 100, 800, 600));
    env.settle(20);

    // After MAX_DRIFT_RETRIES, Dome gives up — window stays at its chosen position
    let dim = w1.get_dim();
    assert_eq!((dim.x as i32, dim.y as i32), (100, 100));
}

#[test]
fn drift_retries_reset_on_new_target() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.settle(10);

    // Exhaust retries
    env.simulate_resist(&w1, (100, 100, 800, 600));
    env.settle(20);

    // Stop resisting and add a new window — target changes, retries reset
    w1.set_override_position(None);
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w2.clone());
    env.settle(10);

    let dim = w1.get_dim();
    assert!(
        (dim.width as i32) < 1900,
        "w1 should be half-screen, got width {}",
        dim.width as i32
    );
}

#[test]
fn drift_correction_repositions_window() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.settle(10);
    let expected = w1.get_dim();

    *w1.dimension.lock().unwrap() = Dimension {
        x: 50.0,
        y: 50.0,
        width: 800.0,
        height: 600.0,
    };
    w1.simulate_external_move();
    env.settle(10);

    assert_eq!(w1.get_dim(), expected);
}

#[test]
fn offscreen_window_fights_hide() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.settle(10);

    env.run_actions("focus workspace 1");
    env.settle(10);
    assert!(w1.is_offscreen());

    env.simulate_resist(&w1, (100, 100, 800, 600));

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
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.settle(10);

    env.run_actions("focus workspace 1");
    env.settle(10);

    // Exhaust retries
    env.simulate_resist(&w1, (100, 100, 800, 600));
    env.settle(20);

    // Switch back, stop resisting, switch away again — retries reset
    w1.set_override_position(None);
    env.run_actions("focus workspace 0");
    env.settle(10);
    assert!(!w1.is_offscreen());

    env.run_actions("focus workspace 1");
    env.settle(10);
    assert!(w1.is_offscreen());

    // Fight again — should get fresh retries
    env.simulate_resist(&w1, (100, 100, 800, 600));
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
