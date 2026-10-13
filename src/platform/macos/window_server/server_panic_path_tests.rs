struct PanicPathExit(mpsc::Sender<()>);
impl Drop for PanicPathExit {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}
struct PanicPathRestore {
    entered: mpsc::Sender<(&'static str, f32)>,
    gate: Option<mpsc::Receiver<()>>,
}
impl Drop for PanicPathRestore {
    fn drop(&mut self) {
        let _ = self.entered.send(("restore", 0.0));
        if let Some(gate) = self.gate.take() {
            let _ = gate.recv();
        }
    }
}
struct PanicPathNative {
    site: Option<&'static str>,
    lifetime: Arc<ApplicationLifetime>,
    results: mpsc::Sender<ApplicationResult>,
    calls: mpsc::Sender<(&'static str, f32)>,
    restore_gate: Option<mpsc::Receiver<()>>,
    drop_gate: Option<mpsc::Receiver<()>>,
    active: Arc<std::sync::atomic::AtomicUsize>,
}
impl PanicPathNative {
    fn fail(&mut self) {
        let _restore = PanicPathRestore {
            entered: self.calls.clone(),
            gate: self.restore_gate.take(),
        };
        panic!("injected {:?} panic through server", self.site);
    }
}
impl ApplicationNative for PanicPathNative {
    fn probe(&mut self, _: &DispatcherMarker) {
        assert_eq!(
            self.active.fetch_add(1, Ordering::SeqCst),
            0,
            "replacement overlapped native lifetime"
        );
        self.calls.send(("probe", 0.0)).unwrap();
        if self.site == Some("probe") {
            self.fail();
        }
    }
    fn selected(&mut self, operation: &ApplicationOperation, _: &DispatcherMarker) {
        if self.site == Some("selection") && matches!(operation, ApplicationOperation::Read(_)) {
            self.fail();
        }
    }
    fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
        match operation {
            ApplicationOperation::Read(kind) => {
                if self.site == Some("read") {
                    self.fail();
                }
                self.results
                    .send(ApplicationResult::Inspection(InspectionResult {
                        lifetime: self.lifetime.clone(),
                        kind,
                        outcome: InspectionOutcome::Failed,
                        handle_updates: Vec::new(),
                    }))
                    .unwrap();
                self.calls.send(("completion", 0.0)).unwrap();
                if self.site == Some("completion") {
                    self.fail();
                }
            }
            ApplicationOperation::SetFrame { frame, .. } => {
                self.calls
                    .send(("frame", frame.to_dimension().x.value()))
                    .unwrap();
            }
            ApplicationOperation::Terminate => {
                self.calls.send(("terminate", 0.0)).unwrap();
                if self.site == Some("teardown") {
                    self.fail();
                }
            }
            _ => {}
        }
    }
}
impl Drop for PanicPathNative {
    fn drop(&mut self) {
        let _ = self.calls.send(("native-drop", 0.0));
        if let Some(gate) = self.drop_gate.take() {
            let _ = gate.recv();
        }
        assert_eq!(self.active.fetch_sub(1, Ordering::SeqCst), 1);
    }
}

#[test]
fn send_after_server_intake_worker_panics_replaces_only_returned_command_without_overlap() {
    for site in ["probe", "selection", "read", "completion"] {
        let mut fixture = Fixture::new();
        let (calls, received) = mpsc::channel();
        let (restore, restore_gate) = mpsc::channel();
        let restore_gate = RefCell::new(Some(restore_gate));
        let (drop_release, drop_gate) = mpsc::channel();
        let drop_gate = RefCell::new(Some(drop_gate));
        let (exits, exited) = mpsc::channel();
        let attempts = std::rc::Rc::new(std::cell::Cell::new(0));
        let attempted = attempts.clone();
        let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let native_active = active.clone();
        let lifetime = fixture.lifetime.clone();
        fixture.server.applications.get_mut(&123).unwrap().owner =
            AccessibilityApplication::with_native(
                lifetime.clone(),
                false,
                false,
                Box::new(move |results, _| {
                    let restore_gate = restore_gate.borrow_mut().take();
                    Box::new(PanicPathNative {
                        site: restore_gate.as_ref().map(|_| site),
                        lifetime: lifetime.clone(),
                        results,
                        calls: calls.clone(),
                        restore_gate,
                        drop_gate: drop_gate.borrow_mut().take(),
                        active: native_active.clone(),
                    })
                }),
                Box::new(move |job| {
                    attempted.set(attempted.get() + 1);
                    let exits = exits.clone();
                    std::thread::Builder::new().spawn(move || {
                        let _exit = PanicPathExit(exits);
                        job();
                    })
                }),
            );
        fixture
            .server
            .handles
            .insert(1, AXWindow::test_handle(123, 1));
        fixture.event(ApplicationEvent::WindowsChanged);
        let now = Instant::now();
        fixture.server.ingest_native_events(now);
        let next = || received.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(next(), ("probe", 0.0));
        if site == "completion" {
            assert_eq!(next(), ("completion", 0.0));
        }
        assert_eq!(next(), ("restore", 0.0));
        let old_connection = fixture.server.applications[&123]
            .owner
            .connection()
            .unwrap();
        fixture.server.set_frame(1, test_frame(10.0));
        assert_eq!(attempts.get(), 1);
        assert!(Arc::ptr_eq(
            &old_connection,
            &fixture.server.applications[&123]
                .owner
                .connection()
                .unwrap()
        ));
        restore.send(()).unwrap();
        assert_eq!(next(), ("native-drop", 0.0));
        fixture.server.set_frame(1, test_frame(15.0));
        assert_eq!(attempts.get(), 1);
        assert_eq!(active.load(Ordering::SeqCst), 1);
        drop_release.send(()).unwrap();
        exited.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(active.load(Ordering::SeqCst), 0);
        for _ in 0..3 {
            fixture.server.ingest_native_events(now);
            fixture.server.ingest_read_results(now);
            fixture.server.refresh_applications(now);
            assert!(fixture.server.take_observations().is_empty());
        }
        assert_eq!(attempts.get(), 1);
        assert!(received.try_recv().is_err());
        fixture.server.set_frame(1, test_frame(20.0));
        assert_eq!(attempts.get(), 2);
        assert!(Arc::ptr_eq(
            &fixture.lifetime,
            &fixture.server.applications[&123].state.lifetime
        ));
        assert!(Arc::ptr_eq(
            &fixture.lifetime,
            &fixture.server.applications[&123].owner.lifetime
        ));
        assert!(!Arc::ptr_eq(
            &old_connection,
            &fixture.server.applications[&123]
                .owner
                .connection()
                .unwrap()
        ));
        assert_eq!(next(), ("probe", 0.0));
        assert_eq!(next(), ("frame", 20.0));
        fixture.event(ApplicationEvent::WindowsChanged);
        fixture.server.ingest_native_events(now);
        assert_eq!(next(), ("completion", 0.0));
        fixture.server.shutdown(Vec::new());
        assert_eq!(next(), ("terminate", 0.0));
        assert_eq!(next(), ("native-drop", 0.0));
        exited.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert!(
            received.try_recv().is_err(),
            "accepted old setters replayed"
        );
    }
}

#[test]
fn retire_through_native_intake_with_teardown_panic_never_restarts_accepted_terminal_work() {
    let mut fixture = Fixture::new();
    let (calls, received) = mpsc::channel();
    let (restore, restore_gate) = mpsc::channel();
    let restore_gate = RefCell::new(Some(restore_gate));
    let (drop_release, drop_gate) = mpsc::channel();
    let drop_gate = RefCell::new(Some(drop_gate));
    let (exits, exited) = mpsc::channel();
    let attempts = std::rc::Rc::new(std::cell::Cell::new(0));
    let attempted = attempts.clone();
    let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let lifetime = fixture.lifetime.clone();
    fixture.server.applications.get_mut(&123).unwrap().owner =
        AccessibilityApplication::with_native(
            lifetime.clone(),
            false,
            false,
            Box::new(move |results, _| {
                Box::new(PanicPathNative {
                    site: Some("teardown"),
                    lifetime: lifetime.clone(),
                    results,
                    calls: calls.clone(),
                    restore_gate: restore_gate.borrow_mut().take(),
                    drop_gate: drop_gate.borrow_mut().take(),
                    active: active.clone(),
                })
            }),
            Box::new(move |job| {
                attempted.set(attempted.get() + 1);
                let exits = exits.clone();
                std::thread::Builder::new().spawn(move || {
                    let _exit = PanicPathExit(exits);
                    job();
                })
            }),
        );
    fixture
        .server
        .handles
        .insert(1, AXWindow::test_handle(123, 1));
    fixture.server.set_frame(1, test_frame(10.0));
    let next = || received.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(next(), ("probe", 0.0));
    assert_eq!(next(), ("frame", 10.0));
    fixture
        .native
        .send(NativeEvent::ApplicationTerminated { process_id: 123 })
        .unwrap();
    let now = Instant::now();
    fixture.server.ingest_native_events(now);
    assert!(matches!(
        fixture.server.take_observations().as_slice(),
        [Observation::AppGone { .. }]
    ));
    assert_eq!(next(), ("terminate", 0.0));
    assert_eq!(next(), ("restore", 0.0));
    fixture.server.set_frame(1, test_frame(20.0));
    restore.send(()).unwrap();
    assert_eq!(next(), ("native-drop", 0.0));
    drop_release.send(()).unwrap();
    exited.recv_timeout(Duration::from_secs(3)).unwrap();
    for _ in 0..3 {
        fixture.server.ingest_native_events(now);
        fixture.server.ingest_read_results(now);
        fixture.server.refresh_applications_with(
            now + SYNC_INTERVAL,
            Vec::<(i32, bool, ())>::new,
            |_, _| panic!("retired app rediscovered"),
        );
        fixture.server.set_frame(1, test_frame(30.0));
    }
    assert_eq!(attempts.get(), 1);
    assert!(fixture.server.applications.is_empty());
    assert!(fixture.server.retired.is_empty());
    assert!(received.try_recv().is_err());
}
