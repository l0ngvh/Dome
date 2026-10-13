mod frames;

use super::*;
use crate::core::{ContainerPlacement, MonitorId, WindowId};
use dome::QueryDisplay;
use dome::events::{ExpectedWindow, MonitorScene, Scene, TilingWindowShow, WindowPlacement};
use handle::submission::{self, Submission};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::io::AsRawHandle;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Condvar, Mutex};
use std::time::Duration;
use ui::{FloatShow, InternalWindow, InternalWindows};
use window_server::NativeTrace;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LRESULT, RECT};
use windows::Win32::System::JobObjects::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

const WAIT: Duration = Duration::from_secs(5);
fn appearance() -> Appearance {
    Appearance {
        theme: crate::theme::Flavor::Latte,
        font: crate::font::FontConfig {
            size: 14.0,
            family: None,
        },
    }
}
#[derive(Debug, PartialEq, Eq)]
enum LoopEvent {
    Content,
    Appearance,
    Stop,
    Returned(bool),
}
enum RecordedContent {
    Monitors(Vec<MonitorScene>),
    Tiling(MonitorId, Vec<TilingWindowShow>, Vec<ContainerPlacement>),
    Floats(Vec<FloatShow>),
    Tabs(Vec<MonitorScene>),
}
struct InternalRecorder {
    content: Sender<RecordedContent>,
    events: Sender<LoopEvent>,
    panic_on_appearance: bool,
    native_overlays: HashMap<String, HWND>,
}
impl InternalWindows for InternalRecorder {
    fn set_monitors(&mut self, monitors: &[MonitorScene]) {
        self.content
            .send(RecordedContent::Monitors(monitors.to_vec()))
            .unwrap();
        self.events.send(LoopEvent::Content).unwrap();
    }
    fn show_tiling(
        &mut self,
        monitor: MonitorId,
        windows: &[TilingWindowShow],
        containers: &[ContainerPlacement],
    ) {
        self.content
            .send(RecordedContent::Tiling(
                monitor,
                windows.to_vec(),
                containers.to_vec(),
            ))
            .unwrap();
    }
    fn show_floats(&mut self, floats: &[FloatShow]) {
        self.content
            .send(RecordedContent::Floats(
                floats
                    .iter()
                    .map(|float| FloatShow {
                        window: float.window,
                        placement: float.placement,
                        scale: float.scale,
                        border_thickness: float.border_thickness,
                        corner_radius: float.corner_radius,
                    })
                    .collect(),
            ))
            .unwrap();
    }
    fn show_tab_bars(&mut self, monitors: &[MonitorScene]) {
        self.content
            .send(RecordedContent::Tabs(monitors.to_vec()))
            .unwrap();
    }
    fn stack(&mut self, window: InternalWindow, order: external::ZOrder) {
        use external::ManageOverlay;
        let name = match window {
            InternalWindow::TilingOverlay(monitor) => format!("tile-{monitor}"),
            InternalWindow::FloatOverlay(window) => format!("float-{window:?}"),
        };
        let window = *self
            .native_overlays
            .entry(name)
            .or_insert_with(frames::create_recording_overlay);
        handle::OverlayHwnd::new(window).set_z_order(order);
    }
    fn focus_tiling_overlay(&mut self, _: MonitorId) {}
    fn set_appearance(&mut self, _: &Appearance) {
        assert!(!self.panic_on_appearance, "native UI callback panic");
        self.events.send(LoopEvent::Appearance).unwrap();
    }
}
impl Drop for InternalRecorder {
    fn drop(&mut self) {
        for window in self.native_overlays.values() {
            let _ = unsafe { DestroyWindow(*window) };
        }
    }
}

