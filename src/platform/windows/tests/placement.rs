use std::sync::Arc;

use super::*;

#[test]
fn single_window_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());
    assert_h_tiled(
        &[w1.get_dim()],
        default_screen().dimension,
        env.config.border_size,
    );
}

#[test]
fn two_windows_split_screen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    let w2 = Arc::new(MockExternalHwnd::with_title(
        2,
        "App2",
        "app2.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    assert_h_tiled(
        &[w1.get_dim(), w2.get_dim()],
        default_screen().dimension,
        env.config.border_size,
    );
}

#[test]
fn three_windows_split_screen() {
    let config = Config {
        automatic_tiling: false,
        ..Default::default()
    };
    let mut env = TestEnv::new_with_config(config);
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    let w2 = Arc::new(MockExternalHwnd::with_title(
        2,
        "App2",
        "app2.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    let w3 = Arc::new(MockExternalHwnd::with_title(
        3,
        "App3",
        "app3.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.add_window(w3.clone());
    assert_h_tiled(
        &[w1.get_dim(), w2.get_dim(), w3.get_dim()],
        default_screen().dimension,
        env.config.border_size,
    );
}

/// distribute_space uses binary search and may produce fractional widths
/// (e.g. 1920/3 ≈ 639.999). The f32→i32 conversion in show_tiling must
/// round, not truncate, or the cumulative error pushes the last window's
/// right edge away from the screen edge.
#[test]
fn positions_are_rounded_not_truncated() {
    let config = Config {
        automatic_tiling: false,
        ..Default::default()
    };
    let mut env = TestEnv::new_with_config(config);
    let wins: Vec<_> = (1..=7)
        .map(|i| {
            Arc::new(MockExternalHwnd::with_title(
                i,
                "App",
                "app.exe",
                env.moves.clone(),
                env.z_model.clone(),
            ))
        })
        .collect();
    for w in &wins {
        env.add_window(w.clone());
    }
    let dims: Vec<_> = wins.iter().map(|w| w.get_dim()).collect();
    assert_h_tiled(&dims, default_screen().dimension, env.config.border_size);
}

#[test]
fn workspace_switch_hides_and_restores() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    let w2 = Arc::new(MockExternalHwnd::with_title(
        2,
        "App2",
        "app2.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    let placed1 = w1.get_dim();
    let placed2 = w2.get_dim();

    env.run_actions("focus workspace 1");
    assert!(w1.is_offscreen());
    assert!(w2.is_offscreen());
    assert!(w1.is_bottom());
    assert!(w2.is_bottom());

    env.run_actions("focus workspace 0");
    assert!(!w1.is_offscreen());
    assert!(!w2.is_offscreen());
    assert!(!w1.is_bottom());
    assert!(!w2.is_bottom());
    assert_eq!(w1.get_dim(), placed1);
    assert_eq!(w2.get_dim(), placed2);
}

#[test]
fn focus_left_right() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    let w2 = Arc::new(MockExternalHwnd::with_title(
        2,
        "App2",
        "app2.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    // w2 is focused (last added). Focus left should move to w1.
    env.run_actions("focus left");
    // Focus right should move back to w2.
    env.run_actions("focus right");

    // Both windows should remain tiled (focus doesn't change layout)
    assert!(!w1.is_offscreen());
    assert!(!w2.is_offscreen());
}

#[test]
fn resize_detects_fullscreen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());

    let border = env.config.border_size;
    let d = w1.get_dim();
    assert_eq!(d.x, border, "should start tiled with border inset");

    // Simulate the user resizing the window to fill the screen
    *w1.dimension.lock().unwrap() = Dimension {
        x: 0.0,
        y: 0.0,
        width: SCREEN_WIDTH,
        height: SCREEN_HEIGHT,
    };

    env.dome.move_size_ended(w1.hwnd_id);
    env.dome.placement_timeout(w1.hwnd_id);
    env.dome
        .window_moved(w1.hwnd_id, ObservedPosition::Fullscreen);
    env.dome.apply_layout();

    // Hub should detect fullscreen — window positioned at full monitor dimensions
    let d = w1.get_dim();
    assert_eq!(d.x, 0.0);
    assert_eq!(d.y, 0.0);
    assert_eq!(d.width, SCREEN_WIDTH);
    assert_eq!(d.height, SCREEN_HEIGHT);
}

#[test]
fn float_move_writes_core_and_does_not_correct() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.run_actions("toggle float");
    env.settle(10);

    // Clear move log to establish baseline
    env.moves.lock().unwrap().clear();

    // Simulate user dragging float to a new position (move_size_ended fires once at drag end)
    env.dome.move_size_ended(w1.hwnd_id);
    env.dome.placement_timeout(w1.hwnd_id);
    env.dome
        .window_moved(w1.hwnd_id, ObservedPosition::Visible(200, 150, 600, 400));

    // Float arm should NOT call set_position
    assert!(
        env.moves.lock().unwrap().is_empty(),
        "float observation should not trigger set_position"
    );

    // Idempotence: fp.target == new_target short-circuits show_float, so no
    // set_position calls are issued across two successive apply_layout rounds.
    env.dome.apply_layout();
    env.settle(10);
    env.dome.apply_layout();
    env.settle(10);
    assert!(
        env.moves.lock().unwrap().is_empty(),
        "two successive apply_layout rounds after float move should be no-ops"
    );

    // Core should store the reverse-inset outer frame
    let border = env.config.border_size;
    let stored = env
        .dome
        .float_frame(w1.hwnd_id)
        .expect("float should have a stored frame");
    assert_eq!(
        stored,
        Dimension {
            x: 200.0 - border,
            y: 150.0 - border,
            width: 600.0 + 2.0 * border,
            height: 400.0 + 2.0 * border,
        }
    );
}
