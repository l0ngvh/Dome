use super::*;
use crate::core::{Length, Logical, Pixels, ScrollingConfig, SizeConstraint, Strategy};
use crate::platform::windows::handle::{ROUND_RADIUS, SMALL_RADIUS};

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
fn reported_min_width_does_not_bind_the_even_split() {
    let mut env = TestEnv::new();
    let w1 = env.window().min_size(1200.0, 0.0).open();
    let w2 = env.open();

    assert_eq!(
        env.dim(w1).width,
        Length::new(952.0),
        "the reported minimum does not bind; w1 keeps its even half"
    );
    assert_eq!(env.dim(w2).width, Length::new(952.0));
    env.assert_horizontally_tiled(&[env.dim(w1), env.dim(w2)]);
}

#[test]
fn dropping_all_limits_keeps_the_even_split() {
    let mut env = TestEnv::new();
    let w1 = env.window().min_size(1200.0, 0.0).open();
    let w2 = env.open();
    assert_eq!(env.dim(w1).width, Length::new(952.0));

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

    assert_eq!(
        env.dim(w1).width,
        Length::new(952.0),
        "the Cleared merge runs without panicking and leaves the even split"
    );
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

    env.change_config("return {}");

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
        is_primary: true,
        scale: 1.0,
    };
    // Physical dimensions at 2.0x: 2560*2=5120, 1440*2=2880, origin 1920
    let secondary = MonitorInfo {
        handle: 2,
        name: "Secondary".to_string(),
        gdi_device: "\\\\.\\DISPLAY2".to_string(),
        work_area: PixelRect::new(1920, 0, 5120, 2880),
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
        is_primary: true,
        scale: 1.0,
    };
    let secondary = MonitorInfo {
        handle: 2,
        name: "Secondary".to_string(),
        gdi_device: "\\\\.\\DISPLAY2".to_string(),
        work_area: PixelRect::new(1920, 0, 3840, 2160),
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
        is_primary: true,
        scale: 1.0,
    };
    let secondary = MonitorInfo {
        handle: 2,
        name: "Secondary".to_string(),
        gdi_device: "\\\\.\\DISPLAY2".to_string(),
        work_area: PixelRect::new(1920, 0, 5120, 2880),
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

fn reserved_area_config(body: &str) -> String {
    format!("return {{ reserved_area = function(monitor) {body} end }}")
}

#[test]
fn a_reserved_area_applies_from_startup() {
    let mut env = TestEnv::builder()
        .config_source(&reserved_area_config("return { top = 30 }"))
        .build();
    let win = env.open();

    assert_eq!(env.dim(win), dim(4, 34, 1912, 1042));
    assert_eq!(env.painted_work_area(0), PixelRect::new(0, 30, 1920, 1050));
}

#[test]
fn a_reserved_area_scales_with_the_monitor() {
    let mut env = TestEnv::builder()
        .monitors(vec![scaled_monitor(1.5)])
        .config_source(&reserved_area_config("return { top = 20 }"))
        .build();
    let win = env.open();

    assert_eq!(env.dim(win), env.inset_at(dim(0, 30, 2880, 1590), 1.5));
}

#[test]
fn a_fullscreen_window_starts_below_the_reserved_area() {
    let mut env = TestEnv::builder()
        .config_source(&reserved_area_config("return { top = 30 }"))
        .build();
    let win = env.open();

    env.clear_moves();
    env.run_actions("toggle fullscreen");

    assert!(
        env.moved(win),
        "apply_layout must emit a placement for the fullscreen window"
    );
    assert_eq!(env.dim(win), dim(0, 30, 1920, 1050));
}

#[test]
fn a_reserved_area_matches_the_numbered_name() {
    let left = MonitorInfo {
        name: "DELL".to_string(),
        ..default_monitor()
    };
    let right = MonitorInfo {
        name: "DELL".to_string(),
        ..second_monitor()
    };
    let mut env = TestEnv::builder()
        .monitors(vec![left, right])
        .config_source(&reserved_area_config(
            "if monitor.name == 'DELL #2' then return { top = 30 } end",
        ))
        .build();
    let w1 = env.open();
    env.settle(10);
    env.run_actions("focus monitor right");
    let w2 = env.open();
    env.settle(10);

    assert_eq!(env.dim(w1), dim(4, 4, 1912, 1072));
    assert_eq!(env.dim(w2), dim(1924, 34, 2552, 1402));
}

#[test]
fn a_reserved_area_applies_to_an_added_monitor() {
    let mut env = TestEnv::builder()
        .config_source(&reserved_area_config(
            "if monitor.name == 'External' then return { top = 40 } end",
        ))
        .build();
    env.add_monitor(second_monitor());
    env.run_actions("focus monitor right");
    let win = env.open();
    env.settle(10);

    assert_eq!(env.dim(win), dim(1924, 44, 2552, 1392));
}

#[test]
fn a_reserved_area_reduces_only_its_own_monitor() {
    let mut env = TestEnv::builder()
        .monitors(vec![default_monitor(), second_monitor()])
        .config_source(&reserved_area_config(
            "if monitor.name == 'Test' then return { top = 30 } end",
        ))
        .build();
    let w1 = env.open();
    env.settle(10);
    env.run_actions("focus monitor right");
    let w2 = env.open();
    env.settle(10);

    assert_eq!(env.dim(w1), dim(4, 34, 1912, 1042));
    assert_eq!(env.dim(w2), dim(1924, 4, 2552, 1432));
}

#[test]
fn a_reload_reapplies_the_reserved_area() {
    let mut env = TestEnv::builder().build();
    let win = env.open();
    assert_eq!(env.dim(win), dim(4, 4, 1912, 1072));

    env.change_config(&reserved_area_config("return { top = 30 }"));
    assert_eq!(env.dim(win), dim(4, 34, 1912, 1042));
    assert_eq!(env.painted_work_area(0), PixelRect::new(0, 30, 1920, 1050));

    env.change_config(&reserved_area_config(
        "return { top = 0, bottom = 0, left = 0, right = 0 }",
    ));
    assert_eq!(env.dim(win), dim(4, 4, 1912, 1072));
    assert_eq!(env.painted_work_area(0), PixelRect::new(0, 0, 1920, 1080));
}

#[test]
fn tiling_border_takes_the_latest_corner_radius() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    assert_eq!(env.painted_corner_radii(0), vec![ROUND_RADIUS]);

    env.set_corner_radius(w1, Length::ZERO);
    env.layout();
    assert_eq!(env.painted_corner_radii(0), vec![Length::<Logical>::ZERO]);
}