struct LoopFixture {
    completed_pass: Sender<Sender<()>>,
    content: Receiver<RecordedContent>,
    thread: thread::JoinHandle<()>,
    readiness: Receiver<UiThreadReady>,
    reports: Receiver<HubEvent>,
    events: Receiver<LoopEvent>,
    native_thread: Receiver<u32>,
    submissions: Receiver<(Instant, Submission)>,
    trace: Receiver<(Instant, NativeTrace)>,
    deadlines: Receiver<(Wake, Option<Instant>)>,
    hub: HubSender,
}
fn start_loop(panic_on_appearance: bool) -> LoopFixture {
    start_loop_with_exit_barrier(panic_on_appearance, None)
}
fn start_loop_with_exit_barrier(
    panic_on_appearance: bool,
    exit_barrier: Option<Receiver<()>>,
) -> LoopFixture {
    let (completed_pass, pass_requests) = mpsc::channel();
    let (content, content_intake) = mpsc::channel();
    let (submissions, submission_intake) = mpsc::channel();
    let (deadlines, deadline_intake) = mpsc::channel();
    let (trace, trace_intake) = mpsc::channel();
    let (handshake, readiness) = mpsc::channel();
    let (events, recorded) = mpsc::channel();
    let (native_thread, native_thread_intake) = mpsc::channel();
    let (reports, report_intake) = mpsc::channel();
    let hub = HubSender {
        thread_id: 0,
        events: reports,
    };
    let ui_hub = hub.clone();
    let thread = thread::spawn(move || {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .ok()
            .unwrap();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            submission::observe_native_submissions(move |submission| {
                let _ = submissions.send((Instant::now(), submission.clone()));
            });
            let (scenes, intake) = mpsc::channel();
            let monitors = dome::Win32Display
                .get_all_monitors()
                .expect("native display enumeration required");
            assert!(!monitors.is_empty(), "interactive display required");
            assert!(
                scenes
                    .send(HubMessage::DisplayRefreshed(monitors.clone()))
                    .is_ok()
            );
            assert!(
                scenes
                    .send(HubMessage::Scene(policy_scene(&monitors)))
                    .is_ok()
            );
            let callbacks = Rc::new(OnceCell::new());
            let binding: Arc<OnceLock<LoopWaker<Wake>>> = Arc::new(OnceLock::new());
            let wake = binding.clone();
            let mut windows = WindowsWindowServer::new(Arc::new(move || {
                wake.get()
                    .expect("bound before native work")
                    .wake(Wake::WindowServer);
            }))
            .unwrap();
            windows.trace_native_acceptance(trace);
            let native = windows.native_sender();
            let app = App::new(
                Icon::from_resource_id(TRAY_ICON_RESOURCE_ID).expect("compiled test icon required"),
                Box::new(GuardedHandler {
                    pass_requests,
                    handler: WindowLoopHandler {
                        ui: UiState::new(
                            intake,
                            ui_hub.clone(),
                            Box::new(InternalRecorder {
                                content,
                                events: events.clone(),
                                panic_on_appearance,
                                native_overlays: HashMap::new(),
                            }),
                        ),
                        windows: Box::new(windows),
                        callbacks: callbacks.clone(),
                        hub_sender: ui_hub,
                        observe_foreground: Box::new(|| {}),
                        stopping: false,
                    },
                }),
            )
            .expect("interactive Windows desktop and shell required");
            assert!(binding.set(app.waker()).is_ok());
            let waker = app.waker();
            let deadline = app.handle();
            let stop = app.handle();
            let terminate = app.handle();
            let stopping = events.clone();
            assert!(
                callbacks
                    .set(WindowLoopCallbacks {
                        wake: Box::new(move |wake| waker.wake(wake)),
                        wake_at: Box::new(move |wake, at| {
                            let _ = deadlines.send((wake, at));
                            deadline.wake_at(wake, at);
                        }),
                        stop_wakes: Box::new(move || {
                            stop.stop_wakes();
                            stopping.send(LoopEvent::Stop).unwrap();
                        }),
                        terminate: Box::new(move || terminate.terminate()),
                    })
                    .is_ok()
            );
            native_thread.send(unsafe { GetCurrentThreadId() }).unwrap();
            assert!(
                handshake
                    .send(UiThreadReady {
                        ui: UiSender {
                            scenes,
                            waker: app.waker()
                        },
                        native
                    })
                    .is_ok()
            );
            app.waker().wake(Wake::Ui);
            app.run();
        }));
        events.send(LoopEvent::Returned(outcome.is_err())).unwrap();
        if let Some(exit_barrier) = exit_barrier {
            exit_barrier
                .recv_timeout(WAIT)
                .expect("release UI exit barrier");
        }
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    });
    wait_for_event(&recorded, LoopEvent::Content);
    LoopFixture {
        completed_pass,
        content: content_intake,
        submissions: submission_intake,
        trace: trace_intake,
        deadlines: deadline_intake,
        thread,
        readiness,
        reports: report_intake,
        events: recorded,
        native_thread: native_thread_intake,
        hub,
    }
}
struct GuardedHandler {
    handler: WindowLoopHandler,
    pass_requests: Receiver<Sender<()>>,
}
impl AppHandler for GuardedHandler {
    type Wake = Wake;
    fn on_wake(&mut self, shell: &Shell, wake: Wake) {
        // A request arriving after collection finishes belongs to the next Ui wake,
        // not the partial output of this callback.
        let completed: Vec<_> = if wake == Wake::Ui {
            self.pass_requests.try_iter().collect()
        } else {
            vec![]
        };
        if wake == Wake::WindowServer {
            self.handler.on_wake(shell, wake);
        } else {
            submission::without_queries(|| self.handler.on_wake(shell, wake));
        }
        for reply in completed {
            let _ = reply.send(());
        }
    }
    fn on_stopping(&mut self) {
        submission::without_queries(|| self.handler.on_stopping());
    }
    fn menu(&mut self) -> Vec<MenuEntry> {
        self.handler.menu()
    }
    fn on_menu_selected(&mut self, id: u32) {
        self.handler.on_menu_selected(id);
    }
    fn on_display_changed(&mut self) {
        self.handler.on_display_changed();
    }
    fn on_work_area_changed(&mut self) {
        self.handler.on_work_area_changed();
    }
}

