use super::*;

#[test]
fn fullscreen_window_restored_from_offscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Toggle fullscreen on cg2 (focused) — it covers the screen, cg1 hidden
    send(&mut dome, "toggle fullscreen");
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg2), (0, 0, 1920, 1080));

    // Switch workspace — fullscreen window goes offscreen
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg2));

    // Switch back — fullscreen window should be placed from offscreen to full screen
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg2), (0, 0, 1920, 1080));
}

#[test]
fn borderless_fullscreen_hidden_on_workspace_switch() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Zoom cg1 to borderless fullscreen
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    // Switch workspace — borderless FS window should be minimized, not moved offscreen
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));
    assert!(!macos.is_offscreen(cg1));

    // Switch back — should unminimize and remain borderless fullscreen
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_minimized(cg1));
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn minimized_window_reappears_non_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Zoom cg1 → borderless fullscreen → workspace switch → minimized
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    // Window reappears at non-fullscreen size (user un-zoomed while minimized)
    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
    macos.settle(&mut dome, 10);

    // Should unminimize and move offscreen (no longer fullscreen)
    assert!(!macos.is_minimized(cg1));
    assert!(macos.is_offscreen(cg1));
}

#[test]
fn native_fullscreen_exit_to_borderless() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Enter native fullscreen
    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Exit native fullscreen but window still covers the screen
    macos.exit_native_fullscreen(&mut dome, cg1, 0, 0, 1920, 1080);
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
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Switch workspace — both offscreen
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));

    // cg1 auto-zooms to fullscreen while offscreen
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    // Should be minimized (borderless FS can't go offscreen)
    assert!(macos.is_minimized(cg1));
}

#[test]
fn new_window_already_borderless_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    // Window already covers the screen before Dome discovers it
    let cg1 = macos.spawn_window_at(100, "Safari", "Google", 0, 0, 1920, 1080);
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
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
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // cg2 is focused (last added). Toggle it to float.
    send(&mut dome, "toggle float");
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
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Enter native fullscreen
    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    let frame_before = macos.window_frame(cg1);

    // Switch workspace — native FS window should not be touched
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_minimized(cg1));
    assert_eq!(macos.window_frame(cg1), frame_before);
}

#[test]
fn hide_noop_for_minimized() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Zoom → workspace switch → minimized
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    // Second workspace switch — minimize should NOT be called again
    send(&mut dome, "focus workspace 2");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));
}

#[test]
fn offscreen_window_rehidden_on_external_move() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Hide both
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));

    // macOS moves cg1 to a visible position
    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
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
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // 1. Zoom → borderless fullscreen
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
    assert!(macos.is_offscreen(cg2));

    // 2. Workspace switch → minimized
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    // 3. Switch back → unminimized, still borderless fullscreen
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_minimized(cg1));
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    // 4. Un-zoom → exits borderless fullscreen, both windows tiled
    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
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
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Zoom → workspace switch → minimized
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    // Window reappears still covering the screen — should re-minimize
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));
}

#[test]
fn zoom_button_triggers_borderless_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn borderless_fullscreen_exit_to_tiling() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Enter borderless fullscreen
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    // Un-zoom
    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn native_fullscreen_enter() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(&mut dome, cg1);
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
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    macos.exit_native_fullscreen(&mut dome, cg1, 200, 200, 800, 600);
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
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle fullscreen");
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
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle fullscreen");
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle fullscreen");
    macos.settle(&mut dome, 10);

    // TODO: after toggling fullscreen off with move event feedback, windows
    // don't restore correctly — separate bug from convergence
}

#[test]
fn native_fullscreen_blocks_toggle_float() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Focus cg1 and enter native fullscreen
    dome.mirror_clicked(cg1);
    macos.settle(&mut dome, 10);
    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    let frame_before = macos.window_frame(cg1);

    // toggle_float should be blocked by ProtectFullscreen restriction
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), frame_before);

    // Exit native fullscreen
    macos.exit_native_fullscreen(&mut dome, cg1, 200, 200, 800, 600);
    macos.settle(&mut dome, 10);

    // toggle_float should now work — cg2 fills the screen since cg1 is floating
    let cg2_before = macos.window_frame(cg2);
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg2));
    // cg1 is now floating; cg2 expands to fill the tiling area
    assert_ne!(macos.window_frame(cg2), cg2_before);
}

