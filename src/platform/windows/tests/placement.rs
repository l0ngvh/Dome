use super::*;
use crate::core::{Length, Pixels};

#[test]
fn single_window_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    env.assert_horizontally_tiled(&[env.dim(w1)]);
}

#[test]
fn two_windows_split_screen() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();
    env.assert_horizontally_tiled(&[env.dim(w1), env.dim(w2)]);
}

#[test]
fn three_windows_split_screen() {
    let mut env = TestEnv::builder()
        .tiling(|tiling| tiling.partition_tree.automatic_tiling = false)
        .build();
    let w1 = env.open();
    let w2 = env.open();
    let w3 = env.open();
    env.assert_horizontally_tiled(&[env.dim(w1), env.dim(w2), env.dim(w3)]);
}

#[test]
fn reported_min_width_binds_while_zero_min_height_is_cleared() {
    let mut env = TestEnv::new();
    let w1 = env.window().min_size(1200.0, 0.0).open();
    env.open();

    // An even split would leave each window near 952, so the minimum binds. The
    // shell forwards it untouched and core outsets it by the border, so the app
    // gets back exactly the content width it asked for.
    assert_eq!(env.dim(w1).width, Length::new(1200.0));
    // The zero height component reads as Cleared, not a zero-height minimum.
    assert_eq!(env.dim(w1).height, SCREEN_HEIGHT - env.border() * 2.0);
}

#[test]
fn dropping_all_limits_restores_the_even_split() {
    let mut env = TestEnv::new();
    let w1 = env.window().min_size(1200.0, 0.0).open();
    let w2 = env.open();
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
    env.layout();

    assert_eq!(env.dim(w1).width, Length::new(952.0));
    assert_eq!(env.dim(w2).width, Length::new(952.0));
    env.assert_horizontally_tiled(&[env.dim(w1), env.dim(w2)]);
}

#[test]
fn workspace_switch_hides_and_restores() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

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
fn resize_detects_fullscreen() {
    let mut env = TestEnv::new();
    let w1 = env.open();

    let d = env.dim(w1);
    assert_eq!(d.x, env.border(), "should start tiled with border inset");

    env.move_window_to(
        w1,
        Dimension::new(Length::ZERO, Length::ZERO, SCREEN_WIDTH, SCREEN_HEIGHT),
    );
    env.flush_moves();
    let d = env.dim(w1);
    assert_eq!(d.x, Length::ZERO);
    assert_eq!(d.y, Length::ZERO);
    assert_eq!(d.width, SCREEN_WIDTH);
    assert_eq!(d.height, SCREEN_HEIGHT);
}

#[test]
fn dont_correct_float_move() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    env.run_actions("toggle float");
    env.settle(10);

    env.clear_moves();

    env.move_window_to(w1, dim(200, 150, 600, 400));

    env.flush_moves();

    env.assert_settled("float observation should not trigger set_position");

    // Idempotence: fp.target == new_target short-circuits show_float, so no
    // set_position calls are issued across two successive apply_layout rounds.
    env.layout();
    env.settle(10);
    env.layout();
    env.settle(10);
    env.assert_settled("two successive apply_layout rounds after float move should be no-ops");
}

/// distribute_space uses binary search and may produce fractional widths
/// (e.g. 1920/3 ≈ 639.999). The f32→i32 conversion in show_tiling must
/// round, not truncate, or the cumulative error pushes the last window's
/// right edge away from the screen edge.
#[test]
fn positions_are_rounded_not_truncated() {
    let mut env = TestEnv::builder()
        .tiling(|tiling| tiling.partition_tree.automatic_tiling = false)
        .build();
    let wins = env.open_many(7);
    let dims: Vec<_> = wins.iter().map(|w| env.dim(*w)).collect();
    env.assert_horizontally_tiled(&dims);
}

#[test]
fn tiling_border_scales_with_dpi() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let mut env = TestEnv::builder()
            .monitors(vec![scaled_monitor(scale)])
            .build();
        let w1 = env.open();
        let placement = env.only_painted_window();

        assert_eq!(
            env.dim(w1),
            placement.content_box.to_dimension(),
            "scale {scale}"
        );
        assert_content_box_centered_in_border_box(&placement);
        assert_eq!(
            placement.content_box.x() - placement.border_box.x(),
            env.border_at(scale),
            "scale {scale}"
        );
    }
}