fn policy_scene(monitors: &[dome::MonitorInfo]) -> Scene {
    let (runtime, _configuration) =
        crate::config::lua::test_support::loaded_runtime("native_monitor_policy", "return {}");
    let mut hub = crate::core::Hub::new(
        (&monitors[0]).into(),
        crate::config::tests::tiling_config(),
        PreferredLayouts::default(),
        runtime,
    );
    let mut identifiers = vec![hub.primary_monitor()];
    identifiers.extend(
        monitors[1..]
            .iter()
            .map(|monitor| hub.add_monitor(monitor.into())),
    );
    let mut scene = empty_scene(&[]);
    scene.monitors = monitors
        .iter()
        .enumerate()
        .map(|(index, monitor)| MonitorScene {
            id: identifiers[index],
            native: monitor.handle,
            work_area: monitor.work_area,
            system_work_area: monitor.work_area,
            scale: monitor.scale,
            border_thickness: crate::core::Pixels::new(2),
            tiling_windows: vec![],
            float_windows: vec![],
            containers: vec![],
        })
        .collect();
    scene
}

fn wait_for_event(events: &Receiver<LoopEvent>, expected: LoopEvent) {
    assert_eq!(
        events
            .recv_timeout(WAIT)
            .expect("native UI made no progress"),
        expected
    );
}
fn wait_for_discovery(reports: &Receiver<HubEvent>, window: HwndId) {
    let until = Instant::now() + WAIT;
    loop {
        let report = reports
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .expect("concrete discovery completion did not wake UI intake");
        if let HubEvent::WindowsDiscovered(discovered) = report {
            assert!(discovered.iter().any(|entry| entry.key == window));
            assert!(
                discovered
                    .iter()
                    .all(|entry| entry.process_id != std::process::id())
            );
            return;
        }
    }
}
fn empty_scene(windows: &[HwndId]) -> Scene {
    Scene {
        windows: windows
            .iter()
            .enumerate()
            .map(|(index, &key)| {
                (
                    key,
                    ExpectedWindow {
                        id: WindowId::new(index),
                        placement: WindowPlacement::Hidden,
                    },
                )
            })
            .collect(),
        monitors: vec![],
        focused_window: None,
        focus_request: None,
        workspaces: vec![],
    }
}
fn coordinate_case(unwind: bool, acknowledgment: Option<bool>) {
    let main_thread = unsafe { GetCurrentThreadId() };
    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    let (release_ui, exit_barrier) = mpsc::channel();
    let fixture = start_loop_with_exit_barrier(false, Some(exit_barrier));
    let (startup, ready) = mpsc::channel();
    let (enumerated, enumeration) = mpsc::channel();
    let (finished, completion) = mpsc::channel();
    let domain = thread::spawn(move || {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            coordinate_ui_startup(
                fixture.thread,
                fixture.readiness,
                startup,
                fixture.hub,
                |_| {
                    let mut count = 0;
                    handle::enum_windows(|_| count += 1).unwrap();
                    enumerated.send(count).unwrap();
                    assert!(!unwind, "native domain unwind");
                },
            );
        }));
        finish_domain_thread(main_thread, outcome.is_err());
        finished.send(outcome.is_err()).unwrap();
    });
    let ready = ready
        .recv_timeout(WAIT)
        .expect("startup readiness stranded");
    assert!(
        matches!(
            enumeration.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "enumeration ran before hook acknowledgment"
    );
    match acknowledgment {
        Some(success) => ready.acknowledged.send(success).unwrap(),
        None => drop(ready.acknowledged),
    }
    if acknowledgment == Some(true) {
        assert!(
            enumeration.recv_timeout(WAIT).unwrap() > 0,
            "native enumeration never ran"
        );
    }
    wait_for_event(&fixture.events, LoopEvent::Stop);
    wait_for_event(&fixture.events, LoopEvent::Returned(false));
    assert!(
        matches!(
            completion.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "domain exited without joining the UI thread"
    );
    assert!(
        !unsafe { PeekMessageW(&mut message, None, WM_QUIT, WM_QUIT, PM_REMOVE) }.as_bool(),
        "main quit preceded UI join"
    );
    release_ui.send(()).unwrap();
    assert_eq!(
        completion
            .recv_timeout(WAIT)
            .expect("domain waited forever for UI join"),
        unwind
    );
    domain.join().unwrap();
    assert!(
        unsafe { PeekMessageW(&mut message, None, WM_QUIT, WM_QUIT, PM_REMOVE) }.as_bool(),
        "domain exit did not signal main after joining UI"
    );
    assert!(
        fixture.events.try_recv().is_err(),
        "stopping ran more than once"
    );
    if acknowledgment != Some(true) {
        assert!(
            enumeration.try_recv().is_err(),
            "failed hooks allowed enumeration"
        );
    }
}
unsafe extern "system" fn find_tray(window: HWND, context: LPARAM) -> windows::core::BOOL {
    let mut name = [0u16; 64];
    let length = unsafe { GetClassNameW(window, &mut name) };
    if String::from_utf16_lossy(&name[..length as usize]) == "DomeTrayWindow" {
        unsafe { *(context.0 as *mut HWND) = window };
        return windows::core::BOOL(0);
    }
    windows::core::BOOL(1)
}
fn open_tray_menu(thread_id: u32) {
    let mut tray = HWND::default();
    let _ = unsafe {
        EnumThreadWindows(
            thread_id,
            Some(find_tray),
            LPARAM(&mut tray as *mut _ as isize),
        )
    };
    assert!(
        !tray.is_invalid(),
        "fixture tray window not found on its UI thread"
    );
    unsafe {
        PostMessageW(
            Some(tray),
            WM_APP + 1,
            WPARAM(0),
            LPARAM(WM_RBUTTONUP as isize),
        )
    }
    .unwrap();
    let until = Instant::now() + WAIT;
    loop {
        let mut state = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetGUIThreadInfo(thread_id, &mut state) }.unwrap();
        if state.flags & GUI_INMENUMODE != GUITHREADINFO_FLAGS(0) {
            assert_eq!(state.hwndMenuOwner, tray);
            return;
        }
        assert!(
            Instant::now() < until,
            "tray never entered native menu tracking"
        );
        thread::sleep(Duration::from_millis(2));
    }
}
fn menu_exit_case(panic: bool) {
    let fixture = start_loop(panic);
    let mut ready = fixture.readiness.recv_timeout(WAIT).unwrap();
    let thread_id = fixture.native_thread.recv_timeout(WAIT).unwrap();
    open_tray_menu(thread_id);
    let guard = DomainExitGuard {
        ui: ready.ui.clone(),
        thread: Some(fixture.thread),
    };
    if panic {
        ready.ui.send(HubMessage::AppearanceChanged(appearance()));
    }
    drop(guard);
    wait_for_event(&fixture.events, LoopEvent::Stop);
    wait_for_event(&fixture.events, LoopEvent::Returned(panic));
    ready.ui.send(HubMessage::AppearanceChanged(appearance()));
    assert!(
        ready.ui.scenes.send(HubMessage::Shutdown).is_err(),
        "UI intake survived join"
    );
    assert!(
        fixture.events.try_recv().is_err(),
        "late callback or repeated recovery after join"
    );
}
fn completion_case() {
    let foreign = ForeignWindow::spawn();
    let fixture = start_loop(false);
    let mut ready = fixture.readiness.recv_timeout(WAIT).unwrap();
    ready.native.send(NativeEvent::Discover(foreign.window));
    wait_for_discovery(&fixture.reports, foreign.window);
    ready.ui.send(HubMessage::AppearanceChanged(appearance()));
    wait_for_event(&fixture.events, LoopEvent::Appearance);
    drop(DomainExitGuard {
        ui: ready.ui.clone(),
        thread: Some(fixture.thread),
    });
    wait_for_event(&fixture.events, LoopEvent::Stop);
    wait_for_event(&fixture.events, LoopEvent::Returned(false));
    ready.native.send(NativeEvent::Title(foreign.window));
    assert!(fixture.events.try_recv().is_err());
}
fn menu_delivery_case() {
    frames::menu_case();
}

