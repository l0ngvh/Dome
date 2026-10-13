#[path = "handler_tests.rs"]
mod handler;

use super::*;
use crate::platform::windows::handle::submission::{self, Submission};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use windows::Win32::UI::WindowsAndMessaging::{
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
};

fn server() -> WindowsWindowServer {
    WindowsWindowServer::new(Arc::new(|| {})).unwrap()
}
fn register(server: &mut WindowsWindowServer, window: HwndId, now: Instant) {
    server.native_sender().send(NativeEvent::Discover(window));
    server.ingest_native_events(now);
}
fn placement(unaware: bool, child: Option<(HwndId, i32, i32)>) -> PlacementRead {
    PlacementRead {
        invisible_border: (
            Pixels::new(7),
            Pixels::new(2),
            Pixels::new(9),
            Pixels::new(6),
        ),
        unaware,
        deferred_children: Vec::new(),
        owned_windows: child
            .into_iter()
            .map(|(window, x, y)| OwnedWindowRead {
                window,
                offset: Some((Pixels::new(x), Pixels::new(y))),
            })
            .collect(),
    }
}
fn install(server: &mut WindowsWindowServer, window: HwndId, read: PlacementRead, now: Instant) {
    server.accept_result(
        InspectionResult {
            window,
            kind: InspectionKind::Placement,
            outcome: InspectionOutcome::Completed {
                observation: None,
                placement: Some(read),
                owner: None,
            },
        },
        now,
    );
}
fn monitor(rectangle: PixelRect, dots_per_inch: u32) -> MonitorInfo {
    MonitorInfo {
        handle: 1,
        name: String::new(),
        gdi_device: String::new(),
        work_area: PixelRect::new(0, 0, 1, 1),
        full_rect: rectangle,
        dots_per_inch,
        scale: dots_per_inch as f32 / 96.0,
        is_primary: false,
    }
}

#[test]
fn send_movement_before_intake_closes_geometry_gate() {
    let mut server = server();
    let window = HwndId::test(1);
    let now = Instant::now();
    register(&mut server, window, now);
    server.native_sender().send(NativeEvent::Moved {
        window,
        observed_at: now,
    });
    assert_eq!(server.schedule.may_read(window, now), Some(false));
    assert_eq!(
        server
            .schedule
            .may_read(window, now + Duration::from_millis(99)),
        Some(false)
    );
    assert_eq!(
        server
            .schedule
            .may_read(window, now + Duration::from_millis(100)),
        Some(true)
    );
    server.ingest_native_events(now + Duration::from_secs(1));
    assert_eq!(
        server
            .schedule
            .may_read(window, now + Duration::from_secs(1)),
        Some(true)
    );
}

#[test]
fn expire_drag_safety_with_newer_movement_preserves_quiet() {
    let mut server = server();
    let window = HwndId::test(2);
    let start = Instant::now();
    register(&mut server, window, start);
    server.discovered.insert(window);
    let sender = server.native_sender();
    sender.send(NativeEvent::MoveStarted {
        window,
        observed_at: start,
    });
    let movement = start + Duration::from_millis(59_950);
    sender.send(NativeEvent::Moved {
        window,
        observed_at: movement,
    });
    server.ingest_native_events(movement);
    let safety = start + Duration::from_secs(60);
    let mut schedule = server.schedule.lock();
    let state = schedule.windows.get_mut(&window).unwrap();
    assert!(state.take_ready(safety).is_empty());
    assert!(state.drag_started_at.is_none());
    assert!(
        !state
            .take_ready(movement + Duration::from_millis(100))
            .is_empty()
    );
    assert!(
        state
            .take_ready(movement + Duration::from_millis(101))
            .is_empty()
    );
    drop(schedule);
    assert_eq!(server.next_deadline(), None);
    let newer = safety + Duration::from_secs(1);
    sender.send(NativeEvent::Moved {
        window,
        observed_at: newer,
    });
    sender.send(NativeEvent::MoveEnded {
        window,
        observed_at: newer,
    });
    server.ingest_native_events(newer);
    assert_eq!(server.schedule.may_read(window, newer), Some(false));
}

