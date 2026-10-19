use super::fixture::{PARTITION_CONFIG, TestDome, monitor};
use crate::platform::macos::window_server::{DebounceBurst, WindowNotification};
use dome_core::PixelRect;
use std::time::{Duration, Instant};

#[test]
fn resist_placement_exhausts_retries_and_new_target_rearms_them() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    let drift = PixelRect::new(8, 4, 30, 10);
    dome.resist_frame(window, drift);
    let target = dome.frame(window);
    let initial = dome.frame_requests(window).len();
    for _ in 0..8 {
        dome.moved(window, drift);
    }
    assert_eq!(&dome.frame_requests(window)[initial..], &[target; 5]);
    assert_eq!(dome.frame(window), drift);
    let peer = dome.open("Peer");
    let changed = dome.frame_requests(window);
    assert_eq!(changed.len(), initial + 6);
    let target = PixelRect::new(1, 1, 48, 18);
    assert_eq!(changed.last(), Some(&target));
    dome.moved(window, drift);
    assert_eq!(&dome.frame_requests(window)[changed.len()..], &[target]);
    dome.remove(peer);
}

#[test]
fn resurface_parked_window_stops_hiding_after_retry_budget_then_rearms_on_show() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    dome.run("focus workspace first");
    let window = dome.open("Window");
    dome.run("focus workspace second");
    let drift = PixelRect::new(8, 4, 30, 10);
    for _ in 0..5 {
        dome.moved(window, drift);
        assert_eq!(dome.frame(window), PixelRect::new(99, 19, 30, 10));
    }
    for _ in 0..3 {
        dome.moved(window, drift);
        assert_eq!(dome.frame(window), drift);
    }
    dome.run("focus workspace first");
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));
    dome.run("focus workspace second");
    dome.moved(window, drift);
    assert_eq!(dome.frame(window), PixelRect::new(99, 19, 30, 10));
}

#[test]
fn resurface_borderless_window_stops_minimizing_after_retry_budget() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    dome.run("focus workspace first");
    let window = dome.open("Window");
    let fullscreen = PixelRect::new(0, 0, 100, 20);
    dome.moved(window, fullscreen);
    dome.run("focus workspace second");
    assert!(dome.minimized(window));
    for _ in 0..5 {
        dome.moved(window, fullscreen);
        assert!(dome.minimized(window));
    }
    for _ in 0..3 {
        dome.moved(window, fullscreen);
        assert!(!dome.minimized(window));
    }
    dome.moved(window, PixelRect::new(10, 4, 30, 8));
    assert_eq!(dome.frame(window), PixelRect::new(99, 19, 30, 8));
    dome.run("focus workspace first");
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));
}

#[test]
fn move_across_monitors_ignores_clamped_echo_as_size_constraint() {
    let mut dome = TestDome::new(&PARTITION_CONFIG.replace("\"partition_tree\"", "\"master\""));
    let window = dome.open("Window");
    dome.monitors_changed(vec![monitor(1, 0.0), monitor(2, 100.0)]);
    dome.run("move monitor right");
    dome.run("focus monitor right");
    let target = dome.frame(window);
    assert_eq!(target, PixelRect::new(101, 1, 98, 18));
    dome.moved(window, PixelRect::new(101, 1, 40, 18));
    let mut destination = monitor(2, 100.0);
    destination.width = 120.0;
    dome.monitors_changed(vec![monitor(1, 0.0), destination]);
    assert_eq!(dome.frame(window), PixelRect::new(101, 1, 118, 18));
}

#[test]
fn end_movement_resumes_deferred_placement() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    let original = dome.frame(window);
    dome.deliver_notification(WindowNotification::MovementStarted {
        pid: dome.pid(window),
    });
    let peer = dome.open("Peer");
    assert_eq!(dome.frame(window), original);
    dome.moved(window, original);
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 48, 18));
    assert_eq!(dome.frame(peer), PixelRect::new(51, 1, 48, 18));
}

#[test]
fn observe_movement_when_stale_does_not_teach_obsolete_constraint() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    let mut stale = dome.app_observation(dome.pid(window));
    let peer = dome.open("Peer");
    let now = Instant::now();
    stale.requested.get_mut(&window).unwrap().rect = Some(PixelRect::new(1, 1, 20, 18));
    dome.deliver_notification(WindowNotification::Movement {
        app: stale,
        observed_at: DebounceBurst {
            first: now - Duration::from_secs(10),
            last: now - Duration::from_secs(5),
        },
    });
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 48, 18));
    dome.remove(peer);
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));
}

#[test]
fn resize_window_at_maximum_width_learns_limit_and_retains_it_after_display_change() {
    let mut dome = TestDome::new(&PARTITION_CONFIG.replace("\"partition_tree\"", "\"master\""));
    dome.open("Master");
    let window = dome.open("Secondary");
    let original = dome.frame(window);
    let width = original.width().value() - 10;
    dome.maximum_size(window, width, 100);
    dome.moved(
        window,
        PixelRect::new(
            original.x().value(),
            original.y().value(),
            width,
            original.height().value(),
        ),
    );
    let constrained = dome.frame(window);
    assert_eq!(constrained.width().value(), width);
    assert!(constrained.x() > original.x());
    dome.monitors_changed(vec![monitor(1, 0.0), monitor(2, 100.0)]);
    assert_eq!(dome.frame(window), constrained);
}
