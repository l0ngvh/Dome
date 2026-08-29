use std::sync::Arc;

use super::*;
use crate::config::{Config, LayoutConfig, PartitionTreeConfig, WindowMatcher};
use crate::core::GlobalLayoutConfig;

/// Count minimized windows tracked by the daemon by parsing the same JSON
/// blob external launchers consume via `Query::MinimizedWindows`.
fn minimized_json_len(dome: &Dome) -> usize {
    let json = dome.query_minimized_windows_json();
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(&json).expect("query_minimized_windows_json is well-formed");
    arr.len()
}

#[test]
fn window_destroyed_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.destroy_window(w1);

    assert!(!env.is_offscreen(w2));
    assert_h_tiled(
        &[env.dim(w2)],
        default_monitor().work_area,
        env.config.border_size,
    );
}

#[test]
fn window_minimized_removes_from_tiling() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.minimize_window(w2);

    assert_h_tiled(
        &[env.dim(w1)],
        default_monitor().work_area,
        env.config.border_size,
    );
    // w2 stays tracked as a minimized window (not deleted), reachable
    // via the external launcher query surface.
    assert_eq!(minimized_json_len(&env.dome), 1);
}

#[test]
fn user_minimize_then_restore() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.minimize_window(w2);
    assert_eq!(minimized_json_len(&env.dome), 1);

    env.unminimize_window(w2);
    assert_eq!(minimized_json_len(&env.dome), 0);
    assert_h_tiled(
        &[env.dim(w1), env.dim(w2)],
        default_monitor().work_area,
        env.config.border_size,
    );
}

#[test]
fn move_size_suppresses_placement() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    let placed = env.dim(w1);

    env.dome.move_size_started(w1);

    // Add a second window -- triggers relayout, but w1 should be skipped
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    assert_eq!(env.dim(w1), placed);

    env.dome.clear_move_state(w1);
    env.dome.apply_layout();

    assert!(!env.is_offscreen(w1));
    assert!(!env.is_offscreen(w2));
}

#[test]
fn monitors_changed_updates_layout() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    let before = env.dim(w1);

    let new_monitor = MonitorInfo {
        handle: 1,
        name: "Test".to_string(),
        gdi_device: "\\\\.\\DISPLAY1".to_string(),
        work_area: PixelRect::new(0, 0, 1280, 720),
        bounds: Dimension::new(
            Length::ZERO,
            Length::ZERO,
            Length::new(1280.0),
            Length::new(720.0),
        ),
        is_primary: true,
        scale: 1.0,
    };
    env.dome.monitors_changed(vec![new_monitor]);
    env.dome.apply_layout();

    let after = env.dim(w1);
    assert!(
        after.width < before.width,
        "window should be narrower after monitor shrink"
    );
    assert!(
        after.height < before.height,
        "window should be shorter after monitor shrink"
    );
}

#[test]
fn parked_monitor_windows_hide_on_unplug() {
    let mut env = TestEnv::new();
    env.add_monitor(second_monitor());
    let w = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("move monitor right");
    assert!(
        !env.is_offscreen(w),
        "window should be visible on the second monitor before unplug"
    );

    env.remove_monitor(second_monitor().handle);

    assert!(
        env.is_offscreen(w),
        "parked workspace's window rides the hide diff after unplug, no workspace switch"
    );
}

#[test]
fn parked_monitor_windows_unhide_on_visit() {
    let mut env = TestEnv::new();
    env.add_monitor(second_monitor());
    let w = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("move monitor right");
    assert!(
        !env.is_offscreen(w),
        "window should be visible on the second monitor before unplug"
    );

    env.remove_monitor(second_monitor().handle);
    assert!(
        env.is_offscreen(w),
        "parked workspace's window rides the hide diff after unplug"
    );

    // Visiting the parked workspace by its name plus origin monitor points the
    // primary's active workspace at it, so the window surfaces on the primary
    // with no reattach to a monitor.
    env.run_actions("focus workspace 0 --monitor External");
    assert!(
        !env.is_offscreen(w),
        "visiting the parked workspace surfaces its window on the primary"
    );
}

#[test]
fn unmanageable_window_is_ignored() {
    let mut env = TestEnv::new();
    let arc = Arc::new(
        MockExternalHwnd::with_title(
            1,
            "App1",
            "app1.exe",
            env.moves.clone(),
            env.z_stack.clone(),
            env.focus_target.clone(),
        )
        .with_manageable(false),
    );
    let initial = arc.get_dim();

    assert!(!arc.manageable, "precondition");
    let w1 = env.open_with(arc);

    assert_eq!(env.dim(w1), initial);
}