#[test]
fn ingest_native_events_with_empty_batch_services_limits_demand() {
    let wake_count = Arc::new(AtomicUsize::new(0));
    let count = wake_count.clone();
    let mut server = WindowsWindowServer::new(Arc::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
    }))
    .unwrap();
    let (entered, observed) = mpsc::channel();
    server
        .dispatcher
        .as_mut()
        .unwrap()
        .set_primitives(Arc::new(move |window, kind| {
            entered.send((window, kind)).unwrap();
            InspectionOutcome::Failed
        }));
    let window = HwndId::test(3);
    let now = Instant::now();
    register(&mut server, window, now);
    server
        .schedule
        .lock()
        .windows
        .get_mut(&window)
        .unwrap()
        .discovery
        .pending = false;
    let before = wake_count.load(Ordering::SeqCst);
    server.request_size_limits(vec![SizeLimitRead { window }]);
    assert!(observed.try_recv().is_err());
    assert_eq!(wake_count.load(Ordering::SeqCst), before + 1);
    server.ingest_native_events(now);
    server.ingest_read_results(now);
    server.read_settled_windows(now);
    assert_eq!(
        observed.recv_timeout(Duration::from_secs(2)).unwrap(),
        (window, InspectionKind::SizeLimits)
    );
}

#[test]
fn ingest_native_events_for_gone_window_discards_queued_placement() {
    let mut server = server();
    let window = HwndId::test(4);
    let now = Instant::now();
    register(&mut server, window, now);
    let (sender, results) = mpsc::channel();
    server.results = results;
    sender
        .send(InspectionResult {
            window,
            kind: InspectionKind::Placement,
            outcome: InspectionOutcome::Completed {
                observation: None,
                placement: Some(placement(false, None)),
                owner: None,
            },
        })
        .unwrap();
    server.native_sender().send(NativeEvent::Gone(window));
    server.ingest_native_events(now);
    server.ingest_read_results(now);
    assert!(!server.placement.contains_key(&window));
    server.native_sender().send(NativeEvent::Moved {
        window,
        observed_at: now,
    });
    server.ingest_native_events(now);
    let quiet = now + Duration::from_millis(100);
    install(&mut server, window, placement(true, None), quiet);
    assert!(!server.placement.contains_key(&window));
    assert!(
        matches!(server.take_observations().as_slice(), [Observation::Gone(key)] if *key == window)
    );
    register(&mut server, window, quiet);
    install(&mut server, window, placement(false, None), quiet);
    assert!(server.placement.contains_key(&window));
}

#[test]
fn accept_placement_during_movement_defers_input_update() {
    let mut server = server();
    let window = HwndId::test(5);
    let now = Instant::now();
    register(&mut server, window, now);
    install(&mut server, window, placement(false, None), now);
    server.native_sender().send(NativeEvent::Moved {
        window,
        observed_at: now,
    });
    install(&mut server, window, placement(true, None), now);
    assert!(!server.placement[&window].unaware);
    assert!(server.schedule.lock().windows[&window].placement.pending);
    install(
        &mut server,
        window,
        placement(true, None),
        now + Duration::from_millis(100),
    );
    assert!(server.placement[&window].unaware);
    server.accept_result(
        InspectionResult {
            window,
            kind: InspectionKind::Placement,
            outcome: InspectionOutcome::Failed,
        },
        now + Duration::from_millis(100),
    );
    assert!(server.placement[&window].unaware);
}

#[test]
fn complete_inspection_preserves_later_demand_without_failure_retry() {
    let now = Instant::now();
    for kind in [
        InspectionKind::Discovery,
        InspectionKind::Title,
        InspectionKind::SizeLimits,
        InspectionKind::Placement,
    ] {
        let mut state = schedule::WindowInspectionState::default();
        state.progress(kind).unwrap().request();
        assert_eq!(state.take_ready(now), vec![kind]);
        state.progress(kind).unwrap().request();
        state.complete(kind, false);
        assert_eq!(state.take_ready(now), vec![kind]);
        state.complete(kind, true);
        assert_eq!(state.take_ready(now), vec![kind]);
        state.complete(kind, false);
        assert!(state.take_ready(now).is_empty());
    }
}

#[test]
fn execute_queued_geometry_after_movement_defers_before_native_body() {
    for kind in [
        InspectionKind::Discovery,
        InspectionKind::SizeLimits,
        InspectionKind::Placement,
        InspectionKind::Title,
    ] {
        let mut server = server();
        let window = HwndId::test(6);
        register(&mut server, window, Instant::now());
        let (arrived, arrival) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let released = std::sync::Mutex::new(released);
        let body_count = Arc::new(AtomicUsize::new(0));
        let count = body_count.clone();
        let dispatcher = server.dispatcher.as_mut().unwrap();
        dispatcher.set_before_gate(Arc::new(move || {
            arrived.send(()).unwrap();
            released
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
        }));
        dispatcher.set_primitives(Arc::new(move |_, _| {
            count.fetch_add(1, Ordering::SeqCst);
            InspectionOutcome::Failed
        }));
        dispatcher.submit(window, kind);
        arrival.recv_timeout(Duration::from_secs(2)).unwrap();
        server.native_sender().send(NativeEvent::Moved {
            window,
            observed_at: Instant::now() + Duration::from_secs(1),
        });
        release.send(()).unwrap();
        let result = server.results.recv_timeout(Duration::from_secs(2)).unwrap();
        if kind == InspectionKind::Title {
            assert!(matches!(result.outcome, InspectionOutcome::Failed));
            assert_eq!(body_count.load(Ordering::SeqCst), 1);
        } else {
            assert!(matches!(result.outcome, InspectionOutcome::Deferred));
            assert_eq!(body_count.load(Ordering::SeqCst), 0);
        }
    }
}

