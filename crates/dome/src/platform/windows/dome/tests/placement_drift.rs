use super::super::window::MAX_DRIFT_RETRIES;
use super::{
    PARTITION_CONFIG,
    fixture::{TestDome, monitor},
};
use crate::platform::windows::ui::FloatBorderState;
use crate::platform::windows::window_server::{PlacementObservation, WindowNotification};
use dome_core::PixelRect;
use std::time::{Duration, Instant};

#[test]
fn move_during_drag_defers_tiling_until_settlement() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    let original = dome.frame(window);
    dome.deliver_notification(WindowNotification::MovementStarted(window));
    let peer = dome.open("Peer");
    assert_eq!(dome.frame(window), original);
    assert_eq!(dome.frame(peer), PixelRect::new(51, 1, 48, 18));
    dome.moved(window, original, 1);
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 48, 18));

    dome.run("toggle layout");
    let borders = |dome: &TestDome| {
        let state = dome.overlays();
        let tiling = state
            .requested
            .as_ref()
            .unwrap()
            .tiling
            .values()
            .next()
            .unwrap();
        (
            tiling
                .windows
                .iter()
                .map(|show| (show.placement.id, show.placement.border_box))
                .collect::<Vec<_>>(),
            tiling
                .containers
                .iter()
                .map(|container| {
                    (
                        container.id,
                        container.border_box,
                        container.visible_border_box,
                    )
                })
                .collect::<Vec<_>>(),
        )
    };
    let before = borders(&dome);
    assert_eq!(before.0.len(), 1);
    assert_eq!(before.0[0].0, peer);
    assert_eq!(before.1.len(), 1);
    assert_eq!(before.1[0].1, PixelRect::new(0, 0, 100, 20));
    dome.complete_placement_observations(1);
    assert_eq!(borders(&dome), before);
    dome.deliver_notification(WindowNotification::MovementStarted(peer));
    dome.moved(peer, dome.frame(peer), 1);
    assert_eq!(borders(&dome), before);
    dome.remove(peer);
    dome.remove(window);
    assert_eq!(borders(&dome), (Vec::new(), Vec::new()));
}

#[test]
fn complete_stale_placement_leaves_newer_native_frame_until_fresh_observation() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    let old = dome.frame(window);
    dome.open("Peer");
    let current = PixelRect::new(10, 4, 35, 12);
    dome.change(window, |window| window.state.window_rect = current);
    dome.deliver_notification(WindowNotification::PlacementChanged {
        window_id: window,
        observed_at: Instant::now() - Duration::from_secs(5),
        placement: Some(PlacementObservation::Visible {
            rect: old,
            monitor: 1,
        }),
        constraints: None,
    });
    assert_eq!(dome.frame(window), current);
    dome.moved(window, current, 1);
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 48, 18));
}

#[test]
fn retry_resisted_peer_resize_aligns_application_with_desired_border() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));

    dome.server.resisted.borrow_mut().insert(window);
    let peer = dome.open("Peer");
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(peer), PixelRect::new(51, 1, 48, 18));
    {
        let state = dome.overlays();
        let tiling = state
            .requested
            .as_ref()
            .unwrap()
            .tiling
            .values()
            .next()
            .unwrap();
        let border = &tiling
            .windows
            .iter()
            .find(|show| show.placement.id == window)
            .unwrap()
            .placement;
        assert_eq!(border.border_box, PixelRect::new(0, 0, 50, 20));
        assert_eq!(border.visible_border_box, PixelRect::new(0, 0, 50, 20));
        assert_eq!(border.content_box, PixelRect::new(1, 1, 48, 18));
    }

    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));
    dome.server.resisted.borrow_mut().remove(&window);
    dome.deliver_notification(WindowNotification::PeriodicTick);

    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 48, 18));
    assert_eq!(dome.frame(peer), PixelRect::new(51, 1, 48, 18));
    let state = dome.overlays();
    let tiling = state
        .requested
        .as_ref()
        .unwrap()
        .tiling
        .values()
        .next()
        .unwrap();
    let border = &tiling
        .windows
        .iter()
        .find(|show| show.placement.id == window)
        .unwrap()
        .placement;
    assert_eq!(border.border_box, PixelRect::new(0, 0, 50, 20));
    assert_eq!(border.content_box, PixelRect::new(1, 1, 48, 18));
    let peer_border = &tiling
        .windows
        .iter()
        .find(|show| show.placement.id == peer)
        .unwrap()
        .placement;
    assert_eq!(peer_border.border_box, PixelRect::new(50, 0, 50, 20));
    assert_eq!(peer_border.content_box, PixelRect::new(51, 1, 48, 18));
}

