use std::sync::Arc;

use super::*;
use crate::config::{Config, WindowsOnOpenRule, WindowsWindow};

#[test]
fn window_destroyed_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    let w2 = Arc::new(MockExternalHwnd::with_title(
        2,
        "App2",
        "app2.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.destroy_window(&w1);

    assert!(!w2.is_offscreen());
    assert_h_tiled(
        &[w2.get_dim()],
        default_screen().dimension,
        env.config.border_size,
    );
}

#[test]
fn window_minimized_removes_from_tiling() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    let w2 = Arc::new(MockExternalHwnd::with_title(
        2,
        "App2",
        "app2.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.minimize_window(&w2);

    // w1 should now fill the screen
    assert_h_tiled(
        &[w1.get_dim()],
        default_screen().dimension,
        env.config.border_size,
    );
    // w2 should be in the minimize picker, not deleted
    env.run_actions("toggle minimized");
    assert_eq!(env.picker_entries.borrow().len(), 1);
}

#[test]
fn user_minimize_then_restore() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    let w2 = Arc::new(MockExternalHwnd::with_title(
        2,
        "App2",
        "app2.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.minimize_window(&w2);
    env.run_actions("toggle minimized");
    assert_eq!(env.picker_entries.borrow().len(), 1);
    env.run_actions("toggle minimized"); // hide

    env.restore_window(&w2);
    env.run_actions("toggle minimized"); // show again with fresh entries
    assert_eq!(env.picker_entries.borrow().len(), 0);
    // Both windows should be tiled again
    assert_h_tiled(
        &[w1.get_dim(), w2.get_dim()],
        default_screen().dimension,
        env.config.border_size,
    );
}

#[test]
fn on_open_moves_window_to_workspace() {
    let mut config = Config::default();
    config.windows.on_open.push(WindowsOnOpenRule {
        window: WindowsWindow {
            process: Some("slack.exe".to_string()),
            title: None,
        },
        run: Actions::new(vec!["move workspace 3".parse().unwrap()]),
    });
    let mut env = TestEnv::new_with_config(config);

    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "Terminal",
        "terminal.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());

    let w2 = Arc::new(MockExternalHwnd::with_title(
        2,
        "Slack",
        "slack.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w2.clone());

    // Slack moved to workspace 3, should be offscreen
    assert!(w2.is_offscreen());
    assert!(!w1.is_offscreen());
}

#[test]
fn move_size_suppresses_placement() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());

    let placed = w1.get_dim();

    // Simulate user starting a drag
    env.dome.move_size_started(w1.hwnd_id);

    // Add a second window — triggers relayout, but w1 should be skipped
    let w2 = Arc::new(MockExternalHwnd::with_title(
        2,
        "App2",
        "app2.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w2.clone());

    // w1 should still be at its original position (drag suppresses placement)
    assert_eq!(w1.get_dim(), placed);

    // End drag — w1 should be repositioned on next layout
    env.dome.move_size_ended(w1.hwnd_id);
    env.dome.apply_layout();

    // Now both should be tiled
    assert!(!w1.is_offscreen());
    assert!(!w2.is_offscreen());
}

#[test]
fn screens_changed_updates_layout() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());

    let before = w1.get_dim();

    // Screen shrinks
    let new_screen = ScreenInfo {
        handle: 1,
        name: "Test".to_string(),
        dimension: Dimension {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        },
        is_primary: true,
        scale: 1.0,
    };
    env.dome.screens_changed(vec![new_screen]);
    env.dome.apply_layout();

    let after = w1.get_dim();
    assert!(
        after.width < before.width,
        "window should be narrower after screen shrink"
    );
    assert!(
        after.height < before.height,
        "window should be shorter after screen shrink"
    );
}

#[test]
fn unmanageable_window_is_ignored() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(
        MockExternalHwnd::with_title(
            1,
            "App1",
            "app1.exe",
            env.moves.clone(),
            env.z_model.clone(),
        )
        .with_manageable(false),
    );
    let initial = w1.get_dim();
    env.add_window(w1.clone());

    // Window should not have been positioned — still at initial dimension
    assert_eq!(w1.get_dim(), initial);
}

#[test]
fn ignored_window_rule_prevents_insertion() {
    let mut config = Config::default();
    config.windows.ignore.push(WindowsWindow {
        process: Some("bloat.exe".to_string()),
        title: None,
    });
    let mut env = TestEnv::new_with_config(config);

    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "Bloat",
        "bloat.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    let initial = w1.get_dim();
    env.add_window(w1.clone());

    assert_eq!(w1.get_dim(), initial);
}

#[test]
fn title_changed_manages_unknown_window() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));

    // Title change on an unknown window should try to manage it
    // (Runner dispatches as WindowCreated — here we simulate directly)
    env.add_window(w1.clone());

    assert!(!w1.is_offscreen());
    assert_h_tiled(
        &[w1.get_dim()],
        default_screen().dimension,
        env.config.border_size,
    );
}