#[test]
fn submit_job_with_panicking_primitive_completes_its_own_flight() {
    let (wake, wakes) = mpsc::channel();
    let mut server = WindowsWindowServer::new(Arc::new(move || {
        let _ = wake.send(());
    }))
    .unwrap();
    server
        .dispatcher
        .as_mut()
        .unwrap()
        .set_primitives(Arc::new(|_, _| panic!("native primitive panic")));
    let window = HwndId::test(7);
    let now = Instant::now();
    register(&mut server, window, now);
    while wakes.try_recv().is_ok() {}
    for kind in [
        InspectionKind::Discovery,
        InspectionKind::Title,
        InspectionKind::SizeLimits,
        InspectionKind::Placement,
        InspectionKind::Settle { observed_at: now },
    ] {
        if let Some(progress) = server
            .schedule
            .lock()
            .windows
            .get_mut(&window)
            .unwrap()
            .progress(kind)
        {
            progress.in_flight = true;
        }
        server.dispatcher.as_ref().unwrap().submit(window, kind);
        let result = server.results.recv_timeout(Duration::from_secs(2)).unwrap();
        wakes.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(result.window, window);
        assert_eq!(result.kind, kind);
        assert!(matches!(result.outcome, InspectionOutcome::Failed));
        server.accept_result(result, now);
        let mut schedule = server.schedule.lock();
        let state = schedule.windows.get_mut(&window).unwrap();
        if let Some(progress) = state.progress(kind) {
            assert!(!progress.in_flight);
            progress.request();
            assert!(state.take_ready(now).contains(&kind));
            state.complete(kind, false);
        }
    }
}

#[test]
fn change_unmanaged_owned_window_after_quiet_refreshes_parent_offset() {
    let mut server = server();
    let parent = HwndId::test(8);
    let child = HwndId::test(9);
    let now = Instant::now();
    register(&mut server, parent, now);
    install(
        &mut server,
        parent,
        placement(false, Some((child, 20, 30))),
        now,
    );
    server.native_sender().send(NativeEvent::Moved {
        window: child,
        observed_at: now,
    });
    server.ingest_native_events(now);
    install(
        &mut server,
        parent,
        placement(false, Some((child, 80, 90))),
        now,
    );
    assert_eq!(
        server.placement[&parent].owned_windows[0].offset,
        Some((Pixels::new(20), Pixels::new(30)))
    );
    assert!(server.schedule.lock().windows[&parent].placement.pending);
    assert!(server.take_observations().is_empty());
    install(
        &mut server,
        parent,
        placement(false, Some((child, 80, 90))),
        now + Duration::from_millis(100),
    );
    assert_eq!(
        server.placement[&parent].owned_windows[0].offset,
        Some((Pixels::new(80), Pixels::new(90)))
    );
    server.native_sender().send(NativeEvent::Gone(child));
    server.ingest_native_events(now);
    install(
        &mut server,
        parent,
        placement(false, Some((child, 80, 90))),
        now,
    );
    assert!(server.placement[&parent].owned_windows.is_empty());
}

#[test]
fn set_frame_then_z_order_submits_distinct_geometry_and_raise_flags() {
    let mut server = server();
    let parent = HwndId::test(10);
    let child = HwndId::test(11);
    let now = Instant::now();
    register(&mut server, parent, now);
    install(
        &mut server,
        parent,
        placement(false, Some((child, 20, 30))),
        now,
    );
    let writes = submission::record_submissions(|| {
        submission::without_queries(|| {
            server.set_frame(parent, PixelRect::new(100, 200, 500, 400));
            server.set_z_order(parent, ZOrder::Topmost);
        })
    });
    assert_eq!(writes.len(), 3);
    assert!(
        matches!(writes[0], Submission::Position { rectangle: (93, 198, 516, 408), flags, .. } if flags == SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOZORDER)
    );
    assert!(
        matches!(writes[1], Submission::Position { window, rectangle: (113, 228, 0, 0), flags, .. } if window == child && flags == SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE)
    );
    assert!(
        matches!(writes[2], Submission::Position { flags, .. } if flags == SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)
    );
}

