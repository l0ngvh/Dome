use std::sync::Arc;

use crate::platform::windows::external::ManageExternalHwnd;

use super::*;

#[test]
fn single_window_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    env.add_window(w1.clone());
    assert_h_tiled(
        &[w1.get_dim()],
        default_screen().dimension,
        env.dome.config().border_size,
    );
}

#[test]
fn two_windows_split_screen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    assert_h_tiled(
        &[w1.get_dim(), w2.get_dim()],
        default_screen().dimension,
        env.dome.config().border_size,
    );
}

#[test]
fn three_windows_split_screen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    let w3 = Arc::new(MockExternalHwnd::with_title(3, "App3", "app3.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.add_window(w3.clone());
    assert_h_tiled(
        &[w1.get_dim(), w2.get_dim(), w3.get_dim()],
        default_screen().dimension,
        env.dome.config().border_size,
    );
}

#[test]
fn workspace_switch_hides_and_restores() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    let placed1 = w1.get_dim();
    let placed2 = w2.get_dim();

    env.run_actions("focus workspace 1");
    assert!(w1.is_offscreen());
    assert!(w2.is_offscreen());

    env.run_actions("focus workspace 0");
    assert!(!w1.is_offscreen());
    assert!(!w2.is_offscreen());
    assert_eq!(w1.get_dim(), placed1);
    assert_eq!(w2.get_dim(), placed2);
}

#[test]
fn focus_left_right() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    // w2 is focused (last added). Focus left should move to w1.
    env.run_actions("focus left");
    // Focus right should move back to w2.
    env.run_actions("focus right");

    // Both windows should remain tiled (focus doesn't change layout)
    assert!(!w1.is_offscreen());
    assert!(!w2.is_offscreen());
}

#[test]
fn resize_detects_fullscreen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    env.add_window(w1.clone());

    let border = env.dome.config().border_size;
    let d = w1.get_dim();
    assert_eq!(d.x, border, "should start tiled with border inset");

    // Simulate the user resizing the window to fill the screen
    *w1.dimension.lock().unwrap() = Dimension {
        x: 0.0,
        y: 0.0,
        width: SCREEN_WIDTH,
        height: SCREEN_HEIGHT,
    };

    env.dome
        .move_size_ended(w1.clone() as Arc<dyn ManageExternalHwnd>);
    env.dome.apply_layout();

    // Hub should detect fullscreen — window positioned at full monitor dimensions
    let d = w1.get_dim();
    assert_eq!(d.x, 0.0);
    assert_eq!(d.y, 0.0);
    assert_eq!(d.width, SCREEN_WIDTH);
    assert_eq!(d.height, SCREEN_HEIGHT);
}
