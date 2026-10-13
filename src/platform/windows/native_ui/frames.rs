use super::*;
use crate::config::lua::test_support::{TempFile, loaded_runtime};
use crate::core::{FloatWindowPlacement, Physical, PixelRect, TilingWindowPlacement};
use crate::platform::windows::dome::events::FocusTarget;
use crate::platform::windows::window_server::PlacementRead;
use std::cell::RefCell;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_UNAWARE, SetThreadDpiAwarenessContext};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEINPUT, SendInput,
};

unsafe extern "system" fn overlay_procedure(
    window: HWND,
    message: u32,
    word: WPARAM,
    long: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(window, message, word, long) }
}

pub(super) fn create_recording_overlay() -> HWND {
    let class = WNDCLASSW {
        lpfnWndProc: Some(overlay_procedure),
        lpszClassName: w!("DomeRecordingOwnershipOverlay"),
        ..Default::default()
    };
    unsafe { RegisterClassW(&class) };
    let window = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            class.lpszClassName,
            w!("Native ordering endpoint, no rendered content"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            None,
            None,
        )
    }
    .expect("native ordering endpoint required");
    unsafe {
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
    }
    window
}

pub(super) fn set_foreign_awareness() {
    let context = if std::env::var("DOME_FOREIGN_UNAWARE").as_deref() == Ok("1") {
        DPI_AWARENESS_CONTEXT_UNAWARE
    } else {
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
    };
    assert!(!unsafe { SetThreadDpiAwarenessContext(context) }.0.is_null());
}

pub(super) fn foreign_command(window: HwndId, command: &str) {
    let operation = match command {
        "dialog" => 1,
        "foreground" => 2,
        "shift" => 3,
        _ => panic!("unknown fixture command {command}"),
    };
    unsafe {
        PostMessageW(
            Some(window.into()),
            WM_APP + 44,
            WPARAM(operation),
            LPARAM(0),
        )
    }
    .unwrap();
}

pub(super) fn foreign_operation(window: HWND, operation: WPARAM, _: LPARAM) -> LRESULT {
    match operation.0 {
        1 => {
            let prior =
                unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
            assert!(!prior.0.is_null());
            let bounds = outer(window.into());
            let child = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_DLGMODALFRAME,
                    w!("DomeForeignOwnershipFixture"),
                    w!("Owned fixture dialog"),
                    WS_POPUP | WS_CAPTION | WS_SYSMENU,
                    bounds.left + 43,
                    bounds.top + 61,
                    240,
                    140,
                    Some(window),
                    None,
                    None,
                    None,
                )
            }
            .expect("same-thread owned dialog required");
            unsafe {
                let _ = ShowWindow(child, SW_SHOWNOACTIVATE);
                SetThreadDpiAwarenessContext(prior);
            }
            println!("FIXTURE_DIALOG {}", child.0 as isize);
        }
        2 => {
            let accepted = unsafe { SetForegroundWindow(window) }.as_bool();
            println!("FIXTURE_FOREGROUND {accepted}");
        }
        3 => {
            let bounds = outer(window.into());
            unsafe {
                SetWindowPos(
                    window,
                    None,
                    bounds.left + 29,
                    bounds.top + 17,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                )
            }
            .unwrap();
            println!("FIXTURE_SHIFTED");
        }
        _ => panic!("unknown foreign operation"),
    }
    std::io::stdout().flush().unwrap();
    LRESULT(0)
}

