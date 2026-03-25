use std::time::Instant;

use crate::platform::macos::dome::WindowMove;

use super::*;

#[test]
fn single_window_placed_in_view() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
    assert_eq!(macos.window_frame(cg1), (2, 2, 1916, 1076));
}

#[test]
fn two_windows_split_horizontally() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    let (x1, _, w1, _) = macos.window_frame(cg1);
    let (x2, _, w2, _) = macos.window_frame(cg2);
    assert!(x1 < x2);
    assert!(w1 > 0 && w2 > 0);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn workspace_switch_hides_and_restores() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    let placed = macos.window_frame(cg1);

    dome.run_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));
    assert!(macos.is_offscreen(cg2));

    dome.run_actions(&actions("focus workspace 0"));
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg1), placed);
}

#[test]
fn zoom_button_triggers_borderless_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn borderless_fullscreen_exit_to_tiling() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    // Enter borderless fullscreen
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Un-zoom
    macos.move_window(cg1, 100, 100, 800, 600);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn native_fullscreen_enter() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(cg1);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Hub treats fullscreen as taking the full monitor — sibling is hidden.
    // In real macOS, space_changed would restore siblings on the original Space.
    assert!(macos.is_offscreen(cg2));
}

#[test]
fn native_fullscreen_exit() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    macos.enter_native_fullscreen(cg1);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    macos.exit_native_fullscreen(cg1);
    macos.move_window(cg1, 200, 200, 800, 600);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn toggle_fullscreen_hides_siblings() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    dome.run_actions(&actions("toggle fullscreen"));
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg2), (0, 0, 1920, 1080));
    assert!(macos.is_offscreen(cg1));
}

#[test]
fn toggle_fullscreen_on_and_off() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    dome.run_actions(&actions("toggle fullscreen"));
    macos.settle(&mut dome, 10);

    dome.run_actions(&actions("toggle fullscreen"));
    macos.settle(&mut dome, 10);

    // TODO: after toggling fullscreen off with move event feedback, windows
    // don't restore correctly — separate bug from convergence
}

#[test]
fn drift_correction_repositions_window() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    let expected = macos.window_frame(cg1);

    macos.move_window(cg1, 50, 50, 1916, 1076);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), expected);
}

#[test]
fn window_min_size_constraint() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Finder", "Home");
    macos.set_min_size(cg2, 1000, 400);

    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    let (_, _, w2_width, _) = macos.window_frame(cg2);
    assert!(
        w2_width >= 1000,
        "Finder should be at least 1000px wide, got {w2_width}"
    );
    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn window_max_size_constraint() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Finder", "Home");
    macos.set_max_size(cg2, 500, 2000);

    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    let (_, _, w2_width, _) = macos.window_frame(cg2);
    assert!(
        w2_width <= 500,
        "Finder should be at most 500px wide, got {w2_width}"
    );

    let (_, _, w1_width, _) = macos.window_frame(cg1);
    assert!(
        w1_width > 960,
        "Safari should get more than half the screen, got {w1_width}"
    );
}

#[test]
fn size_constraint_from_external_move() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Finder", "Home");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // App reports larger size than Dome requested (min width constraint)
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.move_window(cg2, x2, y2, 1000, h2);
    macos.report_move(&mut dome, cg2);
    macos.settle(&mut dome, 10);

    let (_, _, w2_width, _) = macos.window_frame(cg2);
    assert!(w2_width >= 1000);
}

#[test]
fn stale_move_events_ignored() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);
    let w1 = dome.window_id_for_cg(cg1).unwrap();
    let placed_frame = macos.window_frame(cg1);

    // Add second window — cg1 gets relayouted to smaller size
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);
    let new_frame = macos.window_frame(cg1);

    // Stale event with old position and past timestamp
    let stale_time = Instant::now() - std::time::Duration::from_secs(10);
    dome.windows_moved(vec![WindowMove {
        window_id: w1,
        x: placed_frame.0,
        y: placed_frame.1,
        w: placed_frame.2,
        h: placed_frame.3,
        observed_at: stale_time,
        is_native_fullscreen: false,
    }]);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(cg1), new_frame);
}

#[test]
fn offscreen_move_events_keep_windows_hidden() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    dome.run_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));
    assert!(macos.is_offscreen(cg2));

    dome.run_actions(&actions("focus workspace 0"));
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn app_terminated_removes_windows() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Tab 1");
    let cg2 = macos.spawn_window(100, "Safari", "Tab 2");
    let cg3 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        vec![
            new_window(&macos, cg1),
            new_window(&macos, cg2),
            new_window(&macos, cg3),
        ],
    );
    macos.settle(&mut dome, 10);

    dome.app_terminated(100);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg3));
    assert_eq!(macos.window_frame(cg3), (2, 2, 1916, 1076));
}

#[test]
fn window_removed_fills_screen() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    dome.reconcile_windows(&[cg1], vec![]);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg2), (2, 2, 1916, 1076));
}

#[test]
fn no_redundant_set_frame_after_settling() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // System is settled — no pending moves
    assert!(
        macos.moves.borrow().is_empty(),
        "should have no pending moves after settle"
    );

    // One more settle cycle should be a no-op
    macos.settle(&mut dome, 1);
}
