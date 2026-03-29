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

    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));
    assert!(macos.is_offscreen(cg2));

    dome.run_hub_actions(&actions("focus workspace 0"));
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg1), placed);
}

#[test]
fn float_window_moved_by_user() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Toggle cg2 (focused) to float
    dome.run_hub_actions(&actions("toggle float"));
    macos.settle(&mut dome, 10);

    // User drags the float to a new position
    macos.move_window(cg2, 200, 150, 600, 400);
    macos.report_move(&mut dome, cg2);
    macos.settle(&mut dome, 10);

    // Float should stay at the user-chosen position, not be corrected
    assert_eq!(macos.window_frame(cg2), (200, 150, 600, 400));
}