#[test]
fn write_frames_repeatedly_keeps_owned_offsets_relative_to_physical_parent() {
    let mut server = server();
    let parent = HwndId::test(12);
    let child = HwndId::test(13);
    let now = Instant::now();
    register(&mut server, parent, now);
    install(
        &mut server,
        parent,
        placement(true, Some((child, 20, 30))),
        now,
    );
    server.observe_displays(&[monitor(PixelRect::new(0, 0, 3000, 2000), 144)]);
    let writes = submission::record_submissions(|| {
        submission::without_queries(|| {
            server.set_frame(parent, PixelRect::new(1500, 200, 500, 400));
            server.set_frame(parent, PixelRect::new(1800, 300, 500, 400));
        })
    });
    let children: Vec<_> = writes
        .iter()
        .filter_map(|write| match write {
            Submission::Position {
                window, rectangle, ..
            } if *window == child => Some(*rectangle),
            _ => None,
        })
        .collect();
    assert_eq!(children, vec![(1513, 228, 0, 0), (1813, 328, 0, 0)]);
    assert_eq!(
        server.placement[&parent].owned_windows[0].offset,
        Some((Pixels::new(20), Pixels::new(30)))
    );
}

#[test]
fn set_frame_with_destination_dots_per_inch_and_failed_swap_uses_observed_inputs() {
    let mut server = server();
    let window = HwndId::test(14);
    let now = Instant::now();
    register(&mut server, window, now);
    install(&mut server, window, placement(true, None), now);
    server.observe_displays(&[
        monitor(PixelRect::new(-1920, 0, 1920, 1080), 96),
        monitor(PixelRect::new(0, 0, 2560, 1440), 144),
    ]);
    let writes = submission::record_submissions(|| {
        submission::without_queries(|| server.set_frame(window, PixelRect::new(150, 150, 600, 600)))
    });
    assert_eq!(writes[0], Submission::Context(-1));
    assert!(matches!(
        writes[1],
        Submission::Position {
            rectangle: (95, 99, 411, 405),
            ..
        }
    ));
    assert_eq!(writes[2], Submission::Context(-4));
    let writes = submission::record_submissions(|| {
        submission::without_queries(|| {
            submission::with_context_result(0, || {
                server.set_frame(window, PixelRect::new(150, 150, 600, 600))
            })
        })
    });
    assert_eq!(writes.len(), 2);
    assert!(matches!(
        writes[1],
        Submission::Position {
            rectangle: (143, 148, 616, 608),
            ..
        }
    ));
}

#[test]
fn submit_external_writes_makes_no_native_queries() {
    let mut server = server();
    let window = HwndId::test(15);
    let now = Instant::now();
    register(&mut server, window, now);
    install(&mut server, window, placement(false, None), now);
    server.native_sender().send(NativeEvent::Moved {
        window,
        observed_at: now,
    });
    let writes = submission::record_submissions(|| {
        submission::without_queries(|| {
            server.set_frame(window, PixelRect::new(0, 0, 100, 100));
            server.move_offscreen(window);
            server.set_z_order(window, ZOrder::NotTopmost);
            server.minimize(window);
            server.unminimize(window);
            server.close(window);
            server.focus(window);
            server.shutdown(vec![RecoveryEntry {
                window,
                is_maximized: true,
            }]);
            server.shutdown(vec![RecoveryEntry {
                window,
                is_maximized: true,
            }]);
            server.set_frame(window, PixelRect::new(0, 0, 200, 200));
        })
    });
    assert_eq!(writes.len(), 10);
    let before = submission::query_count();
    assert!(handle::read_visible_frame(HwndId::test(0)).is_none());
    assert!(submission::query_count() > before);
}

#[test]
fn submit_focus_preserves_observed_foreground_until_invalidated() {
    let mut server = server();
    let window = HwndId::test(16);
    let now = Instant::now();
    server.native_sender().send(NativeEvent::Foreground {
        window,
        internal: false,
        exclusive: None,
    });
    server.ingest_native_events(now);
    let writes = submission::record_submissions(|| {
        submission::without_queries(|| {
            server.focus(window);
            server.invalidate_foreground();
            server.focus(window);
            server.focus(window);
        })
    });
    assert_eq!(
        writes,
        vec![
            Submission::Foreground(window),
            Submission::Foreground(window)
        ]
    );
    server.take_observations();
    server.native_sender().send(NativeEvent::Foreground {
        window: HwndId::test(17),
        internal: true,
        exclusive: None,
    });
    server.ingest_native_events(now);
    assert!(
        !server
            .take_observations()
            .iter()
            .any(|observation| matches!(observation, Observation::Foreground { .. }))
    );
}