#[test]
fn change_target_after_retry_exhaustion_restarts_tiling_placement() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    dome.complete_placement_observations(1);
    dome.server.resisted.borrow_mut().insert(window);
    dome.open("Peer");
    for _ in 0..MAX_DRIFT_RETRIES + 2 {
        dome.deliver_notification(WindowNotification::PeriodicTick);
    }
    dome.server.resisted.borrow_mut().remove(&window);
    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));

    dome.server.resisted.borrow_mut().insert(window);
    let mut display = monitor(1);
    display.work_area = PixelRect::new(0, 0, 120, 20);
    dome.display_changed(vec![display], None);
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));
    dome.server.resisted.borrow_mut().remove(&window);
    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 58, 18));
}

#[test]
fn retry_float_after_resisted_workspace_return_stops_at_cap_and_restarts_on_fresh_return() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    dome.run("focus workspace home");
    let window = dome.open("Floating");
    dome.run("toggle float");
    let target = PixelRect::new(10, 4, 35, 12);
    dome.moved(window, target, 1);
    dome.complete_placement_observations(1);
    dome.run("focus workspace other");
    dome.complete_placement_observations(1);
    dome.server.resisted.borrow_mut().insert(window);
    dome.run("focus workspace home");
    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window).x().value(), -32000);
    dome.server.resisted.borrow_mut().remove(&window);
    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window), target);
    dome.complete_placement_observations(1);

    dome.run("focus workspace other");
    dome.complete_placement_observations(1);
    dome.server.resisted.borrow_mut().insert(window);
    dome.run("focus workspace home");
    for _ in 0..MAX_DRIFT_RETRIES + 2 {
        dome.deliver_notification(WindowNotification::PeriodicTick);
    }
    dome.server.resisted.borrow_mut().remove(&window);
    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window).x().value(), -32000);

    dome.run("focus workspace other");
    dome.server.resisted.borrow_mut().insert(window);
    dome.run("focus workspace home");
    dome.server.resisted.borrow_mut().remove(&window);
    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window), target);
}

#[test]
fn retry_offscreen_after_resisted_hide_stops_at_cap_and_restarts_on_fresh_hide() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    dome.run("focus workspace home");
    let window = dome.open("Window");
    dome.complete_placement_observations(1);
    dome.server.resisted.borrow_mut().insert(window);
    dome.run("focus workspace other");
    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));
    dome.server.resisted.borrow_mut().remove(&window);
    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window).x().value(), -32000);
    dome.complete_placement_observations(1);

    dome.run("focus workspace home");
    dome.complete_placement_observations(1);
    dome.server.resisted.borrow_mut().insert(window);
    dome.run("focus workspace other");
    for _ in 0..MAX_DRIFT_RETRIES + 2 {
        dome.deliver_notification(WindowNotification::PeriodicTick);
    }
    dome.server.resisted.borrow_mut().remove(&window);
    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));

    dome.run("focus workspace home");
    dome.complete_placement_observations(1);
    dome.server.resisted.borrow_mut().insert(window);
    dome.run("focus workspace other");
    dome.server.resisted.borrow_mut().remove(&window);
    dome.deliver_notification(WindowNotification::PeriodicTick);
    assert_eq!(dome.frame(window).x().value(), -32000);
    dome.complete_placement_observations(1);
    assert!(!dome.in_taskbar(window));
}

#[test]
fn complete_stale_float_or_fullscreen_shaped_observation_preserves_border_and_commandability() {
    for floating in [false, true] {
        for stale in [PixelRect::new(5, 5, 40, 10), PixelRect::new(0, 0, 100, 20)] {
            if !floating && stale.width().value() == 40 {
                continue;
            }
            let mut dome = TestDome::new(PARTITION_CONFIG);
            let window = dome.open("Window");
            if floating {
                dome.run("toggle float");
            }
            let frame = dome.frame(window);
            dome.deliver_notification(WindowNotification::PlacementChanged {
                window_id: window,
                observed_at: Instant::now() - Duration::from_secs(5),
                placement: Some(PlacementObservation::Visible {
                    rect: stale,
                    monitor: 1,
                }),
                constraints: None,
            });
            assert_eq!(dome.frame(window), frame);
            if floating {
                let FloatBorderState::Visible(border) = dome.overlays().floats[&window] else {
                    panic!("floating border must remain visible");
                };
                assert_eq!(border.placement.content_box, frame);
            } else {
                let state = dome.overlays();
                let tiling = state
                    .requested
                    .as_ref()
                    .unwrap()
                    .tiling
                    .values()
                    .next()
                    .unwrap();
                assert_eq!(tiling.windows.len(), 1);
                assert_eq!(tiling.windows[0].placement.content_box, frame);
            }
            dome.run("toggle float");
            assert_eq!(dome.overlays().floats.contains_key(&window), !floating);
        }
    }
}