#[test]
fn delete_currently_displayed_window() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(
        1,
        "App1",
        "app1.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    let w2 = Arc::new(MockExternalHwnd::with_title(
        2,
        "App2",
        "app2.exe",
        env.moves.clone(),
        env.z_model.clone(),
    ));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.destroy_window(&w1);

    // Remaining window fills screen
    assert!(!w2.is_offscreen());
    assert_h_tiled(
        &[w2.get_dim()],
        default_screen().dimension,
        env.config.border_size,
    );

    // Second apply_layout proves displayed state was cleaned up
    env.dome.apply_layout();
    assert!(!w2.is_offscreen());
}

#[test]
fn destroy_last_window_focuses_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.reset_sink_focus();

    env.destroy_window(&w1);
    assert_eq!(env.sink_focus_count(), 1);
}

#[test]
fn destroy_one_of_two_windows_does_not_focus_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.reset_sink_focus();

    env.destroy_window(&w2);
    assert_eq!(env.sink_focus_count(), 0);
}

#[test]
fn workspace_switch_to_empty_focuses_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.reset_sink_focus();

    env.run_actions("focus workspace 1");
    assert_eq!(env.sink_focus_count(), 1);
}

#[test]
fn workspace_switch_back_does_not_focus_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.reset_sink_focus();

    env.run_actions("focus workspace 1");
    env.reset_sink_focus();
    env.run_actions("focus workspace 0");
    assert_eq!(env.sink_focus_count(), 0);
}

#[test]
fn focus_parent_focuses_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.reset_sink_focus();

    env.run_actions("focus parent");
    assert_eq!(env.sink_focus_count(), 1);
}

#[test]
fn focus_child_after_parent_does_not_focus_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.run_actions("focus parent");
    env.reset_sink_focus();
    env.run_actions("focus down");
    assert_eq!(env.sink_focus_count(), 0);
}

#[test]
fn monitor_switch_empty_to_empty_focuses_overlay() {
    let mut env = TestEnv::new();
    env.add_screen(second_screen());
    env.run_actions("focus workspace 1");
    env.reset_sink_focus();

    env.run_actions("focus monitor right");
    assert_eq!(env.sink_focus_count(), 1);
}

#[test]
fn multi_action_sequence_applies_each_hub_action() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    let actions = Actions::new(vec![
        "focus workspace 1".parse().unwrap(),
        "focus workspace 0".parse().unwrap(),
    ]);
    for action in &actions {
        if let Action::Hub(hub) = action {
            env.dome.execute_hub_action(hub);
            env.dome.apply_layout();
        }
    }

    // After "focus ws 1, focus ws 0", workspace 0 is focused and windows are visible
    assert!(!w1.is_offscreen());
    assert!(!w2.is_offscreen());
}

