use super::*;
use crate::platform::macos::accessibility::AXApp;
use crate::platform::macos::dispatcher::{ApplicationNative, DispatcherMarker};
use crate::platform::macos::listeners::native_source::{self, SourceCall, SourceProbe};
use crate::platform::macos::window_server::inspect::ApplicationAccess;
use dome_auxiliary_window::{App, AppHandler, Icon, LoopHandle, Shell};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSRunningApplication};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Lifetime,
    FailedRemoval,
    Replacement,
    LateSetup,
    Disconnect,
    WrongLifetime,
}

struct Native {
    access: ApplicationAccess,
    calls: mpsc::Sender<SourceCall>,
    probes: mpsc::Sender<SourceProbe>,
    removal: Option<mpsc::Receiver<()>>,
    setup: Option<mpsc::Receiver<()>>,
    case: Case,
    buffered: mpsc::Sender<()>,
    panic_on_read: bool,
}
impl ApplicationNative for Native {
    fn probe(&mut self, _: &DispatcherMarker) {}
    fn execute(&mut self, operation: ApplicationOperation, marker: &DispatcherMarker) {
        match operation {
            ApplicationOperation::RegisterObserver => {
                let calls = self.calls.clone();
                let probes = self.probes.clone();
                let removal = self.removal.take();
                let setup = self.setup.take();
                let case = self.case;
                self.access.register_with(marker, move |context, marker| {
                    Ok(native_source::registration(
                        context,
                        marker,
                        calls,
                        probes,
                        native_source::SourceControl {
                            removal,
                            setup,
                            nested_callback: matches!(case, Case::Lifetime | Case::FailedRemoval),
                            fail_removal: case == Case::FailedRemoval,
                        },
                    ))
                });
            }
            ApplicationOperation::Read(InspectionKind::Title(99)) if self.panic_on_read => {
                self.buffered.send(()).unwrap();
                panic!("injected worker death after buffered setup");
            }
            ApplicationOperation::Read(_) => {}
            operation => self.access.execute(operation, marker),
        }
    }
}

