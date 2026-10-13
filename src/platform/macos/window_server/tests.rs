use super::*;
use crate::platform::macos::dispatcher::{ApplicationNative, DispatcherMarker};
use std::cell::RefCell;
use std::time::Duration;

include!("combined_path_tests.rs");

#[test]
fn filter_running_applications_accepts_only_regular_nonself_valid_processes() {
    use objc2_app_kit::NSApplicationActivationPolicy;
    let current = std::process::id() as i32;
    let other = current.checked_add(1).unwrap();
    for policy in [
        NSApplicationActivationPolicy::Regular,
        NSApplicationActivationPolicy::Accessory,
        NSApplicationActivationPolicy::Prohibited,
    ] {
        assert!(!RunningApp::accepts_process(current, policy));
        assert!(!RunningApp::accepts_process(-1, policy));
        assert_eq!(
            RunningApp::accepts_process(other, policy),
            policy == NSApplicationActivationPolicy::Regular,
        );
    }
}

struct NativeRead {
    results: mpsc::Sender<ApplicationResult>,
    calls: mpsc::Sender<&'static str>,
    replies: mpsc::Receiver<InspectionResult>,
    recipients: mpsc::Sender<usize>,
    selected: mpsc::Sender<InspectionKind>,
}
impl ApplicationNative for NativeRead {
    fn probe(&mut self, _: &DispatcherMarker) {}
    fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
        match operation {
            ApplicationOperation::Read(kind) => {
                self.selected.send(kind).unwrap();
                self.calls.send("read").unwrap();
                let result = self.replies.recv().unwrap();
                let _ = self.results.send(ApplicationResult::Inspection(result));
                self.calls.send("completed").unwrap();
            }
            ApplicationOperation::SetFrame { window, .. } => {
                self.recipients.send(window.test_element_address()).unwrap();
                self.calls.send("frame").unwrap();
            }
            ApplicationOperation::Close(_) => {
                self.calls.send("close").unwrap();
            }
            ApplicationOperation::Terminate => {
                let _ = self.calls.send("terminate");
            }
            _ => {}
        }
    }
}
struct Fixture {
    server: MacOsWindowServer,
    native: mpsc::Sender<NativeEvent>,
    lifetime: Arc<ApplicationLifetime>,
    replies: mpsc::Sender<InspectionResult>,
    calls: mpsc::Receiver<&'static str>,
    recipients: mpsc::Receiver<usize>,
    selected: mpsc::Receiver<InspectionKind>,
}
impl Fixture {
    fn new() -> Self {
        let (native, intake) = mpsc::channel();
        let now = Instant::now();
        let mut server = MacOsWindowServer::new(
            intake,
            native.clone(),
            Arc::new(SharedFocus::new()),
            Arc::new(OnceLock::new()),
            Arc::new(AtomicBool::new(false)),
            now,
        );
        server.next_refresh = now + SYNC_INTERVAL;
        let lifetime = Arc::new(ApplicationLifetime { pid: 123 });
        let (reply, replies) = mpsc::channel();
        let replies = RefCell::new(Some(replies));
        let (calls, called) = mpsc::channel();
        let (recipients, received_by) = mpsc::channel();
        let (selected, selections) = mpsc::channel();
        let owner = AccessibilityApplication::with_native(
            lifetime.clone(),
            false,
            false,
            Box::new(move |results, _| {
                Box::new(NativeRead {
                    results,
                    calls: calls.clone(),
                    replies: replies.borrow_mut().take().unwrap(),
                    recipients: recipients.clone(),
                    selected: selected.clone(),
                })
            }),
            Box::new(|job| std::thread::Builder::new().spawn(job)),
        );
        server.applications.insert(
            123,
            Application {
                state: ApplicationState {
                    lifetime: lifetime.clone(),
                    hidden: false,
                    suspended: false,
                    movement: None,
                },
                owner,
            },
        );
        Self {
            server,
            native,
            lifetime,
            replies: reply,
            calls: called,
            recipients: received_by,
            selected: selections,
        }
    }
    fn request(&mut self) {
        self.server
            .applications
            .get_mut(&123)
            .unwrap()
            .owner
            .send(ApplicationCommand::Read(InspectionKind::AppWindows));
        self.expect("read");
        assert_eq!(
            self.selected.recv_timeout(Duration::from_secs(3)).unwrap(),
            InspectionKind::AppWindows
        );
    }
    fn expect(&self, expected: &str) {
        assert_eq!(
            self.calls.recv_timeout(Duration::from_secs(3)).unwrap(),
            expected
        );
    }
    fn result(&self, lifetime: Arc<ApplicationLifetime>) -> InspectionResult {
        InspectionResult {
            lifetime: lifetime.clone(),
            kind: InspectionKind::AppWindows,
            outcome: InspectionOutcome::Completed(Observation::AppWindows {
                lifetime,
                discovered: Vec::new(),
                existing: Vec::new(),
                removed: Vec::new(),
            }),
            handle_updates: Vec::new(),
        }
    }
    fn complete(&self) {
        self.replies
            .send(self.result(self.lifetime.clone()))
            .unwrap();
        self.expect("completed");
    }
    fn event(&self, event: ApplicationEvent) {
        self.native
            .send(NativeEvent::Application {
                lifetime: self.lifetime.clone(),
                event,
            })
            .unwrap();
    }
    fn stop(&mut self) {
        self.server.shutdown(Vec::new());
        match self.calls.recv_timeout(Duration::from_secs(3)).unwrap() {
            "read" => {
                self.complete();
                self.expect("terminate");
            }
            "terminate" => {}
            other => panic!("unexpected shutdown native call {other}"),
        }
    }
}