#[test]
fn painted_thickness_matches_core_inset() {
    let mut env = TestEnv::builder()
        .monitors(vec![scaled_monitor(1.25)])
        .build();
    env.open();

    let placement = env.only_painted_window();
    let border_thickness = env.painted_border_thickness(0);

    // The painter strokes a band of exactly this thickness inside border_box, so any
    // disagreement with the inset core already applied shows up as a hairline.
    assert_eq!(
        border_thickness,
        placement.content_box.x() - placement.border_box.x()
    );
    assert_eq!(
        border_thickness * 2,
        placement.border_box.width() - placement.content_box.width()
    );
}

#[test]
fn degenerate_content_box_hides_window() {
    // 600 physical per edge against a 1080-tall monitor leaves no content height,
    // so core hands the shell an empty content box.
    let mut env = TestEnv::builder()
        .tiling(|tiling| tiling.border_size = Pixels::new(600))
        .monitors(vec![scaled_monitor(1.0)])
        .build();
    let w1 = env.open();

    assert!(env.is_offscreen(w1));

    env.change_config(|config| config.tiling.border_size = baseline_config().tiling.border_size);

    let placement = env.only_painted_window();
    assert!(!env.is_offscreen(w1));
    assert_eq!(env.dim(w1), placement.content_box.to_dimension());
    assert_content_box_centered_in_border_box(&placement);
}

