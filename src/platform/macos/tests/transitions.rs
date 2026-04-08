use super::*;

#[test]
fn fullscreen_window_restored_from_offscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Toggle fullscreen on cg2 (focused) — it covers the screen, cg1 hidden
    dome.run_hub_actions(&actions("toggle fullscreen"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg2), (0, 0, 1920, 1080));

    // Switch workspace — fullscreen window goes offscreen
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg2));

    // Switch back — fullscreen window should be placed from offscreen to full screen
    dome.run_hub_actions(&actions("focus workspace 0"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg2), (0, 0, 1920, 1080));
}

#[test]
fn borderless_fullscreen_hidden_on_workspace_switch() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Zoom cg1 to borderless fullscreen
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    // Switch workspace — borderless FS window should be minimized, not moved offscreen
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.minimize_count(cg1), 1);
    assert!(!macos.is_offscreen(cg1));

    // Switch back — should unminimize and remain borderless fullscreen
    dome.run_hub_actions(&actions("focus workspace 0"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.unminimize_count(cg1), 1);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn minimized_window_reappears_non_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Zoom cg1 → borderless fullscreen → workspace switch → minimized
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.minimize_count(cg1), 1);

    // Window reappears at non-fullscreen size (user un-zoomed while minimized)
    macos.move_window(cg1, 100, 100, 800, 600);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Should unminimize and move offscreen (no longer fullscreen)
    assert_eq!(macos.unminimize_count(cg1), 1);
    assert!(macos.is_offscreen(cg1));
}

#[test]
fn native_fullscreen_exit_to_borderless() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    // Enter native fullscreen
    macos.enter_native_fullscreen(cg1);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Exit native fullscreen but window still covers the screen
    macos.exit_native_fullscreen(cg1);
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Should become borderless fullscreen, not moved offscreen
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn offscreen_window_becomes_borderless_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Switch workspace — both offscreen
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));

    // cg1 auto-zooms to fullscreen while offscreen
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Should be minimized (borderless FS can't go offscreen)
    assert_eq!(macos.minimize_count(cg1), 1);
}

#[test]
fn new_window_already_borderless_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    // Window already covers the screen before Dome discovers it
    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.move_window(cg1, 0, 0, 1920, 1080);
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    // Should stay fullscreen, not be tiled
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn float_focus_unfocus_cycle() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // cg2 is focused (last added). Toggle it to float.
    dome.run_hub_actions(&actions("toggle float"));
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg2));

    // Click cg1 — unfocused float cg2 goes offscreen
    dome.mirror_clicked(cg1);
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg2));

    // Click float cg2 — it comes back from offscreen
    dome.mirror_clicked(cg2);
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn hide_noop_for_native_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Enter native fullscreen
    macos.enter_native_fullscreen(cg1);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    let frame_before = macos.window_frame(cg1);

    // Switch workspace — native FS window should not be touched
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.minimize_count(cg1), 0);
    assert_eq!(macos.window_frame(cg1), frame_before);
}

#[test]
fn hide_noop_for_minimized() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Zoom → workspace switch → minimized
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.minimize_count(cg1), 1);

    // Second workspace switch — minimize should NOT be called again
    dome.run_hub_actions(&actions("focus workspace 2"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.minimize_count(cg1), 1);
}

#[test]
fn offscreen_window_rehidden_on_external_move() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Hide both
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));

    // macOS moves cg1 to a visible position
    macos.move_window(cg1, 100, 100, 800, 600);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Dome should re-hide it
    assert!(macos.is_offscreen(cg1));
}

#[test]
fn borderless_fullscreen_full_lifecycle() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // 1. Zoom → borderless fullscreen
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
    assert!(macos.is_offscreen(cg2));

    // 2. Workspace switch → minimized
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.minimize_count(cg1), 1);

    // 3. Switch back → unminimized, still borderless fullscreen
    dome.run_hub_actions(&actions("focus workspace 0"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.unminimize_count(cg1), 1);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    // 4. Un-zoom → exits borderless fullscreen, both windows tiled
    macos.move_window(cg1, 100, 100, 800, 600);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn minimized_borderless_reappears_still_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Zoom → workspace switch → minimized
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.minimize_count(cg1), 1);

    // Window reappears still covering the screen — should re-minimize
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.minimize_count(cg1), 2);
}

#[test]
fn zoom_button_triggers_borderless_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn borderless_fullscreen_exit_to_tiling() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    // Enter borderless fullscreen
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Un-zoom
    macos.move_window(cg1, 100, 100, 800, 600);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn native_fullscreen_enter() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(cg1);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Hub treats fullscreen as taking the full monitor — sibling is hidden.
    // In real macOS, space_changed would restore siblings on the original Space.
    assert!(macos.is_offscreen(cg2));
}

#[test]
fn native_fullscreen_exit() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(cg1);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    macos.exit_native_fullscreen(cg1);
    macos.move_window(cg1, 200, 200, 800, 600);
    macos.simulate_external_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn toggle_fullscreen_hides_siblings() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    dome.run_hub_actions(&actions("toggle fullscreen"));
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg2), (0, 0, 1920, 1080));
    assert!(macos.is_offscreen(cg1));
}

#[test]
fn toggle_fullscreen_on_and_off() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    dome.run_hub_actions(&actions("toggle fullscreen"));
    macos.settle(&mut dome, 10);

    dome.run_hub_actions(&actions("toggle fullscreen"));
    macos.settle(&mut dome, 10);

    // TODO: after toggling fullscreen off with move event feedback, windows
    // don't restore correctly — separate bug from convergence
}