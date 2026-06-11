use std::sync::Arc;

use super::*;
use crate::core::{Length, Logical};

#[test]
fn single_window_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    assert_h_tiled(
        &[env.dim(w1)],
        default_monitor().dimension,
        env.config.border_size,
    );
}

#[test]
fn two_windows_split_screen() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    assert_h_tiled(
        &[env.dim(w1), env.dim(w2)],
        default_monitor().dimension,
        env.config.border_size,
    );
}

#[test]
fn three_windows_split_screen() {
    let mut config = Config::default();
    config.layout.partition_tree.automatic_tiling = false;
    let mut env = TestEnv::new_with_config(config);
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);
    assert_h_tiled(
        &[env.dim(w1), env.dim(w2), env.dim(w3)],
        default_monitor().dimension,
        env.config.border_size,
    );
}

/// distribute_space uses binary search and may produce fractional widths
/// (e.g. 1920/3 ≈ 639.999). The f32→i32 conversion in show_tiling must
/// round, not truncate, or the cumulative error pushes the last window's
/// right edge away from the screen edge.
#[test]
fn positions_are_rounded_not_truncated() {
    let mut config = Config::default();
    config.layout.partition_tree.automatic_tiling = false;
    let mut env = TestEnv::new_with_config(config);
    let wins: Vec<HwndId> = (1..=7)
        .map(|i| env.open(i, "App", "app.exe", SPAWN_DIM))
        .collect();
    let dims: Vec<_> = wins.iter().map(|w| env.dim(*w)).collect();
    assert_h_tiled(&dims, default_monitor().dimension, env.config.border_size);
}

#[test]
fn workspace_switch_hides_and_restores() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    let placed1 = env.dim(w1);
    let placed2 = env.dim(w2);

    env.run_actions("focus workspace 1");
    assert!(env.is_offscreen(w1));
    assert!(env.is_offscreen(w2));
    assert!(env.is_bottom(w1));
    assert!(env.is_bottom(w2));

    env.run_actions("focus workspace 0");
    assert!(!env.is_offscreen(w1));
    assert!(!env.is_offscreen(w2));
    assert_eq!(env.dim(w1), placed1);
    assert_eq!(env.dim(w2), placed2);
}

#[test]
fn focus_left_right() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // w2 is focused (last added). Focus left should move to w1.
    env.run_actions("focus left");
    // Focus right should move back to w2.
    env.run_actions("focus right");

    // Both windows should remain tiled (focus doesn't change layout)
    assert!(!env.is_offscreen(w1));
    assert!(!env.is_offscreen(w2));
}

#[test]
fn resize_detects_fullscreen() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    let border = Length::new(env.config.border_size);
    let d = env.dim(w1);
    assert_eq!(d.x, border, "should start tiled with border inset");

    // Simulate the user resizing the window to fill the screen
    // window positioned at full monitor dimensions
    env.set_dim(
        w1,
        Dimension::new(Length::ZERO, Length::ZERO, SCREEN_WIDTH, SCREEN_HEIGHT),
    );
    let d = env.dim(w1);
    assert_eq!(d.x, Length::ZERO);
    assert_eq!(d.y, Length::ZERO);
    assert_eq!(d.width, SCREEN_WIDTH);
    assert_eq!(d.height, SCREEN_HEIGHT);
}

#[test]
fn float_move_writes_core_and_does_not_correct() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    // Clear move log to establish baseline
    env.moves.lock().unwrap().clear();

    env.window_moved(w1, dim(200, 150, 600, 400), 1);

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
}

// These tests verify that show_tiling, show_float, and show_fullscreen_window
// pass physical-native frames from Hub directly to SetWindowPos. Border inset
// uses `config.border_size * monitor.scale` (config-to-physical scaling).

fn scaled_monitor(scale: f32) -> MonitorInfo {
    // MonitorInfo.dimension is physical pixels. At non-1.0 scales the physical
    // extent is the logical resolution multiplied by scale.
    MonitorInfo {
        handle: 1,
        name: "Test".to_string(),
        dimension: Dimension::new(
            Length::ZERO,
            Length::ZERO,
            SCREEN_WIDTH * scale,
            SCREEN_HEIGHT * scale,
        ),
        is_primary: true,
        scale,
    }
}

