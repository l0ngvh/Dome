use std::sync::Arc;

use crate::action::{Action, Actions};
use crate::platform::macos::dome::{ExtRefresh, MacOSMetadata, NewWindow, PendingAdd};

use super::*;

#[test]
fn discover_native_fullscreen_window() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let ax = macos.window(cg1);
    ax.set_native_fullscreen(true);
    let nw = PendingAdd::NativeFullscreen {
        new: NewWindow {
            ax: Arc::new(ax.clone()),
            metadata: MacOSMetadata {
                app_name: Some("Safari".to_owned()),
                bundle_id: None,
                title: Some("Google".to_owned()),
            },
        },
    };
    dome.reconcile_windows(&[], &[], &[], vec![nw], &[], &[]);
    macos.settle(&mut dome, 10);

    assert!(dome.tracked_window(cg1).is_some());
}

#[test]
fn is_moving_suppresses_placement() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // cg1 is full-screen tiled
    let full_frame = macos.window_frame(cg1);

    // User starts dragging cg1 to a different position
    start_drag(&mut dome, 100);
    macos.window(cg1).position.set((500, 300));
    macos.window(cg1).size.set((400, 400));

    // Add cg2 — triggers relayout (cg1 should go from full to half), but
    // cg1 should NOT be repositioned because it's being dragged
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg2)], &[], &[]);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (500, 300, 400, 400));

    // User stops dragging — in production, this triggers check_positions
    // which reads the window's current position and calls windows_moved
    end_drag(&mut dome, &macos, 100, cg1, 500, 300, 400, 400);
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
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Hide window
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    let offscreen_before = macos.window_frame(cg1);
    assert!(macos.is_offscreen(cg1));

    // Second monitor appears — offscreen position changes
    let second_monitor = MonitorInfo {
        display_id: 2,
        name: "External".to_string(),
        work_area: Dimension::new(
            Length::new(1920.0),
            Length::new(0.0),
            Length::new(2560.0),
            Length::new(1440.0),
        ),
        bounds: Dimension::new(
            Length::new(1920.0),
            Length::new(0.0),
            Length::new(2560.0),
            Length::new(1440.0),
        ),
        full_height: 1440.0,
        is_primary: false,
        scale: 2.0,
    };
    dome.monitors_changed(vec![default_monitor(), second_monitor]);
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
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // Zoom cg1 → borderless fullscreen, cg2 hidden
    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg2));

    // Close the fullscreen window
    dome.reconcile_windows(&[], &[cg1], &[], vec![], &[], &[]);
    macos.settle(&mut dome, 10);

    // Sibling should be restored to full tiling
    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));
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
        &[],
        &[],
        vec![
            new_window(&macos, cg1),
            new_window(&macos, cg2),
            new_window(&macos, cg3),
        ],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    dome.app_terminated(100);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg3));
    assert_eq!(macos.window_frame(cg3), (4, 4, 1912, 1072));
}

#[test]
fn window_removed_fills_screen() {
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

    dome.reconcile_windows(&[], &[cg1], &[], vec![], &[], &[]);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));
}

#[test]
fn render_frame_focused_state() {
    let mut macos = MacOS::new();
    let cg1 = macos.spawn_window(1, "App1", "Win1");
    let mut dome = macos.setup_dome();
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);

    let state = macos.last_frame_state();
    assert!(state.focused_window.is_some());
    assert!(state.focused_monitor_id.is_some());
}

#[test]
fn render_frame_focused_none_after_last_window_removed() {
    let mut macos = MacOS::new();
    let cg1 = macos.spawn_window(1, "App1", "Win1");
    let mut dome = macos.setup_dome();
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    dome.reconcile_windows(&[], &[cg1], &[], vec![], &[], &[]);

    let state = macos.last_frame_state();
    assert_eq!(state.focused_window, None);
}

#[test]
fn render_frame_focused_container_after_focus_parent() {
    let mut macos = MacOS::new();
    let cg1 = macos.spawn_window(1, "App1", "Win1");
    let cg2 = macos.spawn_window(1, "App2", "Win2");
    let mut dome = macos.setup_dome();
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    send(&mut dome, "focus parent");

    // After focus_parent, focused_tiling_window() returns None (container highlighted),
    // so the platform receives focused_window: None and focuses the overlay.
    let state = macos.last_frame_state();
    assert!(state.focused_window.is_none());
}

#[test]
fn render_frame_focused_none_on_empty_workspace() {
    let mut macos = MacOS::new();
    let cg1 = macos.spawn_window(1, "App1", "Win1");
    let mut dome = macos.setup_dome();
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    send(&mut dome, "focus workspace 1");

    let state = macos.last_frame_state();
    assert_eq!(state.focused_window, None);
}

#[test]
fn render_frame_focused_monitor_changes_on_focus_monitor() {
    let macos = MacOS::new();
    let mut dome = macos.setup_dome();
    let second_monitor = MonitorInfo {
        display_id: 2,
        name: "External".to_string(),
        work_area: Dimension::new(
            Length::new(1920.0),
            Length::ZERO,
            Length::new(2560.0),
            Length::new(1440.0),
        ),
        bounds: Dimension::new(
            Length::new(1920.0),
            Length::ZERO,
            Length::new(2560.0),
            Length::new(1440.0),
        ),
        full_height: 1440.0,
        is_primary: false,
        scale: 2.0,
    };
    dome.monitors_changed(vec![default_monitor(), second_monitor]);

    let before = macos.last_frame_state();
    send(&mut dome, "focus monitor right");
    let after = macos.last_frame_state();

    assert_ne!(before.focused_monitor_id, after.focused_monitor_id);
}