#[test]
fn reconcile_when_key_is_present_routes_next_write_to_replacement_element() {
    use crate::config::PreferredLayouts;
    use crate::config::lua::test_support::loaded_runtime;
    use crate::platform::keymap::{KeymapPublisher, KeymapView};
    use crate::platform::macos::dome::events::{
        DiscoveredWindow, HubEvent, HubMessage, MacOsMetadata, WindowPlacement,
    };
    use crate::platform::macos::dome::{Dome, SceneSender};
    use crate::platform::macos::runner::apply_report;

    struct Scenes(mpsc::Sender<HubMessage>);
    impl SceneSender for Scenes {
        fn send(&self, message: HubMessage) {
            self.0.send(message).unwrap();
        }
    }
    let (scenes, scene_receiver) = mpsc::channel();
    let (keymap, _keymap_receiver) = mpsc::channel();
    let (runtime, _file) = loaded_runtime("recycled_handle_identity", "");
    let mut dome = Dome::new(
        &[crate::platform::macos::MonitorInfo {
            display_id: 1,
            name: "Test".into(),
            work_area: PixelRect::new(0, 0, 1920, 1080),
            bounds: PixelRect::new(0, 0, 1920, 1080).to_dimension(),
            full_height: 1080.0,
            is_primary: true,
            backing_scale: 2.0,
        }],
        crate::config::tests::tiling_config(),
        PreferredLayouts::default(),
        Box::new(Scenes(scenes)),
        runtime,
        KeymapPublisher::new(KeymapView::new(), keymap),
        HashMap::new(),
    );
    let discovery = DiscoveredWindow {
        key: 1,
        process_id: 123,
        metadata: MacOsMetadata {
            app_name: Some("Test".into()),
            title: Some("Retained identity".into()),
            bundle_id: Some("test.recycled-handle".into()),
        },
        frame: Some(PixelRect::new(10, 10, 300, 200)),
        corner_radius: Length::new(12.0),
        minimized: false,
        fullscreen: None,
    };
    assert!(
        apply_report(
            &mut dome,
            HubEvent::WindowsDiscovered(vec![discovery.clone()])
        )
        .is_ok()
    );
    let initial_scene = scene_receiver
        .try_iter()
        .filter_map(|message| match message {
            HubMessage::Scene(scene) => Some(scene),
            _ => None,
        })
        .last()
        .unwrap();
    let original_id = initial_scene.windows[&1].id;
    let mut fixture = Fixture::new();
    let original = AXWindow::test_handle(123, 1);
    let replacement = AXWindow::test_handle(123, 1);
    let original_address = original.test_element_address();
    let replacement_address = replacement.test_element_address();
    assert_ne!(original_address, replacement_address);
    fixture.server.handles.insert(1, original);
    fixture.request();
    let mut result = fixture.result(fixture.lifetime.clone());
    result.handle_updates.push(replacement);
    let InspectionOutcome::Completed(Observation::AppWindows { discovered, .. }) =
        &mut result.outcome
    else {
        unreachable!()
    };
    discovered.push(discovery);
    fixture.replies.send(result).unwrap();
    fixture.expect("completed");
    fixture.server.ingest_read_results(Instant::now());
    let observations = fixture.server.take_observations();
    assert_eq!(observations.len(), 1);
    for observation in observations {
        let Observation::AppWindows { discovered, .. } = observation else {
            unreachable!()
        };
        assert!(apply_report(&mut dome, HubEvent::WindowsDiscovered(discovered)).is_ok());
    }
    let replacement_scene = scene_receiver
        .try_iter()
        .filter_map(|message| match message {
            HubMessage::Scene(scene) => Some(scene),
            _ => None,
        })
        .last()
        .unwrap();
    assert_eq!(replacement_scene.windows[&1].id, original_id);
    let WindowPlacement::Tiling { placement, .. } = replacement_scene.windows[&1].placement else {
        panic!("reconciled window must remain tiled");
    };
    fixture.server.set_frame(1, placement.visible_content_box);
    fixture.expect("frame");
    assert_eq!(
        fixture
            .recipients
            .recv_timeout(Duration::from_secs(3))
            .unwrap(),
        replacement_address
    );
    fixture.stop();
}

#[test]
fn inspect_after_enumeration_failure_keeps_previous_write_recipient() {
    let mut fixture = Fixture::new();
    let original = AXWindow::test_handle(123, 1);
    let original_address = original.test_element_address();
    fixture.server.handles.insert(1, original);
    fixture.request();
    let mut result = fixture.result(fixture.lifetime.clone());
    result.outcome = InspectionOutcome::Failed;
    result.handle_updates.push(AXWindow::test_handle(123, 1));
    fixture.replies.send(result).unwrap();
    fixture.expect("completed");
    fixture.server.ingest_read_results(Instant::now());
    assert!(fixture.server.take_observations().is_empty());
    fixture.server.set_frame(
        1,
        crate::core::PixelRect::from_dimension(crate::core::Dimension::new(
            Length::new(1.0),
            Length::new(2.0),
            Length::new(80.0),
            Length::new(60.0),
        )),
    );
    fixture.expect("frame");
    assert_eq!(
        fixture
            .recipients
            .recv_timeout(Duration::from_secs(3))
            .unwrap(),
        original_address
    );
    fixture.stop();
}

#[test]
fn accept_read_during_movement_filters_geometry_but_delivers_title_and_focus() {
    for kind in [
        InspectionKind::AppWindows,
        InspectionKind::Positions(PositionReadOrigin::Periodic),
        InspectionKind::Positions(PositionReadOrigin::Settle(DebounceBurst {
            first: Instant::now(),
            last: Instant::now(),
        })),
        InspectionKind::Space,
        InspectionKind::FocusedWindow,
        InspectionKind::Title(1),
    ] {
        let mut fixture = Fixture::new();
        fixture.request();
        let observed_at = Instant::now();
        fixture.event(ApplicationEvent::MovedOrResized { observed_at });
        fixture.server.ingest_native_events(observed_at);
        let mut result = fixture.result(fixture.lifetime.clone());
        result.kind = kind;
        result.outcome = InspectionOutcome::Completed(match kind {
            InspectionKind::FocusedWindow => Observation::FocusReported {
                lifetime: fixture.lifetime.clone(),
                window: Some(1),
            },
            InspectionKind::Title(key) => Observation::TitleChanged {
                lifetime: fixture.lifetime.clone(),
                key,
                title: "changed".into(),
            },
            InspectionKind::Positions(origin) => Observation::Settled {
                lifetime: fixture.lifetime.clone(),
                origin,
                windows: Vec::new(),
            },
            InspectionKind::Space => Observation::Space {
                lifetime: fixture.lifetime.clone(),
                windows: Vec::new(),
            },
            InspectionKind::AppWindows => Observation::AppWindows {
                lifetime: fixture.lifetime.clone(),
                discovered: Vec::new(),
                existing: Vec::new(),
                removed: Vec::new(),
            },
        });
        fixture.replies.send(result).unwrap();
        fixture.expect("completed");
        fixture.server.ingest_read_results(observed_at);
        let observations = fixture.server.take_observations();
        if geometry_read(kind) {
            assert!(observations.is_empty(), "{kind:?}");
        } else {
            assert_eq!(observations.len(), 1, "{kind:?}");
        }
        fixture.stop();
    }
}