fn hung_pump_case() {
    let mut foreign = ForeignWindow::spawn();
    let mut peer = ForeignWindow::spawn();
    let fixture = start_loop(false);
    let mut ready = fixture.readiness.recv_timeout(WAIT).unwrap();
    for window in [foreign.window, peer.window] {
        ready.native.send(NativeEvent::Discover(window));
        wait_for_discovery(&fixture.reports, window);
    }
    foreign.command("pause");
    foreign.response("PAUSED");
    ready.native.send(NativeEvent::Title(foreign.window));
    ready.ui.send(HubMessage::Scene(empty_scene(&[
        foreign.window,
        peer.window,
    ])));
    wait_for_event(&fixture.events, LoopEvent::Content);
    peer.wait_origin(-32000, -32000);
    drop(DomainExitGuard {
        ui: ready.ui.clone(),
        thread: Some(fixture.thread),
    });
    wait_for_event(&fixture.events, LoopEvent::Stop);
    wait_for_event(&fixture.events, LoopEvent::Returned(false));
    peer.wait_origin(100, 100);
    foreign.command("resume");
    foreign.response("RESUMED");
    foreign.wait_origin(100, 100);
    assert!(
        fixture.events.try_recv().is_err(),
        "receiver loss delivered a callback after join"
    );
}
struct ForeignWindow {
    child: Child,
    commands: ChildStdin,
    replies: Receiver<String>,
    window: HwndId,
}
impl ForeignWindow {
    fn spawn() -> Self {
        Self::spawn_with_awareness(false)
    }
    fn spawn_with_awareness(unaware: bool) -> Self {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "platform::windows::native_ui::control_foreign_pump_serves_bounds_and_resumes_before_exit", "--ignored", "--nocapture", "--test-threads=1"])
            .env("DOME_FOREIGN_FIXTURE", "1")
            .env("DOME_FOREIGN_UNAWARE", if unaware { "1" } else { "0" })
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
            .spawn().expect("spawn foreign-process window fixture");
        let commands = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (replies, intake) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                if replies.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        let mut fixture = Self {
            child,
            commands,
            replies: intake,
            window: HwndId::test(0),
        };
        let line = fixture.response("READY ");
        fixture.window = HwndId::test(line.trim().parse().expect("native fixture handle"));
        fixture
    }
    fn command(&mut self, command: &str) {
        writeln!(self.commands, "{command}").unwrap();
        self.commands.flush().unwrap();
    }
    fn response(&self, prefix: &str) -> String {
        let until = Instant::now() + WAIT;
        loop {
            let line = self
                .replies
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .expect("foreign window made no progress");
            if let Some((_, suffix)) = line.split_once(&format!("FIXTURE_{prefix}")) {
                return suffix.into();
            }
        }
    }
    fn wait_origin(&mut self, x: i32, y: i32) {
        let until = Instant::now() + WAIT;
        loop {
            self.command("bounds");
            let bounds = self.response("BOUNDS ");
            let actual: Vec<i32> = bounds
                .split_whitespace()
                .map(|part| part.parse().unwrap())
                .collect();
            if actual[0..2] == [x, y] {
                return;
            }
            assert!(
                Instant::now() < until,
                "foreign native origin {actual:?}, expected {x},{y}"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for ForeignWindow {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let until = Instant::now() + WAIT;
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < until {
            thread::sleep(Duration::from_millis(10));
        }
    }
}
static FOREIGN_PAUSE: OnceLock<(Mutex<Option<bool>>, Condvar)> = OnceLock::new();

#[test]
#[ignore = "foreign-process child entry, requires parent protocol and Windows desktop"]
fn control_foreign_pump_serves_bounds_and_resumes_before_exit() {
    assert_eq!(std::env::var("DOME_FOREIGN_FIXTURE").as_deref(), Ok("1"));
    frames::set_foreign_awareness();
    FOREIGN_PAUSE
        .set((Mutex::new(Some(false)), Condvar::new()))
        .unwrap();
    let class = WNDCLASSW {
        lpfnWndProc: Some(foreign_procedure),
        lpszClassName: w!("DomeForeignOwnershipFixture"),
        ..Default::default()
    };
    assert_ne!(unsafe { RegisterClassW(&class) }, 0);
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            w!("Dome foreign ownership fixture"),
            WS_OVERLAPPEDWINDOW,
            240,
            240,
            640,
            480,
            None,
            None,
            None,
            None,
        )
    }
    .expect("native fixture window required");
    unsafe {
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
    }
    println!("FIXTURE_READY {}", window.0 as isize);
    std::io::stdout().flush().unwrap();
    let key = HwndId::from(window);
    thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            match line.unwrap().as_str() {
                "pause" => {
                    unsafe { PostMessageW(Some(key.into()), WM_APP + 41, WPARAM(0), LPARAM(0)) }
                        .unwrap()
                }
                "bounds" => {
                    unsafe { PostMessageW(Some(key.into()), WM_APP + 42, WPARAM(0), LPARAM(0)) }
                        .unwrap()
                }
                "title" => {
                    unsafe { PostMessageW(Some(key.into()), WM_APP + 43, WPARAM(0), LPARAM(0)) }
                        .unwrap()
                }
                "resume" => {
                    let (paused, resume) = FOREIGN_PAUSE.get().unwrap();
                    *paused.lock().unwrap() = Some(false);
                    resume.notify_all();
                }
                other => frames::foreign_command(key, other),
            }
        }
        let (paused, resume) = FOREIGN_PAUSE.get().unwrap();
        *paused.lock().unwrap() = None;
        resume.notify_all();
        let _ = unsafe { PostMessageW(Some(key.into()), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    });
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