fn outer(window: HwndId) -> RECT {
    let mut bounds = RECT::default();
    unsafe { GetWindowRect(window.into(), &mut bounds) }
        .expect("fixture HWND must remain readable");
    bounds
}
fn pump() {
    let mut message = MSG::default();
    while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
        assert_ne!(message.message, WM_QUIT, "hook pump unexpectedly stopped");
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
fn until(mut condition: impl FnMut() -> bool, failure: &str) {
    let deadline = Instant::now() + WAIT;
    loop {
        pump();
        if condition() {
            return;
        }
        assert!(Instant::now() < deadline, "{failure}");
        thread::sleep(Duration::from_millis(2));
    }
}
fn menu_open(thread: u32) {
    let mut state = GUITHREADINFO {
        cbSize: size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetGUIThreadInfo(thread, &mut state) }.unwrap();
    assert_ne!(
        state.flags & GUI_INMENUMODE,
        GUITHREADINFO_FLAGS(0),
        "work required menu dismissal"
    );
}

struct NativeSession {
    completed_pass: Sender<Sender<()>>,
    content: Receiver<RecordedContent>,
    ready: UiThreadReady,
    reports: Receiver<HubEvent>,
    pending_reports: std::collections::VecDeque<HubEvent>,
    events: Receiver<LoopEvent>,
    submissions: Receiver<(Instant, Submission)>,
    trace: Receiver<(Instant, NativeTrace)>,
    deadlines: Receiver<(Wake, Option<Instant>)>,
    writes: Vec<(Instant, Submission)>,
    reads: Vec<(Instant, NativeTrace)>,
    thread: u32,
    hub: HubSender,
    _hooks: event_listener::EventHooks,
    _exit: DomainExitGuard,
}
impl NativeSession {
    fn new() -> Self {
        let fixture = start_loop(false);
        let ready = fixture.readiness.recv_timeout(WAIT).unwrap();
        let hooks = install_event_hooks(ready.native.clone()).expect("native hooks required");
        let exit = DomainExitGuard {
            ui: ready.ui.clone(),
            thread: Some(fixture.thread),
        };
        Self {
            completed_pass: fixture.completed_pass,
            content: fixture.content,
            ready,
            reports: fixture.reports,
            pending_reports: std::collections::VecDeque::new(),
            events: fixture.events,
            submissions: fixture.submissions,
            trace: fixture.trace,
            deadlines: fixture.deadlines,
            writes: vec![],
            reads: vec![],
            thread: fixture.native_thread.recv_timeout(WAIT).unwrap(),
            hub: fixture.hub,
            _hooks: hooks,
            _exit: exit,
        }
    }
    fn finish_pass(&mut self) {
        let (reply, completed) = mpsc::channel();
        self.completed_pass.send(reply).unwrap();
        self.ready.ui.waker.wake(Wake::Ui);
        completed
            .recv_timeout(WAIT)
            .expect("real App Ui pass did not complete");
        self.collect();
    }
    fn await_owned(&mut self, child: HwndId, expected: (i32, i32)) {
        until(
            || {
                self.collect();
                owned_targets(self, child).contains(&expected)
            },
            "owned child submission did not arrive",
        );
        self.finish_pass();
    }
    fn assert_content(&mut self, scene: &Scene) {
        self.finish_pass();
        let content: Vec<_> = self.content.try_iter().collect();
        let start = content
            .iter()
            .rposition(|entry| matches!(entry, RecordedContent::Monitors(_)))
            .expect("scene never reached internal content");
        let mut batch = content[start..].iter();
        let Some(RecordedContent::Monitors(monitors)) = batch.next() else {
            unreachable!()
        };
        assert_monitors(monitors, &scene.monitors);
        for monitor in &scene.monitors {
            let Some(RecordedContent::Tiling(id, tiles, containers)) = batch.next() else {
                panic!("missing tiling content boundary");
            };
            assert_eq!(*id, monitor.id);
            let expected: Vec<_> = monitor
                .tiling_windows
                .iter()
                .filter(|tile| !tile.content_box.is_empty())
                .collect();
            let actual: Vec<_> = tiles.iter().map(|tile| &tile.placement).collect();
            assert_eq!(
                format!("{actual:?}"),
                format!("{expected:?}"),
                "tile border, clip or highlight changed"
            );
            let expected: Vec<_> = monitor
                .containers
                .iter()
                .filter(|container| container.is_tabbed || container.is_highlighted)
                .collect();
            assert_eq!(
                format!("{containers:?}"),
                format!("{expected:?}"),
                "container or tab band changed"
            );
        }
        let Some(RecordedContent::Floats(floats)) = batch.next() else {
            panic!("missing float content boundary")
        };
        let expected: Vec<_> = scene
            .monitors
            .iter()
            .flat_map(|monitor| {
                monitor
                    .float_windows
                    .iter()
                    .filter(|placement| !placement.content_box.is_empty())
                    .map(move |placement| (monitor, placement))
            })
            .collect();
        assert_eq!(floats.len(), expected.len());
        for (actual, (monitor, placement)) in floats.iter().zip(expected) {
            assert_eq!(scene.windows[&actual.window].id, placement.id);
            assert_eq!(
                format!("{:?}", actual.placement),
                format!("{placement:?}"),
                "float border or highlight changed"
            );
            assert_eq!(actual.scale, monitor.scale);
            assert_eq!(actual.border_thickness, monitor.border_thickness);
        }
        let Some(RecordedContent::Tabs(monitors)) = batch.next() else {
            panic!("missing tab content boundary")
        };
        assert_monitors(monitors, &scene.monitors);
        assert!(
            batch.next().is_none(),
            "unexpected extra content in completed scene pass"
        );
    }
    fn collect(&mut self) {
        self.writes.extend(self.submissions.try_iter());
        self.reads.extend(self.trace.try_iter());
    }
    fn await_trace(&mut self, accepts: impl Fn(&NativeTrace) -> bool) -> Instant {
        let mut accepted = None;
        until(
            || {
                self.collect();
                accepted = self
                    .reads
                    .iter()
                    .find(|(_, trace)| accepts(trace))
                    .map(|(at, _)| *at);
                accepted.is_some()
            },
            "native event or accepted concrete read did not arrive",
        );
        accepted.unwrap()
    }
    fn placement(&mut self, key: HwndId) -> PlacementRead {
        self.await_trace(
            |trace| matches!(trace, NativeTrace::Placement(window, _) if *window == key),
        );
        self.reads
            .iter()
            .rev()
            .find_map(|(_, trace)| match trace {
                NativeTrace::Placement(window, read) if *window == key => Some(read.clone()),
                _ => None,
            })
            .unwrap()
    }
    fn scene(&mut self, scene: Scene) {
        self.finish_pass();
        self.content.try_iter().for_each(drop);
        self.ready.ui.send(HubMessage::Scene(scene.clone()));
        self.assert_content(&scene);
    }
    fn report(&mut self, accepts: impl Fn(&HubEvent) -> bool) -> HubEvent {
        let mut accepted = None;
        until(
            || {
                self.pending_reports.extend(self.reports.try_iter());
                if let Some(index) = self.pending_reports.iter().position(&accepts) {
                    accepted = self.pending_reports.remove(index);
                    return true;
                }
                false
            },
            "native semantic report did not arrive",
        );
        accepted.unwrap()
    }
    fn clear(&mut self) {
        self.finish_pass();
        self.writes.clear();
        self.reads.clear();
        self.content.try_iter().for_each(drop);
        self.events.try_iter().for_each(drop);
    }
    fn await_frame(&mut self, key: HwndId) -> (Instant, (i32, i32, i32, i32)) {
        let mut frame = None;
        until(
            || {
                self.collect();
                frame = self
                    .writes
                    .iter()
                    .rev()
                    .find_map(|(at, write)| match write {
                        Submission::Position {
                            window,
                            rectangle,
                            flags,
                            ..
                        } if *window == key
                            && !flags.contains(SWP_NOMOVE)
                            && !flags.contains(SWP_NOSIZE) =>
                        {
                            Some((*at, *rectangle))
                        }
                        _ => None,
                    });
                frame.is_some()
            },
            "no real native frame submission",
        );
        frame.unwrap()
    }
}

fn assert_monitors(actual: &[MonitorScene], expected: &[MonitorScene]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.native, expected.native);
        assert_eq!(actual.work_area, expected.work_area);
        assert_eq!(actual.system_work_area, expected.system_work_area);
        assert_eq!(actual.scale, expected.scale);
        assert_eq!(actual.border_thickness, expected.border_thickness);
        assert_eq!(
            format!("{:?}", actual.tiling_windows),
            format!("{:?}", expected.tiling_windows)
        );
        assert_eq!(
            format!("{:?}", actual.float_windows),
            format!("{:?}", expected.float_windows)
        );
        assert_eq!(
            format!("{:?}", actual.containers),
            format!("{:?}", expected.containers),
            "tab titles, active index or band changed"
        );
    }
}