#[test]
fn set_frame_after_worker_panic_discards_old_results_without_replaying_accepted_work() {
    struct Native {
        lifetime: Arc<ApplicationLifetime>,
        results: mpsc::Sender<ApplicationResult>,
        calls: mpsc::Sender<(&'static str, f32)>,
        panic_gate: Option<mpsc::Receiver<()>>,
    }
    impl ApplicationNative for Native {
        fn probe(&mut self, _: &DispatcherMarker) {}
        fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
            match operation {
                ApplicationOperation::Read(kind) => {
                    let title = if self.panic_gate.is_some() {
                        "old"
                    } else {
                        "fresh"
                    };
                    self.results
                        .send(ApplicationResult::Inspection(InspectionResult {
                            lifetime: self.lifetime.clone(),
                            kind,
                            outcome: InspectionOutcome::Completed(Observation::TitleChanged {
                                lifetime: self.lifetime.clone(),
                                key: 1,
                                title: title.into(),
                            }),
                            handle_updates: Vec::new(),
                        }))
                        .unwrap();
                    self.calls.send((title, 0.0)).unwrap();
                    if let Some(gate) = self.panic_gate.take() {
                        gate.recv().unwrap();
                        panic!("injected completion panic");
                    }
                }
                ApplicationOperation::SetFrame { frame, .. } => {
                    self.calls
                        .send(("frame", frame.to_dimension().x.value()))
                        .unwrap();
                }
                ApplicationOperation::Terminate => {
                    let _ = self.calls.send(("terminate", 0.0));
                }
                _ => {}
            }
        }
    }
    struct Exit(mpsc::Sender<()>);
    impl Drop for Exit {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    let mut fixture = Fixture::new();
    let lifetime = fixture.lifetime.clone();
    let (calls, called) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let gate = RefCell::new(Some(gate));
    let attempts = std::rc::Rc::new(std::cell::Cell::new(0));
    let attempted = attempts.clone();
    let (exits, exited) = mpsc::channel();
    fixture.server.applications.get_mut(&123).unwrap().owner =
        AccessibilityApplication::with_native(
            fixture.lifetime.clone(),
            false,
            false,
            Box::new(move |results, _| {
                Box::new(Native {
                    lifetime: lifetime.clone(),
                    results,
                    calls: calls.clone(),
                    panic_gate: gate.borrow_mut().take(),
                })
            }),
            Box::new(move |job| {
                attempted.set(attempted.get() + 1);
                let exits = exits.clone();
                std::thread::Builder::new().spawn(move || {
                    let _exit = Exit(exits);
                    job();
                })
            }),
        );
    fixture
        .server
        .handles
        .insert(1, AXWindow::test_handle(123, 1));
    fixture
        .server
        .forward_event(&fixture.lifetime, ApplicationEvent::TitleChanged(1));
    assert_eq!(
        called.recv_timeout(Duration::from_secs(3)).unwrap().0,
        "old"
    );
    let frame = |x| {
        crate::core::PixelRect::from_dimension(crate::core::Dimension::new(
            Length::new(x),
            Length::new(2.0),
            Length::new(80.0),
            Length::new(60.0),
        ))
    };
    fixture.server.set_frame(1, frame(10.0));
    release.send(()).unwrap();
    exited.recv_timeout(Duration::from_secs(3)).unwrap();
    fixture.server.refresh_applications(Instant::now());
    assert_eq!(attempts.get(), 1);
    fixture.server.set_frame(1, frame(20.0));
    assert_eq!(
        called.recv_timeout(Duration::from_secs(3)).unwrap(),
        ("frame", 20.0)
    );
    assert_eq!(attempts.get(), 2);
    fixture.server.ingest_read_results(Instant::now());
    assert!(fixture.server.take_observations().is_empty());
    fixture
        .server
        .forward_event(&fixture.lifetime, ApplicationEvent::TitleChanged(1));
    assert_eq!(
        called.recv_timeout(Duration::from_secs(3)).unwrap().0,
        "fresh"
    );
    fixture.server.ingest_read_results(Instant::now());
    assert!(matches!(fixture.server.take_observations().as_slice(),
        [Observation::TitleChanged { title, .. }] if title == "fresh"));
    fixture.server.shutdown(Vec::new());
    assert_eq!(
        called.recv_timeout(Duration::from_secs(3)).unwrap().0,
        "terminate"
    );
    assert!(called.try_recv().is_err());
}

#[test]
fn suspend_during_read_updates_keyboard_flag_and_rejects_facts_until_resume() {
    let mut fixture = Fixture::new();
    let keyboard_suspended = fixture.server.is_suspended.clone();
    fixture.request();
    fixture.native.send(NativeEvent::Suspended(true)).unwrap();
    fixture.server.ingest_native_events(Instant::now());
    assert!(keyboard_suspended.load(Ordering::Relaxed));
    fixture.complete();
    fixture.server.ingest_read_results(Instant::now());
    assert!(fixture.server.take_observations().is_empty());
    fixture.native.send(NativeEvent::Suspended(false)).unwrap();
    fixture.server.ingest_native_events(Instant::now());
    assert!(!keyboard_suspended.load(Ordering::Relaxed));
    fixture.expect("read");
    fixture.complete();
    fixture.server.ingest_read_results(Instant::now());
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppWindows { .. }]
    ));
    fixture.stop();
}

