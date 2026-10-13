include!("server_panic_path_tests.rs");

struct FocusNative {
    access: super::super::inspect::ApplicationAccess,
    calls: mpsc::Sender<(&'static str, u32)>,
    probe_gate: Option<mpsc::Receiver<()>>,
    selection_gate: Option<mpsc::Receiver<()>>,
}
impl ApplicationNative for FocusNative {
    fn probe(&mut self, _: &DispatcherMarker) {
        if let Some(gate) = self.probe_gate.take() {
            self.calls.send(("probe", 0)).unwrap();
            gate.recv().unwrap();
        }
    }
    fn selected(&mut self, operation: &ApplicationOperation, _: &DispatcherMarker) {
        if let ApplicationOperation::Focus(window) = operation {
            self.calls.send(("selected", window.cg_id())).unwrap();
            if let Some(gate) = self.selection_gate.take() {
                gate.recv().unwrap();
            }
        }
    }
    fn execute(&mut self, operation: ApplicationOperation, marker: &DispatcherMarker) {
        match operation {
            ApplicationOperation::Focus(window) => {
                let key = window.cg_id();
                self.access
                    .execute(ApplicationOperation::Focus(window), marker);
                self.calls.send(("completed", key)).unwrap();
            }
            ApplicationOperation::Terminate => {
                let _ = self.calls.send(("terminate", 0));
            }
            _ => {}
        }
    }
}

#[test]
fn submit_focus_before_native_validation_preserves_pending_and_rejects_invalidated_targets() {
    for case in ["pending", "clear", "overlay", "submitted", "dequeue"] {
        let mut fixture = Fixture::new();
        let focus = fixture.server.focus.clone();
        let (waker, wakes) = mpsc::channel();
        focus.bind_waker(move || {
            waker.send(()).unwrap();
        });
        let (calls, received) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = RefCell::new(Some(gate));
        for key in [1, 2] {
            let calls = calls.clone();
            let focus = focus.clone();
            fixture.server.handles.insert(
                key,
                AXWindow::test_handle(123, key).with_native_focus(move || {
                    focus.assert_unlocked();
                    assert!(focus.is_submitted(key));
                    calls.send(("native", key)).unwrap();
                    Ok(())
                }),
            );
        }
        let app = fixture.server.handles[&1].test_app();
        let lifetime = fixture.lifetime.clone();
        let native_focus = focus.clone();
        let events = fixture.native.clone();
        fixture.server.applications.get_mut(&123).unwrap().owner =
            AccessibilityApplication::with_native(
                lifetime.clone(),
                false,
                false,
                Box::new(move |results, handles| {
                    Box::new(FocusNative {
                        access: super::super::inspect::ApplicationAccess::new(
                            lifetime.clone(),
                            app.clone(),
                            native_focus.clone(),
                            events.clone(),
                            results,
                            Arc::new(OnceLock::new()),
                            handles,
                        ),
                        calls: calls.clone(),
                        probe_gate: if case == "dequeue" {
                            gate.borrow_mut().take()
                        } else {
                            None
                        },
                        selection_gate: if case == "dequeue" {
                            None
                        } else {
                            gate.borrow_mut().take()
                        },
                    })
                }),
                Box::new(|job| std::thread::Builder::new().spawn(job)),
            );
        let next = || received.recv_timeout(Duration::from_secs(3)).unwrap();
        let now = Instant::now();
        focus.request(1);
        focus.submit_pending_focus(now, &mut fixture.server);
        assert_eq!(
            next(),
            if case == "dequeue" {
                ("probe", 0)
            } else {
                ("selected", 1)
            }
        );
        focus.request(2);
        match case {
            "clear" => focus.clear_pending(),
            "overlay" => focus.clear_external(),
            "submitted" | "dequeue" => {
                focus.submit_pending_focus(now + Duration::from_millis(100), &mut fixture.server)
            }
            _ => {}
        }
        assert_eq!(
            wakes.try_iter().count(),
            2,
            "server submission queued another request"
        );
        release.send(()).unwrap();
        match case {
            "pending" | "clear" => {
                assert_eq!(next(), ("native", 1));
                assert_eq!(next(), ("completed", 1));
            }
            "overlay" => assert_eq!(next(), ("completed", 1)),
            "submitted" | "dequeue" => {
                if case == "submitted" {
                    assert_eq!(next(), ("completed", 1));
                }
                assert_eq!(next(), ("selected", 2));
                assert_eq!(next(), ("native", 2));
                assert_eq!(next(), ("completed", 2));
            }
            _ => unreachable!(),
        }
        assert!(wakes.try_recv().is_err(), "worker queued back to request");
        fixture.server.shutdown(Vec::new());
        assert_eq!(next(), ("terminate", 0));
        assert!(received.try_recv().is_err());
    }
}

#[test]
fn clear_focus_after_native_entry_allows_completion_and_another_app_to_progress() {
    let mut fixture = Fixture::new();
    let focus = fixture.server.focus.clone();
    let (wake, wakes) = mpsc::channel();
    focus.bind_waker(move || {
        wake.send(()).unwrap();
    });
    let (calls, received) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let gate = std::sync::Mutex::new(Some(gate));
    for (pid, key) in [(123, 1), (124, 2)] {
        let native_calls = calls.clone();
        let native_focus = focus.clone();
        let window = if key == 1 {
            let gate = gate.lock().unwrap().take().unwrap();
            let gate = std::sync::Mutex::new(Some(gate));
            AXWindow::test_handle(pid, key).with_native_focus(move || {
                native_focus.assert_unlocked();
                assert!(native_focus.is_submitted(1));
                native_calls.send(("native", 1)).unwrap();
                let gate = gate.lock().unwrap().take().unwrap();
                gate.recv().unwrap();
                Ok(())
            })
        } else {
            AXWindow::test_handle(pid, key).with_native_focus(move || {
                native_focus.assert_unlocked();
                assert!(native_focus.is_submitted(2));
                native_calls.send(("native", 2)).unwrap();
                Ok(())
            })
        };
        let app = window.test_app();
        fixture.server.handles.insert(key, window);
        let lifetime = Arc::new(ApplicationLifetime { pid });
        let native_lifetime = lifetime.clone();
        let native_focus = focus.clone();
        let events = fixture.native.clone();
        let calls = calls.clone();
        let owner = AccessibilityApplication::with_native(
            lifetime.clone(),
            false,
            false,
            Box::new(move |results, handles| {
                Box::new(FocusNative {
                    access: super::super::inspect::ApplicationAccess::new(
                        native_lifetime.clone(),
                        app.clone(),
                        native_focus.clone(),
                        events.clone(),
                        results,
                        Arc::new(OnceLock::new()),
                        handles,
                    ),
                    calls: calls.clone(),
                    probe_gate: None,
                    selection_gate: None,
                })
            }),
            Box::new(|job| std::thread::Builder::new().spawn(job)),
        );
        fixture.server.applications.insert(
            pid,
            Application {
                state: ApplicationState {
                    lifetime,
                    hidden: false,
                    suspended: false,
                    movement: None,
                },
                owner,
            },
        );
    }
    let next = || received.recv_timeout(Duration::from_secs(3)).unwrap();
    let now = Instant::now();
    focus.request(1);
    focus.submit_pending_focus(now, &mut fixture.server);
    assert_eq!(next(), ("selected", 1));
    assert_eq!(next(), ("native", 1));
    focus.request(2);
    focus.clear_external();
    assert!(!focus.is_submitted(1));
    focus.request(2);
    focus.submit_pending_focus(now + Duration::from_millis(100), &mut fixture.server);
    assert_eq!(next(), ("selected", 2));
    assert_eq!(next(), ("native", 2));
    assert_eq!(next(), ("completed", 2));
    assert_eq!(wakes.try_iter().count(), 3);
    release.send(()).unwrap();
    assert_eq!(next(), ("completed", 1));
    fixture.server.shutdown(Vec::new());
    assert_eq!(next(), ("terminate", 0));
    assert_eq!(next(), ("terminate", 0));
    assert!(received.try_recv().is_err());
}