struct Taskbar;
impl taskbar::ManageTaskbar for Taskbar {
    fn add_tab(&self, _: HwndId) {}
    fn delete_tab(&self, _: HwndId) {}
}
struct DomainScenes {
    ui: UiSender,
    latest: Rc<RefCell<Option<Scene>>>,
}
impl SceneSender for DomainScenes {
    fn send(&mut self, message: HubMessage) {
        if let HubMessage::Scene(scene) = &message {
            *self.latest.borrow_mut() = Some(scene.clone());
        }
        self.ui.send(message);
    }
}
struct NativeDomain {
    dome: Dome,
    latest: Rc<RefCell<Option<Scene>>>,
    _configuration: TempFile,
}
impl NativeDomain {
    fn new(ui: UiSender) -> Self {
        let (runtime, configuration) = loaded_runtime(
            "native_ownership",
            r#"
            return { keymaps = { main = {
                ["ctrl+l"] = function(a) a.move_to_monitor_right() end,
                ["ctrl+h"] = function(a) a.move_to_monitor_left() end,
                ["ctrl+f"] = function(a) a.toggle_float() end
            } } }
        "#,
        );
        let latest = Rc::new(RefCell::new(None));
        let (publisher, _) = mpsc::channel();
        let mut dome = Dome::new(
            crate::config::tests::tiling_config(),
            PreferredLayouts::default(),
            Rc::new(Taskbar),
            Box::new(dome::Win32Display),
            Box::new(DomainScenes {
                ui,
                latest: latest.clone(),
            }),
            runtime,
            KeymapPublisher::new(KeymapView::new(), publisher),
            HashMap::new(),
        )
        .unwrap();
        dome.apply_layout();
        Self {
            dome,
            latest,
            _configuration: configuration,
        }
    }
    fn handle(&mut self, report: HubEvent) {
        if let Err(event) = self.dome.handle_report(report) {
            match event {
                HubEvent::Action(actions) => self.dome.handle_actions(&actions, 0),
                other => {
                    let _ = other;
                    panic!("unexpected nonsemantic fixture report");
                }
            }
        }
    }
    fn bind(&mut self, binding: &str) {
        self.dome.run_binding("main", &binding.parse().unwrap(), 0);
    }
    fn scene(&self) -> Scene {
        self.latest.borrow().as_ref().unwrap().clone()
    }
}

fn target(scene: &Scene, key: HwndId) -> (MonitorId, PixelRect<Physical>) {
    match scene.windows[&key].placement {
        WindowPlacement::Tiling { monitor, placement } => (monitor, placement.content_box),
        WindowPlacement::Float { monitor, placement } => (monitor, placement.content_box),
        _ => panic!("visible tiled or float target required"),
    }
}
fn native_rectangle(frame: PixelRect, read: &PlacementRead, dpi: u32) -> (i32, i32, i32, i32) {
    let (left, top, right, bottom) = read.invisible_border;
    let edge = |value: i32| {
        if read.unaware {
            (f64::from(value) * 96.0 / f64::from(dpi)).round() as i32
        } else {
            value
        }
    };
    let x = edge(frame.x().value() - left.value());
    let y = edge(frame.y().value() - top.value());
    (
        x,
        y,
        edge(frame.right().value() + right.value()) - x,
        edge(frame.bottom().value() + bottom.value()) - y,
    )
}
fn await_visible(key: HwndId, expected: PixelRect) {
    until(
        || {
            let actual = handle::read_visible_frame(key).expect("native visible frame required");
            [
                (actual.x().value(), expected.x().value()),
                (actual.y().value(), expected.y().value()),
                (actual.right().value(), expected.right().value()),
                (actual.bottom().value(), expected.bottom().value()),
            ]
            .into_iter()
            .all(|(actual, expected)| (actual - expected).abs() <= 1)
        },
        "submitted frame did not become native visible geometry within edge-rounding tolerance",
    );
}

fn await_outer(key: HwndId, x: i32, y: i32) {
    until(
        || {
            let bounds = outer(key);
            bounds.left == x && bounds.top == y
        },
        "native origin never applied",
    );
}