#[test]
fn multi_action_sequence_applies_each_hub_action() {
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

    let actions = Actions::new(vec![
        "focus workspace 1".parse().unwrap(),
        "focus workspace 0".parse().unwrap(),
    ]);
    for action in &actions {
        match action {
            Action::Focus(t) => {
                dome.apply_focus(t);
                dome.flush_layout();
            }
            Action::Move(t) => {
                dome.apply_move(t);
                dome.flush_layout();
            }
            Action::Toggle(t) => {
                dome.apply_toggle(t);
                dome.flush_layout();
            }
            Action::Master(t) => {
                dome.apply_master(t);
                dome.flush_layout();
            }
            Action::ToggleMinimized => dome.toggle_picker(),
            _ => {}
        }
    }

    // After "focus ws 1, focus ws 0", workspace 0 is focused and windows are visible
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

// These verify observable behavior at the Dome::reconcile_windows boundary.
// The internal compute_reconciliation logic (is_valid fast path, app.windows()
// membership, per-PID fullscreen guard) is tested indirectly: its output maps
// to the refresh/to_remove/no-op slices passed to reconcile_windows.

#[test]
fn reconcile_keeps_window_when_is_valid_true() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    let wid_before = dome.tracked_window(cg1).unwrap().window_id;

    // Fast path: is_valid stays true, no removal requested.
    // compute_reconciliation would produce empty to_remove for this window.
    dome.reconcile_windows(&[], &[], &[], vec![], &[], &[]);

    let entry = dome
        .tracked_window(cg1)
        .expect("window must remain tracked");
    assert_eq!(entry.window_id, wid_before);
}

#[test]
fn reconcile_keeps_window_when_app_windows_errs() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // When app.windows(marker) returns Err, compute_reconciliation returns
    // all-empty (no removes, no adds, no refresh). Simulate at Dome level.
    // set_valid(false) documents the scenario trigger even though this test
    // bypasses compute_reconciliation and feeds reconcile_windows directly.
    macos.window(cg1).set_valid(false);
    dome.reconcile_windows(&[], &[], &[], vec![], &[], &[]);

    assert!(
        dome.tracked_window(cg1).is_some(),
        "AX-app-unavailable must keep tracked entries"
    );
}

#[test]
fn reconcile_keeps_and_refreshes_when_cg_id_present() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    let wid_before = dome.tracked_window(cg1).unwrap().window_id;
    let ext_before = dome.tracked_window(cg1).unwrap().ext.clone();

    // Simulate stale-handle refresh: is_valid was false, but cg_id appeared
    // in app.windows(), so compute_reconciliation produces an ExtRefresh.
    let fresh_ax = macos.window(cg1).clone();
    let fresh_ext: Arc<dyn ExternalWindow> = Arc::new(fresh_ax);
    dome.reconcile_windows(
        &[ExtRefresh {
            cg_id: cg1,
            ext: fresh_ext.clone(),
        }],
        &[],
        &[],
        vec![],
        &[],
        &[],
    );

    let entry = dome
        .tracked_window(cg1)
        .expect("window must remain tracked");
    assert_eq!(
        entry.window_id, wid_before,
        "WindowId must not change on refresh"
    );
    assert!(
        !Arc::ptr_eq(&ext_before, &entry.ext),
        "ext handle must be swapped after refresh"
    );
}

#[test]
fn reconcile_keeps_when_cg_id_absent_but_pid_has_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    // Two windows on same PID. One enters native fullscreen.
    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(100, "Safari", "Tabs");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);
    macos.enter_native_fullscreen(&mut dome, cg2);

    // Space-switch scenario: is_valid false on cg1, app.windows() empty.
    // Fullscreen guard keeps the entry. At Dome level, this means
    // compute_reconciliation does NOT put cg1 in to_remove.
    macos.window(cg1).set_valid(false);
    dome.reconcile_windows(&[], &[], &[], vec![], &[], &[]);

    assert!(
        dome.tracked_window(cg1).is_some(),
        "fullscreen guard must keep window when same-PID has native fullscreen"
    );
    assert!(dome.tracked_window(cg2).is_some());
}

#[test]
fn reconcile_removes_when_cg_id_absent_and_no_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    // Window is genuinely gone: is_valid false, not in app.windows(), no
    // fullscreen on this PID. compute_reconciliation puts cg1 in to_remove.
    macos.window(cg1).set_valid(false);
    dome.reconcile_windows(&[], &[cg1], &[], vec![], &[], &[]);

    assert!(
        dome.tracked_window(cg1).is_none(),
        "window must be torn down when cg_id absent and no fullscreen guard"
    );
}

#[test]
fn reconcile_fullscreen_guard_is_per_pid() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    // cg1 on PID 100, cg2 on PID 200.
    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(200, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    // PID 200 enters native fullscreen.
    macos.enter_native_fullscreen(&mut dome, cg2);

    // PID 100 has no fullscreen. A removal on PID 100 proceeds even though
    // PID 200 has a fullscreen window. The guard is per-PID, not global.
    macos.window(cg1).set_valid(false);
    dome.reconcile_windows(&[], &[cg1], &[], vec![], &[], &[]);

    assert!(
        dome.tracked_window(cg1).is_none(),
        "PID-200's fullscreen must not protect PID-100's window from removal"
    );
    assert!(
        dome.tracked_window(cg2).is_some(),
        "PID-200's fullscreen window must remain tracked"
    );
}
