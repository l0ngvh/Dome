use super::*;
use crate::core::{GlobalLayoutConfig, Length, Logical, Pixels};

#[test]
fn single_window_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    assert_h_tiled(
        &[env.dim(w1)],
        default_monitor().work_area,
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
        default_monitor().work_area,
        env.config.border_size,
    );
}

#[test]
fn three_windows_split_screen() {
    let config = Config::default();
    let mut layout = GlobalLayoutConfig::default();
    layout.partition_tree.automatic_tiling = false;
    let mut env = TestEnv::new_with_layout_settings(config, layout, Vec::new());
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);
    assert_h_tiled(
        &[env.dim(w1), env.dim(w2), env.dim(w3)],
        default_monitor().work_area,
        env.config.border_size,
    );
}

#[test]
fn reported_min_width_binds_while_zero_min_height_is_cleared() {
    let mut env = TestEnv::new();
    let w1 = env.open_with_min_size(1, "App1", "app1.exe", SPAWN_DIM, (1200.0, 0.0));
    env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // An even split would leave each window near 952, so the minimum binds. The
    // shell forwards it untouched and core outsets it by the border, so the app
    // gets back exactly the content width it asked for.
    assert_eq!(env.dim(w1).width, Length::new(1200.0));
    // The zero height component reads as Cleared, not a zero-height minimum.
    assert_eq!(
        env.dim(w1).height,
        SCREEN_HEIGHT - env.config.border_size.to_unit(1.0) * 2.0
    );
}

#[test]
fn dropping_all_limits_restores_the_even_split() {
    let mut env = TestEnv::new();
    let w1 = env.open_with_min_size(1, "App1", "app1.exe", SPAWN_DIM, (1200.0, 0.0));
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    assert_eq!(env.dim(w1).width, Length::new(1200.0));

    // Mirrors dispatch_constraint_read re-reading an app that no longer reports a
    // minimum. Discarding an all-clear observation would strand the 1200 forever.
    env.dome.set_constraints_for(
        w1,
        LimitObservation {
            min_width: LimitUpdate::Cleared,
            min_height: LimitUpdate::Cleared,
            max_width: LimitUpdate::Cleared,
            max_height: LimitUpdate::Cleared,
        },
    );
    env.dome.apply_layout();

    assert_eq!(env.dim(w1).width, Length::new(952.0));
    assert_eq!(env.dim(w2).width, Length::new(952.0));
    assert_h_tiled(
        &[env.dim(w1), env.dim(w2)],
        default_monitor().work_area,
        env.config.border_size,
    );
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

    let border = Length::new(env.config.border_size.logical());
    let d = env.dim(w1);
    assert_eq!(d.x, border, "should start tiled with border inset");

    // Simulate the user resizing the window to fill the screen
    // window positioned at full monitor dimensions
    env.move_window_to(
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
fn dont_correct_float_move() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    // Clear move log to establish baseline
    env.moves.lock().unwrap().clear();

    env.move_window_to(w1, dim(200, 150, 600, 400));

    // Float arm should NOT call set_position
    env.flush_moves();

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

/// distribute_space uses binary search and may produce fractional widths
/// (e.g. 1920/3 ≈ 639.999). The f32→i32 conversion in show_tiling must
/// round, not truncate, or the cumulative error pushes the last window's
/// right edge away from the screen edge.
#[test]
fn positions_are_rounded_not_truncated() {
    let config = Config::default();
    let mut layout = GlobalLayoutConfig::default();
    layout.partition_tree.automatic_tiling = false;
    let mut env = TestEnv::new_with_layout_settings(config, layout, Vec::new());
    let wins: Vec<HwndId> = (1..=7)
        .map(|i| env.open(i, "App", "app.exe", SPAWN_DIM))
        .collect();
    let dims: Vec<_> = wins.iter().map(|w| env.dim(*w)).collect();
    assert_h_tiled(&dims, default_monitor().work_area, env.config.border_size);
}

// These tests verify that show_tiling, show_float, and show_fullscreen_window
// pass physical-native frames from Hub directly to SetWindowPos. The shell no
// longer insets anything: it places core's `content_box` verbatim.

fn scaled_monitor(scale: f32) -> MonitorInfo {
    // MonitorInfo.work_area is physical pixels. At non-1.0 scales the physical
    // extent is the logical resolution multiplied by scale.
    MonitorInfo {
        handle: 1,
        name: "Test".to_string(),
        work_area: PixelRect::from_dimension(Dimension::new(
            Length::ZERO,
            Length::ZERO,
            SCREEN_WIDTH * scale,
            SCREEN_HEIGHT * scale,
        )),
        bounds: Dimension::new(
            Length::ZERO,
            Length::ZERO,
            SCREEN_WIDTH * scale,
            SCREEN_HEIGHT * scale,
        ),
        is_primary: true,
        scale,
    }
}

fn only_recorded_tiling(env: &TestEnv) -> TilingWindowPlacement {
    let TilingOverlayState::Visible { windows, .. } = env.tiling_overlays()[0].state.clone() else {
        panic!(
            "tiling overlay should be visible, got {:?}",
            env.tiling_overlays()[0].state
        );
    };
    assert_eq!(windows.len(), 1, "these tests tile exactly one window");
    windows[0]
}

/// Holds whatever the border resolves to, so it pins the shape of the inset
/// without restating the scale multiply the assertion is meant to check.
fn assert_content_box_centered_in_border_box(wp: &TilingWindowPlacement) {
    let inset = wp.content_box.x() - wp.border_box.x();
    assert_eq!(wp.content_box.y() - wp.border_box.y(), inset);
    assert_eq!(wp.content_box.width(), wp.border_box.width() - inset * 2);
    assert_eq!(wp.content_box.height(), wp.border_box.height() - inset * 2);
}

#[test]
fn tiling_border_scales_with_dpi() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let mut env = TestEnv::new_with_monitors(
            Config::default(),
            LayoutConfig::default(),
            vec![scaled_monitor(scale)],
        );
        let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
        let wp = only_recorded_tiling(&env);
        let expected_inset = Pixels::new((env.config.border_size.logical() * scale).round() as i32);

        assert_eq!(env.dim(w1), wp.content_box.to_dimension(), "scale {scale}");
        assert_content_box_centered_in_border_box(&wp);
        assert_eq!(
            wp.content_box.x() - wp.border_box.x(),
            expected_inset,
            "scale {scale}"
        );
    }
}

