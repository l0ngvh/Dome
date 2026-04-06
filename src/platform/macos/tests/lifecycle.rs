use std::sync::Arc;

use crate::action::Actions;
use crate::config::{Config, MacosOnOpenRule, MacosWindow};
use crate::platform::macos::dome::NewWindow;

use super::*;

#[test]
fn discover_native_fullscreen_window() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let ax = macos.window(cg1);
    ax.set_native_fullscreen(true);
    let nw = NewWindow {
        ax: Arc::new(ax.clone()),
        app_name: Some("Safari".to_owned()),
        bundle_id: None,
        title: Some("Google".to_owned()),
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
        is_native_fullscreen: true,
    };
    dome.reconcile_windows(&[], vec![nw]);
    macos.settle(&mut dome, 10);

    assert!(dome.tracked_window(cg1).is_some());
}

#[test]
fn on_open_moves_window_to_other_workspace() {
    let mut macos = MacOS::new();
    let mut config = Config::default();
    config.macos.on_open.push(MacosOnOpenRule {
        window: MacosWindow {
            app: Some("Slack".to_owned()),
            bundle_id: None,
            title: None,
        },
        run: Actions::new(vec!["move workspace 3".parse().unwrap()]),
    });
    let mut dome = macos.setup_dome_with_config(config);

    let cg1 = macos.spawn_window(100, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    // on_open rule moves Slack to workspace 3; hide_window called while Offscreen
    let cg2 = macos.spawn_window(200, "Slack", "General");
    let on_open = dome.reconcile_windows(&[], vec![new_window(&macos, cg2)]);
    for actions in on_open {
        dome.run_hub_actions(&actions);
    }
    macos.settle(&mut dome, 10);

    assert!(macos.is_offscreen(cg2));
    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn is_moving_suppresses_placement() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    // cg1 is full-screen tiled
    let full_frame = macos.window_frame(cg1);

    // User starts dragging cg1 to a different position
    dome.set_pid_moving(100, true);
    macos.move_window(cg1, 500, 300, 400, 400);

    // Add cg2 — triggers relayout (cg1 should go from full to half), but
    // cg1 should NOT be repositioned because it's being dragged
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (500, 300, 400, 400));

    // User stops dragging — in production, this triggers check_positions
    // which reads the window's current position and calls windows_moved
    dome.set_pid_moving(100, false);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    let new_frame = macos.window_frame(cg1);
    assert_ne!(new_frame, (500, 300, 400, 400));
    assert_ne!(new_frame, full_frame);
}

#[test]
fn monitor_change_rehides_offscreen_windows() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1)]);
    macos.settle(&mut dome, 10);

    // Hide window
    dome.run_hub_actions(&actions("focus workspace 1"));
    macos.settle(&mut dome, 10);
    let offscreen_before = macos.window_frame(cg1);
    assert!(macos.is_offscreen(cg1));

    // Second monitor appears — offscreen position changes
    let second_monitor = MonitorInfo {
        display_id: 2,
        name: "External".to_string(),
        dimension: Dimension {
            x: 1920.0,
            y: 0.0,
            width: 2560.0,
            height: 1440.0,
        },
        full_height: 1440.0,
        is_primary: false,
        scale: 2.0,
    };
    dome.screens_changed(vec![default_screen(), second_monitor]);
    macos.settle(&mut dome, 10);

    // Window should be re-hidden at the new offscreen position (based on second monitor)
    assert!(macos.is_offscreen(cg1));
    let offscreen_after = macos.window_frame(cg1);
    assert_ne!(
        offscreen_before, offscreen_after,
        "offscreen position should change with new monitor"
    );
}

#[test]
fn remove_borderless_fullscreen_window_restores_siblings() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    // Zoom cg1 → borderless fullscreen, cg2 hidden
    macos.move_window(cg1, 0, 0, 1920, 1080);
    macos.report_move(&mut dome, cg1);
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg2));

    // Close the fullscreen window
    dome.reconcile_windows(&[cg1], vec![]);
    macos.settle(&mut dome, 10);

    // Sibling should be restored to full tiling
    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg2), (2, 2, 1916, 1076));
}

#[test]
fn app_terminated_removes_windows() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

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
    let mut dome = macos.setup_dome();

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
fn delete_currently_displayed_window() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], vec![new_window(&macos, cg1), new_window(&macos, cg2)]);
    macos.settle(&mut dome, 10);

    dome.reconcile_windows(&[cg1], vec![]);
    macos.settle(&mut dome, 10);

    // Remaining window fills screen
    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg2), (2, 2, 1916, 1076));

    // Second settle proves displayed state was cleaned up
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg2));
}
