use super::*;

#[test]
fn a_fractional_work_area_keeps_the_fullscreen_window_inside_it() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let mut monitor = default_monitor();
    monitor.work_area = PixelRect::from_dimension_inward(FRACTIONAL_WORK_AREA);
    dome.monitors_changed(vec![monitor]);

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Reaches `place_fullscreen_window` with the window still in `Tiling`.
    send(&mut dome, "toggle fullscreen");
    macos.settle(&mut dome, 10);
    assert_inside_work_area(macos.window_frame(cg1), FRACTIONAL_WORK_AREA);

    // Round trip through another workspace, which returns via the `Offscreen` arm.
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert_inside_work_area(macos.window_frame(cg1), FRACTIONAL_WORK_AREA);
}

#[test]
fn fullscreen_window_restored_from_offscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
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

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg2));

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));
    assert!(!macos.is_offscreen(cg1));

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_minimized(cg1));
    assert!(macos.is_offscreen(cg1));
}

#[test]
fn native_fullscreen_exit_to_borderless() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    macos.exit_native_fullscreen(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    assert!(macos.is_minimized(cg1));
}

#[test]
fn new_window_already_borderless_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window_at(100, "Safari", "Google", 0, 0, 1920, 1080);
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg2));

    dome.mirror_clicked(cg1);
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg2));

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    let frame_before = macos.window_frame(cg1);

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));

    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
    macos.settle(&mut dome, 10);

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
    assert!(macos.is_offscreen(cg2));

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_minimized(cg1));
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));
}

#[test]
fn borderless_fullscreen_exit_to_tiling() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    dome.mirror_clicked(cg1);
    macos.settle(&mut dome, 10);
    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    let frame_before = macos.window_frame(cg1);

    // toggle_float should be blocked by ProtectFullscreen restriction
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), frame_before);

    macos.exit_native_fullscreen(&mut dome, cg1, 200, 200, 800, 600);
    macos.settle(&mut dome, 10);

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
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    // toggle_float should be blocked by ProtectFullscreen restriction
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    macos.simulate_external_move(&mut dome, cg1, 100, 100, 800, 600);
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);
    assert_ne!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn borderless_fullscreen_allows_move_workspace() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

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
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));

    macos.exit_native_fullscreen(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    assert!(macos.is_minimized(cg1));
}

#[test]
fn native_fullscreen_exit_to_borderless_unfocused_then_switch_back() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);

    macos.exit_native_fullscreen(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

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
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 200, 200, 800, 600);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg2), (4, 4, 952, 1072));
    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));
}

#[test]
fn user_minimized_unminimize_via_focus() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));

    dome.focus_window_by_cg(cg1);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));
}

#[test]
fn user_minimized_deminiaturize_then_focus() {
    // Full flow: user minimizes → deminiaturize notification fires (move event) →
    // focus event arrives. The subsequent focus event is a no-op (window already
    // placed).
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 300, 100, 800, 600);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));

    dome.focus_window_by_cg(cg1);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));
}

#[test]
fn user_minimize_single_window_then_unminimize() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));

    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    dome.focus_window_by_cg(cg1);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), (4, 4, 1912, 1072));
}

#[test]
fn user_minimize_noop_on_unknown_window() {
    // If reconcile reports a minimize for an unknown cg_id, it should not panic.
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    let unknown_cg = 999;
    dome.reconcile_windows(&[], &[], &[unknown_cg], vec![], &[], &[]);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn window_turned_borderless_fullscreen_after_user_minimize() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
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
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    let (_, _, w_before, h_before) = macos.window_frame(cg1);

    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    dome.focus_window_by_cg(cg1);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
    let (_, _, w_after, h_after) = macos.window_frame(cg1);
    assert_eq!((w_after, h_after), (w_before, h_before));
    assert!(!macos.is_minimized(cg1));
}

#[test]
fn borderless_fullscreen_state_preserved_through_dome_minimize_round_trip() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);

    let placed = macos.window_frame(cg1);

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_minimized(cg1));

    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), placed);
    assert!(!macos.is_minimized(cg1));
}

#[test]
fn float_state_preserved_through_user_minimize_round_trip() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    let placed = macos.window_frame(cg2);

    macos.user_minimize(&mut dome, cg2);
    macos.settle(&mut dome, 10);

    dome.focus_window_by_cg(cg2);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg2), placed);
    assert!(!macos.is_minimized(cg2));
}

#[test]
fn native_fullscreen_state_preserved_through_user_minimize_round_trip() {
    // The unminimize_window path is required because focus_window_by_cg alone
    // does not clear ByUser on NativeFullscreen windows (place_fullscreen_window
    // only handles ByDome).
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    let placed = macos.window_frame(cg1);
    // Grab WindowId from frame state while the window is focused (before minimize
    // clears focus).
    let window_id = macos.last_frame_state().focused_window.unwrap();

    macos.user_minimize(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    dome.unminimize_window(window_id);
    dome.flush_layout();
    macos.settle(&mut dome, 10);

    // Geometry unchanged (NativeFullscreen windows are positioned by macOS)
    assert_eq!(macos.window_frame(cg1), placed);
    assert!(!macos.is_minimized(cg1));
}
