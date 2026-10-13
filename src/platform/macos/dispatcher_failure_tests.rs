fn removal() -> ApplicationOperation {
    ApplicationOperation::RemoveObserver(super::super::listeners::tests::removal_barrier())
}

#[test]
fn enqueue_across_every_barrier_preserves_setters_reads_and_focus() {
    for barrier in 0..6 {
        for demand in 0..3 {
            let mut fixture = Fixture::new(None);
            fixture.enter_probe();
            let operation = |value| match demand {
                0 => setter(0, 1, value),
                1 => ApplicationOperation::Read(InspectionKind::Positions(if value == 10 {
                    PositionReadOrigin::Settle(DebounceBurst {
                        first: Instant::now(),
                        last: Instant::now(),
                    })
                } else {
                    PositionReadOrigin::Periodic
                })),
                _ => ApplicationOperation::Focus(window(value as u32)),
            };
            fixture.operation(operation(10));
            fixture.operation(match barrier {
                0 | 1 => ApplicationOperation::Close(window(if barrier == 0 { 1 } else { 2 })),
                2 | 3 => ApplicationOperation::Unminimize(window(if barrier == 2 { 1 } else { 2 })),
                4 => ApplicationOperation::RegisterObserver,
                _ => removal(),
            });
            fixture.operation(operation(30));
            fixture.release_probe();
            fixture.skip_initial();
            let first = fixture.next();
            assert_eq!(first.kind, ["frame", "positions", "focus"][demand]);
            assert_eq!(
                fixture.next().kind,
                [
                    "close",
                    "close",
                    "unminimize",
                    "unminimize",
                    "register",
                    "remove"
                ][barrier]
            );
            let last = fixture.next();
            assert_eq!(last.kind, first.kind);
            match demand {
                0 => assert_eq!((first.value, last.value), (10, 30)),
                1 => {
                    assert!(matches!(first.origin, Some(PositionReadOrigin::Settle(_))));
                    assert_eq!(last.origin, Some(PositionReadOrigin::Periodic));
                }
                _ => assert_eq!((first.key, last.key), (10, 30)),
            }
            fixture.finish();
        }
    }
}

#[test]
fn enqueue_setters_replaces_each_setter_kind_without_reordering_other_windows() {
    for previous in 0..4 {
        for replacement in 0..4 {
            let mut fixture = Fixture::new(None);
            fixture.enter_probe();
            fixture.operation(setter(previous, 1, 10));
            fixture.operation(setter(0, 2, 20));
            fixture.operation(setter(replacement, 1, 30));
            fixture.release_probe();
            fixture.skip_initial();
            let call = fixture.next();
            assert_eq!(
                (call.kind, call.key, call.value),
                (
                    ["frame", "offscreen", "minimize", "recover"][replacement],
                    1,
                    if replacement == 2 { 0 } else { 30 }
                )
            );
            let call = fixture.next();
            assert_eq!((call.kind, call.key, call.value), ("frame", 2, 20));
            fixture.finish();
        }
    }
}

#[test]
fn enqueue_commands_after_native_entry_and_completion_never_deduplicates_history() {
    for unminimize in [false, true] {
        let command = || {
            if unminimize {
                ApplicationOperation::Unminimize(window(1))
            } else {
                ApplicationOperation::Close(window(1))
            }
        };
        let expected = if unminimize { "unminimize" } else { "close" };
        let mut fixture = Fixture::new(Some(expected));
        fixture.enter_probe();
        fixture.operation(command());
        fixture.release_probe();
        fixture.skip_initial();
        assert_eq!(fixture.next().kind, expected);
        fixture.operation(command());
        fixture.release_operation();
        assert_eq!(fixture.next().kind, expected);
        fixture.operation(setter(0, 2, 20));
        assert_eq!(fixture.next().kind, "frame");
        fixture.operation(command());
        assert_eq!(fixture.next().kind, expected);
        fixture.finish();

        let mut fixture = Fixture::new(None);
        fixture.enter_probe();
        fixture.operation(command());
        fixture.operation(setter(0, 2, 20));
        fixture.operation(command());
        fixture.release_probe();
        fixture.skip_initial();
        assert_eq!(fixture.next().kind, expected);
        assert_eq!(fixture.next().kind, "frame");
        assert_eq!(fixture.next().kind, expected);
        fixture.finish();
    }
}

