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
    env.run_actions("toggle minimize_picker");
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
    env.run_actions("toggle minimize_picker");
    assert_eq!(env.picker_entries.borrow().len(), 1);
    env.run_actions("toggle minimize_picker"); // hide

    env.restore_window(&w2);
    env.run_actions("toggle minimize_picker"); // show again with fresh entries
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
    env.reset_overlay_focus();

    env.destroy_window(&w1);
    assert_eq!(env.overlay_focus_count(), 1);
}

#[test]
fn destroy_one_of_two_windows_does_not_focus_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.reset_overlay_focus();

    env.destroy_window(&w2);
    assert_eq!(env.overlay_focus_count(), 0);
}

#[test]
fn workspace_switch_to_empty_focuses_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.reset_overlay_focus();

    env.run_actions("focus workspace 1");
    assert_eq!(env.overlay_focus_count(), 1);
}

#[test]
fn workspace_switch_back_does_not_focus_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());
    env.reset_overlay_focus();

    env.run_actions("focus workspace 1");
    env.reset_overlay_focus();
    env.run_actions("focus workspace 0");
    assert_eq!(env.overlay_focus_count(), 0);
}

#[test]
fn focus_parent_focuses_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.reset_overlay_focus();

    env.run_actions("focus parent");
    assert_eq!(env.overlay_focus_count(), 1);
}

#[test]
fn focus_child_after_parent_does_not_focus_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.run_actions("focus parent");
    env.reset_overlay_focus();
    env.run_actions("focus down");
    assert_eq!(env.overlay_focus_count(), 0);
}

#[test]
fn monitor_switch_empty_to_empty_focuses_overlay() {
    let mut env = TestEnv::new();
    env.add_screen(second_screen());
    env.run_actions("focus workspace 1");
    env.reset_overlay_focus();

    env.run_actions("focus monitor right");
    assert_eq!(env.overlay_focus_count(), 1);
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
