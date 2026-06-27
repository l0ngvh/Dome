use std::sync::Arc;

use super::*;
use crate::config::{Config, PartitionTreeConfig, WindowsOnOpenRule, WindowsWindow};

#[test]
fn window_destroyed_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.destroy_window(w1);

    assert!(!env.is_offscreen(w2));
    assert_h_tiled(
        &[env.dim(w2)],
        default_monitor().dimension,
        env.config.border_size,
    );
}

#[test]
fn window_minimized_removes_from_tiling() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.minimize_window(w2);

    // w1 should now fill the screen
    assert_h_tiled(
        &[env.dim(w1)],
        default_monitor().dimension,
        env.config.border_size,
    );
    // w2 should be in the minimize picker, not deleted
    env.run_actions("toggle minimized");
    assert_eq!(env.picker.borrow().entries.borrow().len(), 1);
}

#[test]
fn user_minimize_then_restore() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.minimize_window(w2);
    env.run_actions("toggle minimized");
    assert_eq!(env.picker.borrow().entries.borrow().len(), 1);
    env.run_actions("toggle minimized"); // hide

    env.unminimize_window(w2);
    env.run_actions("toggle minimized"); // show again with fresh entries
    assert_eq!(env.picker.borrow().entries.borrow().len(), 0);
    // Both windows should be tiled again
    assert_h_tiled(
        &[env.dim(w1), env.dim(w2)],
        default_monitor().dimension,
        env.config.border_size,
    );
}

#[test]
fn on_open_moves_window_to_workspace() {
    let mut config = Config::default();
    config.windows.on_open.push(WindowsOnOpenRule {
        process: Some("slack.exe".to_string()),
        title: None,
        class: None,
        aumid: None,
        mode: None,
        workspace: Some("3".to_string()),
    });
    let mut env = TestEnv::new_with_config(config);

    let w1 = env.open(1, "Terminal", "terminal.exe", SPAWN_DIM);
    let w2 = env.open(2, "Slack", "slack.exe", SPAWN_DIM);

    // Slack moved to workspace 3, should be offscreen
    assert!(env.is_offscreen(w2));
    assert!(!env.is_offscreen(w1));
}

#[test]
fn move_size_suppresses_placement() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    let placed = env.dim(w1);

    // Simulate user starting a drag
    env.dome.move_size_started(w1);

    // Add a second window -- triggers relayout, but w1 should be skipped
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // w1 should still be at its original position (drag suppresses placement)
    assert_eq!(env.dim(w1), placed);

    // End drag -- w1 should be repositioned on next layout
    env.dome.clear_move_state(w1);
    env.dome.apply_layout();

    // Now both should be tiled
    assert!(!env.is_offscreen(w1));
    assert!(!env.is_offscreen(w2));
}