#[test]
fn show_tiling_places_at_100pct() {
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![scaled_monitor(1.0)]);
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let border = Length::new(env.config.border_size);
    let d = env.dim(w1);
    // At 1.0 scale, physical == logical. Border scaled by 1.0 is unchanged.
    assert_eq!(d.x, border);
    assert_eq!(d.y, border);
    assert_eq!(d.width, SCREEN_WIDTH - 2.0 * border);
    assert_eq!(d.height, SCREEN_HEIGHT - 2.0 * border);
}

#[test]
fn show_tiling_places_at_150pct() {
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![scaled_monitor(1.5)]);
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let border = Length::new(env.config.border_size);
    let phys_w = SCREEN_WIDTH * 1.5;
    let phys_h = SCREEN_HEIGHT * 1.5;
    let scaled_border = border * 1.5;
    let d = env.dim(w1);
    // Hub places in physical; border is config.border_size * scale.
    assert_eq!(d.x, (scaled_border).round());
    assert_eq!(d.y, (scaled_border).round());
    assert_eq!(d.width, (phys_w - 2.0 * scaled_border).round());
    assert_eq!(d.height, (phys_h - 2.0 * scaled_border).round());
}

#[test]
fn show_tiling_places_at_200pct_offset_monitor() {
    let primary = MonitorInfo {
        handle: 1,
        name: "Primary".to_string(),
        dimension: Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(1920.0),
            Length::new(1080.0),
        ),
        is_primary: true,
        scale: 1.0,
    };
    // Physical dimensions at 2.0x: 2560*2=5120, 1440*2=2880, origin 1920
    let secondary = MonitorInfo {
        handle: 2,
        name: "Secondary".to_string(),
        dimension: Dimension::new(
            Length::new(1920.0),
            Length::new(0.0),
            Length::new(5120.0),
            Length::new(2880.0),
        ),
        is_primary: false,
        scale: 2.0,
    };
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![primary, secondary]);
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    // Move to the secondary monitor
    env.run_actions("move monitor right");
    env.settle(10);
    let border = Length::new(env.config.border_size);
    let scaled_border = border * 2.0;
    let d = env.dim(w1);
    // Hub places directly in physical coords on the secondary monitor.
    assert_eq!(d.x, (Length::new(1920.0) + scaled_border).round());
    assert_eq!(d.y, (scaled_border).round());
    assert_eq!(d.width, (Length::new(5120.0) - 2.0 * scaled_border).round());
    assert_eq!(
        d.height,
        (Length::new(2880.0) - 2.0 * scaled_border).round()
    );
}

#[test]
fn show_float_places_at_125pct() {
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![scaled_monitor(1.25)]);
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    // Clear moves to baseline
    env.moves.lock().unwrap().clear();

    env.window_moved(w1, dim(200, 150, 600, 400), 1);
    // Drive the next placement cycle
    env.dome.apply_layout();
    env.settle(10);

    // Under physical-native core, the observation (200,150,600,400) is stored
    // directly (after reverse_inset for Hub, then apply_inset for show_float).
    // Round-trip is identity: no conversion.
    let d = env.dim(w1);
    assert_eq!(d.x, Length::new(200.0));
    assert_eq!(d.y, Length::new(150.0));
    assert_eq!(d.width, Length::new(600.0));
    assert_eq!(d.height, Length::new(400.0));
}

#[test]
fn show_fullscreen_window_places_at_175pct() {
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![scaled_monitor(1.75)]);
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    let phys_w = SCREEN_WIDTH * 1.75;
    let phys_h = SCREEN_HEIGHT * 1.75;

    // Simulate the user resizing to fill the screen (triggers fullscreen detection).
    // The mock dimension must match the physical monitor extent.
    env.set_dim(
        w1,
        Dimension::new(Length::ZERO, Length::ZERO, phys_w, phys_h),
    );
    env.window_moved(
        w1,
        Dimension::new(Length::ZERO, Length::ZERO, phys_w, phys_h),
        1,
    );
    env.dome.apply_layout();

    let d = env.dim(w1);
    // Fullscreen covers the full physical monitor dimension directly.
    assert_eq!(d.x, Length::ZERO);
    assert_eq!(d.y, Length::ZERO);
    assert_eq!(d.width, phys_w.round());
    assert_eq!(d.height, phys_h.round());
}