#[test]
fn float_border_takes_the_window_corner_radius() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    env.set_corner_radius(w1, SMALL_RADIUS);
    env.run_actions("toggle float");
    env.settle(10);
    assert_eq!(env.painted_float_corner_radius(), Some(SMALL_RADIUS));
}

/// Where a tile with a content box of `width` by `height` parks while a thumbnail shows it.
fn parked(width: i32, height: i32) -> Dimension {
    Dimension::new(
        OFFSCREEN_POS,
        OFFSCREEN_POS,
        Length::new(width as f32),
        Length::new(height as f32),
    )
}

#[test]
fn a_cut_unfocused_scrolling_column_parks_at_full_size_behind_a_thumbnail() {
    let (mut env, [_, w2, w3, w4]) = four_scrolling_columns();

    assert_eq!(
        env.painted_thumbnails(0),
        vec![PaintedThumbnail {
            source: w2,
            frame: PixelRect::new(0, 0, 384, 1080),
            source_rect: PixelRect::new(384, 0, 384, 1080),
        }],
        "the focused w4 sits against the right edge, which leaves the right half of w2 on screen"
    );
    assert_eq!(env.dim(w2), parked(768, 1080));
    assert_eq!(env.dim(w3), dim(384, 0, 768, 1080));
    assert_eq!(env.dim(w4), dim(1152, 0, 768, 1080));

    env.click_thumbnail(w2);
    env.settle(10);

    assert_eq!(env.dim(w2), dim(0, 0, 768, 1080));
    assert_eq!(env.focus_target(), FocusTarget::Window(w2));
    assert_eq!(
        env.painted_thumbnails(0),
        vec![PaintedThumbnail {
            source: w4,
            frame: PixelRect::new(1536, 0, 384, 1080),
            source_rect: PixelRect::new(0, 0, 384, 1080),
        }]
    );
    assert_eq!(env.dim(w4), parked(768, 1080));
}