#[test]
fn monitors_changed_updates_layout() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    let before = env.dim(w1);

    // Monitor shrinks
    let new_monitor = MonitorInfo {
        handle: 1,
        name: "Test".to_string(),
        dimension: Dimension::new(
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
    config.windows.ignore.push(WindowsWindow {
        process: Some("bloat.exe".to_string()),
        title: None,
        class: None,
        aumid: None,
    });
    let mut env = TestEnv::new_with_config(config);

    let w1 = env.open(1, "Bloat", "bloat.exe", SPAWN_DIM);

    assert_eq!(env.dim(w1), SPAWN_DIM);
}

#[test]
fn ignored_window_rule_by_class_prevents_insertion() {
    let mut config = Config::default();
    config.windows.ignore.push(WindowsWindow {
        process: None,
        title: None,
        class: Some("Shell_TrayWnd".to_string()),
        aumid: None,
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
        default_monitor().dimension,
        env.config.border_size,
    );
}

#[test]
fn delete_currently_displayed_window() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.destroy_window(w1);

    // Remaining window fills screen
    assert!(!env.is_offscreen(w2));
    assert_h_tiled(
        &[env.dim(w2)],
        default_monitor().dimension,
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
    let mut env = TestEnv::new_with_layout(
        Config::default(),
        LayoutConfig {
            partition_tree: PartitionTreeConfig {
                automatic_tiling: false,
                tab_bar_height: Length::new(24.0),
            },
            ..Default::default()
        },
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
            Action::ToggleMinimized => env.dome.toggle_picker(),
            _ => {}
        }
    }

    // After "focus ws 1, focus ws 0", workspace 0 is focused and windows are visible
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

    // Overlay must remain visible with both tiling windows; an echo round-
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

    // Dragged window should not have been repositioned.
    assert_eq!(env.dim(w1), placed_w1);
    // Overlay must remain visible with both tiling windows; w2's border
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

    // Monitor has zero tiling windows and zero containers, so the overlay
    // must be hidden.
    assert!(matches!(
        env.tiling_overlays()[0].state,
        TilingOverlayState::Hidden
    ));
}

#[test]
fn monitor_dpi_changed_unknown_handle_noop() {
    let mut env = TestEnv::new();
    let w = env.open(1, "App", "app.exe", SPAWN_DIM);
    let before = env.dim(w);

    // Call with a bogus handle; should not panic or change placement.
    env.dome.monitor_dpi_changed(0xDEAD_BEEF_u64 as isize, 192);
    env.dome.apply_layout();

    let after = env.dim(w);
    assert_eq!(after.x, before.x);
    assert_eq!(after.y, before.y);
    assert_eq!(after.width, before.width);
    assert_eq!(after.height, before.height);
}

#[test]
fn monitor_dpi_changed_same_scale_is_noop() {
    let mut env = TestEnv::new();
    let w = env.open(1, "App", "app.exe", SPAWN_DIM);
    let before = env.dim(w);

    // DPI 96 == scale 1.0, same as the fixture default. Placement must not change.
    env.dome.monitor_dpi_changed(1, 96);
    env.dome.apply_layout();
    let after1 = env.dim(w);
    assert_eq!(after1.x, before.x);
    assert_eq!(after1.y, before.y);
    assert_eq!(after1.width, before.width);
    assert_eq!(after1.height, before.height);

    // Call again with the same DPI; still a no-op.
    env.dome.monitor_dpi_changed(1, 96);
    env.dome.apply_layout();
    let after2 = env.dim(w);
    assert_eq!(after2.x, before.x);
    assert_eq!(after2.y, before.y);
    assert_eq!(after2.width, before.width);
    assert_eq!(after2.height, before.height);
}

#[test]
fn dpi_change_then_apply_layout_places_at_new_scale() {
    let mut env = TestEnv::new();
    let w = env.open(1, "App", "app.exe", SPAWN_DIM);

    let before = env.dim(w);
    assert!(before.width > Length::new(0.0));

    // Change primary monitor from 96 DPI (1.0x) to 144 DPI (1.5x).
    env.dome.monitor_dpi_changed(1, 144);
    env.dome.apply_layout();

    let after = env.dim(w);
    // Hub delivers frames in physical pixels. DPI change scales border but
    // not the physical monitor dimension, so the content rect shrinks by
    // 2 * border * (scale - 1) per axis.
    let border = Length::new(env.config.border_size);
    let expected_x = (before.x * 1.5).round();
    let expected_y = (before.y * 1.5).round();
    let expected_w = (before.width - border).round();
    let expected_h = (before.height - border).round();

    assert!(
        (after.x - expected_x).abs() < Length::new(2.0),
        "x: expected ~{expected_x}, got {}",
        after.x
    );
    assert!(
        (after.y - expected_y).abs() < Length::new(2.0),
        "y: expected ~{expected_y}, got {}",
        after.y
    );
    assert!(
        (after.width - expected_w).abs() < Length::new(2.0),
        "w: expected ~{expected_w}, got {}",
        after.width
    );
    assert!(
        (after.height - expected_h).abs() < Length::new(2.0),
        "h: expected ~{expected_h}, got {}",
        after.height
    );
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

    // Add one window on primary.
    let w_a = env.open(1, "WinA", "a.exe", SPAWN_DIM);
    let before_a = env.dim(w_a);

    // Add one window on secondary.
    env.run_actions("focus monitor right");
    let w_b = env.open(2, "WinB", "b.exe", SPAWN_DIM);
    let before_b = env.dim(w_b);

    // Simulate DPI change only on secondary (192 DPI = 2.0x).
    env.dome.monitor_dpi_changed(2, 192);
    env.dome.apply_layout();

    // Primary window placement must be unchanged (scale stayed 1.0).
    let after_a = env.dim(w_a);
    assert_eq!(after_a.x, before_a.x);
    assert_eq!(after_a.y, before_a.y);
    assert_eq!(after_a.width, before_a.width);
    assert_eq!(after_a.height, before_a.height);

    // Secondary window placement must reflect the 2.0x scale change.
    // Hub delivers frames in physical pixels. DPI change scales border but
    // not the physical monitor dimension, so the content rect shifts by
    // border * (new_scale - old_scale) per axis and shrinks by 2 * that.
    let after_b = env.dim(w_b);
    let border = Length::new(env.config.border_size);
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