/// Proves that the physical round-trip converges at non-100% scales.
/// Under agnostic-core, no conversion occurs, so this is a pure identity check.
#[test]
fn float_round_trip_converges_at_125pct() {
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![scaled_monitor(1.25)]);
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);
    env.moves.lock().unwrap().clear();

    env.window_moved(w1, dim(300, 200, 500, 400), 1);
    env.dome.apply_layout();
    env.settle(10);

    let d1 = env.dim(w1);

    // Simulate the OS reporting back the position we just set (as window_drifted would)
    env.window_moved(w1, d1, 1);
    env.dome.apply_layout();
    env.settle(10);

    let d2 = env.dim(w1);

    // Position must be stable across iterations
    assert_eq!(d1.x, d2.x, "x diverged");
    assert_eq!(d1.y, d2.y, "y diverged");
    assert_eq!(d1.width, d2.width, "width diverged");
    assert_eq!(d1.height, d2.height, "height diverged");

    // Identity: values round-trip back to original physical coords
    assert_eq!(d2.x, Length::new(300.0));
    assert_eq!(d2.y, Length::new(200.0));
    assert_eq!(d2.width, Length::new(500.0));
    assert_eq!(d2.height, Length::new(400.0));
}

#[test]
fn window_drifted_float_ignores_unknown_monitor_handle() {
    let primary = MonitorInfo {
        handle: 1,
        name: "Primary".to_string(),
        dimension: Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(1920.0),
            Length::new(1080.0),
        ),
        is_primary: true,
        scale: 1.0,
    };
    let secondary = MonitorInfo {
        handle: 2,
        name: "Secondary".to_string(),
        dimension: Dimension::new(
            Length::new(1920.0),
            Length::new(0.0),
            Length::new(3840.0),
            Length::new(2160.0),
        ),
        is_primary: false,
        scale: 2.0,
    };
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![primary, secondary]);
    let win = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    let original_dim = env.dim(win);

    // Clear moves to establish baseline
    env.moves.lock().unwrap().clear();

    // Report an unknown monitor handle (999). The observation should be
    // dropped entirely -- no position change, no dimension change.
    env.window_moved(win, dim(3000, 500, 600, 400), 999);
    env.dome.apply_layout();
    env.settle(10);

    assert!(
        env.moves.lock().unwrap().is_empty(),
        "unknown monitor handle should not trigger set_position"
    );
    assert_eq!(
        env.dim(win),
        original_dim,
        "unknown monitor handle should not change window dimension"
    );
}

#[test]
fn monitor_dpi_changed_reruns_layout_with_new_scale() {
    let monitor = MonitorInfo {
        handle: 1,
        name: "Test".to_string(),
        dimension: Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(1920.0),
            Length::new(1080.0),
        ),
        is_primary: true,
        scale: 1.0,
    };
    let mut config = Config::default();
    config.layout.partition_tree.tab_bar_height = Length::<Logical>::new(30.0);
    let mut env = TestEnv::new_with_monitors(config, vec![monitor]);
    let win = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    // Put into a tabbed container so tab_bar_height participates in layout
    env.run_actions("toggle stacking");
    env.settle(10);

    let d_before = env.dim(win);
    // At scale 1.0, tab bar is 30px: y == border + 30, height == 1080 - 2*border - 30
    let border = Length::new(env.config.border_size);
    let tab_h_1x = Length::new(30.0);
    assert_eq!(d_before.y, (border + tab_h_1x).round());

    // Simulate DPI change to 192 (scale 2.0)
    let handle = 1_isize;
    env.dome.monitor_dpi_changed(handle, 192);
    env.dome.apply_layout();
    env.settle(10);

    let d_after = env.dim(win);
    // At scale 2.0, tab bar is 30*2=60px, border is still logical but scaled by 2.0
    let scaled_border = border * 2.0;
    let tab_h_2x = Length::new(30.0 * 2.0);
    assert_eq!(d_after.y, (scaled_border + tab_h_2x).round());
    assert_eq!(
        d_after.height,
        (Length::new(1080.0) - 2.0 * scaled_border - tab_h_2x).round()
    );
}