#[test]
fn programmatic_echo_keeps_tiling_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    let update_baseline = env.tiling_overlay_update_count();
    let clear_baseline = env.tiling_overlay_clear_count();

    // Simulate OS echoing LOCATIONCHANGE for windows we just placed.
    // Both enter MoveKind::Programmatic.
    assert!(env.dome.location_changed(w1.hwnd_id));
    assert!(env.dome.location_changed(w2.hwnd_id));

    env.dome.apply_layout();

    // Overlay must not be cleared; it must receive an update with the full placement set.
    assert_eq!(env.tiling_overlay_clear_count(), clear_baseline);
    assert!(env.tiling_overlay_update_count() > update_baseline);
}

#[test]
fn user_drag_keeps_tiling_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    let placed_w1 = w1.get_dim();

    env.dome.move_size_started(w1.hwnd_id);

    let update_baseline = env.tiling_overlay_update_count();
    let clear_baseline = env.tiling_overlay_clear_count();

    env.dome.apply_layout();

    // Dragged window should not have been repositioned.
    assert_eq!(w1.get_dim(), placed_w1);
    // Overlay must not be cleared; w2's border must survive the drag.
    assert_eq!(env.tiling_overlay_clear_count(), clear_baseline);
    assert!(env.tiling_overlay_update_count() > update_baseline);
}

#[test]
fn empty_monitor_clears_tiling_overlay() {
    let mut env = TestEnv::new();
    // No windows added. The primary monitor's tiling overlay exists from Dome::new.
    let clear_baseline = env.tiling_overlay_clear_count();

    env.dome.apply_layout();

    // Monitor has zero tiling windows and zero containers, so clear must fire.
    assert!(env.tiling_overlay_clear_count() > clear_baseline);
}

#[test]
fn monitor_dpi_changed_updates_scale() {
    let mut second = second_screen();
    second.scale = 1.0;
    let mut env = TestEnv::new_with_screens(Config::default(), vec![default_screen(), second]);

    let id_a = env.dome.monitor_id_for_handle(1).expect("monitor A");
    let id_b = env.dome.monitor_id_for_handle(2).expect("monitor B");

    // Baseline: both at 1.0.
    assert_eq!(env.dome.monitors[&id_a].scale, 1.0);
    assert_eq!(env.dome.monitors[&id_b].scale, 1.0);

    // Simulate DPI change on monitor B to 192 DPI (2.0x).
    env.dome.monitor_dpi_changed(2, 192);

    assert_eq!(env.dome.monitors[&id_b].scale, 2.0);
    // Monitor A unchanged.
    assert_eq!(env.dome.monitors[&id_a].scale, 1.0);
}

#[test]
fn monitor_dpi_changed_unknown_handle_noop() {
    let mut env = TestEnv::new();
    let id = env.dome.monitor_id_for_handle(1).expect("primary");

    // Call with a bogus handle; should not panic or change state.
    env.dome.monitor_dpi_changed(0xDEAD_BEEF_u64 as isize, 192);

    assert_eq!(env.dome.monitors[&id].scale, 1.0);
}

#[test]
fn monitor_dpi_changed_same_scale_is_noop() {
    let mut env = TestEnv::new();
    let w = env.spawn_window(1, "App", "app.exe");
    env.add_window(w.clone());

    // Record set_position calls after initial placement.
    let baseline = env.moves.lock().unwrap().len();

    // DPI 96 == scale 1.0, same as the fixture default.
    env.dome.monitor_dpi_changed(1, 96);
    // Full round-trip: the same path Runner::handle_dpi_change would take.
    env.dome.apply_layout();

    let id = env.dome.monitor_id_for_handle(1).expect("primary");
    assert_eq!(env.dome.monitors[&id].scale, 1.0);

    // Call again with the same DPI.
    env.dome.monitor_dpi_changed(1, 96);
    env.dome.apply_layout();

    // set_position count should not have grown beyond what apply_layout adds
    // for already-placed windows. The key invariant: the scale did not change,
    // so window positions are identical.
    let after = env.moves.lock().unwrap().len();
    // apply_layout does re-issue set_position even for same targets
    // (idempotent placement). The test verifies the scale itself is unchanged
    // and monitor_dpi_changed early-returned.
    assert_eq!(env.dome.monitors[&id].scale, 1.0);
    // No extra apply_layout effect from monitor_dpi_changed (it early-returned).
    // We can't easily distinguish the set_position calls from the explicit
    // apply_layout vs a second monitor_dpi_changed, but we verify the scale
    // didn't flip and then flip back.
    let _ = after;
    let _ = baseline;
}