#[test]
fn terminate_with_queued_result_discards_completion_before_reporting_gone() {
    let mut fixture = Fixture::new();
    fixture.request();
    fixture.complete();
    fixture
        .native
        .send(NativeEvent::ApplicationTerminated { process_id: 123 })
        .unwrap();
    let now = Instant::now();
    fixture.server.ingest_native_events(now);
    fixture.server.ingest_read_results(now);
    assert!(fixture.server.applications.is_empty());
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppGone { .. }]
    ));
    fixture.expect("terminate");
    assert!(fixture.server.take_observations().is_empty());
}

#[test]
fn ingest_old_lifetime_events_and_results_keeps_current_application() {
    let mut fixture = Fixture::new();
    let old = Arc::new(ApplicationLifetime { pid: 123 });
    fixture
        .native
        .send(NativeEvent::Application {
            lifetime: old.clone(),
            event: ApplicationEvent::Hidden,
        })
        .unwrap();
    fixture.server.ingest_native_events(Instant::now());
    assert!(fixture.server.take_observations().is_empty());
    fixture.request();
    fixture.replies.send(fixture.result(old)).unwrap();
    fixture.expect("completed");
    fixture.server.ingest_read_results(Instant::now());
    assert!(fixture.server.take_observations().is_empty());
    fixture.request();
    fixture.complete();
    fixture.server.ingest_read_results(Instant::now());
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppWindows { .. }]
    ));
    fixture.stop();
}

#[test]
fn move_or_resize_rejects_completed_geometry_and_renews_worker_read() {
    let mut fixture = Fixture::new();
    fixture.request();
    fixture.complete();
    let observed_at = Instant::now();
    fixture.event(ApplicationEvent::MovedOrResized { observed_at });
    fixture.server.ingest_native_events(observed_at);
    fixture.server.ingest_read_results(observed_at);
    assert!(fixture.server.take_observations().is_empty());
    let window = AXWindow::test_handle(123, 1);
    fixture.server.handles.insert(1, window);
    fixture.server.set_frame(
        1,
        crate::core::PixelRect::from_dimension(crate::core::Dimension::new(
            Length::new(1.0),
            Length::new(2.0),
            Length::new(80.0),
            Length::new(60.0),
        )),
    );
    fixture.expect("frame");
    fixture.expect("read");
    assert!(Instant::now() >= observed_at + QUIET_INTERVAL);
    fixture.complete();
    fixture.server.ingest_read_results(Instant::now());
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppWindows { .. }]
    ));
    fixture.stop();
}

#[test]
fn hide_purges_output_and_show_requests_fresh_reads() {
    let mut fixture = Fixture::new();
    fixture.request();
    fixture.complete();
    fixture.server.ingest_read_results(Instant::now());
    fixture.request();
    fixture.send_read(InspectionKind::Space);
    fixture.event(ApplicationEvent::Hidden);
    fixture.server.ingest_native_events(Instant::now());
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppHidden { .. }]
    ));
    fixture.complete();
    fixture.server.ingest_read_results(Instant::now());
    assert!(fixture.server.take_observations().is_empty());
    fixture.event(ApplicationEvent::Shown);
    fixture.server.ingest_native_events(Instant::now());
    assert_eq!(fixture.selected_read(), InspectionKind::AppWindows);
    fixture.complete();
    fixture.server.ingest_read_results(Instant::now());
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppWindows { .. }]
    ));
    fixture.stop();
}

fn test_frame(x: f32) -> PixelRect<Logical> {
    PixelRect::from_dimension(crate::core::Dimension::new(
        Length::new(x),
        Length::new(2.0),
        Length::new(80.0),
        Length::new(60.0),
    ))
}
impl Fixture {
    fn send_read(&mut self, kind: InspectionKind) {
        self.server
            .applications
            .get_mut(&123)
            .unwrap()
            .owner
            .send(ApplicationCommand::Read(kind));
    }
    fn selected_read(&self) -> InspectionKind {
        self.expect("read");
        self.selected.recv_timeout(Duration::from_secs(3)).unwrap()
    }
    fn finish_read(&self, kind: InspectionKind) {
        let lifetime = self.lifetime.clone();
        let observation = match kind {
            InspectionKind::AppWindows => Observation::AppWindows {
                lifetime: lifetime.clone(),
                discovered: Vec::new(),
                existing: Vec::new(),
                removed: Vec::new(),
            },
            InspectionKind::Positions(origin) => Observation::Settled {
                lifetime: lifetime.clone(),
                origin,
                windows: Vec::new(),
            },
            InspectionKind::Space => Observation::Space {
                lifetime: lifetime.clone(),
                windows: Vec::new(),
            },
            InspectionKind::FocusedWindow => Observation::FocusReported {
                lifetime: lifetime.clone(),
                window: Some(1),
            },
            InspectionKind::Title(key) => Observation::TitleChanged {
                lifetime: lifetime.clone(),
                key,
                title: "fresh".into(),
            },
        };
        self.replies
            .send(InspectionResult {
                lifetime,
                kind,
                outcome: InspectionOutcome::Completed(observation),
                handle_updates: Vec::new(),
            })
            .unwrap();
        self.expect("completed");
    }
}

#[test]
fn move_gates_every_geometry_kind_without_holding_writes_title_or_focus() {
    let origin = DebounceBurst {
        first: Instant::now(),
        last: Instant::now(),
    };
    for kind in [
        InspectionKind::AppWindows,
        InspectionKind::Space,
        InspectionKind::Positions(PositionReadOrigin::Periodic),
        InspectionKind::Positions(PositionReadOrigin::Settle(origin)),
    ] {
        let mut fixture = Fixture::new();
        fixture.send_read(InspectionKind::Title(99));
        assert_eq!(fixture.selected_read(), InspectionKind::Title(99));
        let observed_at = Instant::now();
        fixture.event(ApplicationEvent::MovedOrResized { observed_at });
        fixture.server.ingest_native_events(observed_at);
        fixture.send_read(kind);
        fixture
            .server
            .handles
            .insert(1, AXWindow::test_handle(123, 1));
        fixture.server.set_frame(1, test_frame(10.0));
        fixture.send_read(InspectionKind::Title(1));
        fixture.send_read(InspectionKind::FocusedWindow);
        fixture.finish_read(InspectionKind::Title(99));
        fixture.expect("frame");
        assert_eq!(
            fixture.selected_read(),
            InspectionKind::Title(1),
            "{kind:?}"
        );
        fixture.finish_read(InspectionKind::Title(1));
        assert_eq!(
            fixture.selected_read(),
            InspectionKind::FocusedWindow,
            "{kind:?}"
        );
        fixture.finish_read(InspectionKind::FocusedWindow);
        fixture.server.ingest_read_results(observed_at);
        assert_eq!(fixture.server.take_observations().len(), 3, "{kind:?}");
        let selected = fixture.selected_read();
        assert!(Instant::now() >= observed_at + QUIET_INTERVAL, "{kind:?}");
        match (kind, selected) {
            (
                InspectionKind::Positions(_),
                InspectionKind::Positions(PositionReadOrigin::Settle(burst)),
            ) => {
                assert_eq!(burst.first, observed_at);
                assert_eq!(burst.last, observed_at);
            }
            _ => assert_eq!(selected, kind),
        }
        fixture.finish_read(selected);
        fixture.server.ingest_read_results(Instant::now());
        assert_eq!(fixture.server.take_observations().len(), 1);
        fixture.stop();
    }
}

