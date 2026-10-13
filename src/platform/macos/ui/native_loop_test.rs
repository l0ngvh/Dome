use super::test_support::*;
use super::*;
use crate::core::{Length, PixelRect};
use crate::platform::macos::dispatcher::{
    AccessibilityApplication, ApplicationCommand, ApplicationNative, ApplicationOperation,
    DispatcherMarker,
};
use crate::platform::macos::dome::events::{FocusTarget, WindowCommand};
use crate::platform::macos::window_server::InspectionKind;
use crate::platform::macos::window_server::{ApplicationLifetime, Observation, RecoveryEntry};
use dome_auxiliary_window::LoopHandle;
use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const CASES: &[&str] = &[
    "typed_passes_and_keyed_deadlines",
    "shutdown_while_worker_is_held",
    "domain_exit_return",
    "domain_exit_unwind",
    "observer_lifetime",
    "observer_failed_removal",
    "observer_replacement",
    "observer_late_setup",
    "observer_disconnect",
    "observer_wrong_lifetime",
];

pub(crate) fn run() {
    let arguments: Vec<_> = std::env::args().collect();
    if let Some(index) = arguments.iter().position(|argument| argument == "--child") {
        assert!(MainThreadMarker::new().is_some());
        match arguments.get(index + 1).map(String::as_str) {
            Some("typed_passes_and_keyed_deadlines") => typed_passes_and_keyed_deadlines(),
            Some("shutdown_while_worker_is_held") => shutdown_while_worker_is_held(),
            Some("domain_exit_return") => domain_exit_wakes_ui(false),
            Some("domain_exit_unwind") => domain_exit_wakes_ui(true),
            Some(case) if case.starts_with("observer_") => {
                crate::platform::macos::window_server::run_native_observer_test(case)
            }
            other => panic!("unknown native UI case {other:?}"),
        }
        return;
    }
    if !arguments.iter().any(|argument| argument == "--desktop") {
        println!("UNVERIFIED native UI loop. Run with --desktop on an approved macOS desktop.");
        return;
    }
    let selected = arguments
        .iter()
        .position(|argument| argument == "--case")
        .map(|index| arguments.get(index + 1).expect("--case requires a name"));
    if let Some(selected) = selected {
        assert!(CASES.contains(&selected.as_str()), "unknown native UI case");
    }
    for case in CASES
        .iter()
        .filter(|case| selected.is_none_or(|name| name == **case))
    {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--child", case])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                let mut trace = String::new();
                child
                    .stdout
                    .take()
                    .unwrap()
                    .read_to_string(&mut trace)
                    .unwrap();
                print!("{trace}");
                assert!(
                    status.success(),
                    "native UI case {case} failed with {status}"
                );
                assert_eq!(
                    trace
                        .lines()
                        .filter(|line| *line == format!("VALIDATED {case}"))
                        .count(),
                    1,
                    "native UI case {case} exited without exactly one case-specific validation"
                );
                println!("PASS {case}");
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("native UI case {case} timed out");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

fn bind_callbacks(app: &App<Wake>, callbacks: &OnceCell<WindowLoopCallbacks>, log: &Log) {
    let waker = app.waker();
    let deadline_handle = app.handle();
    let stop_handle = app.handle();
    let terminate_handle = app.handle();
    let log = log.clone();
    assert!(
        callbacks
            .set(WindowLoopCallbacks {
                wake: Box::new(move |wake| waker.wake(wake)),
                wake_at: Box::new(move |wake, deadline| deadline_handle.wake_at(wake, deadline)),
                stop_wakes: Box::new(move || {
                    stop_handle.stop_wakes();
                    log.borrow_mut().push("stop wakes".into());
                }),
                terminate: Box::new(move || terminate_handle.terminate()),
            })
            .is_ok()
    );
}

#[derive(Debug, Default)]
struct Progress {
    first_focus: Option<Instant>,
    last_focus: Option<Instant>,
    pending_focus_passes: usize,
    refreshed: bool,
    captured: bool,
    commands: bool,
    stop_queued: bool,
    stopped: usize,
}

struct NativeHandler {
    handler: WindowLoopHandler,
    log: Log,
    progress: Rc<RefCell<Progress>>,
    server_deadline: Rc<Cell<Instant>>,
    sender: mpsc::Sender<HubMessage>,
    ui_sender: Rc<OnceCell<UiSender>>,
    capture_sender: Rc<OnceCell<CaptureSender>>,
    focus: Arc<SharedFocus>,
    recovery: Rc<RefCell<Vec<RecoveryEntry>>>,
}

impl AppHandler for NativeHandler {
    type Wake = Wake;

    fn on_started(&mut self, shell: &Shell) {
        self.server_deadline
            .set(Instant::now() + Duration::from_millis(50));
        self.handler.on_started(shell);
    }

    fn on_wake(&mut self, shell: &Shell, wake: Wake) {
        let start = self.log.borrow().len();
        let dispatch_started = Instant::now();
        self.handler.on_wake(shell, wake);
        let entries = self.log.borrow()[start..].to_vec();
        let mut progress = self.progress.borrow_mut();
        match wake {
            Wake::WindowServer => {
                assert_eq!(&entries[..2], ["native", "results"]);
                assert!(
                    entries
                        .iter()
                        .all(|entry| matches!(entry.as_str(), "native" | "results" | "refresh"))
                );
                progress.refreshed |= entries.iter().any(|entry| entry == "refresh");
            }
            Wake::FocusRequest => {
                assert!(entries.iter().all(|entry| entry.starts_with("focus ")));
                if entries == ["focus 1"] {
                    assert!(progress.first_focus.replace(dispatch_started).is_none());
                    self.sender
                        .send(HubMessage::Command(WindowCommand::Close(1)))
                        .unwrap();
                    self.capture_sender
                        .get()
                        .unwrap()
                        .send(CaptureMessage::Frame {
                            cg_id: 1,
                            surface: IOSurface::new(),
                        });
                } else if entries == ["focus 3"] {
                    let submitted = Instant::now();
                    assert!(
                        submitted.duration_since(progress.first_focus.unwrap())
                            >= Duration::from_millis(100)
                    );
                    assert!(progress.last_focus.replace(submitted).is_none());
                } else {
                    assert!(
                        entries.is_empty(),
                        "unexpected focus submission {entries:?}"
                    );
                    progress.pending_focus_passes += 1;
                }
            }
            Wake::Capture => {
                assert_eq!(entries, ["capture 1"]);
                assert!(!progress.captured);
                progress.captured = true;
                let sender = self.ui_sender.get().unwrap().clone();
                thread::spawn(move || {
                    sender.send(HubMessage::Command(WindowCommand::Unminimize(1)))
                })
                .join()
                .unwrap();
            }
            Wake::Ui if entries.iter().any(|entry| entry == "shutdown") => {
                assert_eq!(entries, ["observations", "stop wakes", "shutdown"]);
            }
            Wake::Ui if entries.iter().any(|entry| entry == "close 1") => {
                assert_eq!(entries, ["observations", "close 1", "unminimize 1"]);
                progress.commands = true;
                let focus = self.focus.clone();
                thread::spawn(move || {
                    focus.request(2);
                    focus.request(3);
                })
                .join()
                .unwrap();
            }
            Wake::Ui => {
                assert_eq!(
                    entries,
                    [
                        "observations",
                        "park 1",
                        "monitors",
                        "tiling",
                        "floats",
                        "tabs"
                    ]
                );
            }
        }
        if progress.refreshed
            && progress.last_focus.is_some()
            && progress.captured
            && progress.commands
            && !progress.stop_queued
        {
            assert!(progress.pending_focus_passes >= 2);
            progress.stop_queued = true;
            self.focus.request(4);
            self.sender
                .send(HubMessage::Command(WindowCommand::Close(1)))
                .unwrap();
            self.ui_sender.get().unwrap().send(HubMessage::Shutdown);
        }
    }

    fn on_stopping(&mut self) {
        self.handler.on_stopping();
        self.progress.borrow_mut().stopped += 1;
        let progress = self.progress.borrow();
        assert!(
            progress.stop_queued,
            "native delivery stopped before all positive controls {progress:?}"
        );
        assert_eq!(progress.stopped, 1);
        assert!(!self.focus.is_submitted(3));
        assert!(self.focus.next_deadline(Instant::now()).is_none());
        assert_eq!(self.recovery.borrow().len(), 1);
        assert_eq!(
            self.recovery.borrow()[0].original_rect,
            PixelRect::new(810, 440, 300, 200)
        );
        assert_eq!(
            self.log
                .borrow()
                .iter()
                .filter(|entry| *entry == "shutdown")
                .count(),
            1
        );
        validated("typed_passes_and_keyed_deadlines");
    }
}

fn typed_passes_and_keyed_deadlines() {
    let mut fixture = Fixture::new();
    fixture.discover();
    let mut scene = fixture.scene();
    scene.focus_request = Some(FocusTarget::Window(1));
    fixture.sender.send(HubMessage::Scene(scene)).unwrap();
    let focus = Arc::new(SharedFocus::new());
    fixture.handler.ui.focus = focus.clone();
    let callbacks = Rc::new(OnceCell::new());
    fixture.handler.callbacks = callbacks.clone();
    let progress = Rc::new(RefCell::new(Progress::default()));
    let ui_sender = Rc::new(OnceCell::new());
    let capture_sender = Rc::new(OnceCell::new());
    let app = App::new(
        Icon::from_png(STATUS_BAR_ICON_PNG).unwrap(),
        Box::new(NativeHandler {
            handler: fixture.handler,
            log: fixture.log.clone(),
            progress: progress.clone(),
            server_deadline: fixture.server_deadline,
            sender: fixture.sender.clone(),
            ui_sender: ui_sender.clone(),
            capture_sender: capture_sender.clone(),
            focus: focus.clone(),
            recovery: fixture.recovery.clone(),
        }),
    )
    .unwrap();
    bind_callbacks(&app, &callbacks, &fixture.log);
    let waker = app.waker();
    focus.bind_waker(move || waker.wake(Wake::FocusRequest));
    assert!(
        ui_sender
            .set(UiSender {
                sender: fixture.sender,
                waker: app.waker()
            })
            .is_ok()
    );
    assert!(
        capture_sender
            .set(CaptureSender {
                sender: fixture.capture_sender,
                waker: app.waker()
            })
            .is_ok()
    );
    run_bounded_native_test(app);
}

struct DomainExitHandler {
    log: Log,
    handler: WindowLoopHandler,
    sender: Rc<OnceCell<UiSender>>,
    unwind: bool,
    stopped: Rc<Cell<usize>>,
}
impl AppHandler for DomainExitHandler {
    type Wake = Wake;
    fn on_started(&mut self, _: &Shell) {
        let sender = self.sender.get().unwrap().clone();
        let unwind = self.unwind;
        let domain = thread::spawn(move || {
            let _guard = crate::platform::macos::runner::DomainExitGuard::new(Box::new(sender));
            assert!(!unwind, "injected domain setup panic");
        });
        assert_eq!(domain.join().is_err(), unwind);
    }
    fn on_wake(&mut self, shell: &Shell, wake: Wake) {
        assert_eq!(wake, Wake::Ui);
        self.handler.on_wake(shell, wake);
    }
    fn on_stopping(&mut self) {
        self.handler.on_stopping();
        self.stopped.set(self.stopped.get() + 1);
        assert_eq!(self.stopped.get(), 1);
        assert_eq!(
            *self.log.borrow(),
            ["observations", "stop wakes", "shutdown"]
        );
        validated(if self.unwind {
            "domain_exit_unwind"
        } else {
            "domain_exit_return"
        });
    }
}

fn domain_exit_wakes_ui(unwind: bool) {
    let mut fixture = Fixture::new();
    let callbacks = Rc::new(OnceCell::new());
    fixture.handler.callbacks = callbacks.clone();
    let sender = Rc::new(OnceCell::new());
    let stopped = Rc::new(Cell::new(0));
    let app = App::new(
        Icon::from_png(STATUS_BAR_ICON_PNG).unwrap(),
        Box::new(DomainExitHandler {
            log: fixture.log.clone(),
            handler: fixture.handler,
            sender: sender.clone(),
            unwind,
            stopped: stopped.clone(),
        }),
    )
    .unwrap();
    bind_callbacks(&app, &callbacks, &fixture.log);
    assert!(
        sender
            .set(UiSender {
                sender: fixture.sender,
                waker: app.waker(),
            })
            .is_ok()
    );
    run_bounded_native_test(app);
}

struct HeldNative {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    completed: mpsc::Sender<&'static str>,
}
impl ApplicationNative for HeldNative {
    fn probe(&mut self, _: &DispatcherMarker) {
        self.entered.send(()).unwrap();
        self.release.recv().unwrap();
    }
    fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
        match operation {
            ApplicationOperation::Restore(_) => self.completed.send("restore").unwrap(),
            ApplicationOperation::Terminate => self.completed.send("terminate").unwrap(),
            _ => panic!("shutdown failed to prune queued native work"),
        }
    }
}

struct ShutdownServer {
    owner: AccessibilityApplication,
    log: Log,
}
impl WindowServer for ShutdownServer {
    fn ingest_native_events(&mut self, _: Instant) {
        panic!("shutdown polled native events")
    }
    fn ingest_read_results(&mut self, _: Instant) {
        panic!("shutdown polled results")
    }
    fn has_observations(&self) -> bool {
        panic!("shutdown polled output")
    }
    fn take_observations(&mut self) -> Vec<Observation> {
        Vec::new()
    }
    fn next_deadline(&self) -> Option<Instant> {
        panic!("shutdown polled deadline")
    }
    fn refresh_applications(&mut self, _: Instant) {
        panic!("shutdown refreshed applications")
    }
    fn set_frame(&mut self, _: CGWindowID, _: PixelRect) {
        panic!("shutdown placed a window")
    }
    fn move_offscreen(&mut self, _: CGWindowID, _: (Length, Length)) {
        panic!("shutdown parked a window")
    }
    fn minimize(&mut self, _: CGWindowID) {
        panic!("shutdown minimized a window")
    }
    fn unminimize(&mut self, _: CGWindowID) {
        panic!("shutdown restored a window")
    }
    fn close(&mut self, _: CGWindowID) {
        panic!("shutdown closed a window")
    }
    fn focus(&mut self, _: CGWindowID) {
        panic!("shutdown focused a window")
    }
    fn recover(&mut self, _: CGWindowID, _: PixelRect) {
        panic!("shutdown recovered inline")
    }
    fn shutdown(&mut self, recovery: Vec<RecoveryEntry>) {
        assert_eq!(*self.log.borrow(), ["stop wakes"]);
        self.owner.shutdown(recovery);
        self.log.borrow_mut().push("shutdown".into());
    }
}

struct ShutdownHandler {
    log: Log,
    release: Option<mpsc::Sender<()>>,
    completion: mpsc::Receiver<&'static str>,
    handler: WindowLoopHandler,
    sender: Rc<OnceCell<UiSender>>,
    handle: Rc<OnceCell<LoopHandle<Wake>>>,
    stopped: Rc<Cell<usize>>,
}
impl AppHandler for ShutdownHandler {
    type Wake = Wake;
    fn on_started(&mut self, _: &Shell) {
        let deadline = Instant::now() + Duration::from_secs(1);
        for wake in [Wake::FocusRequest, Wake::WindowServer] {
            self.handle.get().unwrap().wake_at(wake, Some(deadline));
        }
        self.sender.get().unwrap().send(HubMessage::Shutdown);
    }
    fn on_wake(&mut self, shell: &Shell, wake: Wake) {
        assert_eq!(wake, Wake::Ui);
        self.handler.on_wake(shell, wake);
    }
    fn on_stopping(&mut self) {
        self.handler.on_stopping();
        self.stopped.set(self.stopped.get() + 1);
        assert_eq!(self.stopped.get(), 1);
        assert_eq!(*self.log.borrow(), ["stop wakes", "shutdown"]);
        assert!(self.completion.try_recv().is_err());
        self.release.take().unwrap().send(()).unwrap();
        assert_eq!(
            self.completion
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            "restore"
        );
        assert_eq!(
            self.completion
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            "terminate"
        );
        validated("shutdown_while_worker_is_held");
    }
}

fn shutdown_while_worker_is_held() {
    let (entered, entering) = mpsc::channel();
    let (release, waiting) = mpsc::channel();
    let (completed, completion) = mpsc::channel();
    let native = RefCell::new(Some(HeldNative {
        entered,
        release: waiting,
        completed,
    }));
    let mut owner = AccessibilityApplication::with_native(
        Arc::new(ApplicationLifetime { pid: 99 }),
        false,
        false,
        Box::new(move |_, _| Box::new(native.borrow_mut().take().unwrap())),
        Box::new(|job| thread::Builder::new().spawn(job)),
    );
    owner.send(ApplicationCommand::Read(InspectionKind::AppWindows));
    entering.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut fixture = Fixture::new();
    let callbacks = Rc::new(OnceCell::new());
    fixture.handler.callbacks = callbacks.clone();
    fixture.handler.windows = Box::new(ShutdownServer {
        owner,
        log: fixture.log.clone(),
    });
    let sender = Rc::new(OnceCell::new());
    let handle = Rc::new(OnceCell::new());
    let stopped = Rc::new(Cell::new(0));
    let app = App::new(
        Icon::from_png(STATUS_BAR_ICON_PNG).unwrap(),
        Box::new(ShutdownHandler {
            log: fixture.log.clone(),
            release: Some(release),
            completion,
            handler: fixture.handler,
            sender: sender.clone(),
            handle: handle.clone(),
            stopped: stopped.clone(),
        }),
    )
    .unwrap();
    bind_callbacks(&app, &callbacks, &fixture.log);
    assert!(
        sender
            .set(UiSender {
                sender: fixture.sender,
                waker: app.waker()
            })
            .is_ok()
    );
    assert!(handle.set(app.handle()).is_ok());
    run_bounded_native_test(app);
}

static WATCHDOG_EXPIRED: AtomicBool = AtomicBool::new(false);

pub(in crate::platform::macos) fn validated(case: &str) {
    assert!(
        !WATCHDOG_EXPIRED.load(Ordering::SeqCst),
        "watchdog cannot validate {case}"
    );
    println!("VALIDATED {case}");
    std::io::stdout().flush().unwrap();
}

/// AppKit termination exits the process before this function returns.
pub(in crate::platform::macos) fn run_bounded_native_test(app: App<Wake>) {
    let handle = app.handle();
    let mut deadline =
        crate::platform::macos::throttle::Throttle::new(Duration::from_secs(2), move |expired| {
            if expired {
                WATCHDOG_EXPIRED.store(true, Ordering::SeqCst);
                handle.terminate();
            }
        });
    deadline.submit(false);
    deadline.submit(true);
    app.run();
}