#[test]
fn show_tiling_places_at_200pct_offset_monitor() {
    let primary = MonitorInfo {
        handle: 1,
        name: "Primary".to_string(),
        gdi_device: "\\\\.\\DISPLAY1".to_string(),
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
        gdi_device: "\\\\.\\DISPLAY2".to_string(),
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
    let mut env = TestEnv::builder()
        .monitors(vec![primary, secondary.clone()])
        .build();
    let w1 = env.open();
    env.run_actions("move monitor right");
    env.settle(10);

    assert_eq!(
        env.dim(w1),
        env.inset_at(secondary.work_area.to_dimension(), secondary.scale)
    );
}

#[test]
fn show_float_places_at_125pct() {
    let mut env = TestEnv::builder()
        .monitors(vec![scaled_monitor(1.25)])
        .build();
    let w1 = env.open();
    env.run_actions("toggle float");
    env.settle(10);

    env.move_window_to(w1, dim(200, 150, 600, 400));
    env.layout();
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
    let mut env = TestEnv::builder()
        .monitors(vec![scaled_monitor(1.75)])
        .build();
    let w1 = env.open();

    let phys_w = SCREEN_WIDTH * 1.75;
    let phys_h = SCREEN_HEIGHT * 1.75;

    // Toggle rather than simulate a resize: Dome leaves an already fullscreen-shaped
    // window alone, so that path emits no write to observe.
    env.clear_moves();
    env.run_actions("toggle fullscreen");

    // Guards against the earlier tautology, where the assertions below read back the
    // rect the test itself wrote.
    assert!(
        env.moved(w1),
        "apply_layout must emit a placement for the fullscreen window"
    );

    let d = env.dim(w1);
    assert_eq!(d.x, Length::ZERO);
    assert_eq!(d.y, Length::ZERO);
    assert_eq!(d.width, phys_w);
    assert_eq!(d.height, phys_h);
}

/// Under agnostic-core, no conversion occurs, so this is a pure identity check.
#[test]
fn float_round_trip_converges_at_125pct() {
    let mut env = TestEnv::builder()
        .monitors(vec![scaled_monitor(1.25)])
        .build();
    let w1 = env.open();
    env.run_actions("toggle float");
    env.settle(10);
    env.clear_moves();

    env.move_window_to(w1, dim(300, 200, 500, 400));
    env.layout();
    env.settle(10);

    let d1 = env.dim(w1);

    // Simulate the OS reporting back the position we just set (as window_drifted would)
    env.move_window_to(w1, d1);
    env.layout();
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

/// The default border size at scale 1.3 is fractional, so both crossings must round the border.
#[test]
fn float_settle_does_not_drift_at_fractional_scaled_border() {
    let mut env = TestEnv::builder()
        .monitors(vec![scaled_monitor(1.3)])
        .build();
    let w1 = env.open();
    env.run_actions("toggle float");
    env.settle(10);
    env.clear_moves();

    env.move_window_to(w1, dim(300, 200, 500, 400));
    env.layout();
    env.settle(10);

    let d1 = env.dim(w1);

    env.move_window_to(w1, d1);
    env.layout();
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
        gdi_device: "\\\\.\\DISPLAY1".to_string(),
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
        gdi_device: "\\\\.\\DISPLAY2".to_string(),
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
    let mut env = TestEnv::builder()
        .monitors(vec![primary, secondary])
        .build();
    let win = env.open();
    env.run_actions("toggle float");
    env.settle(10);

    let original_dim = env.dim(win);

    env.clear_moves();

    env.dome.handle_window_moved(
        win,
        PixelRect::new(3000, 500, 600, 400),
        999,
        Instant::now(),
    );
    env.settle(10);

    env.assert_settled("unknown monitor handle should not trigger set_position");
    assert_eq!(
        env.dim(win),
        original_dim,
        "unknown monitor handle should not change window dimension"
    );
}

#[test]
fn dpi_reconcile_reruns_layout_with_new_scale() {
    let monitor = MonitorInfo {
        handle: 1,
        name: "Test".to_string(),
        gdi_device: "\\\\.\\DISPLAY1".to_string(),
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
    let mut env = TestEnv::builder()
        .tiling(|tiling| tiling.partition_tree.tab_bar_height = Pixels::new(30))
        .monitors(vec![monitor.clone()])
        .build();
    let _w1 = env.open();
    let w2 = env.open();
    env.run_actions("toggle layout");
    env.settle(10);

    let d_before = env.dim(w2);
    let border = env.border();
    let tab_h_1x = Length::new(30.0);
    assert_eq!(d_before.y, (border + tab_h_1x));

    env.set_monitor_scale(monitor.handle, 2.0);
    env.dome.handle_dpi_change();
    env.layout();
    env.settle(10);

    let d_after = env.dim(w2);
    let scaled_border = Length::from_pixels(env.border_at(2.0));
    let tab_h_2x = Length::new(30.0 * 2.0);
    assert_eq!(d_after.y, (scaled_border + tab_h_2x));
    assert_eq!(
        d_after.height,
        (Length::new(1080.0) - 2.0 * scaled_border - tab_h_2x)
    );
}

#[test]
fn handle_window_moved_signals_monitor_change() {
    let mut env = TestEnv::builder()
        .monitors(vec![default_monitor(), second_monitor()])
        .build();
    let w = env.open();
    env.run_actions("toggle float");
    env.move_window_to(w, dim(200, 150, 600, 400));
    env.settle(10);

    // Settled on monitor 1: a report on monitor 1 is no change.
    assert!(
        !env.dome
            .handle_window_moved(w, PixelRect::new(200, 150, 600, 400), 1, Instant::now()),
        "same monitor must not signal a constraint re-read"
    );
    // Crossing to monitor 2 signals a re-read.
    assert!(
        env.dome
            .handle_window_moved(w, PixelRect::new(2000, 100, 600, 400), 2, Instant::now()),
        "monitor change must signal a constraint re-read"
    );
    // Staying on monitor 2 is no change.
    assert!(
        !env.dome
            .handle_window_moved(w, PixelRect::new(2200, 200, 600, 400), 2, Instant::now()),
        "same monitor must not signal a constraint re-read"
    );
    // Unknown window: no entry to compare, no re-read.
    assert!(
        !env.dome.handle_window_moved(
            HwndId::test(0x9999),
            PixelRect::new(200, 150, 600, 400),
            1,
            Instant::now(),
        ),
        "unknown window must not signal a constraint re-read"
    );
}

#[test]
fn float_move_monitor_same_dpi_preserves_content_rect() {
    let mut env = TestEnv::builder()
        .monitors(vec![default_monitor(), second_monitor()])
        .build();
    let w1 = env.open();
    env.run_actions("toggle float");
    env.settle(10);

    env.move_window_to(w1, dim(200, 150, 600, 400));
    env.settle(10);

    let overlay_rect = env
        .painted_float()
        .expect("Float invisible")
        .visible_border_box;

    assert_eq!(
        overlay_rect.to_dimension(),
        env.outset(dim(200, 150, 600, 400))
    );

    env.clear_moves();
    env.move_window_to(w1, dim(2020, 100, 400, 300));
    env.settle(10);

    let overlay_rect = env
        .painted_float()
        .expect("Float invisible")
        .visible_border_box;

    assert_eq!(
        overlay_rect.to_dimension(),
        env.outset(dim(2020, 100, 400, 300))
    );
}

#[test]
fn float_move_monitor_different_dpi_rescales_border() {
    let primary = MonitorInfo {
        handle: 1,
        name: "Primary".to_string(),
        gdi_device: "\\\\.\\DISPLAY1".to_string(),
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
        gdi_device: "\\\\.\\DISPLAY2".to_string(),
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
    let mut env = TestEnv::builder()
        .monitors(vec![primary, secondary])
        .build();
    let w1 = env.open();
    env.run_actions("toggle float");
    env.settle(10);

    env.move_window_to(w1, dim(100, 100, 400, 300));
    env.layout();
    env.settle(10);

    env.clear_moves();
    env.move_window_to(w1, dim(2020, 100, 400, 300));

    env.settle(10);

    let overlay_rect = env
        .painted_float()
        .expect("Float invisible")
        .visible_border_box;

    assert_eq!(
        overlay_rect.to_dimension(),
        env.outset_at(dim(2020, 100, 400, 300), 2.0)
    );
}

#[test]
fn dome_new_assigns_per_monitor_scale() {
    let primary = MonitorInfo {
        handle: 1,
        name: "Primary".to_string(),
        gdi_device: "\\\\.\\DISPLAY1".to_string(),
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
        gdi_device: "\\\\.\\DISPLAY2".to_string(),
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
    let mut env = TestEnv::builder()
        .monitors(vec![primary, secondary])
        .build();
    let border = env.border();

    let w_a = env.open();
    let d_a = env.dim(w_a);
    assert_eq!(d_a.x, border * 1.5);

    // Its origin carries the primary's scaled width, so this pins both monitors' scales at once.
    let w_b = env.open();
    env.run_actions("move monitor right");
    env.settle(10);
    let d_b = env.dim(w_b);
    assert_eq!(d_b.x, SCREEN_WIDTH * 1.5 + border * 2.0);
}

#[test]
fn float_drift_repositions_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open();
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
    let visible_border_box = env
        .painted_float()
        .expect("float overlay must be visible after drag")
        .visible_border_box;
    assert_eq!(
        visible_border_box.to_dimension(),
        env.outset(dim(500, 300, 400, 250)),
        "overlay should receive the emitted border box as visible_border_box"
    );
}

#[test]
fn float_dragged_past_the_screen_origin_paints_a_clipped_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open();
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
    let visible_border_box = env
        .painted_float()
        .expect("float overlay must be visible after drag")
        .visible_border_box;
    assert_eq!(
        visible_border_box.to_dimension(),
        env.clipped_outset(dim(0, 0, 400, 250)),
        "overlay surface must match core's clipped border box, not the unclipped one"
    );
}

#[test]
fn float_overlay_geometry_is_stable_across_repeated_apply_layout() {
    let mut env = TestEnv::new();
    let w1 = env.open();
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

    let after_drift = env
        .painted_float()
        .expect("float overlay must be visible after drag")
        .visible_border_box;

    env.layout();
    env.settle(10);

    let after_settle = env
        .painted_float()
        .expect("float overlay must stay visible")
        .visible_border_box;
    assert_eq!(
        after_settle, after_drift,
        "apply_layout after drift must re-emit the same overlay geometry"
    );
}

#[test]
fn open_bar_shrinks_work_area() {
    let mut env = TestEnv::new();
    let win = env.open();
    env.settle(10);
    env.open_bar();
    env.settle(10);

    assert_eq!(env.dim(win), dim(4, 34, 1912, 1042));
}

#[test]
fn bar_move_updates_work_area() {
    let mut env = TestEnv::new();
    let win = env.open();
    env.settle(10);
    let bar_id = env.open_bar();
    env.settle(10);
    assert_eq!(env.dim(win), dim(4, 34, 1912, 1042));

    env.dome
        .bar_moved(bar_id, 1, PixelRect::new(0, 0, 1920, 60));
    env.settle(10);

    assert_eq!(env.dim(win), dim(4, 64, 1912, 1012));
}

#[test]
fn destroy_bar_restores_work_area() {
    let mut env = TestEnv::new();
    let win = env.open();
    env.settle(10);
    let bar_id = env.open_bar();
    env.settle(10);
    assert_eq!(env.dim(win), dim(4, 34, 1912, 1042));

    env.destroy_window(bar_id);
    env.settle(10);

    assert_eq!(env.dim(win), env.full_work_area());
}

#[test]
fn open_bar_adjust_multiple_monitors() {
    let mut env = TestEnv::builder()
        .monitors(vec![default_monitor(), second_monitor()])
        .build();
    let w1 = env.open();
    env.settle(10);
    env.run_actions("focus monitor right");
    let w2 = env.open();
    env.settle(10);

    env.open_bar();
    env.settle(10);

    assert_eq!(env.dim(w1), dim(4, 34, 1912, 1042));
    assert_eq!(env.dim(w2), dim(1924, 4, 2552, 1432));
}