#[test]
fn borderless_fullscreen_blocks_toggle_float() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Zoom to borderless fullscreen
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    // toggle_float should be blocked by ProtectFullscreen restriction
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    // Exit borderless fullscreen
    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
    macos.settle(&mut dome, 10);

    // toggle_float should now work
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);
    assert_ne!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn borderless_fullscreen_allows_move_workspace() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Zoom to borderless fullscreen
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    // move_workspace should be allowed despite ProtectFullscreen restriction
    send(&mut dome, "move workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));
}

#[test]
fn native_fullscreen_exit_to_borderless_on_unfocused_workspace() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Enter native fullscreen
    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Switch to workspace 1
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));

    // Exit native fullscreen while on unfocused workspace, window still covers screen
    macos.exit_native_fullscreen(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    // Window should be minimized because its workspace is not focused
    assert!(macos.is_minimized(cg1));
}

#[test]
fn native_fullscreen_exit_to_borderless_unfocused_then_switch_back() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Enter native fullscreen, switch away, exit native fullscreen
    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);

    macos.exit_native_fullscreen(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    // Switch back to workspace 0 — window should be restored
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
    assert!(!macos.is_minimized(cg1));
}

#[test]
fn user_minimize_via_reconcile() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // User minimizes cg1 — detected by reconcile
    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // cg2 should take full screen since cg1 was removed from tiling.
    // Width is less than 1920 due to border inset but should be close to full.
    assert!(!macos.is_offscreen(cg2));
    let (_, _, w, _) = macos.window_frame(cg2);
    assert!(
        w > 1900,
        "cg2 should take (nearly) full screen, got width {w}"
    );
}

#[test]
fn user_minimized_window_receives_move_event() {
    // Simulates kAXWindowDeminiaturizedNotification routed through the move path.
    // A UserMinimized window receiving a move event should be moved offscreen,
    // waiting for the focus event to trigger layout reattachment.
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // User minimizes cg1
    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Deminiaturize (user clicked Dock icon) fires as a move event
    macos.simulate_external_move(&mut dome, cg1, 200, 200, 800, 600);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg2), (4, 4, 952, 1072));
    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));
}

#[test]
fn user_minimized_unminimize_via_focus() {
    // When set_focus is called on a minimized window (e.g. from the picker),
    // hub.set_focus checks the is_minimized flag, calls unminimize_window to
    // clear it and reattach to the current workspace. flush_layout then
    // positions the window via show_tiling / show_float.
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // User minimizes cg1
    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // cg2 should now be full screen (border inset makes it slightly less than 1920)
    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));

    // User selects cg1 from the picker → focus_window_by_cg
    dome.focus_window_by_cg(cg1);
    macos.settle(&mut dome, 10);

    // cg1 should be placed back in a tiling layout (half screen with cg2)
    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));
}

#[test]
fn user_minimized_deminiaturize_then_focus() {
    // Full flow: user minimizes → deminiaturize notification fires (move event) →
    // focus event arrives. The move event detects ByUser, calls hub.unminimize_window,
    // clears the minimize flag, and layout places the window. The subsequent focus
    // event is a no-op (window already placed).
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Minimize cg1
    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Deminiaturize notification arrives as move event
    macos.simulate_external_move(&mut dome, cg1, 300, 100, 800, 600);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));

    // Focus arrives (Dock click also raises the window)
    dome.focus_window_by_cg(cg1);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));
}

#[test]
fn user_minimize_single_window_then_unminimize() {
    // Edge case: minimize the only window on a workspace, then unminimize it.
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));

    // Minimize the only window
    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Unminimize via focus
    dome.focus_window_by_cg(cg1);
    macos.settle(&mut dome, 10);

    // Should be placed full screen again (border inset makes it slightly less than 1920)
    assert_eq!(macos.window_frame(cg1), (4, 4, 1912, 1072));
}