#[test]
fn window_min_size_constraint_on_high_dpi_monitor() {
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![scaled_monitor(2.0)]);
    let arc = Arc::new(
        MockExternalHwnd::with_title(
            1,
            "App1",
            "app1.exe",
            env.moves.clone(),
            env.z_stack.clone(),
            env.focus_target.clone(),
        )
        .with_min_size(2000.0, 100.0),
    );
    let w1 = env.open_with(arc);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.settle(10);

    // physical_border = config.border_size * monitor_scale.
    // Dome::set_constraints converts client min to frame min by adding 2 * physical_border.
    let physical_border = env.config.border_size * 2.0; // 2.0 is the monitor scale
    let expected_min_frame_width = (2000.0 + 2.0 * physical_border).round();

    // The min-size constraint binds (expected > half of physical 3840), so w1 is
    // placed at exactly the minimum frame width.
    assert_eq!(env.dim(w1).width, Length::new(expected_min_frame_width));
    // Sibling receives the remaining width, not an even split.
    assert!(env.dim(w2).width < env.dim(w1).width);
}

#[test]
fn float_move_monitor_same_dpi_preserves_content_rect() {
    let mut env =
        TestEnv::new_with_monitors(Config::default(), vec![default_monitor(), second_monitor()]);
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    // Anchor the float at a known position on the primary monitor
    env.window_moved(w1, dim(200, 150, 600, 400), 1);
    env.settle(10);

    let baseline_dim = env.dim(w1);
    // At scale 1.0, content rect equals the observed rect (identity round-trip)
    assert_eq!(
        baseline_dim,
        Dimension::new(
            Length::new(200.0),
            Length::new(150.0),
            Length::new(600.0),
            Length::new(400.0)
        )
    );

    env.moves.lock().unwrap().clear();
    env.run_actions("move monitor right");
    env.settle(10);

    assert!(
        env.moves.lock().unwrap().iter().any(|(id, ..)| *id == w1),
        "cross-monitor float move should trigger set_position"
    );
    // Both monitors are scale 1.0, so border inset is identical and content
    // rect is preserved byte-for-byte across the workspace move.
    assert_eq!(
        env.dim(w1),
        baseline_dim,
        "same-DPI move should preserve the content rect byte-for-byte"
    );
}

#[test]
fn float_move_monitor_different_dpi_rescales_border() {
    let primary = MonitorInfo {
        handle: 1,
        name: "Primary".to_string(),
        dimension: Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(1920.0),
            Length::new(1080.0),
        ),
        is_primary: true,
        scale: 1.0,
    };
    let secondary = MonitorInfo {
        handle: 2,
        name: "Secondary".to_string(),
        dimension: Dimension::new(
            Length::new(1920.0),
            Length::new(0.0),
            Length::new(5120.0),
            Length::new(2880.0),
        ),
        is_primary: false,
        scale: 2.0,
    };
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![primary, secondary]);
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    // Anchor the float at a known content rect on monitor 1
    env.window_moved(w1, dim(100, 100, 400, 300), 1);
    env.dome.apply_layout();
    env.settle(10);

    let border = Length::new(env.config.border_size);
    env.moves.lock().unwrap().clear();
    env.run_actions("move monitor right");
    env.settle(10);

    assert!(
        env.moves.lock().unwrap().iter().any(|(id, ..)| *id == w1),
        "cross-monitor float move should trigger set_position"
    );

    // The outer frame is preserved across the workspace move. On the target
    // monitor at scale 2.0, physical_border = border * 2.0. The content rect
    // is apply_inset(outer, border * 2.0) which differs from the original
    // apply_inset(outer, border * 1.0).
    let d = env.dim(w1);
    assert_eq!(d.x, (Length::new(100.0) + border).round());
    assert_eq!(d.y, (Length::new(100.0) + border).round());
    assert_eq!(d.width, (Length::new(400.0) - 2.0 * border).round());
    assert_eq!(d.height, (Length::new(300.0) - 2.0 * border).round());
}