#[test]
fn drop_pool_with_blocked_job_returns_before_release() {
    let mut server = server();
    let window = HwndId::test(18);
    register(&mut server, window, Instant::now());
    let (entered, entry) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = std::sync::Mutex::new(released);
    let (finished, finish) = mpsc::channel();
    server
        .dispatcher
        .as_mut()
        .unwrap()
        .set_primitives(Arc::new(move |_, _| {
            entered.send(()).unwrap();
            released
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            finished.send(()).unwrap();
            InspectionOutcome::Failed
        }));
    server.dispatcher.as_ref().unwrap().title(window);
    entry.recv_timeout(Duration::from_secs(2)).unwrap();
    let dispatcher = server.dispatcher.take().unwrap();
    let (dropped, drop_acknowledgment) = mpsc::channel();
    let dropper = std::thread::spawn(move || {
        drop(dispatcher);
        dropped.send(()).unwrap();
    });
    let result = drop_acknowledgment.recv_timeout(Duration::from_secs(2));
    release.send(()).unwrap();
    finish.recv_timeout(Duration::from_secs(2)).unwrap();
    dropper.join().unwrap();
    result.unwrap();
}

#[test]
fn set_frame_across_display_boundaries_uses_full_bounds_and_stable_ties() {
    let mut server = server();
    let window = HwndId::test(19);
    let now = Instant::now();
    register(&mut server, window, now);
    let mut read = placement(true, None);
    read.invisible_border = (Pixels::ZERO, Pixels::ZERO, Pixels::ZERO, Pixels::ZERO);
    install(&mut server, window, read, now);
    server.observe_displays(&[
        monitor(PixelRect::new(-1920, 0, 1920, 1080), 96),
        monitor(PixelRect::new(0, 0, 2560, 1440), 144),
    ]);
    server.observe_displays(&[]);
    for (frame, expected) in [
        (PixelRect::new(150, 150, 600, 600), (100, 100, 400, 400)),
        (PixelRect::new(-1500, 100, 600, 600), (-1500, 100, 600, 600)),
        (PixelRect::new(-100, 0, 400, 300), (-67, 0, 267, 200)),
        (PixelRect::new(-200, 0, 400, 300), (-200, 0, 400, 300)),
        (PixelRect::new(3000, 150, 600, 600), (2000, 100, 400, 400)),
        (PixelRect::new(-200, -600, 400, 300), (-200, -600, 400, 300)),
    ] {
        let writes = submission::record_submissions(|| {
            submission::without_queries(|| server.set_frame(window, frame))
        });
        assert!(
            matches!(writes[1], Submission::Position { rectangle, .. } if rectangle == expected),
            "{frame:?}: {writes:?}"
        );
    }
    server.observe_displays(&[monitor(PixelRect::new(0, 0, 2560, 1440), 0)]);
    let writes = submission::record_submissions(|| {
        submission::without_queries(|| server.set_frame(window, PixelRect::new(150, 150, 600, 600)))
    });
    assert!(matches!(
        writes[1],
        Submission::Position {
            rectangle: (150, 150, 600, 600),
            ..
        }
    ));
}

#[test]
fn accept_child_without_offset_waits_for_refresh_before_propagating() {
    let mut server = server();
    let parent = HwndId::test(20);
    let child = HwndId::test(21);
    let now = Instant::now();
    register(&mut server, parent, now);
    let mut read = placement(false, Some((child, 10, 20)));
    read.owned_windows[0].offset = None;
    read.deferred_children.push(child);
    install(&mut server, parent, read, now);
    assert!(server.schedule.lock().windows[&parent].placement.pending);
    let writes = submission::record_submissions(|| {
        submission::without_queries(|| server.set_frame(parent, PixelRect::new(100, 100, 500, 500)))
    });
    assert_eq!(writes.len(), 1);
    install(
        &mut server,
        parent,
        placement(false, Some((child, 10, 20))),
        now,
    );
    let writes = submission::record_submissions(|| {
        submission::without_queries(|| server.set_frame(parent, PixelRect::new(100, 100, 500, 500)))
    });
    assert_eq!(writes.len(), 2);
    install(&mut server, parent, placement(false, None), now);
    let writes = submission::record_submissions(|| {
        submission::without_queries(|| server.set_frame(parent, PixelRect::new(100, 100, 500, 500)))
    });
    assert_eq!(writes.len(), 1);
}