#[test]
fn painted_thickness_matches_core_inset() {
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![scaled_monitor(1.25)],
    );
    env.open(1, "App1", "app1.exe", SPAWN_DIM);

    let TilingOverlayState::Visible {
        windows,
        border_thickness,
    } = env.tiling_overlays()[0].state.clone()
    else {
        panic!(
            "tiling overlay should be visible, got {:?}",
            env.tiling_overlays()[0].state
        );
    };
    assert_eq!(windows.len(), 1, "expected exactly one recorded window");
    let wp = windows[0];

    // The painter strokes a band of exactly this thickness inside border_box, so any
    // disagreement with the inset core already applied shows up as a hairline.
    assert_eq!(border_thickness, wp.content_box.x() - wp.border_box.x());
    assert_eq!(
        border_thickness * 2,
        wp.border_box.width() - wp.content_box.width()
    );
}

#[test]
fn degenerate_content_box_hides_window() {
    // 600 physical per edge against a 1080-tall monitor leaves no content height,
    // so core hands the shell an empty content box.
    let mut env = TestEnv::new_with_monitors(
        Config {
            border_size: Length::new(600.0),
            ..Config::default()
        },
        LayoutConfig::default(),
        vec![scaled_monitor(1.0)],
    );
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    assert!(env.is_offscreen(w1));

    let mut restored = env.config.clone();
    restored.border_size = Config::default().border_size;
    env.dome.config_changed(restored);
    env.dome.apply_layout();

    let wp = only_recorded_tiling(&env);
    assert!(!env.is_offscreen(w1));
    assert_eq!(env.dim(w1), wp.content_box.to_dimension());
    assert_content_box_centered_in_border_box(&wp);
}

