#[test]
fn replace_each_read_kind_moves_latest_demand_after_other_windows() {
    for (kind, expected, key) in [
        (InspectionKind::AppWindows, "enumerate", 0),
        (InspectionKind::Space, "space", 0),
        (InspectionKind::FocusedWindow, "focused", 0),
        (InspectionKind::Title(1), "title", 1),
        (InspectionKind::Title(2), "title", 2),
        (
            InspectionKind::Positions(PositionReadOrigin::Periodic),
            "positions",
            0,
        ),
    ] {
        let mut fixture = Fixture::new(None);
        fixture.enter_probe();
        fixture.read(kind);
        fixture.operation(setter(0, 1, 10));
        fixture.operation(setter(0, 2, 20));
        fixture.read(kind);
        fixture.operation(setter(0, 1, 30));
        fixture.release_probe();
        fixture.skip_initial();
        let first = fixture.next();
        assert_eq!((first.kind, first.key, first.value), ("frame", 1, 30));
        let second = fixture.next();
        assert_eq!((second.kind, second.key, second.value), ("frame", 2, 20));
        let read = fixture.next();
        assert_eq!((read.kind, read.key), (expected, key));
        fixture.finish();
    }
}

#[test]
fn send_after_worker_failure_retains_returned_command_and_unsent_tail_not_accepted_prefix() {
    struct FailAfterFrame {
        recorder: NativeRecorder,
    }
    impl ApplicationNative for FailAfterFrame {
        fn probe(&mut self, marker: &DispatcherMarker) {
            self.recorder.probe(marker);
        }
        fn execute(&mut self, operation: ApplicationOperation, marker: &DispatcherMarker) {
            if matches!(
                operation,
                ApplicationOperation::Read(InspectionKind::Positions(_))
            ) {
                panic!("injected failure after accepted prefix");
            }
            self.recorder.execute(operation, marker);
        }
    }
    let mut fixture = Fixture::new(None);
    let (calls, receiver) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let gate = RefCell::new(Some(gate));
    fixture.owner.native = Box::new(move |_, _| {
        Box::new(FailAfterFrame {
            recorder: NativeRecorder {
                calls: calls.clone(),
                probe_gate: gate.borrow_mut().take(),
                operation_gate: None,
            },
        })
    });
    fixture.calls = receiver;
    fixture.probe_release = Some(release);
    let attempts = Rc::new(Cell::new(0));
    let attempted = attempts.clone();
    let (exited, exits) = mpsc::channel();
    fixture.owner.spawn = Box::new(move |job| {
        attempted.set(attempted.get() + 1);
        if attempted.get() == 2 {
            return Err(io::Error::other("replacement spawn rejected"));
        }
        let exited = exited.clone();
        thread::Builder::new().spawn(move || {
            let _exit = ThreadExit(exited);
            job();
        })
    });
    fixture.enter_probe();
    fixture.operation(setter(0, 1, 10));
    fixture.read(InspectionKind::Positions(PositionReadOrigin::Periodic));
    fixture.operation(setter(0, 2, 20));
    fixture.release_probe();
    fixture.skip_initial();
    assert_eq!(fixture.next().value, 10);
    exits.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(attempts.get(), 1);
    fixture.operation(setter(0, 3, 30));
    fixture.operation(setter(0, 4, 40));
    fixture.operation(setter(0, 3, 35));
    assert_eq!(attempts.get(), 2);
    let never_enqueued: Vec<_> = fixture
        .owner
        .waiting_to_start
        .iter()
        .map(|command| {
            let ApplicationCommand::Operation(ApplicationOperation::SetFrame { window, frame }) =
                command
            else {
                panic!("only unsent setters belong here")
            };
            (window.cg_id(), frame.to_dimension().x.value() as i32)
        })
        .collect();
    assert_eq!(never_enqueued, [(3, 30), (4, 40), (3, 35)]);
    for _ in 0..3 {
        fixture.owner.take_results();
    }
    assert_eq!(attempts.get(), 2);
    let mut other = Fixture::new(None);
    other.enter_probe();
    other.operation(setter(0, 5, 50));
    other.release_probe();
    other.skip_initial();
    assert_eq!(other.next().value, 50);
    other.finish();
    fixture.replace_native();
    fixture.owner.retry_start_on_refresh();
    assert_eq!(attempts.get(), 3);
    assert_eq!(fixture.next().kind, "probe");
    fixture.release_probe();
    assert_eq!(fixture.next().kind, "register");
    for expected in [(3, 35), (4, 40)] {
        let call = fixture.next();
        assert_eq!((call.key, call.value), expected);
    }
    fixture.finish();
}
