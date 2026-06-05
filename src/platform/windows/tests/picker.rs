use super::*;

#[test]
fn picker_scale_reflects_focused_monitor() {
    let mut monitor = default_monitor();
    monitor.scale = 1.75;
    let mut env = TestEnv::new_with_monitors(Config::default(), vec![monitor]);
    let w = env.open(1, "App", "app.exe", SPAWN_DIM);
    env.minimize_window(w);
    env.run_actions("toggle minimized");
    assert_eq!(env.picker_scale(), Some(1.75));
}

#[test]
fn picker_scale_none_when_hidden() {
    let mut env = TestEnv::new();
    // No picker shown yet.
    assert_eq!(env.picker_scale(), None);

    let w = env.open(1, "App", "app.exe", SPAWN_DIM);
    env.minimize_window(w);

    // Show picker.
    env.run_actions("toggle minimized");
    assert!(env.picker_scale().is_some());

    // Hide picker.
    env.run_actions("toggle minimized");
    assert_eq!(env.picker_scale(), None);
}

#[test]
fn picker_show_same_scale_preserves_icon_cache() {
    let mut env = TestEnv::new();
    let w = env.open(1, "App", "app.exe", SPAWN_DIM);
    env.minimize_window(w);

    // Open picker at scale 1.0.
    env.run_actions("toggle minimized");

    // Load icons.
    let to_load = env.picker_icons_to_load();
    assert!(!to_load.is_empty());
    for (app_id, _hwnd) in &to_load {
        env.picker_receive_icon(app_id.clone());
    }
    let icons_before = env.picker_loaded_icons();
    assert!(!icons_before.is_empty());

    // Close and reopen at the same scale (1.0).
    env.run_actions("toggle minimized"); // hide
    env.run_actions("toggle minimized"); // show at 1.0 again

    // Cache should be preserved: no new icons to load.
    assert!(env.picker_icons_to_load().is_empty());
    assert_eq!(env.picker_loaded_icons(), icons_before);
}