#[test]
fn ignored_window_rule_prevents_insertion() {
    let mut config = Config::default();
    config.ignore.push(WindowMatcher {
        process: Some("bloat.exe".to_string()),
        ..Default::default()
    });
    let mut env = TestEnv::new_with_config(config);

    let w1 = env.open(1, "Bloat", "bloat.exe", SPAWN_DIM);

    assert_eq!(env.dim(w1), SPAWN_DIM);
}

#[test]
fn ignored_window_rule_by_class_prevents_insertion() {
    let mut config = Config::default();
    config.ignore.push(WindowMatcher {
        class: Some("Shell_TrayWnd".to_string()),
        ..Default::default()
    });
    let mut env = TestEnv::new_with_config(config);

    let ext = Arc::new(
        MockExternalHwnd::with_title(
            1,
            "Taskbar",
            "explorer.exe",
            env.moves.clone(),
            env.z_stack.clone(),
            env.focus_target.clone(),
        )
        .with_class("Shell_TrayWnd")
        .with_dimension(SPAWN_DIM),
    );
    let w1 = env.open_with(ext);

    assert_eq!(env.dim(w1), SPAWN_DIM);
}

#[test]
fn title_changed_manages_unknown_window() {
    let mut env = TestEnv::new();

    // Title change on an unknown window should try to manage it
    // (Runner dispatches as WindowCreated -- here we simulate directly)
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    assert!(!env.is_offscreen(w1));
    assert_h_tiled(
        &[env.dim(w1)],
        default_monitor().work_area,
        env.config.border_size,
    );
}

#[test]
fn delete_currently_displayed_window() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.destroy_window(w1);

    assert!(!env.is_offscreen(w2));
    assert_h_tiled(
        &[env.dim(w2)],
        default_monitor().work_area,
        env.config.border_size,
    );

    // Second apply_layout proves displayed state was cleaned up
    env.dome.apply_layout();
    assert!(!env.is_offscreen(w2));
}

#[test]
fn destroy_last_window_focuses_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    env.destroy_window(w1);
    assert_eq!(env.focus_target(), FocusTarget::Overlay);
}

#[test]
fn destroy_one_of_two_windows_does_not_focus_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.destroy_window(w2);
    assert_eq!(env.focus_target(), FocusTarget::Window(w1));
}

#[test]
fn workspace_switch_to_empty_focuses_overlay() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    env.run_actions("focus workspace 1");
    assert_eq!(env.focus_target(), FocusTarget::Overlay);
}

#[test]
fn workspace_switch_back_does_not_focus_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    env.run_actions("focus workspace 1");
    env.run_actions("focus workspace 0");
    assert_eq!(env.focus_target(), FocusTarget::Window(w1));
}

#[test]
fn focus_parent_focuses_overlay() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.run_actions("focus parent");
    assert_eq!(env.focus_target(), FocusTarget::Overlay);
}

#[test]
fn focus_child_after_parent_does_not_focus_overlay() {
    let mut env = TestEnv::new_with_layout_settings(
        Config::default(),
        GlobalLayoutConfig {
            partition_tree: PartitionTreeConfig {
                automatic_tiling: false,
                tab_bar_height: Pixels::new(24),
            },
            ..GlobalLayoutConfig::default()
        },
        Vec::new(),
    );

    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle spawn");
    let _w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);

    env.run_actions("focus parent");
    env.run_actions("focus left");
    assert!(
        matches!(env.focus_target(), FocusTarget::Window(_)),
        "after focus left from container, a window must be the focus target, got {:?}",
        env.focus_target()
    );
}

#[test]
fn monitor_switch_empty_to_empty_focuses_overlay() {
    let mut env = TestEnv::new();
    env.add_monitor(second_monitor());
    env.run_actions("focus workspace 1");

    env.run_actions("focus monitor right");
    assert_eq!(env.focus_target(), FocusTarget::Overlay);
}