#[test]
fn user_minimize_noop_on_unknown_window() {
    // If reconcile reports a minimize for an unknown cg_id, it should not panic.
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Minimize a non-existent window — should be silently ignored
    let unknown_cg = 999;
    dome.reconcile_windows(&[], &[unknown_cg], vec![], &[], &[]);
    macos.settle(&mut dome, 10);

    // Original window should be unaffected
    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn window_turned_borderless_fullscreen_after_user_minimize() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Minimize cg1
    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Zoom cg1 to borderless fullscreen
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn native_fullscreen_enter_detected_via_reconcile() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Enter native fullscreen
    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    assert!(macos.is_offscreen(cg2));
}

#[test]
fn native_fullscreen_exit_detected_via_reconcile() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Enter native fullscreen
    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Exit native fullscreen
    macos.exit_native_fullscreen(&mut dome, cg1, 200, 200, 800, 600);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn tiling_state_preserved_through_user_minimize_round_trip() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    let (_, _, w_before, h_before) = macos.window_frame(cg1);

    // User minimizes cg1
    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Unminimize via focus (simulates picker or Dock click)
    dome.focus_window_by_cg(cg1);
    macos.settle(&mut dome, 10);

    // Window is visible and has same tiling slot dimensions (not lost to
    // offscreen or resized as float)
    assert!(!macos.is_offscreen(cg1));
    let (_, _, w_after, h_after) = macos.window_frame(cg1);
    assert_eq!((w_after, h_after), (w_before, h_before));
    // AX unminimize was called to drive the OS-side restore
    assert!(!macos.is_minimized(cg1));
}

#[test]
fn borderless_fullscreen_state_preserved_through_dome_minimize_round_trip() {
    // ByDome path: borderless fullscreen window hidden on workspace switch
    // should restore to its original fullscreen frame on switch-back.
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Zoom cg1 to borderless fullscreen
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    let placed = macos.window_frame(cg1);

    // Switch workspace away -- Dome minimizes cg1
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    // Switch back -- Dome unminimizes cg1
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);

    // Geometry restored to borderless fullscreen frame
    assert_eq!(macos.window_frame(cg1), placed);
    assert!(!macos.is_minimized(cg1));
}

#[test]
fn float_state_preserved_through_user_minimize_round_trip() {
    // Round-trip: float window user-minimized via the OS should return to
    // its original position when restored, confirming the float slot is
    // preserved (not demoted to offscreen or promoted to tiling).
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // cg2 is focused (last added). Toggle it to float.
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    let placed = macos.window_frame(cg2);

    // User minimizes cg2
    macos.user_minimize(&mut dome, cg2);
    macos.settle(&mut dome, 10);

    // Unminimize via focus (simulates picker or Dock click)
    dome.focus_window_by_cg(cg2);
    macos.settle(&mut dome, 10);

    // Geometry restored to original float position
    assert_eq!(macos.window_frame(cg2), placed);
    // AX unminimize was called to drive the OS-side restore
    assert!(!macos.is_minimized(cg2));
}

#[test]
fn native_fullscreen_state_preserved_through_user_minimize_round_trip() {
    // Round-trip: native fullscreen window user-minimized via the OS should
    // return to its original position when restored through the picker.
    // The picker path is required because focus_window_by_cg alone does not
    // clear ByUser on NativeFullscreen windows (place_fullscreen_window only
    // handles ByDome).
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Enter native fullscreen
    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    let placed = macos.window_frame(cg1);
    // Grab WindowId from frame state while the window is focused (before minimize
    // clears focus). Needed to drive the picker restore path.
    let window_id = macos.last_frame_state().focused_window.unwrap();

    // User minimizes cg1
    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Restore via picker (the picker handles ByUser and preserves NativeFullscreen
    // state, unlike the window_moved path which would exit the fullscreen Space).
    dome.picker_unminimize_window(window_id);
    dome.flush_layout();
    macos.settle(&mut dome, 10);

    // Geometry unchanged (NativeFullscreen windows are positioned by macOS)
    assert_eq!(macos.window_frame(cg1), placed);
    // AX unminimize was called to drive the OS-side restore
    assert!(!macos.is_minimized(cg1));
}
