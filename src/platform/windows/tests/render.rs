use super::*;
use crate::platform::windows::dpi;

#[test]
fn create_tiling_overlay_records_primary_scale() {
    let mut screen = default_screen();
    screen.scale = 1.5;
    let env = TestEnv::new_with_screens(Config::default(), vec![screen]);
    // Dome::new creates one overlay for the primary monitor; the factory
    // should receive that monitor's scale, not a placeholder.
    assert_eq!(env.tiling_overlay_creation_scales(), vec![1.5]);
}

#[test]
fn create_tiling_overlay_records_monitor_scale() {
    let mut primary = default_screen();
    primary.scale = 1.0;
    let mut secondary = second_screen();
    secondary.scale = 2.0;
    let env = TestEnv::new_with_screens(Config::default(), vec![primary, secondary]);
    // Two overlays created: primary at 1.0, secondary at 2.0.
    let scales = env.tiling_overlay_creation_scales();
    assert_eq!(scales.len(), 2);
    assert_eq!(scales[0], 1.0);
    assert_eq!(scales[1], 2.0);
}

#[test]
fn tiling_overlay_update_passes_monitor_scale() {
    let mut screen = default_screen();
    screen.scale = 1.5;
    let mut env = TestEnv::new_with_screens(Config::default(), vec![screen]);
    let w = env.spawn_window(1, "win", "app.exe");
    env.add_window(w);
    assert_eq!(env.last_tiling_overlay_scale(), 1.5);
}

#[test]
fn float_overlay_update_passes_monitor_scale() {
    let mut screen = default_screen();
    screen.scale = 2.0;
    let mut env = TestEnv::new_with_screens(Config::default(), vec![screen]);
    let w = env.spawn_window(1, "win", "app.exe");
    env.add_window(w.clone());
    // Toggle to float to trigger float overlay update.
    env.run_actions("toggle float");
    assert_eq!(env.last_float_overlay_scale(), 2.0);
}

#[test]
fn picker_show_passes_monitor_scale() {
    let mut screen = default_screen();
    screen.scale = 1.75;
    let mut env = TestEnv::new_with_screens(Config::default(), vec![screen]);
    let w = env.spawn_window(1, "win", "app.exe");
    env.add_window(w.clone());
    env.minimize_window(&w);
    env.run_actions("toggle_minimize_picker");
    assert_eq!(env.last_picker_scale(), 1.75);
}

/// Conversion-helper backstop: the noop overlay cannot observe TilingOverlay's
/// physical_size() derivation, so we verify the arithmetic that the real
/// rerender() relies on to produce physical surface dimensions.
#[test]
fn tiling_overlay_rerender_uses_cached_physical_size() {
    let mut screen = default_screen();
    screen.scale = 2.0;
    screen.dimension = Dimension {
        x: 0.0,
        y: 0.0,
        width: 1440.0,
        height: 900.0,
    };
    let mut env = TestEnv::new_with_screens(Config::default(), vec![screen]);
    let w = env.spawn_window(1, "win", "app.exe");
    env.add_window(w);
    assert_eq!(env.last_tiling_overlay_scale(), 2.0);

    let dim = Dimension {
        x: 0.0,
        y: 0.0,
        width: 2880.0,
        height: 1800.0,
    };
    let (_x_phys, _y_phys, w_phys, h_phys) = dpi::surface_size_from_physical(dim);
    assert_eq!(w_phys, 2880);
    assert_eq!(h_phys, 1800);
}

#[test]
fn picker_show_clears_icon_cache_on_scale_change() {
    let mut screen_a = default_screen();
    screen_a.scale = 1.0;
    let mut screen_b = second_screen();
    screen_b.scale = 2.0;

    let mut env = TestEnv::new_with_screens(Config::default(), vec![screen_a, screen_b]);

    // Add a window on monitor A, then minimize it so the picker has entries.
    let w = env.spawn_window(1, "App", "app.exe");
    env.add_window(w.clone());
    env.minimize_window(&w);

    // Open picker on monitor A (scale 1.0).
    env.run_actions("toggle_minimize_picker");
    assert_eq!(env.last_picker_scale(), 1.0);

    // Drain pending icon loads and receive them.
    let to_load = env.picker_icons_to_load();
    assert!(
        !to_load.is_empty(),
        "expected icons to load after first show"
    );
    for (app_id, _hwnd) in &to_load {
        env.picker_receive_icon(app_id.clone());
    }

    // No more icons to load now.
    assert!(env.picker_icons_to_load().is_empty());
    assert!(!env.picker_loaded_icons().is_empty());

    // Move focus to monitor B and reopen picker at scale 2.0.
    env.run_actions("move monitor right");
    env.run_actions("toggle_minimize_picker"); // hide
    env.run_actions("toggle_minimize_picker"); // show on B
    assert_eq!(env.last_picker_scale(), 2.0);

    // The scale change should have cleared the icon cache, so icons_to_load
    // returns entries again.
    let reloaded = env.picker_icons_to_load();
    assert!(
        !reloaded.is_empty(),
        "expected icons to reload after scale change"
    );
}

#[test]
fn picker_scale_none_when_hidden() {
    let mut env = TestEnv::new();
    // No picker shown yet.
    assert_eq!(env.picker_scale(), None);

    let w = env.spawn_window(1, "App", "app.exe");
    env.add_window(w.clone());
    env.minimize_window(&w);

    // Show picker.
    env.run_actions("toggle_minimize_picker");
    assert!(env.picker_scale().is_some());

    // Hide picker.
    env.run_actions("toggle_minimize_picker");
    assert_eq!(env.picker_scale(), None);
}

#[test]
fn picker_show_same_scale_preserves_icon_cache() {
    let mut env = TestEnv::new();
    let w = env.spawn_window(1, "App", "app.exe");
    env.add_window(w.clone());
    env.minimize_window(&w);

    // Open picker at scale 1.0.
    env.run_actions("toggle_minimize_picker");

    // Load icons.
    let to_load = env.picker_icons_to_load();
    assert!(!to_load.is_empty());
    for (app_id, _hwnd) in &to_load {
        env.picker_receive_icon(app_id.clone());
    }
    let icons_before = env.picker_loaded_icons();
    assert!(!icons_before.is_empty());

    // Close and reopen at the same scale (1.0).
    env.run_actions("toggle_minimize_picker"); // hide
    env.run_actions("toggle_minimize_picker"); // show at 1.0 again
    assert_eq!(env.last_picker_scale(), 1.0);

    // Cache should be preserved: no new icons to load.
    assert!(env.picker_icons_to_load().is_empty());
    assert_eq!(env.picker_loaded_icons(), icons_before);
}