#[test]
fn multi_action_sequence_applies_each_hub_action() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    let actions = Actions::new(vec![
        "focus workspace 1".parse().unwrap(),
        "focus workspace 0".parse().unwrap(),
    ]);
    for action in &actions {
        match action {
            Action::Focus(t) => {
                env.dome.apply_focus(t);
                env.dome.apply_layout();
            }
            Action::Move(t) => {
                env.dome.apply_move(t);
                env.dome.apply_layout();
            }
            Action::Toggle(t) => {
                env.dome.apply_toggle(t);
                env.dome.apply_layout();
            }
            Action::Master(t) => {
                env.dome.apply_master(t);
                env.dome.apply_layout();
            }
            _ => {}
        }
    }

    assert!(!env.is_offscreen(w1));
    assert!(!env.is_offscreen(w2));
}

#[test]
fn programmatic_echo_keeps_tiling_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // Simulate OS echoing LOCATIONCHANGE for windows we just placed.
    // Both enter MoveKind::Programmatic.
    assert!(env.dome.location_changed(w1));
    assert!(env.dome.location_changed(w2));

    env.dome.apply_layout();

    // Overlay must remain visible with both tiling windows. An echo round-
    // trip must not blink the borders off.
    let TilingOverlayState::Visible { windows, .. } = env.tiling_overlays()[0].state.clone() else {
        panic!(
            "tiling overlay should be visible after programmatic echo, got {:?}",
            env.tiling_overlays()[0].state
        );
    };
    assert_eq!(windows.len(), 2);
}

#[test]
fn user_drag_keeps_tiling_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    let placed_w1 = env.dim(w1);

    env.dome.move_size_started(w1);
    env.dome.apply_layout();

    assert_eq!(env.dim(w1), placed_w1);
    // Overlay must remain visible with both tiling windows -- w2's border
    // must survive the drag.
    let TilingOverlayState::Visible { windows, .. } = env.tiling_overlays()[0].state.clone() else {
        panic!(
            "tiling overlay should be visible during drag, got {:?}",
            env.tiling_overlays()[0].state
        );
    };
    assert_eq!(windows.len(), 2);
}

#[test]
fn empty_monitor_clears_tiling_overlay() {
    let mut env = TestEnv::new();
    // No windows added. The primary monitor's tiling overlay exists from Dome::new.
    env.dome.apply_layout();

    assert!(matches!(
        env.tiling_overlays()[0].state,
        TilingOverlayState::Hidden
    ));
}

#[test]
fn dpi_reconcile_with_unchanged_scale_does_not_move_windows() {
    let mut env = TestEnv::new();
    let w = env.open(1, "App", "app.exe", SPAWN_DIM);
    let before = env.dim(w);

    env.dome.handle_dpi_change();
    env.dome.apply_layout();

    let after = env.dim(w);
    assert_eq!(after.x, before.x);
    assert_eq!(after.y, before.y);
    assert_eq!(after.width, before.width);
    assert_eq!(after.height, before.height);
}

#[test]
fn dpi_change_then_apply_layout_places_at_new_scale() {
    let mut env = TestEnv::new();
    let w = env.open(1, "App", "app.exe", SPAWN_DIM);

    let before = env.dim(w);
    assert!(before.width > Length::new(0.0));

    let mut scaled = default_monitor();
    scaled.scale = 1.5;
    *env.monitors.lock().unwrap() = vec![scaled];
    env.dome.handle_dpi_change();
    env.dome.apply_layout();

    let after = env.dim(w);
    // Frames are physical pixels: a DPI change scales the border but not the work area.
    let border = Length::from_pixels(env.config.border_size).to_unit(1.0);
    let expected_x = before.x * 1.5;
    let expected_y = before.y * 1.5;
    let expected_w = before.width - border;
    let expected_h = before.height - border;

    assert_eq!(after.x, expected_x);
    assert_eq!(after.y, expected_y);
    assert_eq!(after.width, expected_w);
    assert_eq!(after.height, expected_h);
}

