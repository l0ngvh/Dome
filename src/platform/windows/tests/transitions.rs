use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::*;

#[test]
fn toggle_fullscreen_hides_siblings() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    // w2 is focused (last added). Toggle fullscreen.
    env.run_actions("toggle fullscreen");

    assert!(w1.is_offscreen());
    let d2 = w2.get_dim();
    assert_eq!(d2.x, 0.0);
    assert_eq!(d2.y, 0.0);
    assert_eq!(d2.width, SCREEN_WIDTH);
    assert_eq!(d2.height, SCREEN_HEIGHT);
}

#[test]
fn toggle_fullscreen_on_and_off() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    let before1 = w1.get_dim();
    let before2 = w2.get_dim();

    env.run_actions("toggle fullscreen");
    env.run_actions("toggle fullscreen");

    // Both should be back to tiled positions
    assert!(!w1.is_offscreen());
    assert!(!w2.is_offscreen());
    assert_eq!(w1.get_dim(), before1);
    assert_eq!(w2.get_dim(), before2);
}

#[test]
fn toggle_float() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    // w2 is focused. Toggle float.
    env.run_actions("toggle float");

    // w1 should fill the screen (w2 is floating, not part of tiling)
    assert!(!w1.is_offscreen());
    assert!(!w2.is_offscreen());
    let d1 = w1.get_dim();
    let border = env.dome.config().border_size;
    assert!(
        (d1.width - (SCREEN_WIDTH - 2.0 * border)).abs() < 1.0,
        "w1 should fill screen width, got {}",
        d1.width
    );
}

#[test]
fn fullscreen_restored_after_workspace_switch() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.run_actions("toggle fullscreen");
    let fs_dim = w2.get_dim();

    // Switch away
    env.run_actions("focus workspace 1");
    assert!(w2.is_offscreen());

    // Switch back — fullscreen window should be restored
    env.run_actions("focus workspace 0");
    assert_eq!(w2.get_dim(), fs_dim);
}

#[test]
fn window_created_as_fullscreen_borderless() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    env.add_window(w1.clone());

    // Second window arrives already fullscreen
    let w2 = Arc::new(
        MockExternalHwnd::with_title(2, "Game", "game.exe")
            .with_mode(WindowMode::FullscreenBorderless),
    );
    env.add_window(w2.clone());

    // w1 should be hidden (fullscreen window takes over)
    assert!(w1.is_offscreen());
}

#[test]
fn move_window_to_other_workspace() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    // Move w2 (focused) to workspace 1
    env.run_actions("move workspace 1");

    // w2 should be offscreen, w1 should fill the screen
    assert!(w2.is_offscreen());
    assert_h_tiled(
        &[w1.get_dim()],
        default_screen().dimension,
        env.dome.config().border_size,
    );
}

#[test]
fn fullscreen_borderless_minimizes_on_workspace_switch() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(
        MockExternalHwnd::with_title(1, "Game", "game.exe")
            .with_mode(WindowMode::FullscreenBorderless),
    );
    env.add_window(w1.clone());

    env.run_actions("focus workspace 1");

    assert!(w1.iconic.load(Ordering::Relaxed));
}

#[test]
fn fullscreen_exclusive_not_repositioned() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(
        MockExternalHwnd::with_title(1, "Game", "game.exe")
            .with_mode(WindowMode::FullscreenExclusive),
    );
    let initial = w1.get_dim();
    env.add_window(w1.clone());

    // FullscreenExclusive windows are never repositioned by the WM
    assert_eq!(w1.get_dim(), initial);

    // Switching workspace should also not reposition (hide_window is noop for exclusive)
    env.run_actions("focus workspace 1");
    assert_eq!(w1.get_dim(), initial);
}

#[test]
fn iconic_window_restored_before_positioning() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    env.add_window(w1.clone());

    // Simulate the window becoming iconic externally
    w1.iconic.store(true, Ordering::Relaxed);

    // Trigger relayout — show_window should restore before positioning
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w2.clone());

    assert!(!w1.iconic.load(Ordering::Relaxed));
    assert!(!w1.is_offscreen());
}