#[test]
fn show_tiling_places_at_200pct_offset_monitor() {
    let primary = MonitorInfo {
        handle: 1,
        name: "Primary".to_string(),
        work_area: PixelRect::new(0, 0, 1920, 1080),
        bounds: Dimension::new(
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
        work_area: PixelRect::new(1920, 0, 5120, 2880),
        bounds: Dimension::new(
            Length::new(1920.0),
            Length::new(0.0),
            Length::new(5120.0),
            Length::new(2880.0),
        ),
        is_primary: false,
        scale: 2.0,
    };
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![primary, secondary],
    );
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    // Move to the secondary monitor
    env.run_actions("move monitor right");
    env.settle(10);
    let border = Length::new(env.config.border_size.logical());
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
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![scaled_monitor(1.25)],
    );
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    env.move_window_to(w1, dim(200, 150, 600, 400));
    // Drive the next placement cycle
    env.dome.apply_layout();
    env.settle(10);

    // Under physical-native core, the observation (200,150,600,400) is stored
    // directly: core outsets it by the border on the way in and insets it back out.
    // Round-trip is identity: no conversion.
    let d = env.dim(w1);
    assert_eq!(d.x, Length::new(200.0));
    assert_eq!(d.y, Length::new(150.0));
    assert_eq!(d.width, Length::new(600.0));
    assert_eq!(d.height, Length::new(400.0));
}

#[test]
fn show_fullscreen_window_places_at_175pct() {
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![scaled_monitor(1.75)],
    );
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    let phys_w = SCREEN_WIDTH * 1.75;
    let phys_h = SCREEN_HEIGHT * 1.75;

    // Simulate the user resizing to fill the screen (triggers fullscreen detection).
    // The mock work area must match the physical monitor extent.
    env.move_window_to(
        w1,
        Dimension::new(Length::ZERO, Length::ZERO, phys_w, phys_h),
    );
    env.dome.apply_layout();

    let d = env.dim(w1);
    // Fullscreen covers the full physical monitor work area directly.
    assert_eq!(d.x, Length::ZERO);
    assert_eq!(d.y, Length::ZERO);
    assert_eq!(d.width, phys_w.round());
    assert_eq!(d.height, phys_h.round());
}

/// Proves that the physical round-trip converges at non-100% scales.
/// Under agnostic-core, no conversion occurs, so this is a pure identity check.
#[test]
fn float_round_trip_converges_at_125pct() {
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![scaled_monitor(1.25)],
    );
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);
    env.moves.lock().unwrap().clear();

    env.move_window_to(w1, dim(300, 200, 500, 400));
    env.dome.apply_layout();
    env.settle(10);

    let d1 = env.dim(w1);

    // Simulate the OS reporting back the position we just set (as window_drifted would)
    env.move_window_to(w1, d1);
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