struct FixtureProcessTree {
    job: HANDLE,
    child: Option<Child>,
}
impl FixtureProcessTree {
    fn new() -> Self {
        let job = unsafe { CreateJobObjectW(None, None) }.expect("fixture Job Object required");
        let tree = Self { job, child: None };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .expect("fixture kill-on-close containment required");
        tree
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        let terminated = unsafe { TerminateJobObject(self.job, 1) };
        if let Some(child) = &mut self.child {
            let _ = child.kill();
        }
        terminated?;
        let until = Instant::now() + WAIT;
        loop {
            let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            unsafe {
                QueryInformationJobObject(
                    Some(self.job),
                    JobObjectBasicAccountingInformation,
                    &mut accounting as *mut _ as *mut _,
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    None,
                )
            }?;
            let child_exited = match &mut self.child {
                Some(child) => child.try_wait()?.is_some(),
                None => true,
            };
            if accounting.ActiveProcesses == 0 && child_exited {
                return Ok(());
            }
            anyhow::ensure!(
                Instant::now() < until,
                "fixture process tree did not terminate within cleanup deadline"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for FixtureProcessTree {
    fn drop(&mut self) {
        let _ = self.stop();
        let _ = unsafe { CloseHandle(self.job) };
    }
}
struct CapturedOutput {
    bytes: Arc<Mutex<Vec<u8>>>,
    ended: Receiver<()>,
}
impl CapturedOutput {
    fn drain(mut pipe: impl Read + Send + 'static) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let buffer = bytes.clone();
        let (end, ended) = mpsc::channel();
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => buffer.lock().unwrap().extend_from_slice(&chunk[..count]),
                }
            }
            let _ = end.send(());
        });
        Self { bytes, ended }
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes.lock().unwrap()).into_owned()
    }
}
fn read_parent_release(expected: &str) {
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).unwrap();
    assert_eq!(
        line.trim(),
        expected,
        "outer fixture owner must authorize native work"
    );
}
fn run_case(case: &str) {
    let mut input = (case == "foreign_frames").then(frames::InputOwner::capture);
    let mut tree = FixtureProcessTree::new();
    tree.child = Some(Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "platform::windows::native_ui::exercise_startup_transport_and_shutdown_preserves_ui_progress", "--ignored", "--nocapture", "--test-threads=1"])
        .env("DOME_NATIVE_OWNERSHIP_CASE", case)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap());
    let child = tree.child.as_mut().unwrap();
    unsafe { AssignProcessToJobObject(tree.job, HANDLE(child.as_raw_handle())) }
        .expect("child must enter fixture job before bootstrap release");
    let mut commands = child.stdin.take().unwrap();
    let output = CapturedOutput::drain(child.stdout.take().unwrap());
    let errors = CapturedOutput::drain(child.stderr.take().unwrap());
    writeln!(commands, "CONTAINED").unwrap();
    commands.flush().unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    let mut authorized_drags = 0;
    let status = loop {
        let requested = output.text().matches("FIXTURE_INPUT_BEGIN").count();
        while authorized_drags < requested {
            input
                .as_mut()
                .expect("unexpected input injection case")
                .arm();
            writeln!(commands, "INPUT_OWNED").unwrap();
            commands.flush().unwrap();
            authorized_drags += 1;
        }
        if let Some(status) = tree.child.as_mut().unwrap().try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= until {
            break None;
        }
        thread::sleep(Duration::from_millis(10));
    };
    let cleanup = tree.stop();
    drop(tree);
    let input_cleanup = input.as_mut().map(frames::InputOwner::restore).transpose();
    let output_ended = output.ended.recv_timeout(WAIT);
    let errors_ended = errors.ended.recv_timeout(WAIT);
    let stdout = output.text();
    let stderr = errors.text();
    assert!(
        cleanup.is_ok() && input_cleanup.is_ok(),
        "native cleanup failed {cleanup:?} {input_cleanup:?}\n{stdout}\n{stderr}"
    );
    assert!(
        output_ended.is_ok() && errors_ended.is_ok(),
        "fixture pipes did not close within cleanup deadline\n{stdout}\n{stderr}"
    );
    assert!(
        status.is_some_and(|status| status.success()),
        "native case {case} failed or timed out\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains(&format!("OWNERSHIP_COMPLETE {case}")),
        "selected native test did not execute\n{stdout}\n{stderr}"
    );
}