#[test]
fn reject_moving_geometry_renews_each_kind_through_owner() {
    for kind in [
        InspectionKind::AppWindows,
        InspectionKind::Space,
        InspectionKind::Positions(PositionReadOrigin::Periodic),
        InspectionKind::Positions(PositionReadOrigin::Settle(DebounceBurst {
            first: Instant::now(),
            last: Instant::now(),
        })),
    ] {
        let mut fixture = Fixture::new();
        fixture.send_read(kind);
        assert_eq!(fixture.selected_read(), kind);
        fixture.finish_read(kind);
        let observed_at = Instant::now();
        fixture.event(ApplicationEvent::MovedOrResized { observed_at });
        fixture.server.ingest_native_events(observed_at);
        let automatic_settle = fixture.selected_read();
        assert!(matches!(
            automatic_settle,
            InspectionKind::Positions(PositionReadOrigin::Settle(_))
        ));
        fixture.server.ingest_read_results(observed_at);
        assert!(fixture.server.take_observations().is_empty(), "{kind:?}");
        fixture.finish_read(automatic_settle);
        let renewed = fixture.selected_read();
        assert!(Instant::now() >= observed_at + QUIET_INTERVAL);
        match kind {
            InspectionKind::Positions(_) => {
                assert!(matches!(renewed, InspectionKind::Positions(_)))
            }
            _ => assert_eq!(renewed, kind),
        }
        fixture.finish_read(renewed);
        fixture.server.ingest_read_results(Instant::now());
        assert_eq!(fixture.server.take_observations().len(), 2);
        fixture.stop();
    }
}

#[test]
fn accept_geometry_at_quiet_boundary_is_inclusive() {
    for offset in [-1_i64, 0, 1] {
        for kind in [
            InspectionKind::AppWindows,
            InspectionKind::Space,
            InspectionKind::Positions(PositionReadOrigin::Periodic),
            InspectionKind::Positions(PositionReadOrigin::Settle(DebounceBurst {
                first: Instant::now(),
                last: Instant::now(),
            })),
        ] {
            let mut fixture = Fixture::new();
            fixture.send_read(kind);
            assert_eq!(fixture.selected_read(), kind);
            let observed_at = Instant::now();
            fixture.event(ApplicationEvent::MovedOrResized { observed_at });
            fixture.server.ingest_native_events(observed_at);
            fixture.finish_read(kind);
            let deadline = observed_at + QUIET_INTERVAL;
            let acceptance_at = if offset < 0 {
                deadline - Duration::from_nanos(1)
            } else {
                deadline + Duration::from_nanos(offset as u64)
            };
            fixture.server.ingest_read_results(acceptance_at);
            assert_eq!(
                fixture.server.take_observations().len(),
                usize::from(offset >= 0),
                "{kind:?}, {offset}"
            );
            fixture.stop();
        }
    }
}

#[test]
fn show_during_held_read_preserves_owner_and_requests_follow_up() {
    let mut fixture = Fixture::new();
    fixture.send_read(InspectionKind::AppWindows);
    assert_eq!(fixture.selected_read(), InspectionKind::AppWindows);
    let connection = fixture.server.applications[&123]
        .owner
        .connection()
        .unwrap();
    fixture.event(ApplicationEvent::Hidden);
    fixture.event(ApplicationEvent::Shown);
    fixture.server.ingest_native_events(Instant::now());
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppHidden { .. }]
    ));
    fixture.finish_read(InspectionKind::AppWindows);
    fixture.server.ingest_read_results(Instant::now());
    assert_eq!(fixture.server.take_observations().len(), 1);
    assert!(Arc::ptr_eq(
        &connection,
        &fixture.server.applications[&123]
            .owner
            .connection()
            .unwrap()
    ));
    assert_eq!(fixture.selected_read(), InspectionKind::AppWindows);
    fixture.finish_read(InspectionKind::AppWindows);
    fixture.server.ingest_read_results(Instant::now());
    assert_eq!(fixture.server.take_observations().len(), 1);
    fixture.stop();
}

#[test]
fn retire_during_native_read_removes_handles_without_waiting_for_completion() {
    let mut fixture = Fixture::new();
    fixture
        .server
        .handles
        .insert(1, AXWindow::test_handle(123, 1));
    fixture.request();
    fixture
        .native
        .send(NativeEvent::ApplicationTerminated { process_id: 123 })
        .unwrap();
    assert!(fixture.server.take_observations().is_empty());
    assert!(fixture.server.applications.contains_key(&123));
    fixture.server.ingest_native_events(Instant::now());
    assert!(fixture.server.applications.is_empty());
    assert!(fixture.server.handles.is_empty());
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppGone { .. }]
    ));
    fixture.complete();
    fixture.expect("terminate");
    fixture.server.ingest_read_results(Instant::now());
    assert!(fixture.server.take_observations().is_empty());
}