#[test]
fn dome_new_assigns_per_monitor_scale() {
    let primary = MonitorInfo {
        handle: 1,
        name: "Primary".to_string(),
        dimension: Dimension::new(
            Length::ZERO,
            Length::ZERO,
            SCREEN_WIDTH * 1.5,
            SCREEN_HEIGHT * 1.5,
        ),
        is_primary: true,
        scale: 1.5,
    };
    let secondary = MonitorInfo {
        handle: 2,
        name: "Secondary".to_string(),
        dimension: Dimension::new(
            SCREEN_WIDTH * 1.5,
            Length::ZERO,
            Length::new(5120.0),
            Length::new(2880.0),
        ),
        is_primary: false,
        scale: 2.0,
    };
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![primary, secondary]);
    let border = Length::new(env.config.border_size);

    // Verify primary monitor uses 1.5x scale via window placement.
    let w_a = env.open(1, "AppA", "a.exe", SPAWN_DIM);
    let scaled_border = border * 1.5;
    let phys_w = SCREEN_WIDTH * 1.5;
    let phys_h = SCREEN_HEIGHT * 1.5;
    let d_a = env.dim(w_a);
    assert_eq!(d_a.x, scaled_border.round());
    assert_eq!(d_a.y, scaled_border.round());
    assert_eq!(d_a.width, (phys_w - 2.0 * scaled_border).round());
    assert_eq!(d_a.height, (phys_h - 2.0 * scaled_border).round());

    // Verify secondary monitor uses 2.0x scale via window placement.
    let w_b = env.open(2, "AppB", "b.exe", SPAWN_DIM);
    env.run_actions("move monitor right");
    env.settle(10);
    let scaled_border_b = border * 2.0;
    let d_b = env.dim(w_b);
    assert_eq!(d_b.x, (SCREEN_WIDTH * 1.5 + scaled_border_b).round());
    assert_eq!(d_b.y, scaled_border_b.round());
    assert_eq!(
        d_b.width,
        (Length::new(5120.0) - 2.0 * scaled_border_b).round()
    );
    assert_eq!(
        d_b.height,
        (Length::new(2880.0) - 2.0 * scaled_border_b).round()
    );
}

#[test]
fn float_drift_repositions_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    env.set_dim(
        w1,
        Dimension::new(
            Length::new(500.0),
            Length::new(300.0),
            Length::new(400.0),
            Length::new(250.0),
        ),
    );
    env.window_moved(w1, dim(500, 300, 400, 250), 1);

    // The overlay receives the border-expanded outer_dim, not the raw managed-window rect.
    let border = Length::new(env.config.border_size);
    let expected_outer = Dimension::new(
        Length::new(500.0) - border,
        Length::new(300.0) - border,
        Length::new(400.0) + 2.0 * border,
        Length::new(250.0) + 2.0 * border,
    );
    let FloatOverlayState::Visible { visible_frame, .. } = env.float_overlay_state() else {
        panic!(
            "float overlay must be visible after drag, got {:?}",
            env.float_overlay_state()
        );
    };
    assert_eq!(
        visible_frame, expected_outer,
        "overlay should receive the border-expanded outer_dim as visible_frame"
    );
}

#[test]
fn float_drift_does_not_touch_managed_hwnd() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    // Clear move log to establish baseline
    env.moves.lock().unwrap().clear();

    env.set_dim(
        w1,
        Dimension::new(
            Length::new(500.0),
            Length::new(300.0),
            Length::new(400.0),
            Length::new(250.0),
        ),
    );
    env.window_moved(w1, dim(500, 300, 400, 250), 1);

    // The fix must not call set_position on the managed HWND
    assert!(
        env.moves.lock().unwrap().is_empty(),
        "overlay update must not trigger set_position on the managed HWND"
    );
}

#[test]
fn float_drift_overlay_update_does_not_repeat_on_next_apply_layout() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    env.set_dim(
        w1,
        Dimension::new(
            Length::new(500.0),
            Length::new(300.0),
            Length::new(400.0),
            Length::new(250.0),
        ),
    );
    env.window_moved(w1, dim(500, 300, 400, 250), 1);

    // Snapshot the float overlay state after the first update (from window_drifted)
    let after_drift = env.float_overlay_state();

    // show_float's settled branch must remain a no-op
    env.dome.apply_layout();
    env.settle(10);

    assert_eq!(
        env.float_overlay_state(),
        after_drift,
        "apply_layout after drift must not re-update the overlay (settled short-circuit)"
    );
}
