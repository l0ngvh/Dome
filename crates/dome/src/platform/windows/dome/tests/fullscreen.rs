use super::super::window::MAX_DRIFT_RETRIES;
use super::{PARTITION_CONFIG, fixture::TestDome};
use crate::platform::windows::window_server::{PlacementObservation, WindowNotification};
use dome_core::PixelRect;
use std::time::{Duration, Instant};

#[test]
fn admit_window_beneath_fullscreen_hides_until_fullscreen_exits() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let fullscreen = dome.open("Fullscreen");
    dome.run("toggle fullscreen");
    dome.complete_placement_observations(1);
    let covered = dome.open("Covered");
    assert!(dome.frame(covered).x().value() < 0);
    assert_eq!(dome.frame(fullscreen), PixelRect::new(0, 0, 100, 20));
    assert_eq!(dome.focused(), Some(fullscreen));
    assert!(!dome.in_taskbar(covered));
    dome.run("toggle fullscreen");
    assert_eq!(dome.frame(fullscreen), PixelRect::new(1, 1, 48, 18));
    assert_eq!(dome.frame(covered), PixelRect::new(51, 1, 48, 18));
    assert!(dome.in_taskbar(covered));
    assert_eq!(dome.focused(), Some(fullscreen));
    dome.run("toggle float");
    assert!(dome.overlays().floats.contains_key(&fullscreen));
}

#[test]
fn resize_to_native_fullscreen_blocks_float_until_exit() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    dome.moved(window, PixelRect::new(0, 0, 100, 20), 1);
    dome.run("toggle float");
    assert_eq!(dome.frame(window), PixelRect::new(0, 0, 100, 20));
    assert!(dome.overlays().floats.is_empty());
    dome.moved(window, PixelRect::new(5, 5, 40, 10), 1);
    dome.run("toggle float");
    assert!(dome.overlays().floats.contains_key(&window));
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));
}

#[test]
fn return_to_workspace_after_borderless_resurface_exhaustion_restores_fullscreen_and_retry_budget()
{
    let mut dome = TestDome::new(PARTITION_CONFIG);
    dome.run("focus workspace home");
    let window = dome.open("Fullscreen");
    let fullscreen = PixelRect::new(0, 0, 100, 20);
    dome.moved(window, fullscreen, 1);
    dome.run("focus workspace other");
    assert!(dome.minimized(window));
    assert!(!dome.in_taskbar(window));
    dome.deliver_notification(WindowNotification::PlacementChanged {
        window_id: window,
        observed_at: Instant::now() - Duration::from_secs(5),
        placement: Some(PlacementObservation::Visible {
            rect: fullscreen,
            monitor: 1,
        }),
        constraints: None,
    });
    assert!(dome.minimized(window));
    dome.run("focus workspace home");
    assert!(!dome.minimized(window));
    assert_eq!(dome.frame(window), fullscreen);
    assert!(dome.in_taskbar(window));

    dome.run("focus workspace other");
    for _ in 0..MAX_DRIFT_RETRIES {
        dome.change(window, |window| window.state.minimized = false);
        dome.moved(window, fullscreen, 1);
        assert!(dome.minimized(window));
    }
    for _ in 0..2 {
        dome.change(window, |window| window.state.minimized = false);
        dome.moved(window, fullscreen, 1);
        assert!(!dome.minimized(window));
    }
    dome.run("focus workspace home");
    assert_eq!(dome.frame(window), fullscreen);
    assert!(dome.in_taskbar(window));
    dome.run("focus workspace other");
    dome.change(window, |window| window.state.minimized = false);
    dome.moved(window, fullscreen, 1);
    assert!(dome.minimized(window));
    dome.run("focus workspace home");
    assert!(!dome.minimized(window));
    assert_eq!(dome.frame(window), fullscreen);
}

#[test]
fn shutdown_with_hidden_fullscreen_restores_dome_minimized_window_but_preserves_user_minimize() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let fullscreen = dome.open("Fullscreen");
    let minimized = dome.open("User minimized");
    dome.change(minimized, |window| window.state.minimized = true);
    dome.deliver_notification(WindowNotification::WindowMinimized(minimized));
    dome.moved(fullscreen, PixelRect::new(0, 0, 100, 20), 1);
    dome.run("focus workspace other");
    assert!(dome.minimized(fullscreen));
    assert!(dome.minimized(minimized));
    let server = dome.server.clone();
    drop(dome);
    assert!(!server.observe(fullscreen).state.minimized);
    assert!(server.observe(minimized).state.minimized);
    assert!(server.in_taskbar(fullscreen));
    assert!(server.in_taskbar(minimized));
    assert_eq!(
        server.observe(fullscreen).state.window_rect,
        PixelRect::new(0, 0, 100, 20)
    );
}