#[test]
fn collect_lifecycle_keeps_removals_and_allows_already_selected_work() {
    struct SelectedNative {
        recorder: NativeRecorder,
        selected: mpsc::Sender<()>,
        gate: Option<mpsc::Receiver<()>>,
    }
    impl ApplicationNative for SelectedNative {
        fn probe(&mut self, marker: &DispatcherMarker) {
            self.recorder.probe(marker);
        }
        fn selected(&mut self, operation: &ApplicationOperation, _: &DispatcherMarker) {
            if matches!(operation, ApplicationOperation::SetFrame { .. })
                && let Some(gate) = self.gate.take()
            {
                self.selected.send(()).unwrap();
                gate.recv().unwrap();
            }
        }
        fn execute(&mut self, operation: ApplicationOperation, marker: &DispatcherMarker) {
            self.recorder.execute(operation, marker);
        }
    }
    for lifecycle in 0..3 {
        let mut fixture = Fixture::new(None);
        let (calls, receiver) = mpsc::channel();
        let (selected, selection) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = RefCell::new(Some(gate));
        fixture.owner.native = Box::new(move |_, _| {
            Box::new(SelectedNative {
                recorder: NativeRecorder {
                    calls: calls.clone(),
                    probe_gate: None,
                    operation_gate: None,
                },
                selected: selected.clone(),
                gate: gate.borrow_mut().take(),
            })
        });
        fixture.calls = receiver;
        fixture.operation(setter(0, 1, 10));
        assert_eq!(fixture.next().kind, "probe");
        assert_eq!(fixture.next().kind, "register");
        selection.recv_timeout(Duration::from_secs(3)).unwrap();
        fixture.operation(setter(0, 1, 20));
        fixture.read(InspectionKind::AppWindows);
        fixture.operation(removal());
        fixture.owner.send(match lifecycle {
            0 => ApplicationCommand::Event(ApplicationEvent::Hidden),
            1 => ApplicationCommand::Terminate,
            _ => ApplicationCommand::Shutdown(vec![RecoveryEntry {
                window: 1,
                original_rect: frame(5),
            }]),
        });
        assert!(
            fixture.calls.try_recv().is_err(),
            "selected native body remains held after sender returns"
        );
        release.send(()).unwrap();
        let selected = fixture.next();
        assert_eq!((selected.kind, selected.value), ("frame", 10));
        assert_eq!(fixture.next().kind, "remove");
        if lifecycle == 0 {
            fixture.operation(setter(0, 2, 40));
            let after = fixture.next();
            assert_eq!((after.kind, after.value), ("frame", 40));
            fixture.finish();
        } else {
            if lifecycle == 2 {
                let restore = fixture.next();
                assert_eq!(
                    (restore.kind, restore.key, restore.value),
                    ("restore", 1, 1)
                );
            }
            assert_eq!(fixture.next().kind, "terminate");
        }
    }
}

