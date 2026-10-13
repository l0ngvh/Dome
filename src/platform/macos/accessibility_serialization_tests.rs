#[test]
fn set_frame_with_enhanced_ui_guard_serializes_one_app_without_blocking_another() {
    use crate::platform::macos::window_server::InspectionKind;
    use std::cell::RefCell;
    use std::sync::atomic::AtomicUsize;
    struct Native {
        app: i32,
        active: Arc<AtomicBool>,
        calls: mpsc::Sender<(i32, &'static str, f32, thread::ThreadId)>,
    }
    impl ApplicationNative for Native {
        fn probe(&mut self, _: &DispatcherMarker) {}
        fn execute(&mut self, operation: ApplicationOperation, marker: &DispatcherMarker) {
            match operation {
                ApplicationOperation::SetFrame { window, frame } => {
                    window
                        .with_animation_disabled(marker, || {
                            assert!(self.active.load(Ordering::SeqCst));
                            self.calls
                                .send((
                                    self.app,
                                    "frame",
                                    frame.to_dimension().x.value(),
                                    thread::current().id(),
                                ))
                                .unwrap();
                            Ok(())
                        })
                        .unwrap();
                }
                ApplicationOperation::Read(_) => {
                    assert!(!self.active.load(Ordering::SeqCst));
                    self.calls
                        .send((self.app, "read", 0.0, thread::current().id()))
                        .unwrap();
                }
                ApplicationOperation::Terminate => {
                    assert!(!self.active.load(Ordering::SeqCst));
                    self.calls
                        .send((self.app, "terminate", 0.0, thread::current().id()))
                        .unwrap();
                }
                _ => {}
            }
        }
    }
    let (calls, received) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let gate = RefCell::new(Some(gate));
    let create = |pid| {
        let active = Arc::new(AtomicBool::new(false));
        let mut window = AXWindow::test_handle(pid, pid as u32);
        let app = Arc::get_mut(&mut window.app).unwrap();
        app.can_set_enhanced_ui.store(true, Ordering::Relaxed);
        let state = active.clone();
        let toggles = calls.clone();
        let restore_count = AtomicUsize::new(0);
        let wait = std::sync::Mutex::new(if pid == 123 {
            gate.borrow_mut().take()
        } else {
            None
        });
        app.enhanced_ui_setter = Some(Box::new(move |enabled| {
            if !enabled {
                assert!(!state.swap(true, Ordering::SeqCst), "same-app AX overlap");
            }
            toggles
                .send((
                    pid,
                    if enabled { "restore" } else { "disable" },
                    0.0,
                    thread::current().id(),
                ))
                .unwrap();
            if enabled {
                if restore_count.fetch_add(1, Ordering::SeqCst) == 0 {
                    let gate = wait.lock().unwrap().take();
                    if let Some(gate) = gate {
                        gate.recv().unwrap();
                    }
                }
                assert!(state.swap(false, Ordering::SeqCst));
            }
            Ok(())
        }));
        let native_calls = calls.clone();
        let owner = AccessibilityApplication::with_native(
            Arc::new(ApplicationLifetime { pid }),
            false,
            false,
            Box::new(move |_, _| {
                Box::new(Native {
                    app: pid,
                    active: active.clone(),
                    calls: native_calls.clone(),
                })
            }),
            Box::new(|job| thread::Builder::new().spawn(job)),
        );
        (owner, window)
    };
    let (mut first, first_window) = create(123);
    let (mut second, second_window) = create(124);
    let send = |owner: &mut AccessibilityApplication, window: &AXWindow, x| {
        owner.send(ApplicationCommand::Operation(
            ApplicationOperation::SetFrame {
                window: window.clone(),
                frame: PixelRect::from_dimension(Dimension::new(
                    Length::new(x),
                    Length::new(0.0),
                    Length::new(80.0),
                    Length::new(60.0),
                )),
            },
        ));
    };
    let next = || received.recv_timeout(Duration::from_secs(3)).unwrap();
    send(&mut first, &first_window, 10.0);
    let first_thread = next();
    assert_eq!((first_thread.0, first_thread.1), (123, "disable"));
    assert_ne!(first_thread.3, thread::current().id());
    assert_eq!(next().1, "frame");
    assert_eq!(next().1, "restore");
    send(&mut first, &first_window, 20.0);
    send(&mut first, &first_window, 30.0);
    first.send(ApplicationCommand::Read(InspectionKind::AppWindows));
    send(&mut second, &second_window, 40.0);
    for kind in ["disable", "frame", "restore"] {
        let call = next();
        assert_eq!((call.0, call.1), (124, kind));
        assert_ne!(call.3, first_thread.3);
        assert_ne!(call.3, thread::current().id());
    }
    assert!(received.try_recv().is_err());
    release.send(()).unwrap();
    for kind in ["disable", "frame", "restore", "read"] {
        let call = next();
        assert_eq!((call.0, call.1), (123, kind));
        assert_eq!(call.3, first_thread.3);
        if kind == "frame" {
            assert_eq!(call.2, 30.0);
        }
    }
    first.terminate();
    second.terminate();
    let mut terminated = [next(), next()].map(|call| {
        assert_eq!(call.1, "terminate");
        call.0
    });
    terminated.sort();
    assert_eq!(terminated, [123, 124]);
    assert!(received.try_recv().is_err());
}