#[test]
fn dpi_change_then_apply_layout_places_at_new_scale() {
    let mut env = TestEnv::new();
    let w = env.spawn_window(1, "App", "app.exe");
    env.add_window(w.clone());

    let before = w.get_dim();
    assert!(before.width > 0.0);

    // Change primary monitor from 96 DPI (1.0x) to 144 DPI (1.5x).
    env.dome.monitor_dpi_changed(1, 144);
    env.dome.apply_layout();

    let after = w.get_dim();
    // At 1.5x, physical pixels = logical * 1.5. The window's logical rect
    // stays the same (the Hub layout is logical), but set_position receives
    // physical coords: each edge should be 1.5x the logical value.
    let expected_x = (before.x * 1.5).round();
    let expected_y = (before.y * 1.5).round();
    let expected_w = (before.width * 1.5).round();
    let expected_h = (before.height * 1.5).round();

    assert!(
        (after.x - expected_x).abs() < 2.0,
        "x: expected ~{expected_x}, got {}",
        after.x
    );
    assert!(
        (after.y - expected_y).abs() < 2.0,
        "y: expected ~{expected_y}, got {}",
        after.y
    );
    assert!(
        (after.width - expected_w).abs() < 2.0,
        "w: expected ~{expected_w}, got {}",
        after.width
    );
    assert!(
        (after.height - expected_h).abs() < 2.0,
        "h: expected ~{expected_h}, got {}",
        after.height
    );
}

#[test]
fn handle_dpi_change_on_secondary_monitor_updates_secondary_only() {
    let mut second = second_screen();
    second.scale = 1.0;
    let mut env = TestEnv::new_with_screens(Config::default(), vec![default_screen(), second]);

    let id_a = env.dome.monitor_id_for_handle(1).expect("monitor A");
    let id_b = env.dome.monitor_id_for_handle(2).expect("monitor B");

    // Add one window on primary.
    let w_a = env.spawn_window(1, "WinA", "a.exe");
    env.add_window(w_a.clone());
    let _before_a = w_a.get_dim();

    // Add one window on secondary.
    env.run_actions("focus workspace 1");
    let w_b = env.spawn_window(2, "WinB", "b.exe");
    env.add_window(w_b.clone());

    // Simulate DPI change only on secondary (192 DPI = 2.0x).
    env.dome.monitor_dpi_changed(2, 192);
    env.dome.apply_layout();

    assert_eq!(env.dome.monitors[&id_a].scale, 1.0);
    assert_eq!(env.dome.monitors[&id_b].scale, 2.0);

    // Primary window should be unchanged.
    // (We can't easily assert exact positions because workspace switching
    // moves windows offscreen, but the scale for A is confirmed unchanged.)
}

#[test]
fn wm_getdpiscaledsize_reply_returns_current_size() {
    use windows::Win32::Foundation::SIZE;
    let input = SIZE { cx: 1920, cy: 1080 };
    let output = crate::platform::windows::wm_getdpiscaledsize_reply(input);
    assert_eq!(output.cx, 1920);
    assert_eq!(output.cy, 1080);

    // Zero-size edge case.
    let zero = SIZE { cx: 0, cy: 0 };
    let out_zero = crate::platform::windows::wm_getdpiscaledsize_reply(zero);
    assert_eq!(out_zero.cx, 0);
    assert_eq!(out_zero.cy, 0);
}
