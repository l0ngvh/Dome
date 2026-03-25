use super::*;

#[test]
fn single_window_fills_screen() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);

    assert_eq!(macos.window_frame(cg1), (2, 2, 1916, 1076));
}

#[test]
fn two_windows_split_horizontally() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);

    assert_eq!(macos.window_frame(cg1), (2, 2, 956, 1076));
    assert_eq!(macos.window_frame(cg2), (962, 2, 956, 1076));
}

#[test]
fn remove_window_expands_remaining() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);

    dome.reconcile_windows(&[cg2], vec![]);

    assert_eq!(macos.window_frame(cg1), (2, 2, 1916, 1076));
}

#[test]
fn move_to_workspace() {
    let mut macos = MacOS::new();
    let mut dome = setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);

    dome.run_actions(&actions("move workspace 1"));

    // w2 moved to workspace 1, so w1 should fill the screen on workspace 0
    assert_eq!(macos.window_frame(cg2), (2, 2, 1916, 1076));
}