#[test]
fn reuse_process_rejects_old_events_results_and_handles_but_accepts_current_facts() {
    let mut fixture = Fixture::new();
    let old_lifetime = fixture.lifetime.clone();
    fixture.server.retire(123);
    fixture.expect("terminate");
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppGone { .. }]
    ));
    let mut replacement = Fixture::new();
    let application = replacement.server.applications.remove(&123).unwrap();
    fixture.server.applications.insert(123, application);
    fixture.lifetime = replacement.lifetime;
    fixture.replies = replacement.replies;
    fixture.calls = replacement.calls;
    fixture.recipients = replacement.recipients;
    fixture.selected = replacement.selected;
    let current = AXWindow::test_handle(123, 1);
    let current_address = current.test_element_address();
    fixture.server.handles.insert(1, current);
    for event in [
        ApplicationEvent::Hidden,
        ApplicationEvent::Suspended(true),
        ApplicationEvent::MovedOrResized {
            observed_at: Instant::now(),
        },
        ApplicationEvent::Shown,
        ApplicationEvent::Refresh,
        ApplicationEvent::WindowsChanged,
        ApplicationEvent::FocusChanged,
        ApplicationEvent::TitleChanged(1),
        ApplicationEvent::TitleElementChanged(TitleElement(unsafe {
            objc2_application_services::AXUIElement::new_application(123)
        })),
        ApplicationEvent::SpaceChanged,
    ] {
        fixture
            .native
            .send(NativeEvent::Application {
                lifetime: old_lifetime.clone(),
                event,
            })
            .unwrap();
    }
    fixture.server.ingest_native_events(Instant::now());
    assert!(fixture.server.take_observations().is_empty());
    fixture.server.close(1);
    fixture.expect("close");
    fixture.send_read(InspectionKind::AppWindows);
    assert_eq!(fixture.selected_read(), InspectionKind::AppWindows);
    let mut old_result = fixture.result(old_lifetime);
    old_result
        .handle_updates
        .push(AXWindow::test_handle(123, 1));
    fixture.replies.send(old_result).unwrap();
    fixture.expect("completed");
    fixture.server.ingest_read_results(Instant::now());
    assert!(fixture.server.take_observations().is_empty());
    fixture.server.set_frame(1, test_frame(12.0));
    fixture.expect("frame");
    assert_eq!(
        fixture
            .recipients
            .recv_timeout(Duration::from_secs(3))
            .unwrap(),
        current_address
    );
    fixture.send_read(InspectionKind::AppWindows);
    assert_eq!(fixture.selected_read(), InspectionKind::AppWindows);
    fixture.finish_read(InspectionKind::AppWindows);
    fixture.server.ingest_read_results(Instant::now());
    assert_eq!(fixture.server.take_observations().len(), 1);
    fixture.stop();
}

#[test]
fn refresh_snapshot_retires_absent_held_app_discovers_new_app_and_retries_failed_spawn() {
    struct Native {
        pid: i32,
        calls: mpsc::Sender<(i32, &'static str)>,
    }
    impl ApplicationNative for Native {
        fn probe(&mut self, _: &DispatcherMarker) {}
        fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
            let name = match operation {
                ApplicationOperation::Read(InspectionKind::AppWindows) => "enumerate",
                ApplicationOperation::Read(InspectionKind::Positions(_)) => "positions",
                ApplicationOperation::RegisterObserver => "register",
                ApplicationOperation::Close(_) => "barrier",
                ApplicationOperation::Terminate => "terminate",
                _ => return,
            };
            self.calls.send((self.pid, name)).unwrap();
        }
    }
    let (calls, called) = mpsc::channel();
    let make_application = |pid, reject_first| {
        let lifetime = Arc::new(ApplicationLifetime { pid });
        let calls = calls.clone();
        let attempts = std::rc::Rc::new(std::cell::Cell::new(0));
        let attempted = attempts.clone();
        let owner = AccessibilityApplication::with_native(
            lifetime.clone(),
            false,
            false,
            Box::new(move |_, _| {
                Box::new(Native {
                    pid,
                    calls: calls.clone(),
                })
            }),
            Box::new(move |job| {
                attempted.set(attempted.get() + 1);
                if reject_first && attempted.get() == 1 {
                    Err(std::io::Error::other("injected spawn failure"))
                } else {
                    std::thread::Builder::new().spawn(job)
                }
            }),
        );
        (
            Application {
                state: ApplicationState {
                    lifetime,
                    hidden: false,
                    suspended: false,
                    movement: None,
                },
                owner,
            },
            attempts,
        )
    };
    let mut fixture = Fixture::new();
    fixture.request();
    let (failed, attempts) = make_application(456, true);
    fixture.server.admit_application(failed, false);
    assert_eq!(attempts.get(), 1);
    let (new, new_attempts) = make_application(789, false);
    let due = fixture.server.next_refresh;
    fixture.server.refresh_applications_with::<Application>(
        due - Duration::from_nanos(1),
        || panic!("early refresh sampled native applications"),
        |_, _| unreachable!(),
    );
    fixture
        .server
        .ingest_read_results(due - Duration::from_nanos(1));
    assert_eq!(attempts.get(), 1);
    assert_eq!(new_attempts.get(), 0);
    fixture.server.refresh_applications_with(
        due,
        || vec![(456, false, None), (789, false, Some(new))],
        |server, app| server.admit_application(app.unwrap(), false),
    );
    assert_eq!(
        fixture
            .server
            .applications
            .keys()
            .copied()
            .collect::<HashSet<_>>(),
        HashSet::from([456, 789])
    );
    assert_eq!(attempts.get(), 2);
    assert_eq!(new_attempts.get(), 1);
    assert_eq!(fixture.server.next_deadline(), Some(due + SYNC_INTERVAL));
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppGone { .. }]
    ));
    fixture.complete();
    fixture.expect("terminate");
    for pid in [456, 789] {
        let key = pid as u32;
        fixture
            .server
            .handles
            .insert(key, AXWindow::test_handle(pid, key));
        fixture.server.close(key);
    }
    let mut logs = Vec::new();
    while logs.iter().filter(|(_, name)| *name == "barrier").count() < 2 {
        logs.push(called.recv_timeout(Duration::from_secs(3)).unwrap());
    }
    for pid in [456, 789] {
        for expected in ["register", "enumerate", "positions"] {
            assert!(logs.contains(&(pid, expected)), "{logs:?}");
        }
    }
    fixture.server.refresh_applications_with::<Application>(
        due,
        || panic!("unrelated wake resampled applications"),
        |_, _| unreachable!(),
    );
    assert!(called.try_recv().is_err());
    fixture.server.shutdown(Vec::new());
    let mut terminated = HashSet::new();
    for _ in 0..2 {
        let (pid, operation) = called.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(operation, "terminate");
        terminated.insert(pid);
    }
    assert_eq!(terminated, HashSet::from([456, 789]));
}