/// 4.0 logical * 1.3 is 5.2 physical, so both crossings must round the border.
#[test]
fn float_settle_does_not_drift_at_fractional_scaled_border() {
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![scaled_monitor(1.3)],
    );
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);
    env.moves.lock().unwrap().clear();

    env.move_window_to(w1, dim(300, 200, 500, 400));
    env.dome.apply_layout();
    env.settle(10);

    let d1 = env.dim(w1);

    env.move_window_to(w1, d1);
    env.dome.apply_layout();
    env.settle(10);

    let d2 = env.dim(w1);

    assert_eq!(d1.x, d2.x, "x diverged");
    assert_eq!(d1.y, d2.y, "y diverged");
    assert_eq!(d1.width, d2.width, "width diverged");
    assert_eq!(d1.height, d2.height, "height diverged");

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
        work_area: PixelRect::new(0, 0, 1920, 1080),
        bounds: Dimension::new(
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
        work_area: PixelRect::new(1920, 0, 3840, 2160),
        bounds: Dimension::new(
            Length::new(1920.0),
            Length::new(0.0),
            Length::new(3840.0),
            Length::new(2160.0),
        ),
        is_primary: false,
        scale: 2.0,
    };
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![primary, secondary],
    );
    let win = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    let original_dim = env.dim(win);

    // Clear moves to establish baseline
    env.moves.lock().unwrap().clear();

    // Report an unknown monitor handle (999). The observation should be
    // dropped entirely -- no position change, no dimension change.
    env.dome.handle_window_moved(
        win,
        PixelRect::new(3000, 500, 600, 400),
        999,
        Instant::now(),
    );
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
        work_area: PixelRect::new(0, 0, 1920, 1080),
        bounds: Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(1920.0),
            Length::new(1080.0),
        ),
        is_primary: true,
        scale: 1.0,
    };
    let config = Config::default();
    let mut layout = GlobalLayoutConfig::default();
    layout.partition_tree.tab_bar_height = Length::<Logical>::new(30.0);
    let mut config = config;
    config.strategy = layout.strategy;
    config.partition_tree = layout.partition_tree;
    config.master = layout.master.clone();
    config.size_constraints = layout.size_constraints;
    config.float = layout.float;
    config.fullscreen = layout.fullscreen;
    let mut env = TestEnv::new_with_monitors(config, LayoutConfig::default(), vec![monitor]);
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    // Put into a tabbed container so tab_bar_height participates in layout
    env.run_actions("toggle layout");
    env.settle(10);

    let d_before = env.dim(w2);
    // At scale 1.0, tab bar is 30px: y == border + 30, height == 1080 - 2*border - 30
    let border = Length::new(env.config.border_size.logical());
    let tab_h_1x = Length::new(30.0);
    assert_eq!(d_before.y, (border + tab_h_1x).round());

    // Simulate DPI change to 192 (scale 2.0)
    let handle = 1_isize;
    env.dome.monitor_dpi_changed(handle, 192);
    env.dome.apply_layout();
    env.settle(10);

    let d_after = env.dim(w2);
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
fn float_move_monitor_same_dpi_preserves_content_rect() {
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![default_monitor(), second_monitor()],
    );
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    // Anchor the float at a known position on the primary monitor
    env.move_window_to(w1, dim(200, 150, 600, 400));
    env.settle(10);

    let float_overlays = env.float_overlays();
    let float_overlay = &float_overlays[0];
    let FloatOverlayState::Visible {
        visible_border_box: overlay_rect,
        ..
    } = float_overlay.state
    else {
        panic!("Float invisible");
    };

    let overlay_dim = overlay_rect.to_dimension();
    let border = Length::new(env.config.border_size.logical());
    assert_eq!(overlay_dim.x, Length::new(200.0) - border);
    assert_eq!(overlay_dim.y, Length::new(150.0) - border);
    assert_eq!(overlay_dim.width, Length::new(600.0) + 2.0 * border);
    assert_eq!(overlay_dim.height, Length::new(400.0) + 2.0 * border);

    env.moves.lock().unwrap().clear();
    env.move_window_to(w1, dim(2020, 100, 400, 300));
    env.settle(10);

    let float_overlays = env.float_overlays();
    let float_overlay = &float_overlays[0];
    let FloatOverlayState::Visible {
        visible_border_box: overlay_rect,
        ..
    } = float_overlay.state
    else {
        panic!("Float invisible");
    };

    let overlay_dim = overlay_rect.to_dimension();
    assert_eq!(overlay_dim.x, Length::new(2020.0) - border);
    assert_eq!(overlay_dim.y, Length::new(100.0) - border);
    assert_eq!(overlay_dim.width, Length::new(400.0) + 2.0 * border);
    assert_eq!(overlay_dim.height, Length::new(300.0) + 2.0 * border);
}

#[test]
fn float_move_monitor_different_dpi_rescales_border() {
    let primary = MonitorInfo {
        handle: 1,
        name: "Primary".to_string(),
        work_area: PixelRect::new(0, 0, 1920, 1080),
        bounds: Dimension::new(
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
        work_area: PixelRect::new(1920, 0, 5120, 2880),
        bounds: Dimension::new(
            Length::new(1920.0),
            Length::new(0.0),
            Length::new(5120.0),
            Length::new(2880.0),
        ),
        is_primary: false,
        scale: 2.0,
    };
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![primary, secondary],
    );
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    // Anchor the float at a known content rect on monitor 1
    env.move_window_to(w1, dim(100, 100, 400, 300));
    env.dome.apply_layout();
    env.settle(10);

    let border = Length::new(env.config.border_size.logical());
    env.moves.lock().unwrap().clear();
    env.move_window_to(w1, dim(2020, 100, 400, 300));

    env.settle(10);

    let float_overlays = env.float_overlays();
    let float_overlay = &float_overlays[0];
    let FloatOverlayState::Visible {
        visible_border_box: overlay_rect,
        ..
    } = float_overlay.state
    else {
        panic!("Float invisible");
    };

    let overlay_dim = overlay_rect.to_dimension();
    // On the target monitor at scale 2.0 core resolves the border to border * 2.0, so the content
    // rect is the outer box inset by that, which differs from the inset it had at scale 1.0.
    let scaled_border = border * 2.0;
    assert_eq!(overlay_dim.x, Length::new(2020.0) - scaled_border);
    assert_eq!(overlay_dim.y, Length::new(100.0) - scaled_border);
    assert_eq!(overlay_dim.width, Length::new(400.0) + 2.0 * scaled_border);
    assert_eq!(overlay_dim.height, Length::new(300.0) + 2.0 * scaled_border);
}

