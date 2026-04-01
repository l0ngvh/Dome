use std::sync::Arc;

use crate::config::{Config, WindowsOnOpenRule, WindowsWindow};
use crate::platform::windows::external::ManageExternalHwnd;

use super::*;

#[test]
fn window_destroyed_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.destroy_window(&w1);

    assert!(!w2.is_offscreen());
    assert_h_tiled(
        &[w2.get_dim()],
        default_screen().dimension,
        env.dome.config().border_size,
    );
}

#[test]
fn window_minimized_removes_from_tiling() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.minimize_window(&w2);

    // w1 should now fill the screen
    assert_h_tiled(
        &[w1.get_dim()],
        default_screen().dimension,
        env.dome.config().border_size,
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

    let w1 = Arc::new(MockExternalHwnd::with_title(1, "Terminal", "terminal.exe"));
    env.add_window(w1.clone());

    let w2 = Arc::new(MockExternalHwnd::with_title(2, "Slack", "slack.exe"));
    env.add_window(w2.clone());

    // Slack moved to workspace 3, should be offscreen
    assert!(w2.is_offscreen());
    assert!(!w1.is_offscreen());
}

#[test]
fn move_size_suppresses_placement() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    env.add_window(w1.clone());

    let placed = w1.get_dim();

    // Simulate user starting a drag
    env.dome
        .move_size_started(w1.clone() as Arc<dyn ManageExternalHwnd>);

    // Add a second window — triggers relayout, but w1 should be skipped
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w2.clone());

    // w1 should still be at its original position (drag suppresses placement)
    assert_eq!(w1.get_dim(), placed);

    // End drag — w1 should be repositioned on next layout
    env.dome
        .move_size_ended(w1.clone() as Arc<dyn ManageExternalHwnd>);
    env.dome.apply_layout();

    // Now both should be tiled
    assert!(!w1.is_offscreen());
    assert!(!w2.is_offscreen());
}

#[test]
fn screens_changed_updates_layout() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
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
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe").with_manageable(false));
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

    let w1 = Arc::new(MockExternalHwnd::with_title(1, "Bloat", "bloat.exe"));
    let initial = w1.get_dim();
    env.add_window(w1.clone());

    assert_eq!(w1.get_dim(), initial);
}

#[test]
fn title_changed_manages_unknown_window() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));

    // Title change on an unknown window should try to manage it
    env.dome
        .title_changed(w1.clone() as Arc<dyn ManageExternalHwnd>);
    env.dome.apply_layout();

    assert!(!w1.is_offscreen());
    assert_h_tiled(
        &[w1.get_dim()],
        default_screen().dimension,
        env.dome.config().border_size,
    );
}