#[test]
fn a_focused_scrolling_column_wider_than_the_work_area_is_not_parked() {
    let mut env = TestEnv::builder()
        .tiling(|tiling| {
            tiling.border_size = Pixels::ZERO;
            tiling.layout = Strategy::Scrolling;
        })
        .build();
    env.open();
    let wide = env.window().min_size(2400.0, 0.0).open();
    env.settle(10);

    assert!(env.painted_thumbnails(0).is_empty());
    assert_eq!(
        env.dim(wide),
        dim(960, 0, 2400, 1080),
        "the focused wide window shows unclipped"
    );

    env.run_actions("focus left");
    env.settle(10);

    assert_eq!(
        env.painted_thumbnails(0),
        vec![PaintedThumbnail {
            source: wide,
            frame: PixelRect::new(960, 0, 960, 1080),
            source_rect: PixelRect::new(0, 0, 960, 1080),
        }]
    );
    assert_eq!(env.dim(wide), parked(2400, 1080));
}

#[test]
fn a_tile_cut_by_a_scrolled_master_pane_keeps_its_corner_radius_behind_a_thumbnail() {
    let mut env = TestEnv::builder()
        .tiling(|tiling| tiling.layout = Strategy::Master)
        .build();
    env.open();
    let upper = env.window().min_size(0.0, 800.0).open();
    env.set_corner_radius(upper, Length::new(26.0));
    let lower = env.window().min_size(0.0, 800.0).open();
    env.settle(10);

    assert_eq!(
        env.painted_thumbnails(0),
        vec![PaintedThumbnail {
            source: upper,
            frame: PixelRect::new(964, 0, 952, 268),
            source_rect: PixelRect::new(0, 532, 952, 268),
        }],
        "the pane scrolled the focused lower into view, which leaves the bottom of upper on screen"
    );
    assert_eq!(env.dim(upper), parked(952, 800));
    assert_eq!(env.dim(lower), dim(964, 276, 952, 800));
    assert_eq!(env.painted_corner_radius(0, upper), Some(Length::new(26.0)));
}

#[test]
fn a_parked_tile_parks_again_at_its_new_size() {
    let (mut env, [_, w2, _, _]) = four_scrolling_columns();

    env.change_monitors(vec![MonitorInfo {
        work_area: PixelRect::new(0, 0, 1920, 1000),
        ..default_monitor()
    }]);
    env.settle(10);

    assert_eq!(
        env.painted_thumbnails(0),
        vec![PaintedThumbnail {
            source: w2,
            frame: PixelRect::new(0, 0, 384, 1000),
            source_rect: PixelRect::new(384, 0, 384, 1000),
        }]
    );
    assert_eq!(env.dim(w2), parked(768, 1000));
}

#[test]
fn a_tile_without_an_on_screen_part_parks_without_a_thumbnail() {
    let mut env = TestEnv::builder()
        .tiling(|tiling| {
            tiling.border_size = Pixels::new(50);
            tiling.layout = Strategy::Scrolling;
            tiling.scrolling = ScrollingConfig {
                column_width: SizeConstraint::Pixels(Pixels::new(950)),
            };
        })
        .build();
    let windows = env.open_many(3);
    env.settle(10);
    let first = windows[0];
    let first_id = env.dome.window_id_for(first).unwrap();

    let first_tile = env
        .painted_windows(0)
        .into_iter()
        .find(|wp| wp.id == first_id)
        .expect("the first column's border reaches the work area, so it keeps a tile");
    assert_eq!(
        first_tile.visible_border_box,
        PixelRect::new(0, 0, 20, 1080),
        "precondition: only a sliver of the first column's border is on screen"
    );
    assert!(env.painted_thumbnails(0).is_empty());
    assert_eq!(env.dim(first), parked(850, 980));
}

#[test]
fn a_click_on_the_thumbnail_of_a_minimized_window_restores_it() {
    let (mut env, [_, w2, _, _]) = four_scrolling_columns();
    env.minimize_window(w2);

    env.click_thumbnail(w2);

    assert!(
        !env.is_minimized(w2),
        "a click that trails the minimize restores the window"
    );
}