#[test]
fn enumerate_after_failure_keeps_other_space_handles_and_omits_minimized_fullscreen_geometry() {
    struct Native {
        lifetime: Arc<ApplicationLifetime>,
        results: mpsc::Sender<ApplicationResult>,
        calls: mpsc::Sender<&'static str>,
        recipients: mpsc::Sender<usize>,
        handles: HashMap<u32, AXWindow>,
        snapshot: Vec<AXWindow>,
        fail: bool,
    }
    impl ApplicationNative for Native {
        fn probe(&mut self, _: &DispatcherMarker) {}
        fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
            match operation {
                ApplicationOperation::Read(kind) => {
                    let windows = if std::mem::take(&mut self.fail) {
                        None
                    } else {
                        Some(self.snapshot.clone())
                    };
                    let mut updates = Vec::new();
                    let observed = super::super::inspect::enumerate_windows(
                        &self.lifetime,
                        &mut self.handles,
                        &mut updates,
                        windows,
                        |window| window.cg_id() == 1,
                        |window| window.cg_id() == 6,
                        |window| {
                            let key = window.cg_id();
                            super::super::inspect::read_window_with(
                                key,
                                || Some(key == 3),
                                || Some(key == 4),
                                || {
                                    assert!(
                                        ![3, 4].contains(&key),
                                        "geometry read for minimized/fullscreen window"
                                    );
                                    Some((test_frame(key as f32), Length::new(13.5)))
                                },
                            )
                        },
                    );
                    self.results
                        .send(ApplicationResult::Inspection(InspectionResult {
                            lifetime: self.lifetime.clone(),
                            kind,
                            outcome: observed
                                .map_or(InspectionOutcome::Failed, InspectionOutcome::Completed),
                            handle_updates: updates,
                        }))
                        .unwrap();
                    self.calls.send("completed").unwrap();
                }
                ApplicationOperation::SetFrame { window, .. } => {
                    self.recipients.send(window.test_element_address()).unwrap();
                    self.calls.send("frame").unwrap();
                }
                ApplicationOperation::Terminate => {
                    self.calls.send("terminate").unwrap();
                }
                _ => {}
            }
        }
    }
    let mut fixture = Fixture::new();
    let original = AXWindow::test_handle(123, 5);
    let original_address = original.test_element_address();
    let replacement = AXWindow::test_handle(123, 5);
    let replacement_address = replacement.test_element_address();
    assert_ne!(original_address, replacement_address);
    let initial = HashMap::from([
        (1, AXWindow::test_handle(123, 1)),
        (2, AXWindow::test_handle(123, 2)),
        (5, original),
    ]);
    fixture.server.handles = initial.clone();
    let lifetime = fixture.lifetime.clone();
    let (calls, called) = mpsc::channel();
    let (recipients, received_by) = mpsc::channel();
    fixture.calls = called;
    fixture.recipients = received_by;
    let owner = AccessibilityApplication::with_native(
        lifetime.clone(),
        false,
        false,
        Box::new(move |results, _| {
            Box::new(Native {
                lifetime: lifetime.clone(),
                results,
                calls: calls.clone(),
                recipients: recipients.clone(),
                handles: initial.clone(),
                snapshot: vec![
                    AXWindow::test_handle(123, 3),
                    AXWindow::test_handle(123, 4),
                    replacement.clone(),
                    AXWindow::test_handle(123, 6),
                ],
                fail: true,
            })
        }),
        Box::new(|job| std::thread::Builder::new().spawn(job)),
    );
    fixture.server.applications.get_mut(&123).unwrap().owner = owner;
    fixture.send_read(InspectionKind::AppWindows);
    fixture.expect("completed");
    fixture.server.ingest_read_results(Instant::now());
    assert!(fixture.server.take_observations().is_empty());
    fixture.server.set_frame(5, test_frame(20.0));
    fixture.expect("frame");
    assert_eq!(
        fixture
            .recipients
            .recv_timeout(Duration::from_secs(3))
            .unwrap(),
        original_address
    );
    assert!(fixture.server.handles.contains_key(&2));
    for expected_removed in [vec![2], vec![]] {
        fixture.send_read(InspectionKind::AppWindows);
        fixture.expect("completed");
        fixture.server.ingest_read_results(Instant::now());
        let observations = fixture.server.take_observations();
        let [
            Observation::AppWindows {
                discovered,
                existing,
                removed,
                ..
            },
        ] = observations.as_slice()
        else {
            panic!("{observations:?}")
        };
        assert_eq!(*removed, expected_removed);
        assert_eq!(
            existing
                .iter()
                .map(|window| window.key)
                .collect::<HashSet<_>>(),
            HashSet::from([1, 3, 4, 5])
        );
        assert_eq!(
            discovered
                .iter()
                .map(|window| window.key)
                .collect::<HashSet<_>>(),
            HashSet::from([3, 4, 5])
        );
        for window in existing {
            if [3, 4].contains(&window.key) {
                assert!(window.frame.is_none());
            } else {
                assert_eq!(window.frame, Some(test_frame(window.key as f32)));
                assert_eq!(window.corner_radius, Length::new(13.5));
            }
        }
        assert!(
            existing
                .iter()
                .any(|window| window.key == 3 && window.minimized)
        );
        assert!(
            existing
                .iter()
                .any(|window| window.key == 4 && window.fullscreen.is_some())
        );
    }
    assert!(!fixture.server.handles.contains_key(&2));
    assert!(!fixture.server.handles.contains_key(&6));
    fixture.server.set_frame(5, test_frame(30.0));
    fixture.expect("frame");
    assert_eq!(
        fixture
            .recipients
            .recv_timeout(Duration::from_secs(3))
            .unwrap(),
        replacement_address
    );
    fixture.stop();
}