#[test]
#[ignore = "child entry, requires an exact case selected by parent"]
fn exercise_startup_transport_and_shutdown_preserves_ui_progress() {
    let case = std::env::var("DOME_NATIVE_OWNERSHIP_CASE").expect("parent must select native case");
    read_parent_release("CONTAINED");
    ensure_per_monitor_v2_awareness().expect("per-monitor DPI awareness required");
    match case.as_str() {
        "return" => coordinate_case(false, Some(true)),
        "unwind" => coordinate_case(true, Some(true)),
        "hook_failure" => coordinate_case(false, Some(false)),
        "hook_disconnect" => coordinate_case(false, None),
        "completion" => completion_case(),
        "menu_shutdown" => menu_exit_case(false),
        "menu_panic" => menu_exit_case(true),
        "menu_delivery" => menu_delivery_case(),
        "hung_pump" => hung_pump_case(),
        "foreign_frames" => frames::mixed_dpi_case(),
        "native_order" => frames::native_order_case(),
        _ => panic!("unknown native case {case}"),
    }
    println!("OWNERSHIP_COMPLETE {case}");
}

macro_rules! native_case {
    ($name:ident, $($case:literal),+) => {
        #[test]
        #[ignore = "requires interactive Windows desktop and shell"]
        fn $name() { $(run_case($case);)+ }
    };
}
native_case!(
    coordinate_startup_waits_for_hooks_then_joins_ui_on_return_or_unwind,
    "return",
    "unwind",
    "hook_failure",
    "hook_disconnect"
);
native_case!(
    complete_native_discovery_wakes_ui_without_unrelated_delivery,
    "completion"
);
native_case!(
    shutdown_or_panic_during_open_menu_finishes_ui_join,
    "menu_shutdown",
    "menu_panic"
);
native_case!(
    deliver_hooks_completions_and_scene_while_menu_remains_open,
    "menu_delivery"
);
native_case!(
    park_and_recover_with_foreign_pump_stopped_keeps_peer_responsive,
    "hung_pump"
);
#[test]
#[ignore = "requires an isolated Windows desktop without concurrent input and 96/144 DPI displays"]
fn move_foreign_frame_preserves_destination_scale_and_owned_motion() {
    run_case("foreign_frames");
}
native_case!(native_scene_stacking_preserves_adjacency, "native_order");
unsafe extern "system" fn foreign_procedure(
    window: HWND,
    message: u32,
    word: WPARAM,
    long: LPARAM,
) -> LRESULT {
    match message {
        value if value == WM_APP + 41 => {
            let (paused, resume) = FOREIGN_PAUSE.get().unwrap();
            let mut paused = paused.lock().unwrap();
            if paused.is_none() {
                return LRESULT(0);
            }
            *paused = Some(true);
            println!("FIXTURE_PAUSED");
            std::io::stdout().flush().unwrap();
            while *paused == Some(true) {
                paused = resume.wait(paused).unwrap();
            }
            println!("FIXTURE_RESUMED");
            std::io::stdout().flush().unwrap();
            LRESULT(0)
        }
        value if value == WM_APP + 42 => {
            let mut bounds = RECT::default();
            unsafe { GetWindowRect(window, &mut bounds) }.unwrap();
            println!(
                "FIXTURE_BOUNDS {} {} {} {}",
                bounds.left, bounds.top, bounds.right, bounds.bottom
            );
            std::io::stdout().flush().unwrap();
            LRESULT(0)
        }
        value if value == WM_APP + 43 => {
            unsafe { SetWindowTextW(window, w!("Renamed ownership fixture")) }.unwrap();
            LRESULT(0)
        }
        value if value == WM_APP + 44 => frames::foreign_operation(window, word, long),
        WM_DESTROY => {
            if unsafe { GetWindow(window, GW_OWNER) }.is_err() {
                unsafe { PostQuitMessage(0) };
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, word, long) },
    }
}