struct Exit(mpsc::Sender<()>);
impl Drop for Exit {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

struct Handler {
    name: String,
    baseline_windows: Vec<(isize, String, bool)>,
    server: MacOsWindowServer,
    handle: Rc<OnceLock<LoopHandle<Wake>>>,
    case: Case,
    calls: mpsc::Receiver<SourceCall>,
    log: Vec<SourceCall>,
    probes: mpsc::Receiver<SourceProbe>,
    sources: Vec<SourceProbe>,
    removal: Option<mpsc::Sender<()>>,
    setup: Option<mpsc::Sender<()>>,
    exits: mpsc::Receiver<()>,
    buffered: mpsc::Receiver<()>,
    stage: usize,
    complete: Rc<Cell<bool>>,
    retained_context: Option<Arc<ObserverContext>>,
    result_senders: Rc<RefCell<Vec<mpsc::Sender<ApplicationResult>>>>,
}
impl Handler {
    fn drain_calls(&mut self) {
        self.log.extend(self.calls.try_iter());
    }
    fn wait_removal(&mut self) {
        loop {
            let call = self.calls.recv_timeout(Duration::from_secs(3)).unwrap();
            let removing = matches!(call, SourceCall::Removing(_));
            self.log.push(call);
            if removing {
                return;
            }
        }
    }
    fn finish(&mut self) {
        self.drain_calls();
        let main = thread::current().id();
        let mut worker = None;
        for call in &self.log {
            match call {
                SourceCall::Created(on)
                | SourceCall::Added(on)
                | SourceCall::Removing(on)
                | SourceCall::Dropped(on) => {
                    assert_ne!(*on, main);
                    if self.case != Case::Replacement {
                        assert_eq!(*worker.get_or_insert(*on), *on);
                    }
                }
                SourceCall::Attached(on)
                | SourceCall::Detached(on)
                | SourceCall::CallbackEntered(_, on)
                | SourceCall::CallbackExited(_, on) => assert_eq!(*on, main),
            }
        }
        assert!(self.sources.iter().all(|source| !source.attached()));
        self.complete.set(true);
        self.handle.get().unwrap().terminate();
    }
}
impl AppHandler for Handler {
    type Wake = Wake;
    fn on_started(&mut self, _: &Shell) {
        self.baseline_windows = own_windows();
        eprintln!("WINDOW_BASELINE {} {:?}", self.name, self.baseline_windows);
        assert!(
            self.baseline_windows
                .iter()
                .all(|(_, class, visible)| !visible || class == "NSStatusBarWindow"),
            "unexpected visible managed window at launch"
        );
        let lifetime = self.server.applications[&123].state.lifetime.clone();
        self.server
            .forward_event(&lifetime, ApplicationEvent::WindowsChanged);
        if self.case == Case::LateSetup {
            self.sources
                .push(self.probes.recv_timeout(Duration::from_secs(3)).unwrap());
            assert!(self.sources[0].context.upgrade().is_some());
            self.server.retire(123);
            self.setup.take().unwrap().send(()).unwrap();
            self.wait_removal();
            assert!(!self.sources[0].attached());
            assert!(self.sources[0].context.upgrade().is_some());
            self.removal.take().unwrap().send(()).unwrap();
            self.exits.recv_timeout(Duration::from_secs(3)).unwrap();
            self.server.ingest_read_results(Instant::now());
            assert!(self.sources[0].context.upgrade().is_none());
            self.finish();
        }
    }
    fn on_wake(&mut self, _: &Shell, wake: Wake) {
        if self.complete.get() {
            return;
        }
        let now = Instant::now();
        match self.stage {
            0 => {
                assert_eq!(
                    wake,
                    Wake::WindowServer,
                    "setup must wake the concrete server"
                );
                self.server.ingest_native_events(now);
                self.server.ingest_read_results(now);
                self.sources
                    .push(self.probes.recv_timeout(Duration::from_secs(3)).unwrap());
                if self.case == Case::WrongLifetime {
                    assert!(!self.sources[0].attached());
                    let lifetime = self.server.applications[&123].state.lifetime.clone();
                    let original = self.sources[0].context.upgrade().unwrap();
                    self.result_senders.borrow()[0]
                        .send(ApplicationResult::ObserverReady {
                            lifetime: lifetime.clone(),
                            source: ObserverSource(self.sources[0].source.0.clone()),
                            context: original.clone(),
                        })
                        .unwrap();
                    self.server.ingest_read_results(now);
                    assert!(
                        !self.sources[0].attached(),
                        "wrong context lifetime must not attach even with a current result lifetime"
                    );
                    let context = Arc::new(ObserverContext {
                        lifetime: lifetime.clone(),
                        events: original.events.clone(),
                        loop_waker: original.loop_waker.clone(),
                    });
                    self.result_senders.borrow()[0]
                        .send(ApplicationResult::ObserverReady {
                            lifetime,
                            source: ObserverSource(self.sources[0].source.0.clone()),
                            context,
                        })
                        .unwrap();
                    self.server.ingest_read_results(now);
                    assert!(
                        self.sources[0].attached(),
                        "current lifetime control must attach"
                    );
                    self.server.retire(123);
                    self.wait_removal();
                    self.removal.take().unwrap().send(()).unwrap();
                    self.exits.recv_timeout(Duration::from_secs(3)).unwrap();
                    self.finish();
                    return;
                }
                assert!(
                    self.sources[0].attached(),
                    "setup result did not attach in common modes"
                );
                self.stage = 1;
                match self.case {
                    Case::Lifetime | Case::FailedRemoval => self.sources[0].signal(),
                    Case::Disconnect => {
                        self.retained_context = self.sources[0].context.upgrade();
                        assert!(self.retained_context.is_some());
                        drop(self.server.applications.remove(&123).unwrap());
                        self.wait_removal();
                        self.removal.take().unwrap().send(()).unwrap();
                        self.exits.recv_timeout(Duration::from_secs(3)).expect(
                            "retained observer context must not keep worker sender connected",
                        );
                        assert!(self.sources[0].context.upgrade().is_some());
                        self.retained_context = None;
                        assert!(self.sources[0].context.upgrade().is_none());
                        self.finish();
                    }
                    Case::Replacement => {
                        let context = self.sources[0].context.upgrade().unwrap();
                        self.result_senders.borrow()[0]
                            .send(ApplicationResult::Inspection(InspectionResult {
                                lifetime: context.lifetime.clone(),
                                kind: InspectionKind::Title(1),
                                outcome: InspectionOutcome::Completed(Observation::TitleChanged {
                                    lifetime: context.lifetime.clone(),
                                    key: 1,
                                    title: "buffered old worker".into(),
                                }),
                                handle_updates: Vec::new(),
                            }))
                            .unwrap();
                        self.result_senders.borrow()[0]
                            .send(ApplicationResult::ObserverReady {
                                lifetime: context.lifetime.clone(),
                                source: ObserverSource(self.sources[0].source.0.clone()),
                                context,
                            })
                            .unwrap();
                        self.server
                            .applications
                            .get_mut(&123)
                            .unwrap()
                            .owner
                            .send(ApplicationCommand::Read(InspectionKind::Title(99)));
                        self.buffered.recv_timeout(Duration::from_secs(3)).unwrap();
                        self.exits.recv_timeout(Duration::from_secs(3)).unwrap();
                        let lifetime = self.server.applications[&123].state.lifetime.clone();
                        self.server
                            .forward_event(&lifetime, ApplicationEvent::WindowsChanged);
                        assert!(
                            !self.sources[0].attached(),
                            "failed send must detach old source"
                        );
                    }
                    _ => unreachable!(),
                }
            }
            1 if self.case == Case::Replacement => {
                self.sources
                    .push(self.probes.recv_timeout(Duration::from_secs(3)).unwrap());
                self.server.ingest_read_results(now);
                assert!(
                    self.server.take_observations().is_empty(),
                    "buffered old facts survived receiver replacement"
                );
                assert!(
                    !self.sources[0].attached(),
                    "buffered old setup reattached after same-lifetime replacement"
                );
                assert!(
                    self.sources[1].attached(),
                    "replacement did not attach fresh registration"
                );
                let current = &self.server.applications[&123].state.lifetime;
                assert!(Arc::ptr_eq(
                    current,
                    &self.sources[1].context.upgrade().unwrap().lifetime
                ));
                self.server.retire(123);
                self.exits.recv_timeout(Duration::from_secs(3)).unwrap();
                self.finish();
            }
            1 => {
                assert_eq!(wake, Wake::WindowServer);
                self.server.ingest_native_events(now);
                let movement = self.server.applications[&123].state.movement.unwrap();
                self.drain_calls();
                let entered = self
                    .log
                    .iter()
                    .find_map(|call| match call {
                        SourceCall::CallbackEntered(at, _) => Some(*at),
                        _ => None,
                    })
                    .unwrap();
                assert!(
                    movement.first >= entered && movement.last <= now,
                    "callback timestamp must reach UI acceptance"
                );
                assert!(
                    !self
                        .log
                        .iter()
                        .any(|call| matches!(call, SourceCall::CallbackExited(..))),
                    "retirement must occur during the active callback"
                );
                self.server.next_refresh = now;
                self.server.refresh_applications_with(
                    now,
                    Vec::<(i32, bool, ())>::new,
                    |_, _| unreachable!(),
                );
                assert!(!self.server.applications.contains_key(&123));
                assert!(matches!(
                    self.server.take_observations().as_slice(),
                    [Observation::AppGone { .. }]
                ));
                assert!(!self.sources[0].attached());
                self.wait_removal();
                assert!(self.sources[0].context.upgrade().is_some());
                self.removal.take().unwrap().send(()).unwrap();
                self.exits.recv_timeout(Duration::from_secs(3)).unwrap();
                assert!(
                    self.sources[0].context.upgrade().is_some(),
                    "callback must retain context after removal finishes"
                );
                self.stage = 2;
            }
            2 => {
                assert_eq!(wake, Wake::Ui);
                self.drain_calls();
                let exited = self
                    .log
                    .iter()
                    .find_map(|call| match call {
                        SourceCall::CallbackExited(at, _) => Some(*at),
                        _ => None,
                    })
                    .unwrap();
                assert!(exited <= now);
                assert_eq!(
                    self.sources[0].context.upgrade().is_some(),
                    self.case == Case::FailedRemoval
                );
                let detach = self
                    .log
                    .iter()
                    .position(|call| matches!(call, SourceCall::Detached(_)))
                    .unwrap();
                let remove = self
                    .log
                    .iter()
                    .position(|call| matches!(call, SourceCall::Removing(_)))
                    .unwrap();
                assert!(detach < remove);
                self.finish();
            }
            _ => unreachable!(),
        }
    }
    fn on_stopping(&mut self) {
        self.server.shutdown(Vec::new());
        assert!(
            self.complete.get(),
            "observer case stopped before completion at stage {}",
            self.stage
        );
        assert!(self.sources.iter().all(|source| !source.attached()));
        let windows = own_windows();
        eprintln!("WINDOW_STOP {} {:?}", self.name, windows);
        assert_eq!(
            windows, self.baseline_windows,
            "observer case created or changed a private native window"
        );
        crate::platform::macos::ui::validated(&self.name);
    }
}

fn own_windows() -> Vec<(isize, String, bool)> {
    let main = MainThreadMarker::new().unwrap();
    NSApplication::sharedApplication(main)
        .windows()
        .iter()
        .map(|window| {
            (
                window.windowNumber(),
                window.class().name().to_string_lossy().into_owned(),
                window.isVisible(),
            )
        })
        .collect()
}

pub(in crate::platform::macos) fn run(name: &str) {
    let case = match name {
        "observer_lifetime" => Case::Lifetime,
        "observer_failed_removal" => Case::FailedRemoval,
        "observer_replacement" => Case::Replacement,
        "observer_late_setup" => Case::LateSetup,
        "observer_disconnect" => Case::Disconnect,
        "observer_wrong_lifetime" => Case::WrongLifetime,
        _ => panic!("unknown observer case"),
    };
    let main = MainThreadMarker::new().unwrap();
    let native_application = NSApplication::sharedApplication(main);
    assert!(
        native_application.windows().is_empty(),
        "private harness started with native windows"
    );
    let (events, intake) = mpsc::channel();
    let binding = Arc::new(OnceLock::new());
    let focus = Arc::new(SharedFocus::new());
    let mut server = MacOsWindowServer::new(
        intake,
        events.clone(),
        focus.clone(),
        binding.clone(),
        Arc::new(AtomicBool::new(false)),
        Instant::now(),
    );
    let lifetime = Arc::new(ApplicationLifetime { pid: 123 });
    let (calls, call_receiver) = mpsc::channel();
    let (probes, probe_receiver) = mpsc::channel();
    let (release_removal, removal) = mpsc::channel();
    let (release_setup, setup) = mpsc::channel();
    let removal = RefCell::new(Some(removal));
    let setup = RefCell::new((case == Case::LateSetup).then_some(setup));
    let (exited, exits) = mpsc::channel();
    let (buffered, buffered_receiver) = mpsc::channel();
    let factory_binding = binding.clone();
    let factory_lifetime = lifetime.clone();
    let generations = Cell::new(0);
    let result_senders = Rc::new(RefCell::new(Vec::new()));
    let factory_senders = result_senders.clone();
    let native_app = Arc::new(AXApp::new(&NSRunningApplication::currentApplication()));
    let owner = AccessibilityApplication::with_native(
        lifetime.clone(),
        false,
        false,
        Box::new(move |results, handles| {
            let generation = generations.get();
            generations.set(generation + 1);
            factory_senders.borrow_mut().push(results.clone());
            Box::new(Native {
                access: ApplicationAccess::new(
                    if case == Case::WrongLifetime {
                        Arc::new(ApplicationLifetime { pid: 123 })
                    } else {
                        factory_lifetime.clone()
                    },
                    native_app.clone(),
                    focus.clone(),
                    events.clone(),
                    results.clone(),
                    factory_binding.clone(),
                    handles,
                ),
                calls: calls.clone(),
                probes: probes.clone(),
                removal: if case != Case::Replacement {
                    removal.borrow_mut().take()
                } else {
                    None
                },
                setup: setup.borrow_mut().take(),
                case,
                buffered: buffered.clone(),
                panic_on_read: generation == 0 && case == Case::Replacement,
            })
        }),
        Box::new(move |job| {
            let exited = exited.clone();
            thread::Builder::new().spawn(move || {
                let _exit = Exit(exited);
                job();
            })
        }),
    );
    server.applications.insert(
        123,
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
    let handle = Rc::new(OnceLock::new());
    let complete = Rc::new(Cell::new(false));
    let app = App::new(
        Icon::from_png(include_bytes!(
            "../../../../resources/macos/status_bar_icon.png"
        ))
        .unwrap(),
        Box::new(Handler {
            name: name.to_owned(),
            baseline_windows: Vec::new(),
            server,
            handle: handle.clone(),
            case,
            calls: call_receiver,
            log: Vec::new(),
            probes: probe_receiver,
            sources: Vec::new(),
            removal: Some(release_removal),
            setup: Some(release_setup),
            exits,
            buffered: buffered_receiver,
            stage: 0,
            complete: complete.clone(),
            retained_context: None,
            result_senders,
        }),
    )
    .unwrap();
    assert!(binding.set(app.waker()).is_ok());
    assert!(handle.set(app.handle()).is_ok());
    crate::platform::macos::ui::run_bounded_native_test(app);
}