#[test]
fn dome_new_assigns_per_monitor_scale() {
    let primary = MonitorInfo {
        handle: 1,
        name: "Primary".to_string(),
        work_area: PixelRect::from_dimension(Dimension::new(
            Length::ZERO,
            Length::ZERO,
            SCREEN_WIDTH * 1.5,
            SCREEN_HEIGHT * 1.5,
        )),
        bounds: Dimension::new(
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
        work_area: PixelRect::from_dimension(Dimension::new(
            SCREEN_WIDTH * 1.5,
            Length::ZERO,
            Length::new(5120.0),
            Length::new(2880.0),
        )),
        bounds: Dimension::new(
            SCREEN_WIDTH * 1.5,
            Length::ZERO,
            Length::new(5120.0),
            Length::new(2880.0),
        ),
        is_primary: false,
        scale: 2.0,
    };
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![primary, secondary],
    );
    let border = Length::new(env.config.border_size.logical());

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

    env.move_window_to(
        w1,
        Dimension::new(
            Length::new(500.0),
            Length::new(300.0),
            Length::new(400.0),
            Length::new(250.0),
        ),
    );
    env.flush_moves();

    // The overlay paints the emitted visible border box, not the raw managed-window rect.
    let border = Length::new(env.config.border_size.logical());
    let expected_outer = Dimension::new(
        Length::new(500.0) - border,
        Length::new(300.0) - border,
        Length::new(400.0) + 2.0 * border,
        Length::new(250.0) + 2.0 * border,
    );
    let state = env
        .float_overlays()
        .iter()
        .find(|f| f.state.is_visible())
        .map(|f| f.state)
        .unwrap_or(FloatOverlayState::Hidden);
    let FloatOverlayState::Visible {
        visible_border_box, ..
    } = state
    else {
        panic!("float overlay must be visible after drag, got {:?}", state);
    };
    assert_eq!(
        visible_border_box.to_dimension(),
        expected_outer,
        "overlay should receive the emitted border box as visible_border_box"
    );
}

#[test]
fn float_dragged_past_the_screen_origin_paints_a_clipped_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    env.move_window_to(
        w1,
        Dimension::new(
            Length::ZERO,
            Length::ZERO,
            Length::new(400.0),
            Length::new(250.0),
        ),
    );
    env.flush_moves();

    // Core stores the border box at (-border, -border, 400 + 2 * border, 250 + 2 * border) and
    // clips it to the screen before emitting, so the overlay loses one border off each extent.
    let border = Length::new(env.config.border_size.logical());
    let expected_clipped = Dimension::new(
        Length::ZERO,
        Length::ZERO,
        Length::new(400.0) + border,
        Length::new(250.0) + border,
    );
    let state = env
        .float_overlays()
        .iter()
        .find(|f| f.state.is_visible())
        .map(|f| f.state)
        .unwrap_or(FloatOverlayState::Hidden);
    let FloatOverlayState::Visible {
        visible_border_box, ..
    } = state
    else {
        panic!("float overlay must be visible after drag, got {:?}", state);
    };
    assert_eq!(
        visible_border_box.to_dimension(),
        expected_clipped,
        "overlay surface must match core's clipped border box, not the unclipped one"
    );
}