pub(super) fn menu_case() {
    let mut session = NativeSession::new();
    let mut domain = NativeDomain::new(session.ready.ui.clone());
    let first = ForeignWindow::spawn();
    let mut second = ForeignWindow::spawn();
    for key in [first.window, second.window] {
        let report = session.report(|report| matches!(report, HubEvent::WindowsDiscovered(windows) if windows.iter().any(|window| window.key == key)));
        domain.handle(report);
        session.await_frame(key);
    }
    let mut scene = domain.scene();
    scene.focus_request = None;
    session.scene(scene.clone());
    wait_quiet(&mut session, second.window);
    session.clear();
    session.reports.try_iter().for_each(drop);
    session.pending_reports.clear();
    open_tray_menu(session.thread);

    let mut lifecycle = ForeignWindow::spawn();
    session.report(|report| matches!(report, HubEvent::WindowsDiscovered(windows) if windows.iter().any(|window| window.key == lifecycle.window && window.process_id == lifecycle.child.id())));
    lifecycle.command("title");
    session.report(|report| matches!(report, HubEvent::TitleChanged { key, title } if *key == lifecycle.window && title == "Renamed ownership fixture"));
    menu_open(session.thread);

    scene.focus_request = Some(FocusTarget::Window(first.window));
    session.scene(scene.clone());
    let first_focus = await_focus(&mut session, first.window);
    session.deadlines.try_iter().for_each(drop);
    scene.focus_request = Some(FocusTarget::Window(second.window));
    let requested = Instant::now();
    session.scene(scene.clone());
    let mut focus_deadline = None;
    until(
        || {
            focus_deadline = session.deadlines.try_iter().find_map(|(wake, deadline)| {
                if wake == Wake::FocusRequest {
                    deadline
                } else {
                    None
                }
            });
            focus_deadline.is_some()
        },
        "pending focus never armed its dedicated deadline",
    );
    let focus_deadline = focus_deadline.unwrap();
    assert!(
        first_focus < focus_deadline && requested < focus_deadline,
        "fixture failed to queue a genuinely delayed replacement focus"
    );
    let second_focus = await_focus(&mut session, second.window);
    assert!(
        second_focus >= focus_deadline,
        "external focus submitted before its armed deadline"
    );
    menu_open(session.thread);
    session.await_trace(|trace| matches!(trace, NativeTrace::Event(NativeEvent::Foreground { window, internal: false, .. }) if *window == second.window));
    let foreground = session
        .report(|report| matches!(report, HubEvent::FocusReported(key) if *key == second.window));
    assert!(
        Instant::now() < second_focus + Duration::from_millis(100),
        "OS foreground report was throttled behind another focus interval"
    );
    domain.handle(foreground);
    until(
        || unsafe { GetForegroundWindow() } == HWND::from(second.window),
        "submitted focus never became native foreground",
    );
    menu_open(session.thread);

    wait_quiet(&mut session, second.window);
    session.clear();
    let shifted_at = Instant::now();
    second.command("shift");
    second.response("SHIFTED");
    session.await_trace(|trace| matches!(trace, NativeTrace::Event(NativeEvent::Moved { window, observed_at }) if *window == second.window && *observed_at >= shifted_at));
    let moved = session
        .reads
        .iter()
        .filter_map(|(_, trace)| match trace {
            NativeTrace::Event(NativeEvent::Moved {
                window,
                observed_at,
            }) if *window == second.window && *observed_at >= shifted_at => Some(*observed_at),
            _ => None,
        })
        .max()
        .unwrap();
    let settled = session.await_trace(
        |trace| matches!(trace, NativeTrace::Settled(window) if *window == second.window),
    );
    assert!(
        settled >= moved + Duration::from_millis(100),
        "geometry accepted before the 100 ms quiet boundary"
    );
    session.await_frame(second.window);
    menu_open(session.thread);

    let mut parked = domain.scene();
    parked.focus_request = None;
    parked.windows.insert(
        lifecycle.window,
        ExpectedWindow {
            id: WindowId::new(1000),
            placement: WindowPlacement::Hidden,
        },
    );
    session.scene(parked);
    lifecycle.wait_origin(-32000, -32000);
    menu_open(session.thread);

    use interprocess::local_socket::{GenericFilePath, ToFsName, traits::Stream};
    let name = std::path::PathBuf::from(format!(r"\\.\pipe\dome-ownership-{}", std::process::id()))
        .to_fs_name::<GenericFilePath>()
        .unwrap()
        .into_owned();
    let hub = session.hub.clone();
    let server = ipc::serve_native_fixture(name.clone(), move |event| {
        match event {
            ipc::IpcEvent::Action(actions) => hub.send(HubEvent::Action(actions)),
            _ => anyhow::bail!("only fixture action expected"),
        }
        Ok(())
    });
    let client = thread::spawn(move || {
        let mut stream = interprocess::local_socket::Stream::connect(name).unwrap();
        writeln!(stream, r#"{{"type":"action","action":{{"type":"close"}}}}"#).unwrap();
        let mut response = String::new();
        BufReader::new(stream).read_line(&mut response).unwrap();
        let response: dome_ipc::response::Response = serde_json::from_str(&response).unwrap();
        assert!(
            matches!(response, dome_ipc::response::Response::Ok { .. }),
            "IPC action failed"
        );
    });
    let action = session.report(|report| matches!(report, HubEvent::Action(_)));
    domain.handle(action);
    session.report(|report| matches!(report, HubEvent::WindowGone(key) if *key == second.window));
    until(
        || !unsafe { IsWindow(Some(second.window.into())) }.as_bool(),
        "IPC close submission was not applied",
    );
    client.join().unwrap();
    server.join().unwrap().unwrap();
    menu_open(session.thread);
}

fn await_focus(session: &mut NativeSession, key: HwndId) -> Instant {
    let mut result = None;
    until(
        || {
            session.collect();
            result = session.writes.iter().rev().find_map(|(at, write)| {
                matches!(write, Submission::Foreground(window) if *window == key).then_some(*at)
            });
            result.is_some()
        },
        "due FocusRequest did not submit before menu closure",
    );
    result.unwrap()
}

fn shifted_scene(scene: &Scene, key: HwndId, horizontal: i32) -> Scene {
    let mut scene = scene.clone();
    scene.focus_request = None;
    let translate = |frame: PixelRect| {
        PixelRect::new(
            frame.x().value() + horizontal,
            frame.y().value(),
            frame.width().value(),
            frame.height().value(),
        )
    };
    match &mut scene.windows.get_mut(&key).unwrap().placement {
        WindowPlacement::Tiling { placement, monitor } => {
            placement.content_box = translate(placement.content_box);
            let replacement = *placement;
            let monitor = scene
                .monitors
                .iter_mut()
                .find(|entry| entry.id == *monitor)
                .unwrap();
            *monitor
                .tiling_windows
                .iter_mut()
                .find(|entry| entry.id == replacement.id)
                .unwrap() = replacement;
        }
        WindowPlacement::Float { placement, monitor } => {
            placement.content_box = translate(placement.content_box);
            let replacement = *placement;
            let monitor = scene
                .monitors
                .iter_mut()
                .find(|entry| entry.id == *monitor)
                .unwrap();
            *monitor
                .float_windows
                .iter_mut()
                .find(|entry| entry.id == replacement.id)
                .unwrap() = replacement;
        }
        _ => panic!("visible fixture target required"),
    }
    scene
}
fn above(window: HwndId, other: HwndId) -> bool {
    let mut current = unsafe { GetTopWindow(None) }.ok();
    while let Some(candidate) = current {
        if candidate == HWND::from(window) {
            return true;
        }
        if candidate == HWND::from(other) {
            return false;
        }
        current = unsafe { GetWindow(candidate, GW_HWNDNEXT) }.ok();
    }
    panic!("native ordering fixture HWNDs disappeared");
}

fn owned_targets(session: &NativeSession, child: HwndId) -> Vec<(i32, i32)> {
    session
        .writes
        .iter()
        .filter_map(|(_, submission)| match submission {
            Submission::Position {
                window,
                rectangle,
                flags,
                ..
            } if *window == child => {
                assert!(
                    flags.contains(SWP_ASYNCWINDOWPOS | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER)
                );
                Some((rectangle.0, rectangle.1))
            }
            _ => None,
        })
        .collect()
}
fn expected_child(frame: PixelRect, read: &PlacementRead, child: HwndId) -> (i32, i32) {
    let offset = read
        .owned_windows
        .iter()
        .find(|entry| entry.window == child)
        .unwrap()
        .offset
        .expect("accepted owned offset required");
    (
        frame.x().value() - read.invisible_border.0.value() + offset.0.value(),
        frame.y().value() - read.invisible_border.1.value() + offset.1.value(),
    )
}
fn create_dialog(foreign: &mut ForeignWindow) -> HwndId {
    foreign.command("dialog");
    HwndId::test(foreign.response("DIALOG ").trim().parse().unwrap())
}
fn wait_owned(session: &mut NativeSession, parent: HwndId, child: HwndId) -> PlacementRead {
    session.await_trace(|trace| matches!(trace, NativeTrace::Placement(window, read) if *window == parent && read.owned_windows.iter().any(|owned| owned.window == child && owned.offset.is_some())));
    session.placement(parent)
}
fn wait_quiet(session: &mut NativeSession, key: HwndId) {
    let start = Instant::now();
    until(
        || {
            session.collect();
            let last = session
                .reads
                .iter()
                .filter_map(|(_, event)| match event {
                    NativeTrace::Event(NativeEvent::Moved {
                        window,
                        observed_at,
                    }) if *window == key => Some(*observed_at),
                    _ => None,
                })
                .max()
                .unwrap_or(start);
            Instant::now() >= last + Duration::from_millis(150)
        },
        "fixture never became quiet",
    );
    session.collect();
}

pub(super) struct InputOwner {
    cursor: POINT,
    armed: bool,
    prior_context: windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT,
}
impl InputOwner {
    pub(super) fn capture() -> Self {
        let prior_context =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        assert!(
            !prior_context.0.is_null(),
            "physical cursor coordinates required"
        );
        let mut owner = Self {
            cursor: POINT::default(),
            armed: false,
            prior_context,
        };
        assert_input_released();
        unsafe { GetCursorPos(&mut owner.cursor) }.expect("interactive cursor required");
        let mut clip = RECT::default();
        unsafe { GetClipCursor(&mut clip) }.unwrap();
        let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        assert_eq!(
            (clip.left, clip.top, clip.right, clip.bottom),
            (
                left,
                top,
                left + unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
                top + unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }
            ),
            "UNVERIFIED input prerequisite. Cursor must not be clipped"
        );
        owner
    }
    pub(super) fn arm(&mut self) {
        assert_input_released();
        self.armed = true;
    }
    pub(super) fn restore(&mut self) -> anyhow::Result<()> {
        if self.armed {
            let input = mouse_input(false);
            let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
            let cursor = unsafe { SetCursorPos(self.cursor.x, self.cursor.y) };
            anyhow::ensure!(
                sent == 1,
                "outer owner could not release fixture mouse button"
            );
            cursor?;
            self.armed = false;
        }
        Ok(())
    }
}
impl Drop for InputOwner {
    fn drop(&mut self) {
        let _ = self.restore();
        unsafe { SetThreadDpiAwarenessContext(self.prior_context) };
    }
}
fn assert_input_released() {
    for key in 1..=254 {
        assert_eq!(
            unsafe { GetAsyncKeyState(key) } as u16 & 0x8000,
            0,
            "UNVERIFIED input prerequisite. Key or mouse button {key} is already held"
        );
    }
}
fn arm_outer_input_cleanup() {
    println!("FIXTURE_INPUT_BEGIN");
    std::io::stdout().flush().unwrap();
    read_parent_release("INPUT_OWNED");
}
fn mouse_input(down: bool) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: if down {
                    MOUSEEVENTF_LEFTDOWN
                } else {
                    MOUSEEVENTF_LEFTUP
                },
                ..Default::default()
            },
        },
    }
}
fn mouse_button(down: bool) {
    let input = mouse_input(down);
    assert_eq!(
        unsafe { SendInput(&[input], size_of::<INPUT>() as i32) },
        1,
        "interactive input injection prerequisite missing"
    );
}
struct HeldMouse(POINT);
impl Drop for HeldMouse {
    fn drop(&mut self) {
        let _ = unsafe { SendInput(&[mouse_input(false)], size_of::<INPUT>() as i32) };
        let _ = unsafe { SetCursorPos(self.0.x, self.0.y) };
    }
}
fn titlebar_drag(
    session: &mut NativeSession,
    domain: &mut NativeDomain,
    key: HwndId,
    destination: &dome::MonitorInfo,
    floating: bool,
) {
    let accepted = session.placement(key);
    session.pending_reports.extend(session.reports.try_iter());
    session.pending_reports.retain(
        |report| !matches!(report, HubEvent::FloatMoved { key: window, .. } if *window == key),
    );
    let bounds = outer(key);
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor) }.unwrap();
    session.clear();
    arm_outer_input_cleanup();
    unsafe { SetCursorPos(bounds.left + 120, bounds.top + 15) }.unwrap();
    let held = HeldMouse(cursor);
    mouse_button(true);
    unsafe { SetCursorPos(bounds.left + 145, bounds.top + 25) }.unwrap();
    session.await_trace(|trace| matches!(trace, NativeTrace::Event(NativeEvent::MoveStarted { window, .. }) if *window == key));
    let drag_started = session
        .reads
        .iter()
        .find_map(|(_, trace)| match trace {
            NativeTrace::Event(NativeEvent::MoveStarted {
                window,
                observed_at,
            }) if *window == key => Some(*observed_at),
            _ => None,
        })
        .unwrap();
    unsafe {
        SetCursorPos(
            destination.work_area.x().value() + 160,
            destination.work_area.y().value() + 100,
        )
    }
    .unwrap();
    session.await_trace(|trace| matches!(trace, NativeTrace::Event(NativeEvent::Moved { window, .. }) if *window == key));
    domain.bind("ctrl+l");
    let frame = target(&domain.scene(), key).1;
    let (_, submitted) = session.await_frame(key);
    assert_eq!(
        submitted,
        native_rectangle(frame, &accepted, 144),
        "mid-drag keymap used an unaccepted border or source DPI"
    );
    session.assert_content(&domain.scene());
    session.writes.clear();
    let last_drag_motion = Instant::now();
    unsafe {
        SetCursorPos(
            destination.work_area.x().value() + 220,
            destination.work_area.y().value() + 140,
        )
    }
    .unwrap();
    session.await_trace(|trace| matches!(trace, NativeTrace::Event(NativeEvent::Moved { window, observed_at }) if *window == key && *observed_at >= last_drag_motion));
    drop(held);
    session.await_trace(|trace| matches!(trace, NativeTrace::Event(NativeEvent::MoveEnded { window, .. }) if *window == key));
    session.await_trace(|trace| matches!(trace, NativeTrace::Settled(window) if *window == key));
    session.finish_pass();
    let settled = session
        .reads
        .iter()
        .find(|(_, trace)| matches!(trace, NativeTrace::Settled(window) if *window == key))
        .unwrap()
        .0;
    let latest_movement = session
        .reads
        .iter()
        .filter_map(|(_, trace)| match trace {
            NativeTrace::Event(NativeEvent::Moved {
                window,
                observed_at,
            }) if *window == key && *observed_at <= settled => Some(*observed_at),
            _ => None,
        })
        .max()
        .expect("native drag must produce a location event before release");
    assert!(
        settled >= latest_movement + Duration::from_millis(100),
        "drag end bypassed newer quiet movement"
    );
    let quiet_boundary = latest_movement + Duration::from_millis(100);
    let drag_ended = session
        .reads
        .iter()
        .find_map(|(_, trace)| match trace {
            NativeTrace::Event(NativeEvent::MoveEnded {
                window,
                observed_at,
            }) if *window == key => Some(*observed_at),
            _ => None,
        })
        .unwrap();
    assert!(
        !session.reads.iter().any(|(at, trace)| *at >= drag_started
            && (*at < drag_ended || *at < quiet_boundary)
            && matches!(trace, NativeTrace::Placement(window, _) if *window == key)),
        "placement inputs accepted during drag or before post-release quiet"
    );
    let refreshed = session.placement(key);
    assert!(refreshed.unaware);
    assert_ne!(
        refreshed.invisible_border, accepted.invisible_border,
        "UNVERIFIED DWM prerequisite. Cross-DPI settle must expose changed physical borders"
    );
    if floating {
        let report = session.report(|report| matches!(report, HubEvent::FloatMoved { key: window, monitor, .. } if *window == key && *monitor == destination.handle));
        let observed = match &report {
            HubEvent::FloatMoved { frame, .. } => *frame,
            _ => unreachable!(),
        };
        session.collect();
        assert!(!session.writes.iter().any(|(_, write)| matches!(write, Submission::Position { window, flags, .. } if *window == key && !flags.contains(SWP_NOMOVE))), "float observation restored the old target before a response");
        domain.handle(report);
        assert_eq!(target(&domain.scene(), key).1, observed);
        assert_eq!(
            session.await_frame(key).1,
            native_rectangle(observed, &refreshed, 144)
        );
        session.assert_content(&domain.scene());
        await_visible(key, observed);
    } else {
        let (_, correction) = session.await_frame(key);
        assert_eq!(
            correction,
            native_rectangle(frame, &refreshed, 144),
            "tiled correction did not use the accepted post-drag border"
        );
        await_visible(key, frame);
    }
}