struct ThreadExit(mpsc::Sender<()>);
impl Drop for ThreadExit {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

#[test]
fn panic_at_each_worker_boundary_keeps_receiver_through_scoped_and_native_drops() {
    struct ScopedRestore {
        entered: mpsc::Sender<&'static str>,
        gate: Option<mpsc::Receiver<()>>,
    }
    impl Drop for ScopedRestore {
        fn drop(&mut self) {
            let _ = self.entered.send("restore");
            if let Some(gate) = self.gate.take() {
                let _ = gate.recv();
            }
        }
    }
    struct PanicNative {
        site: &'static str,
        entered: mpsc::Sender<&'static str>,
        results: mpsc::Sender<ApplicationResult>,
        lifetime: Arc<ApplicationLifetime>,
        restore_gate: Option<mpsc::Receiver<()>>,
        drop_gate: Option<mpsc::Receiver<()>>,
    }
    impl PanicNative {
        fn fail(&mut self) {
            let _restore = ScopedRestore {
                entered: self.entered.clone(),
                gate: self.restore_gate.take(),
            };
            panic!("injected {} panic", self.site);
        }
    }
    impl ApplicationNative for PanicNative {
        fn probe(&mut self, _: &DispatcherMarker) {
            if self.site == "probe" {
                self.fail();
            }
        }
        fn selected(&mut self, operation: &ApplicationOperation, _: &DispatcherMarker) {
            if self.site == "selection" && matches!(operation, ApplicationOperation::Read(_)) {
                self.fail();
            }
        }
        fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
            if matches!(operation, ApplicationOperation::Read(_)) {
                if self.site == "completion" {
                    use super::super::window_server::{InspectionOutcome, InspectionResult};
                    self.results
                        .send(ApplicationResult::Inspection(InspectionResult {
                            lifetime: self.lifetime.clone(),
                            kind: InspectionKind::AppWindows,
                            outcome: InspectionOutcome::Failed,
                            handle_updates: Vec::new(),
                        }))
                        .unwrap();
                    self.entered.send("read-completed").unwrap();
                }
                if self.site != "teardown" {
                    self.fail();
                }
            }
            if self.site == "teardown"
                && matches!(operation, ApplicationOperation::RemoveObserver(_))
            {
                self.fail();
            }
        }
    }
    impl Drop for PanicNative {
        fn drop(&mut self) {
            let _ = self.entered.send("native-drop");
            if let Some(gate) = self.drop_gate.take() {
                let _ = gate.recv();
            }
        }
    }
    for site in ["probe", "selection", "read", "completion", "teardown"] {
        let mut fixture = Fixture::new(None);
        let (entered, entries) = mpsc::channel();
        let (restore, restore_gate) = mpsc::channel();
        let restore_gate = RefCell::new(Some(restore_gate));
        let (drop_release, drop_gate) = mpsc::channel();
        let drop_gate = RefCell::new(Some(drop_gate));
        let native_lifetime = fixture.owner.lifetime.clone();
        fixture.owner.native = Box::new(move |results, _| {
            Box::new(PanicNative {
                site,
                entered: entered.clone(),
                results,
                lifetime: native_lifetime.clone(),
                restore_gate: restore_gate.borrow_mut().take(),
                drop_gate: drop_gate.borrow_mut().take(),
            })
        });
        let (exited, exits) = mpsc::channel();
        let attempts = Rc::new(Cell::new(0));
        let attempted = attempts.clone();
        fixture.owner.spawn = Box::new(move |job| {
            attempted.set(attempted.get() + 1);
            let exited = exited.clone();
            thread::Builder::new().spawn(move || {
                let _exit = ThreadExit(exited);
                job();
            })
        });
        let lifetime = fixture.owner.lifetime.clone();
        fixture.read(InspectionKind::AppWindows);
        if site == "teardown" {
            fixture.operation(removal());
        }
        let next = || entries.recv_timeout(Duration::from_secs(3)).unwrap();
        if site == "completion" {
            assert_eq!(next(), "read-completed");
        }
        assert_eq!(next(), "restore");
        fixture.operation(setter(0, 1, 10));
        assert_eq!(attempts.get(), 1);
        restore.send(()).unwrap();
        assert_eq!(next(), "native-drop");
        fixture.operation(setter(0, 1, 15));
        assert_eq!(attempts.get(), 1);
        drop_release.send(()).unwrap();
        exits.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(
            fixture.owner.take_results().len(),
            usize::from(site == "completion")
        );
        for _ in 0..3 {
            fixture.owner.take_results();
            fixture.owner.retry_start_on_refresh();
        }
        assert_eq!(attempts.get(), 1);
        fixture.replace_native();
        fixture.operation(setter(0, 2, 20));
        assert_eq!(attempts.get(), 2);
        assert!(Arc::ptr_eq(&lifetime, &fixture.owner.lifetime));
        assert_eq!(fixture.next().kind, "probe");
        fixture.release_probe();
        assert_eq!(fixture.next().kind, "register");
        let returned = fixture.next();
        assert_eq!(
            (returned.kind, returned.key, returned.value),
            ("frame", 2, 20)
        );
        fixture.finish();
    }
}

#[test]
fn disconnect_with_hidden_or_suspended_quiet_demand_exits_without_future_event() {
    for hidden in [false, true] {
        let mut fixture = Fixture::new(None);
        fixture.owner.update_state(hidden, !hidden);
        let (exited, exits) = mpsc::channel();
        fixture.owner.spawn = Box::new(move |job| {
            let exited = exited.clone();
            thread::Builder::new().spawn(move || {
                let _exit = ThreadExit(exited);
                job();
            })
        });
        fixture.owner.send(ApplicationCommand::Event(
            ApplicationEvent::MovedOrResized {
                observed_at: Instant::now() + Duration::from_secs(60),
            },
        ));
        fixture.read(InspectionKind::AppWindows);
        fixture.operation(setter(0, 1, 10));
        fixture.owner.worker = None;
        assert_eq!(fixture.next().kind, "frame");
        assert_eq!(fixture.next().kind, "terminate");
        exits.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(fixture.calls.try_recv().is_err());
    }
    let mut fixture = Fixture::new(None);
    let (exited, exits) = mpsc::channel();
    fixture.owner.spawn = Box::new(move |job| {
        let exited = exited.clone();
        thread::Builder::new().spawn(move || {
            let _exit = ThreadExit(exited);
            job();
        })
    });
    fixture.owner.start_worker().unwrap();
    fixture.owner.worker = None;
    exits.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(
        fixture.calls.try_recv().is_err(),
        "empty disconnected input must never probe"
    );
}