#[test]
fn handle_dpi_change_on_secondary_monitor_updates_secondary_only() {
    let mut second = second_monitor();
    second.scale = 1.0;
    let mut env = TestEnv::new_with_monitors(
        Config::default(),
        LayoutConfig::default(),
        vec![default_monitor(), second],
    );

    let w_a = env.open(1, "WinA", "a.exe", SPAWN_DIM);
    let before_a = env.dim(w_a);

    env.run_actions("focus monitor right");
    let w_b = env.open(2, "WinB", "b.exe", SPAWN_DIM);
    let before_b = env.dim(w_b);

    let mut scaled = second_monitor();
    scaled.scale = 2.0;
    *env.monitors.lock().unwrap() = vec![default_monitor(), scaled];
    env.dome.handle_dpi_change();
    env.dome.apply_layout();

    let after_a = env.dim(w_a);
    assert_eq!(after_a.x, before_a.x);
    assert_eq!(after_a.y, before_a.y);
    assert_eq!(after_a.width, before_a.width);
    assert_eq!(after_a.height, before_a.height);

    // Frames are physical pixels: a DPI change scales the border but not the work area.
    let after_b = env.dim(w_b);
    let border = Length::from_pixels(env.config.border_size).to_unit(1.0);
    let expected_x = before_b.x + border;
    let expected_y = before_b.y + border;
    let expected_w = before_b.width - border * 2.0;
    let expected_h = before_b.height - border * 2.0;
    assert!(
        (after_b.x - expected_x).abs() < Length::new(2.0),
        "x: expected ~{expected_x}, got {}",
        after_b.x
    );
    assert!(
        (after_b.y - expected_y).abs() < Length::new(2.0),
        "y: expected ~{expected_y}, got {}",
        after_b.y
    );
    assert!(
        (after_b.width - expected_w).abs() < Length::new(2.0),
        "w: expected ~{expected_w}, got {}",
        after_b.width
    );
    assert!(
        (after_b.height - expected_h).abs() < Length::new(2.0),
        "h: expected ~{expected_h}, got {}",
        after_b.height
    );
}

#[test]
fn tab_bar_lifecycle_per_container() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.run_actions("toggle layout");

    {
        let tab_bars = env.tab_bars.borrow();
        assert_eq!(tab_bars.len(), 1);
        let mock = tab_bars.values().next().unwrap();
        let upd = mock.last_update().expect("tab bar received an update");
        assert_eq!(upd.titles.len(), 2);
        assert!(upd.active_index < 2);
    }

    env.run_actions("toggle layout");
    assert!(env.tab_bars.borrow().is_empty());
}

#[test]
fn tab_click_focuses_tab_index() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.run_actions("toggle layout");

    let cid = *env.tab_bars.borrow().keys().next().unwrap();
    let initial_active = env
        .tab_bars
        .borrow()
        .get(&cid)
        .unwrap()
        .last_update()
        .unwrap()
        .active_index;
    assert_eq!(initial_active, 1);

    env.dome.tab_clicked(cid, 0);

    let after_active = env
        .tab_bars
        .borrow()
        .get(&cid)
        .unwrap()
        .last_update()
        .unwrap()
        .active_index;
    assert_eq!(after_active, 0);
}

#[test]
fn primary_monitor_answers_to_its_display_name() {
    let env = TestEnv::new();

    let monitors = env.dome.query_monitors_json();
    assert!(monitors.contains("\"unique_name\":\"Test\""), "{monitors}");

    let workspaces = env.dome.query_workspaces_json();
    assert!(workspaces.contains("\"monitor\":\"Test\""), "{workspaces}");
    assert!(!workspaces.contains("primary"), "{workspaces}");
}

#[test]
fn primary_change_to_a_new_display_carries_the_workspaces() {
    let mut env = TestEnv::new();

    let mut demoted = default_monitor();
    demoted.is_primary = false;
    let mut promoted = second_monitor();
    promoted.is_primary = true;
    *env.monitors.lock().unwrap() = vec![demoted, promoted];
    env.dome.handle_display_change();
    env.dome.apply_layout();

    let workspaces = env.dome.query_workspaces_json();
    assert!(
        workspaces.contains("\"monitor\":\"External\""),
        "{workspaces}"
    );
}

#[test]
fn primary_change_to_a_tracked_display_parks_the_displaced_workspaces() {
    let mut env = TestEnv::new();
    env.add_monitor(second_monitor());

    let mut demoted = default_monitor();
    demoted.is_primary = false;
    let mut promoted = second_monitor();
    promoted.is_primary = true;
    *env.monitors.lock().unwrap() = vec![demoted, promoted];
    env.dome.handle_display_change();
    env.dome.apply_layout();

    let workspaces = env.dome.query_workspaces_json();
    assert!(workspaces.contains("\"state\":\"Parked\""), "{workspaces}");
    assert!(
        workspaces.contains("\"monitor\":\"External\""),
        "{workspaces}"
    );
}