#[test]
fn float_overlay_geometry_is_stable_across_repeated_apply_layout() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.settle(10);

    env.move_window_to(
        w1,
        Dimension::new(
            Length::new(500.0),
            Length::new(300.0),
            Length::new(400.0),
            Length::new(250.0),
        ),
    );
    env.flush_moves();

    // Snapshot the float overlay state after the first update (from window_drifted)
    let after_drift = env
        .float_overlays()
        .iter()
        .find(|f| f.state.is_visible())
        .map(|f| f.state)
        .unwrap_or(FloatOverlayState::Hidden);

    // A second apply_layout re-emits the same placement, so the overlay repaints identically
    env.dome.apply_layout();
    env.settle(10);

    let after_settle = env
        .float_overlays()
        .iter()
        .find(|f| f.state.is_visible())
        .map(|f| f.state)
        .unwrap_or(FloatOverlayState::Hidden);
    assert_eq!(
        after_settle, after_drift,
        "apply_layout after drift must re-emit the same overlay geometry"
    );
}

fn full_work_area(env: &TestEnv) -> Dimension {
    let border = Length::new(env.config.border_size.logical());
    Dimension::new(
        border,
        border,
        SCREEN_WIDTH - border * 2.0,
        SCREEN_HEIGHT - border * 2.0,
    )
}

#[test]
fn open_bar_shrinks_work_area() {
    let mut env = TestEnv::new();
    let win = env.open(1, "App", "app.exe", SPAWN_DIM);
    env.settle(10);
    let bar = Arc::new(
        MockExternalHwnd::with_title(
            2,
            "Zebar - vanilla",
            "zebar.exe",
            env.moves.clone(),
            env.z_stack.clone(),
            env.focus_target.clone(),
        )
        .with_class("Tauri Window")
        .with_app_name("Zebar")
        .with_dimension(dim(0, 0, 1920, 30)),
    );
    env.open_with(bar);
    env.settle(10);

    assert_eq!(env.dim(win), dim(4, 34, 1912, 1042));
}

#[test]
fn bar_move_updates_work_area() {
    let mut env = TestEnv::new();
    let win = env.open(1, "App", "app.exe", SPAWN_DIM);
    env.settle(10);
    let bar = Arc::new(
        MockExternalHwnd::with_title(
            2,
            "Zebar - vanilla",
            "zebar.exe",
            env.moves.clone(),
            env.z_stack.clone(),
            env.focus_target.clone(),
        )
        .with_class("Tauri Window")
        .with_app_name("Zebar")
        .with_dimension(dim(0, 0, 1920, 30)),
    );
    let bar_id = env.open_with(bar);
    env.settle(10);
    assert_eq!(env.dim(win), dim(4, 34, 1912, 1042));

    env.dome
        .bar_moved(bar_id, 1, PixelRect::new(0, 0, 1920, 60));
    env.dome.apply_layout();
    env.settle(10);

    assert_eq!(env.dim(win), dim(4, 64, 1912, 1012));
}

#[test]
fn destroy_bar_restores_work_area() {
    let mut env = TestEnv::new();
    let win = env.open(1, "App", "app.exe", SPAWN_DIM);
    env.settle(10);
    let bar = Arc::new(
        MockExternalHwnd::with_title(
            2,
            "Zebar - vanilla",
            "zebar.exe",
            env.moves.clone(),
            env.z_stack.clone(),
            env.focus_target.clone(),
        )
        .with_class("Tauri Window")
        .with_app_name("Zebar")
        .with_dimension(dim(0, 0, 1920, 30)),
    );
    let bar_id = env.open_with(bar);
    env.settle(10);
    assert_eq!(env.dim(win), dim(4, 34, 1912, 1042));

    env.destroy_window(bar_id);
    env.settle(10);

    assert_eq!(env.dim(win), full_work_area(&env));
}

#[test]
fn open_bar_adjust_multiple_monitors() {
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![default_monitor(), second_monitor()],
    );
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.settle(10);
    env.run_actions("focus monitor right");
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.settle(10);

    let bar = Arc::new(
        MockExternalHwnd::with_title(
            3,
            "Zebar - vanilla",
            "zebar.exe",
            env.moves.clone(),
            env.z_stack.clone(),
            env.focus_target.clone(),
        )
        .with_class("Tauri Window")
        .with_app_name("Zebar")
        .with_dimension(dim(0, 0, 1920, 30)),
    );
    env.open_with(bar);
    env.settle(10);

    assert_eq!(env.dim(w1), dim(4, 34, 1912, 1042));
    assert_eq!(env.dim(w2), dim(1924, 4, 2552, 1432));
}
