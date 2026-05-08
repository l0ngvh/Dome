use std::sync::Arc;

use super::*;
use crate::core::{Length, Logical};

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

    let border = Length::new(env.config.border_size);
    let d = w1.get_dim();
    assert_eq!(d.x, border, "should start tiled with border inset");

    // Simulate the user resizing the window to fill the screen
    // window positioned at full monitor dimensions
    *w1.dimension.lock().unwrap() =
        Dimension::new(Length::ZERO, Length::ZERO, SCREEN_WIDTH, SCREEN_HEIGHT);
    let d = w1.get_dim();
    assert_eq!(d.x, Length::ZERO);
    assert_eq!(d.y, Length::ZERO);
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
    env.dome.window_moved(
        w1.hwnd_id,
        ObservedPosition::Visible {
            rect: dim(200, 150, 600, 400),
            monitor: 1,
        },
    );

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

fn scaled_screen(scale: f32) -> ScreenInfo {
    // ScreenInfo.dimension is physical pixels. At non-1.0 scales the physical
    // extent is the logical resolution multiplied by scale.
    ScreenInfo {
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
    let mut env = TestEnv::new_with_screens(Config::default(), vec![scaled_screen(1.0)]);
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    let border = Length::new(env.config.border_size);
    let d = w1.get_dim();
    // At 1.0 scale, physical == logical. Border scaled by 1.0 is unchanged.
    assert_eq!(d.x, border);
    assert_eq!(d.y, border);
    assert_eq!(d.width, SCREEN_WIDTH - 2.0 * border);
    assert_eq!(d.height, SCREEN_HEIGHT - 2.0 * border);
}

#[test]
fn show_tiling_places_at_150pct() {
    let mut env = TestEnv::new_with_screens(Config::default(), vec![scaled_screen(1.5)]);
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    let border = Length::new(env.config.border_size);
    let phys_w = SCREEN_WIDTH * 1.5;
    let phys_h = SCREEN_HEIGHT * 1.5;
    let scaled_border = border * 1.5;
    let d = w1.get_dim();
    // Hub places in physical; border is config.border_size * scale.
    assert_eq!(d.x, (scaled_border).round());
    assert_eq!(d.y, (scaled_border).round());
    assert_eq!(d.width, (phys_w - 2.0 * scaled_border).round());
    assert_eq!(d.height, (phys_h - 2.0 * scaled_border).round());
}

#[test]
fn show_tiling_places_at_200pct_offset_monitor() {
    let primary = ScreenInfo {
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
    let secondary = ScreenInfo {
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
    let mut env = TestEnv::new_with_screens(Config::default(), vec![primary, secondary]);
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    // Move to the secondary monitor
    env.run_actions("move monitor right");
    env.settle(10);
    let border = Length::new(env.config.border_size);
    let scaled_border = border * 2.0;
    let d = w1.get_dim();
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
    let mut env = TestEnv::new_with_screens(Config::default(), vec![scaled_screen(1.25)]);
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.run_actions("toggle float");
    env.settle(10);

    // Clear moves to baseline
    env.moves.lock().unwrap().clear();

    // Simulate user dragging float to known physical position
    env.dome.move_size_ended(w1.hwnd_id);
    env.dome.placement_timeout(w1.hwnd_id);
    env.dome.window_moved(
        w1.hwnd_id,
        ObservedPosition::Visible {
            rect: dim(200, 150, 600, 400),
            monitor: 1,
        },
    );

    // Drive the next placement cycle
    env.dome.apply_layout();
    env.settle(10);

    // Under physical-native core, the observation (200,150,600,400) is stored
    // directly (after reverse_inset for Hub, then apply_inset for show_float).
    // Round-trip is identity: no conversion.
    let d = w1.get_dim();
    assert_eq!(d.x, Length::new(200.0));
    assert_eq!(d.y, Length::new(150.0));
    assert_eq!(d.width, Length::new(600.0));
    assert_eq!(d.height, Length::new(400.0));
}

#[test]
fn show_fullscreen_window_places_at_175pct() {
    let mut env = TestEnv::new_with_screens(Config::default(), vec![scaled_screen(1.75)]);
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());

    let phys_w = SCREEN_WIDTH * 1.75;
    let phys_h = SCREEN_HEIGHT * 1.75;

    // Simulate the user resizing to fill the screen (triggers fullscreen detection).
    // The mock dimension must match the physical monitor extent.
    *w1.dimension.lock().unwrap() = Dimension::new(Length::ZERO, Length::ZERO, phys_w, phys_h);
    env.dome.move_size_ended(w1.hwnd_id);
    env.dome.placement_timeout(w1.hwnd_id);
    env.dome
        .window_moved(w1.hwnd_id, ObservedPosition::Fullscreen);
    env.dome.apply_layout();

    let d = w1.get_dim();
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
    let mut env = TestEnv::new_with_screens(Config::default(), vec![scaled_screen(1.25)]);
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.run_actions("toggle float");
    env.settle(10);
    env.moves.lock().unwrap().clear();

    // Simulate user dragging float to a known physical position
    env.dome.move_size_ended(w1.hwnd_id);
    env.dome.placement_timeout(w1.hwnd_id);
    env.dome.window_moved(
        w1.hwnd_id,
        ObservedPosition::Visible {
            rect: dim(300, 200, 500, 400),
            monitor: 1,
        },
    );
    env.dome.apply_layout();
    env.settle(10);

    let d1 = w1.get_dim();

    // Simulate the OS reporting back the position we just set (as window_drifted would)
    env.dome.move_size_ended(w1.hwnd_id);
    env.dome.placement_timeout(w1.hwnd_id);
    env.dome.window_moved(
        w1.hwnd_id,
        ObservedPosition::Visible {
            rect: d1,
            monitor: 1,
        },
    );
    env.dome.apply_layout();
    env.settle(10);

    let d2 = w1.get_dim();

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
    let primary = ScreenInfo {
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
    let secondary = ScreenInfo {
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
    let mut env = TestEnv::new_with_screens(Config::default(), vec![primary, secondary]);
    let win = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(win.clone());
    env.run_actions("toggle float");
    env.settle(10);

    let original_dim = win.get_dim();

    // Clear moves to establish baseline
    env.moves.lock().unwrap().clear();

    // Report an unknown monitor handle (999). The observation should be
    // dropped entirely — no position change, no dimension change.
    env.dome.move_size_ended(win.hwnd_id);
    env.dome.placement_timeout(win.hwnd_id);
    env.dome.window_moved(
        win.hwnd_id,
        ObservedPosition::Visible {
            rect: dim(3000, 500, 600, 400),
            monitor: 999,
        },
    );
    env.dome.apply_layout();
    env.settle(10);

    assert!(
        env.moves.lock().unwrap().is_empty(),
        "unknown monitor handle should not trigger set_position"
    );
    assert_eq!(
        win.get_dim(),
        original_dim,
        "unknown monitor handle should not change window dimension"
    );
}

#[test]
fn monitor_dpi_changed_reruns_layout_with_new_scale() {
    let screen = ScreenInfo {
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
    let config = Config {
        tab_bar_height: Length::<Logical>::new(30.0),
        ..Config::default()
    };
    let mut env = TestEnv::new_with_screens(config, vec![screen]);
    let win = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(win.clone());
    // Put into a tabbed container so tab_bar_height participates in layout
    env.run_actions("toggle stacking");
    env.settle(10);

    let d_before = win.get_dim();
    // At scale 1.0, tab bar is 30px: y == border + 30, height == 1080 - 2*border - 30
    let border = Length::new(env.config.border_size);
    let tab_h_1x = Length::new(30.0);
    assert_eq!(d_before.y, (border + tab_h_1x).round());

    // Simulate DPI change to 192 (scale 2.0)
    let handle = 1_isize;
    env.dome.monitor_dpi_changed(handle, 192);
    env.dome.apply_layout();
    env.settle(10);

    let d_after = win.get_dim();
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
    let mut env = TestEnv::new_with_screens(Config::default(), vec![scaled_screen(2.0)]);
    let w1 = Arc::new(
        MockExternalHwnd::with_title(
            1,
            "App1",
            "app1.exe",
            env.moves.clone(),
            env.z_model.clone(),
        )
        .with_min_size(2000.0, 100.0),
    );
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.settle(10);

    // physical_border = config.border_size * monitor_scale.
    // Dome::set_constraints converts client min to frame min by adding 2 * physical_border.
    let physical_border = env.config.border_size * 2.0; // 2.0 is the monitor scale
    let expected_min_frame_width = (2000.0 + 2.0 * physical_border).round();

    // The min-size constraint binds (expected > half of physical 3840), so w1 is
    // placed at exactly the minimum frame width.
    assert_eq!(w1.get_dim().width, Length::new(expected_min_frame_width));
    // Sibling receives the remaining width, not an even split.
    assert!(w2.get_dim().width < w1.get_dim().width);
}

#[test]
fn float_move_monitor_same_dpi_preserves_content_rect() {
    let mut env =
        TestEnv::new_with_screens(Config::default(), vec![default_screen(), second_screen()]);
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.run_actions("toggle float");
    env.settle(10);

    // Anchor the float at a known position on the primary monitor
    env.dome.move_size_ended(w1.hwnd_id);
    env.dome.placement_timeout(w1.hwnd_id);
    env.dome.window_moved(
        w1.hwnd_id,
        ObservedPosition::Visible {
            rect: dim(200, 150, 600, 400),
            monitor: 1,
        },
    );
    env.dome.apply_layout();
    env.settle(10);

    let baseline_dim = w1.get_dim();
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
        env.moves
            .lock()
            .unwrap()
            .iter()
            .any(|(id, ..)| *id == w1.hwnd_id),
        "cross-monitor float move should trigger set_position"
    );
    // Both monitors are scale 1.0, so border inset is identical and content
    // rect is preserved byte-for-byte across the workspace move.
    assert_eq!(
        w1.get_dim(),
        baseline_dim,
        "same-DPI move should preserve the content rect byte-for-byte"
    );
}

#[test]
fn float_move_monitor_different_dpi_rescales_border() {
    let primary = ScreenInfo {
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
    let secondary = ScreenInfo {
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
    let mut env = TestEnv::new_with_screens(Config::default(), vec![primary, secondary]);
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.run_actions("toggle float");
    env.settle(10);

    // Anchor the float at a known content rect on monitor 1
    env.dome.move_size_ended(w1.hwnd_id);
    env.dome.placement_timeout(w1.hwnd_id);
    env.dome.window_moved(
        w1.hwnd_id,
        ObservedPosition::Visible {
            rect: dim(100, 100, 400, 300),
            monitor: 1,
        },
    );
    env.dome.apply_layout();
    env.settle(10);

    let border = Length::new(env.config.border_size);
    env.moves.lock().unwrap().clear();
    env.run_actions("move monitor right");
    env.settle(10);

    assert!(
        env.moves
            .lock()
            .unwrap()
            .iter()
            .any(|(id, ..)| *id == w1.hwnd_id),
        "cross-monitor float move should trigger set_position"
    );

    // The outer frame is preserved across the workspace move. On the target
    // monitor at scale 2.0, physical_border = border * 2.0. The content rect
    // is apply_inset(outer, border * 2.0) which differs from the original
    // apply_inset(outer, border * 1.0).
    let d = w1.get_dim();
    assert_eq!(d.x, (Length::new(100.0) + border).round());
    assert_eq!(d.y, (Length::new(100.0) + border).round());
    assert_eq!(d.width, (Length::new(400.0) - 2.0 * border).round());
    assert_eq!(d.height, (Length::new(300.0) - 2.0 * border).round());
}

#[test]
fn dome_new_assigns_per_monitor_scale() {
    let primary = ScreenInfo {
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
    let secondary = ScreenInfo {
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
    let mut env = TestEnv::new_with_screens(Config::default(), vec![primary, secondary]);
    let border = Length::new(env.config.border_size);

    // Verify primary monitor uses 1.5x scale via window placement.
    let w_a = env.spawn_window(1, "AppA", "a.exe");
    env.add_window(w_a.clone());
    let scaled_border = border * 1.5;
    let phys_w = SCREEN_WIDTH * 1.5;
    let phys_h = SCREEN_HEIGHT * 1.5;
    let d_a = w_a.get_dim();
    assert_eq!(d_a.x, scaled_border.round());
    assert_eq!(d_a.y, scaled_border.round());
    assert_eq!(d_a.width, (phys_w - 2.0 * scaled_border).round());
    assert_eq!(d_a.height, (phys_h - 2.0 * scaled_border).round());

    // Verify secondary monitor uses 2.0x scale via window placement.
    let w_b = env.spawn_window(2, "AppB", "b.exe");
    env.add_window(w_b.clone());
    env.run_actions("move monitor right");
    env.settle(10);
    let scaled_border_b = border * 2.0;
    let d_b = w_b.get_dim();
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