pub(super) fn native_order_case() {
    let first = ForeignWindow::spawn();
    let second = ForeignWindow::spawn();
    let mut session = NativeSession::new();
    let mut domain = NativeDomain::new(session.ready.ui.clone());
    let monitors = dome::Win32Display.get_all_monitors().unwrap();
    assert!(!monitors.is_empty(), "interactive display required");
    for key in [first.window, second.window] {
        session.ready.native.send(NativeEvent::Discover(key));
        let report = session.report(|report| matches!(report, HubEvent::WindowsDiscovered(windows) if windows.iter().any(|window| window.key == key)));
        domain.handle(report);
        session.await_frame(key);
        wait_quiet(&mut session, key);
    }
    let container = domain
        .scene()
        .monitors
        .iter()
        .flat_map(|monitor| &monitor.containers)
        .next()
        .expect("domain must provide a container for tab content coverage")
        .id;
    let mut scene = policy_scene(&monitors);
    let monitor = &mut scene.monitors[0];
    let work = monitor.work_area;
    for (index, key) in [first.window, second.window].into_iter().enumerate() {
        let frame = PixelRect::new(
            work.x().value() + 80 + index as i32 * 320,
            work.y().value() + 100,
            300,
            400,
        );
        let placement = TilingWindowPlacement {
            id: WindowId::new(index),
            border_box: frame,
            visible_border_box: frame,
            content_box: frame,
            visible_content_box: frame,
            is_highlighted: index == 0,
            spawn_direction: None,
        };
        scene.windows.insert(
            key,
            ExpectedWindow {
                id: placement.id,
                placement: WindowPlacement::Tiling {
                    monitor: monitor.id,
                    placement,
                },
            },
        );
        monitor.tiling_windows.push(placement);
    }
    monitor.containers.push(ContainerPlacement {
        id: container,
        border_box: work,
        visible_border_box: work,
        tab_bar_band: PixelRect::new(work.x().value(), work.y().value(), work.width().value(), 27),
        is_highlighted: true,
        spawn_direction: None,
        is_tabbed: true,
        active_tab_index: 1,
        titles: vec![
            "First native fixture".into(),
            "Second native fixture".into(),
        ],
    });
    session.clear();
    session.scene(scene.clone());
    let overlay = wait_overlay_after(&mut session, first.window, &[first.window, second.window]);
    until(
        || {
            unsafe { GetWindow(first.window.into(), GW_HWNDNEXT) }.ok()
                == Some(second.window.into())
                && unsafe { GetWindow(second.window.into(), GW_HWNDNEXT) }.ok()
                    == Some(overlay.into())
        },
        "submitted tiled order never became native adjacency",
    );
    scene.monitors[0].containers[0].titles[1] = "Renamed second tab".into();
    scene.monitors[0].containers[0].active_tab_index = 0;
    session.scene(scene.clone());
    let monitor = &mut scene.monitors[0];
    let tile = monitor.tiling_windows.remove(1);
    let placement = FloatWindowPlacement {
        id: tile.id,
        border_box: tile.border_box,
        visible_border_box: tile.visible_border_box,
        content_box: tile.content_box,
        is_highlighted: false,
    };
    monitor.float_windows.push(placement);
    scene.windows.get_mut(&second.window).unwrap().placement = WindowPlacement::Float {
        monitor: monitor.id,
        placement,
    };
    scene.focused_window = Some(second.window);
    session.clear();
    session.scene(scene.clone());
    let float_overlay = wait_overlay_after(
        &mut session,
        second.window,
        &[first.window, second.window, overlay],
    );
    until(
        || {
            unsafe { GetWindow(second.window.into(), GW_HWNDNEXT) }.ok()
                == Some(float_overlay.into())
        },
        "float overlay did not become natively adjacent",
    );
    assert_ne!(
        unsafe { GetWindowLongW(second.window.into(), GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0,
        0
    );
    scene.monitors[0].float_windows.clear();
    scene.monitors[0].tiling_windows.push(tile);
    scene.windows.get_mut(&second.window).unwrap().placement = WindowPlacement::Tiling {
        monitor: scene.monitors[0].id,
        placement: tile,
    };
    scene.focused_window = Some(first.window);
    session.clear();
    session.scene(scene.clone());
    until(|| unsafe { GetWindowLongW(second.window.into(), GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0 == 0, "former float did not leave the native topmost band");
    until(
        || {
            unsafe { GetWindow(first.window.into(), GW_HWNDNEXT) }.ok()
                == Some(second.window.into())
                && unsafe { GetWindow(second.window.into(), GW_HWNDNEXT) }.ok()
                    == Some(overlay.into())
        },
        "retiled window did not regain native adjacency",
    );
    wait_quiet(&mut session, first.window);
    let accepted = session.placement(first.window);
    let mut fullscreen = scene.clone();
    let monitor = &mut fullscreen.monitors[0];
    let frame = monitor.work_area;
    monitor.tiling_windows.clear();
    monitor.float_windows.clear();
    monitor.containers.clear();
    fullscreen.windows.get_mut(&first.window).unwrap().placement = WindowPlacement::Fullscreen {
        monitor: monitor.id,
    };
    fullscreen
        .windows
        .get_mut(&second.window)
        .unwrap()
        .placement = WindowPlacement::Hidden;
    session.clear();
    session.scene(fullscreen);
    assert_eq!(
        session.await_frame(first.window).1,
        native_rectangle(frame, &accepted, monitors[0].dots_per_inch)
    );
    await_visible(first.window, frame);
    await_outer(second.window, -32000, -32000);
    session.clear();
    session.scene(scene.clone());
    session.await_frame(first.window);
    await_visible(first.window, target(&scene, first.window).1);
}
fn wait_overlay_after(
    session: &mut NativeSession,
    reference: HwndId,
    managed: &[HwndId],
) -> HwndId {
    let mut overlay = None;
    until(
        || {
            session.collect();
            overlay = session
                .writes
                .iter()
                .find_map(|(_, submission)| match submission {
                    Submission::Position {
                        window,
                        after: Some(after),
                        flags,
                        ..
                    } if *after == reference
                        && !managed.contains(window)
                        && flags.contains(SWP_NOMOVE | SWP_NOSIZE) =>
                    {
                        Some(*window)
                    }
                    _ => None,
                });
            overlay.is_some()
        },
        "scene did not submit an internal native ordering endpoint",
    );
    overlay.unwrap()
}

pub(super) fn mixed_dpi_case() {
    let monitors = dome::Win32Display.get_all_monitors().unwrap();
    assert_eq!(
        monitors.len(),
        2,
        "UNVERIFIED hardware prerequisite. Exactly two horizontally arranged displays required"
    );
    let source = monitors
        .iter()
        .find(|monitor| monitor.dots_per_inch == 96)
        .expect("UNVERIFIED hardware prerequisite. A 96-DPI source display is required");
    let destination = monitors
        .iter()
        .find(|monitor| monitor.dots_per_inch == 144)
        .expect("UNVERIFIED hardware prerequisite. A 144-DPI destination display is required");
    assert!(
        source.full_rect.right() <= destination.full_rect.x(),
        "UNVERIFIED hardware prerequisite. The 144-DPI destination must be right of the 96-DPI source"
    );
    let mut foreign = ForeignWindow::spawn_with_awareness(true);
    let key = foreign.window;
    let initial = source.work_area;
    unsafe {
        SetWindowPos(
            key.into(),
            None,
            initial.x().value() + 80,
            initial.y().value() + 80,
            640,
            480,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }
    .unwrap();
    let child = create_dialog(&mut foreign);
    let mut session = NativeSession::new();
    let mut domain = NativeDomain::new(session.ready.ui.clone());
    session.ready.native.send(NativeEvent::Discover(key));
    let discovery = session.report(|report| matches!(report, HubEvent::WindowsDiscovered(windows) if windows.iter().any(|window| window.key == key)));
    domain.handle(discovery);
    let foreground = session
        .report(|report| matches!(report, HubEvent::FocusReported(window) if *window == key));
    domain.handle(foreground);
    until(
        || unsafe { GetForegroundWindow() } == HWND::from(key),
        "initial domain focus was not applied natively",
    );
    session.await_frame(key);
    wait_quiet(&mut session, key);
    let observed = wait_owned(&mut session, key, child);
    assert!(observed.unaware, "fixture must actually be DPI unaware");
    assert!(
        observed.invisible_border.0.value() > 0 || observed.invisible_border.2.value() > 0,
        "DWM invisible border prerequisite missing"
    );

    for floating in [false, true] {
        if floating {
            domain.bind("ctrl+f");
            assert!(matches!(
                domain.scene().windows[&key].placement,
                WindowPlacement::Float { .. }
            ));
            wait_quiet(&mut session, key);
        }
        let accepted = session.placement(key);
        let before = outer(key);
        foreign.command("pause");
        foreign.response("PAUSED");
        session.clear();
        domain.bind("ctrl+l");
        let destination_scene = domain.scene();
        let (monitor, destination_frame) = target(&destination_scene, key);
        assert_eq!(
            destination_scene
                .monitors
                .iter()
                .find(|entry| entry.id == monitor)
                .unwrap()
                .native,
            destination.handle,
            "real keymap must select destination"
        );
        let (_, submitted) = session.await_frame(key);
        assert_eq!(
            submitted,
            native_rectangle(destination_frame, &accepted, 144),
            "keymap conversion must use destination DPI and accepted source borders"
        );
        session.await_owned(child, expected_child(destination_frame, &accepted, child));
        session.assert_content(&destination_scene);
        assert_eq!(
            owned_targets(&session, child),
            vec![expected_child(destination_frame, &accepted, child)]
        );
        let still = outer(key);
        assert_eq!(
            (still.left, still.top, still.right, still.bottom),
            (before.left, before.top, before.right, before.bottom),
            "stopped pump unexpectedly applied submitted geometry"
        );
        assert!(
            !session.reads.iter().any(
                |(_, trace)| matches!(trace, NativeTrace::Placement(window, _) if *window == key)
            ),
            "keymap crossing forced a placement refresh"
        );
        session.clear();
        domain.bind("ctrl+h");
        let source_frame = target(&domain.scene(), key).1;
        assert_eq!(
            session.await_frame(key).1,
            native_rectangle(source_frame, &accepted, 96)
        );
        session.await_owned(child, expected_child(source_frame, &accepted, child));
        session.assert_content(&domain.scene());
        assert_eq!(
            owned_targets(&session, child),
            vec![expected_child(source_frame, &accepted, child)],
            "unapplied writes accumulated a child delta"
        );
        foreign.command("resume");
        foreign.response("RESUMED");
        let expected = native_rectangle(source_frame, &accepted, 96);
        await_outer(key, expected.0, expected.1);
        let expected = expected_child(source_frame, &accepted, child);
        await_outer(child, expected.0, expected.1);
        wait_quiet(&mut session, key);
        titlebar_drag(&mut session, &mut domain, key, destination, floating);
        domain.bind("ctrl+h");
        wait_quiet(&mut session, key);
    }

    wait_quiet(&mut session, key);
    let accepted = session.placement(key);
    session.clear();
    let new_child = create_dialog(&mut foreign);
    let before = outer(new_child);
    let changed = shifted_scene(&domain.scene(), key, 19);
    let frame = target(&changed, key).1;
    session.ready.ui.send(HubMessage::Scene(changed.clone()));
    let deadline = Instant::now() + WAIT;
    loop {
        session.collect();
        if session.writes.iter().any(|(_, write)| matches!(write, Submission::Position { window, flags, .. } if *window == key && !flags.contains(SWP_NOMOVE))) { break; }
        assert!(
            Instant::now() < deadline,
            "scene did not submit while hook intake was deliberately held"
        );
        thread::sleep(Duration::from_millis(2));
    }
    session.finish_pass();
    assert!(
        owned_targets(&session, new_child).is_empty(),
        "new dialog propagated before discovery acceptance"
    );
    let after = outer(new_child);
    assert_eq!((before.left, before.top), (after.left, after.top));
    assert_eq!(
        owned_targets(&session, child),
        vec![expected_child(frame, &accepted, child)]
    );
    let refreshed = wait_owned(&mut session, key, new_child);
    wait_quiet(&mut session, key);
    session.clear();
    let changed = shifted_scene(&changed, key, 23);
    let frame = target(&changed, key).1;
    session.scene(changed.clone());
    session.await_frame(key);
    let expected = expected_child(frame, &refreshed, new_child);
    session.await_owned(new_child, expected);
    assert_eq!(owned_targets(&session, new_child), vec![expected]);
    await_outer(new_child, expected.0, expected.1);
    wait_quiet(&mut session, key);

    let mut unfocused = changed.clone();
    unfocused.focused_window = None;
    session.scene(unfocused);
    let peer = ForeignWindow::spawn();
    unsafe {
        SetWindowPos(
            peer.window.into(),
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    }
    .unwrap();
    until(
        || above(peer.window, key),
        "fixture peer never covered the parent in native order",
    );
    session.clear();
    let before = outer(new_child);
    let mut focused = changed;
    focused.focused_window = Some(key);
    session.scene(focused);
    until(
        || {
            session.collect();
            session.writes.iter().any(|(_, write)| matches!(write, Submission::Position { window, flags, after: Some(after), .. } if *window == key && flags.contains(SWP_NOMOVE | SWP_NOSIZE) && *after == HwndId::from(HWND_TOPMOST)))
        },
        "focus-only float change did not raise parent",
    );
    until(
        || above(key, peer.window),
        "focus-only raise was submitted but never applied",
    );
    session.finish_pass();
    assert!(owned_targets(&session, new_child).is_empty());
    assert!(owned_targets(&session, child).is_empty());
    let after = outer(new_child);
    assert_eq!(
        (before.left, before.top, before.right, before.bottom),
        (after.left, after.top, after.right, after.bottom)
    );
}