#[test]
fn deliver_delayed_movement_without_ui_service_preserves_timestamp_gaps() {
    let mut fixture = Fixture::new();
    fixture.send_read(InspectionKind::Title(99));
    assert_eq!(fixture.selected_read(), InspectionKind::Title(99));
    let now = Instant::now();
    let old_burst = now - Duration::from_secs(2);
    let first = now - Duration::from_secs(1);
    let last = first + Duration::from_millis(20);
    for observed_at in [old_burst, first, last] {
        fixture.event(ApplicationEvent::MovedOrResized { observed_at });
    }
    fixture.server.ingest_native_events(now);
    fixture.send_read(InspectionKind::Positions(PositionReadOrigin::Periodic));
    fixture.finish_read(InspectionKind::Title(99));
    let expected =
        InspectionKind::Positions(PositionReadOrigin::Settle(DebounceBurst { first, last }));
    assert_eq!(fixture.selected_read(), expected);
    fixture.send_read(InspectionKind::Positions(PositionReadOrigin::Periodic));
    fixture.finish_read(expected);
    assert_eq!(
        fixture.selected_read(),
        InspectionKind::Positions(PositionReadOrigin::Periodic)
    );
    fixture.finish_read(InspectionKind::Positions(PositionReadOrigin::Periodic));
    fixture.server.ingest_read_results(Instant::now());
    let observations = fixture.server.take_observations();
    assert!(
        matches!(observations.as_slice(), [Observation::TitleChanged { .. },
        Observation::Settled { origin: PositionReadOrigin::Settle(burst), .. },
        Observation::Settled { origin: PositionReadOrigin::Periodic, .. }]
        if *burst == DebounceBurst { first, last })
    );
    fixture.stop();
}

#[test]
fn refresh_while_suspended_advances_deadline_without_snapshot_reads_then_resumes() {
    let mut fixture = Fixture::new();
    fixture.native.send(NativeEvent::Suspended(true)).unwrap();
    fixture.server.ingest_native_events(Instant::now());
    let due = fixture.server.next_refresh;
    fixture.server.refresh_applications_with::<Application>(
        due,
        || panic!("suspended refresh must not sample running apps"),
        |_, _| unreachable!(),
    );
    assert_eq!(fixture.server.next_deadline(), Some(due + SYNC_INTERVAL));
    assert!(fixture.server.applications.contains_key(&123));
    fixture.native.send(NativeEvent::Suspended(false)).unwrap();
    fixture.server.ingest_native_events(due);
    assert_eq!(fixture.selected_read(), InspectionKind::AppWindows);
    fixture.finish_read(InspectionKind::AppWindows);
    fixture.server.ingest_read_results(due);
    assert_eq!(fixture.server.take_observations().len(), 1);
    fixture.stop();
}

#[test]
fn replace_after_lost_lifecycle_command_seeds_current_hidden_or_suspended_state() {
    struct Native {
        calls: mpsc::Sender<&'static str>,
        gate: Option<mpsc::Receiver<()>>,
    }
    impl ApplicationNative for Native {
        fn probe(&mut self, _: &DispatcherMarker) {
            self.calls.send("probe").unwrap();
        }
        fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
            match operation {
                ApplicationOperation::Close(_) => {
                    self.calls.send("held").unwrap();
                    self.gate.take().unwrap().recv().unwrap();
                    panic!("injected old worker death");
                }
                ApplicationOperation::SetFrame { .. } => self.calls.send("frame").unwrap(),
                ApplicationOperation::Read(_) => self.calls.send("read").unwrap(),
                ApplicationOperation::Terminate => {
                    let _ = self.calls.send("terminate");
                }
                _ => {}
            }
        }
    }
    struct Exit(mpsc::Sender<()>);
    impl Drop for Exit {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    for suspend in [false, true] {
        let mut fixture = Fixture::new();
        let (calls, called) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = RefCell::new(Some(gate));
        let (exits, exited) = mpsc::channel();
        let attempts = std::rc::Rc::new(std::cell::Cell::new(0));
        let attempted = attempts.clone();
        fixture.calls = called;
        fixture.server.applications.get_mut(&123).unwrap().owner =
            AccessibilityApplication::with_native(
                fixture.lifetime.clone(),
                false,
                false,
                Box::new(move |_, _| {
                    Box::new(Native {
                        calls: calls.clone(),
                        gate: gate.borrow_mut().take(),
                    })
                }),
                Box::new(move |job| {
                    attempted.set(attempted.get() + 1);
                    let exits = exits.clone();
                    std::thread::Builder::new().spawn(move || {
                        let _exit = Exit(exits);
                        job();
                    })
                }),
            );
        fixture
            .server
            .handles
            .insert(1, AXWindow::test_handle(123, 1));
        fixture.server.close(1);
        fixture.expect("probe");
        fixture.expect("held");
        fixture.send_read(InspectionKind::AppWindows);
        fixture.event(if suspend {
            ApplicationEvent::Suspended(true)
        } else {
            ApplicationEvent::Hidden
        });
        fixture.server.ingest_native_events(Instant::now());
        release.send(()).unwrap();
        exited.recv_timeout(Duration::from_secs(3)).unwrap();
        fixture.server.ingest_read_results(Instant::now());
        assert_eq!(attempts.get(), 1);
        fixture.server.set_frame(1, test_frame(20.0));
        fixture.expect("frame");
        assert_eq!(attempts.get(), 2);
        fixture.send_read(InspectionKind::AppWindows);
        fixture.server.set_frame(1, test_frame(30.0));
        fixture.expect("frame");
        fixture.event(if suspend {
            ApplicationEvent::Suspended(false)
        } else {
            ApplicationEvent::Shown
        });
        fixture.server.ingest_native_events(Instant::now());
        fixture.expect("probe");
        fixture.expect("read");
        fixture.server.shutdown(Vec::new());
        loop {
            match fixture.calls.recv_timeout(Duration::from_secs(3)).unwrap() {
                "read" => {}
                "terminate" => break,
                other => panic!("unexpected replacement call {other}"),
            }
        }
        exited.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(attempts.get(), 2);
    }
}
