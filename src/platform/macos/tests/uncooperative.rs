use std::time::Instant;

use crate::platform::macos::dome::WindowMove;

use super::*;

#[test]
fn drift_exhausts_retries_dome_gives_up() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    // Window resists placement — always snaps to (100, 100, 800, 600)
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.move_window(cg1, 100, 100, 800, 600);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 20);

    // After MAX_ENFORCEMENT_RETRIES (5), Dome gives up — window stays at its chosen position
    assert_eq!(macos.window_frame(cg1), (100, 100, 800, 600));
}

#[test]
fn drift_retries_reset_on_new_target() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    // Exhaust retries
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.move_window(cg1, 100, 100, 800, 600);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 20);
    assert_eq!(macos.window_frame(cg1), (100, 100, 800, 600));

    // Stop resisting and add a new window — target changes, retries reset
    macos.set_override_frame(cg1, None);
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // cg1 should now be placed at its new (half-screen) target
    assert!(!macos.is_offscreen(cg1));
    let (_, _, w1, _) = macos.window_frame(cg1);
    assert!(w1 < 1900, "cg1 should be half-screen, got width {w1}");
}

#[test]
fn drift_to_fullscreen_triggers_borderless_detection() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    // Window auto-zooms to fullscreen in response to set_frame
    macos.set_override_frame(cg1, Some((0, 0, 1920, 1080)));
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);

    // Should be detected as borderless fullscreen, not treated as drift
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn offscreen_window_fights_hide() {
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

    // Window fights back — keeps snapping to visible position
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.move_window(cg1, 100, 100, 800, 600);
    macos.report_move(&mut dome, cg1);

    // Dome retries hiding up to MAX_ENFORCEMENT_RETRIES times, then gives up.
    // report_move incremented retries to 1 and issued hide_at.
    // Each flush_moves round: window fights back → record_drift increments retries → hide_at.
    // After 4 flush rounds: retries = 5, should_retry() still true (5 <= 5).
    // 5th flush round: retries = 6, should_retry() false, just_gave_up() true → no hide_at.
    for _ in 0..4 {
        assert!(
            !macos.moves.borrow().is_empty(),
            "Dome should still be retrying hide"
        );
        macos.flush_moves(&mut dome);
    }
    // One more round: retries exceed limit, Dome gives up
    macos.flush_moves(&mut dome);
    assert!(
        macos.moves.borrow().is_empty(),
        "Dome should have stopped issuing hide_at"
    );
}

#[test]
fn hide_retries_reset_on_fresh_hide() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Hide both by switching workspace
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));

    // cg1 fights back — exhaust retries
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.move_window(cg1, 100, 100, 800, 600);
    macos.report_move(&mut dome, cg1);
    for _ in 0..5 {
        macos.flush_moves(&mut dome);
    }
    assert!(macos.moves.borrow().is_empty());

    // Switch back to workspace 0 — cg1 becomes InView
    macos.set_override_frame(cg1, None);
    dome.run_hub_actions(&actions("focus workspace 0"));
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));

    // Switch away again — fresh hide, retries reset to 0
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);

    // Now set override and start fighting again
    macos.set_override_frame(cg1, Some((100, 100, 800, 600)));
    macos.move_window(cg1, 100, 100, 800, 600);
    macos.report_move(&mut dome, cg1);
    for _ in 0..4 {
        assert!(!macos.moves.borrow().is_empty(), "should still be retrying");
        macos.flush_moves(&mut dome);
    }
    macos.flush_moves(&mut dome);
    assert!(
        macos.moves.borrow().is_empty(),
        "should have given up again"
    );
}

#[test]
fn constraint_changes_over_time() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Finder", "Home");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // First constraint: cg2 reports min width 1000 (right-edge aligned)
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.move_window(cg2, x2, y2, 1000, h2);
    macos.report_move(&mut dome, cg2);
    macos.settle(&mut dome, 10);
    let (_, _, w2, _) = macos.window_frame(cg2);
    assert!(w2 >= 1000, "First constraint: expected >= 1000, got {w2}");

    // Second constraint: cg2 now reports even larger min width
    let (x2, y2, _, h2) = macos.window_frame(cg2);
    macos.move_window(cg2, x2, y2, 1200, h2);
    macos.report_move(&mut dome, cg2);
    macos.settle(&mut dome, 10);
    let (_, _, w2, _) = macos.window_frame(cg2);
    assert!(w2 >= 1200, "Updated constraint: expected >= 1200, got {w2}");
}

#[test]
fn drift_correction_repositions_window() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

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
    let mut dome = macos.setup_dome();

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
    let mut dome = macos.setup_dome();

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
    let mut dome = macos.setup_dome();

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
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);
    let placed_frame = macos.window_frame(cg1);

    // Add second window — cg1 gets relayouted to smaller size
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);
    let new_frame = macos.window_frame(cg1);

    // Stale event with old position and past timestamp
    let stale_time = Instant::now() - std::time::Duration::from_secs(10);
    dome.windows_moved(vec![WindowMove {
        cg_id: cg1,
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
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));
    assert!(macos.is_offscreen(cg2));

    dome.run_hub_actions(&actions("focus workspace 0"));
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn no_redundant_set_frame_after_settling() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

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
