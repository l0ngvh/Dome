use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::{Command, Stdio};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EndMenu, IsWindow, MSG, PM_REMOVE, PeekMessageW, PostMessageW, WM_CLOSE,
    WM_DISPLAYCHANGE, WM_KEYDOWN, WM_QUIT, WM_RBUTTONUP, WM_TIMER,
};

use super::run_loop::current_app;
use super::{WM_APP_TRAY, WM_APP_WAKE};
use crate::{
    App, AppHandler, AuxiliaryWindow, AuxiliaryWindowExtWindows, AuxiliaryWindowHandler, Icon,
    LoopHandle, LoopWaker, MenuEntry, MenuItem, Point, Shell, Size, WindowAttributes,
};

thread_local! {
    static AFTER_GUARD: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
    static MODAL_OBSERVER: RefCell<std::sync::Weak<AtomicBool>> = const { RefCell::new(std::sync::Weak::new()) };
}

pub(super) fn observe_guard_return() {
    let hook = AFTER_GUARD.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

pub(super) struct ModalMarker(Option<Arc<AtomicBool>>);
impl ModalMarker {
    pub(super) fn enter() -> Self {
        let observer = MODAL_OBSERVER.with(|slot| slot.borrow().upgrade());
        if let Some(observer) = &observer {
            assert!(!observer.swap(true, Ordering::SeqCst));
        }
        Self(observer)
    }
}
impl Drop for ModalMarker {
    fn drop(&mut self) {
        if let Some(observer) = &self.0 {
            observer.store(false, Ordering::SeqCst);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Started,
    Wake(u8),
    Display,
    Menu,
    Selected(u32),
    NestedExit,
    Stopping,
}

struct Probe {
    handle: RefCell<Option<LoopHandle<u8>>>,
    waker: RefCell<Option<LoopWaker<u8>>>,
    events: RefCell<Vec<Event>>,
    saved: RefCell<Vec<MSG>>,
    observed: Cell<bool>,
    modal: Arc<AtomicBool>,
    menus: Arc<AtomicUsize>,
    tray: Cell<HWND>,
    due: RefCell<Vec<(u8, Instant)>>,
    case: String,
}
impl Probe {
    fn handle(&self) -> LoopHandle<u8> {
        self.handle.borrow().as_ref().unwrap().clone()
    }
    fn waker(&self) -> LoopWaker<u8> {
        self.waker.borrow().as_ref().unwrap().clone()
    }
    fn push(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }
    fn arm(&self, key: u8, delay: u64) {
        let at = Instant::now() + Duration::from_millis(delay);
        self.due.borrow_mut().push((key, at));
        self.handle().wake_at(key, Some(at));
    }
    fn check_due(&self, key: u8) {
        let mut due = self.due.borrow_mut();
        let index = due
            .iter()
            .position(|(candidate, _)| *candidate == key)
            .unwrap();
        let (_, at) = due.remove(index);
        assert!(Instant::now() >= at, "deadline delivered early");
    }
    fn open_menu(&self) {
        post(message(
            self.tray.get(),
            WM_APP_TRAY,
            0,
            WM_RBUTTONUP as isize,
        ));
    }
}

struct Handler {
    probe: Rc<Probe>,
    started: fn(&Rc<Probe>),
    wake: fn(&Rc<Probe>, u8),
    display: fn(&Rc<Probe>),
    selected: fn(&Rc<Probe>, u32),
}
impl AppHandler for Handler {
    type Wake = u8;
    fn on_started(&mut self, _: &Shell) {
        self.probe.push(Event::Started);
        (self.started)(&self.probe);
    }
    fn on_wake(&mut self, _: &Shell, wake: u8) {
        self.probe.push(Event::Wake(wake));
        (self.wake)(&self.probe, wake);
    }
    fn on_display_changed(&mut self) {
        self.probe.push(Event::Display);
        (self.display)(&self.probe);
    }
    fn menu(&mut self) -> Vec<MenuEntry> {
        self.probe.push(Event::Menu);
        self.probe.menus.fetch_add(1, Ordering::SeqCst);
        vec![MenuEntry::Item(MenuItem {
            id: 73,
            label: "Native delivery test".into(),
            checked: false,
        })]
    }
    fn on_menu_selected(&mut self, id: u32) {
        self.probe.push(Event::Selected(id));
        (self.selected)(&self.probe, id);
    }
    fn on_stopping(&mut self) {
        self.probe.push(Event::Stopping);
    }
}

fn fixture(case: &str) -> (Rc<Probe>, Handler) {
    let probe = Rc::new(Probe {
        handle: RefCell::new(None),
        waker: RefCell::new(None),
        events: RefCell::new(Vec::new()),
        saved: RefCell::new(Vec::new()),
        observed: Cell::new(false),
        modal: Arc::new(AtomicBool::new(false)),
        menus: Arc::new(AtomicUsize::new(0)),
        tray: Cell::new(HWND::default()),
        due: RefCell::new(Vec::new()),
        case: case.into(),
    });
    let handler = Handler {
        probe: probe.clone(),
        started: |_| {},
        wake: |_, wake| panic!("unexpected wake {wake}"),
        display: |_| panic!("unexpected display callback"),
        selected: |_, id| panic!("unexpected selection {id}"),
    };
    (probe, handler)
}

fn create_app(probe: &Probe, handler: Handler) -> App<u8> {
    let app = App::new(
        Icon {
            inner: super::app::Icon::for_test(),
        },
        Box::new(handler),
    )
    .expect("native desktop and tray required");
    *probe.handle.borrow_mut() = Some(app.handle());
    *probe.waker.borrow_mut() = Some(app.waker());
    probe.tray.set(app.inner.tray_window_for_test());
    MODAL_OBSERVER.with(|slot| *slot.borrow_mut() = Arc::downgrade(&probe.modal));
    app
}

fn message(window: HWND, kind: u32, word: usize, long: isize) -> MSG {
    MSG {
        hwnd: window,
        message: kind,
        wParam: WPARAM(word),
        lParam: LPARAM(long),
        ..Default::default()
    }
}
fn post(message: MSG) {
    unsafe {
        PostMessageW(
            (!message.hwnd.is_invalid()).then_some(message.hwnd),
            message.message,
            message.wParam,
            message.lParam,
        )
    }
    .expect("post native test message");
}
fn dispatch(message: &MSG) {
    unsafe { DispatchMessageW(message) };
}
fn consume_quit() {
    let mut message = MSG::default();
    assert!(
        unsafe { PeekMessageW(&mut message, None, WM_QUIT, WM_QUIT, PM_REMOVE) }.as_bool(),
        "termination must post quit"
    );
    assert_eq!(message.message, WM_QUIT);
}
fn pump_for(duration: Duration) {
    let until = Instant::now() + duration;
    loop {
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            assert_ne!(
                message.message, WM_QUIT,
                "unexpected quit in healthy nested pump"
            );
            dispatch(&message);
        }
        if Instant::now() >= until {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
}
fn take_timer() -> MSG {
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        let mut message = MSG::default();
        if unsafe { PeekMessageW(&mut message, None, WM_TIMER, WM_TIMER, PM_REMOVE) }.as_bool() {
            assert_ne!(message.lParam.0, 0, "expected production TIMERPROC");
            return message;
        }
        assert!(Instant::now() < until, "no native timer arrived");
        thread::sleep(Duration::from_millis(1));
    }
}
fn run_and_assert(app: App<u8>, probe: &Probe, panic_expected: bool) {
    let result = catch_unwind(AssertUnwindSafe(|| app.run()));
    if panic_expected {
        let payload = result.expect_err("App::run must resume the callback panic after teardown");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"native test panic"));
    } else {
        result.expect("healthy native loop panicked");
    }
    assert!(
        current_app().is_none(),
        "callback context survived teardown"
    );
    assert!(!unsafe { IsWindow(Some(probe.tray.get())) }.as_bool());
    let events = probe.events.borrow();
    assert_eq!(
        events
            .iter()
            .filter(|event| **event == Event::Stopping)
            .count(),
        1
    );
    assert_eq!(events.last(), Some(&Event::Stopping));
}

fn install_saved_dispatch(probe: &Rc<Probe>) {
    let weak: Weak<Probe> = Rc::downgrade(probe);
    AFTER_GUARD.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(Box::new(move || {
            let probe = weak.upgrade().expect("test observation still live");
            assert!(current_app().unwrap().should_stop());
            let before = probe.events.borrow().len();
            let saved = probe.saved.borrow().clone();
            assert!(!saved.is_empty());
            for message in &saved {
                dispatch(message);
            }
            assert_eq!(
                probe.events.borrow().len(),
                before,
                "handler ran after panic"
            );
            probe.observed.set(true);
        }));
    });
}

fn consumed_quit(case: &str) {
    let (probe, mut handler) = fixture(case);
    handler.started = |probe| {
        if probe.case == "started_quit" {
            probe.handle().terminate();
            consume_quit();
        } else {
            post(message(probe.tray.get(), WM_DISPLAYCHANGE, 0, 0));
        }
    };
    handler.display = |probe| probe.waker().wake(1);
    handler.wake = |probe, wake| {
        assert_eq!(wake, 1);
        probe.handle().terminate();
        consume_quit();
    };
    let app = create_app(&probe, handler);
    run_and_assert(app, &probe, false);
    let expected = if case == "started_quit" {
        vec![Event::Started, Event::Stopping]
    } else {
        vec![
            Event::Started,
            Event::Display,
            Event::Wake(1),
            Event::Stopping,
        ]
    };
    assert_eq!(*probe.events.borrow(), expected);
}

struct CloseHandler(Weak<Probe>);
impl AuxiliaryWindowHandler for CloseHandler {
    fn on_close_requested(&mut self) {
        let probe = self.0.upgrade().unwrap();
        probe.push(Event::Wake(7));
        install_saved_dispatch(&probe);
        panic!("native test panic");
    }
}
fn callback_panic(case: &str) {
    let (probe, mut handler) = fixture(case);
    handler.started = |probe| post(message(probe.tray.get(), WM_DISPLAYCHANGE, 0, 0));
    handler.display = |probe| match probe.case.as_str() {
        "panic_wake" => probe.waker().wake(1),
        "panic_tray" => {
            install_saved_dispatch(probe);
            panic!("native test panic");
        }
        "panic_deadline" => {
            probe.arm(1, 20);
            let timer = take_timer();
            probe.saved.borrow_mut().push(timer);
            post(timer);
        }
        "panic_close" => post(probe.saved.borrow()[0]),
        _ => unreachable!(),
    };
    handler.wake = |probe, wake| {
        assert_eq!(wake, 1);
        if probe.case == "panic_wake" {
            probe.waker().wake(99);
        }
        if probe.case == "panic_deadline" {
            probe.check_due(1);
        }
        install_saved_dispatch(probe);
        panic!("native test panic");
    };
    let app = create_app(&probe, handler);
    let auxiliary = if case == "panic_close" {
        let window = AuxiliaryWindow::new(
            &WindowAttributes {
                position: Point::new(0, 0),
                size: Size::new(20, 20),
                click_through: true,
                focusable: false,
            },
            Box::new(CloseHandler(Rc::downgrade(&probe))),
        )
        .expect("native auxiliary window");
        probe
            .saved
            .borrow_mut()
            .push(message(window.hwnd(), WM_CLOSE, 0, 0));
        Some(window)
    } else {
        None
    };
    match case {
        "panic_wake" => probe.saved.borrow_mut().push(message(
            HWND(probe.waker().target.window as *mut _),
            WM_APP_WAKE,
            0,
            0,
        )),
        "panic_tray" => {
            probe
                .saved
                .borrow_mut()
                .push(message(probe.tray.get(), WM_DISPLAYCHANGE, 0, 0))
        }
        _ => (),
    }
    run_and_assert(app, &probe, true);
    assert!(
        probe.observed.get(),
        "post-guard saved-message observation did not run"
    );
    let mut expected = vec![Event::Started, Event::Display];
    match case {
        "panic_wake" | "panic_deadline" => expected.push(Event::Wake(1)),
        "panic_close" => expected.push(Event::Wake(7)),
        _ => (),
    }
    expected.push(Event::Stopping);
    assert_eq!(*probe.events.borrow(), expected);
    drop(auxiliary);
}

fn typed_wakes() {
    let (probe, mut handler) = fixture("typed");
    handler.wake = |probe, _| {
        if probe.events.borrow().len() == 4 {
            probe.handle().terminate();
        }
    };
    let app = create_app(&probe, handler);
    for wake in [1, 2, 1] {
        app.waker().wake(wake);
    }
    run_and_assert(app, &probe, false);
    assert_eq!(
        *probe.events.borrow(),
        [
            Event::Started,
            Event::Wake(1),
            Event::Wake(2),
            Event::Wake(1),
            Event::Stopping
        ]
    );
}
fn keyed_deadlines() {
    let (probe, mut handler) = fixture("deadlines");
    handler.started = |probe| {
        pump_for(Duration::from_millis(60));
        assert_eq!(*probe.events.borrow(), [Event::Started]);
    };
    handler.wake = |probe, wake| {
        probe.check_due(wake);
        let count = probe
            .events
            .borrow()
            .iter()
            .filter(|event| matches!(event, Event::Wake(_)))
            .count();
        if count == 1 {
            assert_eq!(wake, 1);
            probe.arm(1, 20);
            probe.arm(2, 70);
        } else if count == 3 {
            probe.handle().terminate();
        }
    };
    let app = create_app(&probe, handler);
    probe.arm(1, 20);
    pump_for(Duration::from_millis(60));
    assert!(probe.events.borrow().is_empty());
    run_and_assert(app, &probe, false);
    let events = probe.events.borrow();
    assert_eq!(events.len(), 5);
    assert_eq!(events[..2], [Event::Started, Event::Wake(1)]);
    assert!(events[2..4].contains(&Event::Wake(1)));
    assert!(events[2..4].contains(&Event::Wake(2)));
    assert!(probe.due.borrow().is_empty());
}
fn nested_delivery() {
    let (probe, mut handler) = fixture("nested");
    handler.started = |probe| post(message(probe.tray.get(), WM_DISPLAYCHANGE, 0, 0));
    handler.display = |probe| probe.waker().wake(1);
    handler.wake = |probe, wake| match wake {
        1 => {
            probe.waker().wake(2);
            probe.arm(3, 20);
            pump_for(Duration::from_millis(100));
            assert_eq!(
                *probe.events.borrow(),
                [Event::Started, Event::Display, Event::Wake(1)]
            );
            probe.push(Event::NestedExit);
        }
        2 => assert_eq!(probe.events.borrow()[3], Event::NestedExit),
        3 => {
            probe.check_due(3);
            probe.handle().terminate();
        }
        _ => unreachable!(),
    };
    let app = create_app(&probe, handler);
    run_and_assert(app, &probe, false);
    assert_eq!(
        *probe.events.borrow(),
        [
            Event::Started,
            Event::Display,
            Event::Wake(1),
            Event::NestedExit,
            Event::Wake(2),
            Event::Wake(3),
            Event::Stopping
        ]
    );
}

fn drop_context() {
    let mut previous_timer = None;
    for run in [true, false] {
        let (probe, mut handler) = fixture("drop");
        handler.started = |probe| post(message(probe.tray.get(), WM_DISPLAYCHANGE, 0, 0));
        handler.display = |probe| {
            probe.arm(1, 20);
            probe.saved.borrow_mut().push(take_timer());
            probe.handle().terminate();
        };
        let app = create_app(&probe, handler);
        let wake_window = HWND(probe.waker().target.window as *mut _);
        probe
            .saved
            .borrow_mut()
            .push(message(wake_window, WM_APP_WAKE, 0, 0));
        probe
            .saved
            .borrow_mut()
            .push(message(probe.tray.get(), WM_DISPLAYCHANGE, 0, 0));
        if run {
            run_and_assert(app, &probe, false);
            previous_timer = probe
                .saved
                .borrow()
                .iter()
                .find(|saved| saved.message == WM_TIMER)
                .copied();
            assert!(previous_timer.is_some());
        } else {
            probe.saved.borrow_mut().push(previous_timer.unwrap());
            probe.arm(1, 20);
            probe.waker().wake(8);
            drop(app);
        }
        assert!(current_app().is_none());
        assert!(!unsafe { IsWindow(Some(wake_window)) }.as_bool());
        let before = probe.events.borrow().len();
        for saved in probe.saved.borrow().iter() {
            dispatch(saved);
        }
        let waker = probe.waker();
        assert!(waker.sender.send(9).is_err(), "receiver survived app drop");
        waker.wake(9);
        probe.handle().wake_at(2, Some(Instant::now()));
        assert!(current_app().is_none());
        assert_eq!(probe.events.borrow().len(), before);
        assert!(Rc::strong_count(&probe) == 1, "handler retained test state");
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {}
    }
}

fn menu_case(case: &str) {
    let (probe, mut handler) = fixture(case);
    handler.started = |probe| probe.open_menu();
    handler.wake = |probe, wake| {
        assert!(
            probe.modal.load(Ordering::SeqCst),
            "delivery escaped TrackPopupMenu"
        );
        match probe.case.as_str() {
            "menu_healthy" => match wake {
                1 => {
                    probe.arm(2, 20);
                    probe.arm(3, 60);
                }
                2 | 3 => {
                    probe.check_due(wake);
                    if probe.due.borrow().is_empty() {
                        probe.waker().wake(4);
                    }
                }
                4 => {
                    unsafe { EndMenu() }.expect("healthy UI-thread EndMenu");
                    probe.open_menu();
                }
                6 => (),
                5 => {
                    post(message(probe.tray.get(), WM_KEYDOWN, 0x28, 0));
                    post(message(probe.tray.get(), WM_KEYDOWN, 0x0D, 0));
                }
                _ => unreachable!(),
            },
            "menu_stop" => probe.handle().terminate(),
            "menu_panic_wake" => {
                probe.waker().wake(99);
                install_saved_dispatch(probe);
                panic!("native test panic");
            }
            "menu_panic_deadline" => {
                if wake == 1 {
                    probe.arm(2, 20);
                } else {
                    assert_eq!(wake, 2);
                    probe.check_due(2);
                    install_saved_dispatch(probe);
                    panic!("native test panic");
                }
            }
            _ => unreachable!(),
        }
    };
    handler.selected = |probe, id| {
        assert_eq!(id, 73);
        assert!(!probe.modal.load(Ordering::SeqCst));
        probe.handle().terminate();
    };
    let app = create_app(&probe, handler);
    probe.saved.borrow_mut().push(message(
        HWND(probe.waker().target.window as *mut _),
        WM_APP_WAKE,
        0,
        0,
    ));
    probe
        .saved
        .borrow_mut()
        .push(message(probe.tray.get(), WM_DISPLAYCHANGE, 0, 0));
    let waker = app.waker();
    let modal = probe.modal.clone();
    let healthy = case == "menu_healthy";
    let worker = thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(3);
        while !modal.load(Ordering::SeqCst) {
            assert!(Instant::now() < until, "real menu never entered");
            thread::sleep(Duration::from_millis(1));
        }
        if healthy {
            waker.wake(6);
        }
        waker.wake(1);
    });
    let second_worker = if case == "menu_healthy" {
        let waker = app.waker();
        let modal = probe.modal.clone();
        let menus = probe.menus.clone();
        Some(thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                if menus.load(Ordering::SeqCst) == 2 && modal.load(Ordering::SeqCst) {
                    waker.wake(5);
                    break;
                }
                assert!(Instant::now() < until, "second menu never entered");
                thread::sleep(Duration::from_millis(1));
            }
        }))
    } else {
        None
    };
    run_and_assert(app, &probe, case.starts_with("menu_panic"));
    worker.join().expect("menu worker failed");
    if let Some(worker) = second_worker {
        worker.join().expect("selection worker failed");
    }
    if case.starts_with("menu_panic") {
        assert!(probe.observed.get());
    }
    if case == "menu_healthy" {
        let events = probe.events.borrow();
        assert_eq!(events.len(), 11);
        assert_eq!(
            events[..4],
            [Event::Started, Event::Menu, Event::Wake(6), Event::Wake(1)]
        );
        assert!(events[4..6].contains(&Event::Wake(2)));
        assert!(events[4..6].contains(&Event::Wake(3)));
        assert_eq!(
            events[6..],
            [
                Event::Wake(4),
                Event::Menu,
                Event::Wake(5),
                Event::Selected(73),
                Event::Stopping
            ]
        );
    } else {
        let mut expected = vec![Event::Started, Event::Menu, Event::Wake(1)];
        if case == "menu_panic_deadline" {
            expected.push(Event::Wake(2));
        }
        expected.push(Event::Stopping);
        assert_eq!(*probe.events.borrow(), expected);
    }
}

fn run_native_case(case: &str) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "windows::tests::native_loop_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("DOME_WINDOWS_NATIVE_CASE", case)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn native test child");
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        if child.try_wait().expect("poll native child").is_some() {
            break;
        }
        if Instant::now() >= until {
            child.kill().expect("kill timed-out native child");
            let output = child.wait_with_output().unwrap();
            panic!(
                "native case {case} timed out\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().expect("collect native child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "native case {case} failed\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains(&format!("NATIVE_COMPLETE {case}")),
        "child did not complete selected case\n{stdout}\n{stderr}"
    );
}

#[test]
#[ignore = "child entry point, requires an exact case from the parent"]
fn native_loop_child() {
    let case = std::env::var("DOME_WINDOWS_NATIVE_CASE").expect("parent must select native case");
    match case.as_str() {
        "started_quit" | "wake_quit" => consumed_quit(&case),
        "panic_wake" | "panic_tray" | "panic_close" | "panic_deadline" => callback_panic(&case),
        "typed" => typed_wakes(),
        "deadlines" => keyed_deadlines(),
        "drop" => drop_context(),
        "nested" => nested_delivery(),
        "menu_healthy" | "menu_stop" | "menu_panic_wake" | "menu_panic_deadline" => {
            menu_case(&case)
        }
        _ => panic!("unknown native case {case}"),
    }
    println!("NATIVE_COMPLETE {case}");
}

macro_rules! native_case {
    ($name:ident, $($case:literal),+) => {
        #[test]
        #[ignore = "requires a Windows desktop and shell"]
        fn $name() { $(run_native_case($case);)+ }
    };
}
native_case!(
    native_started_termination_survives_consumed_quit,
    "started_quit"
);
native_case!(native_wake_termination_survives_consumed_quit, "wake_quit");
native_case!(
    native_callback_panics_reach_stopping_once,
    "panic_wake",
    "panic_tray",
    "panic_close"
);
native_case!(native_typed_wakes_before_run_preserve_duplicates, "typed");
native_case!(
    native_keyed_deadlines_wait_for_startup_and_rearm,
    "deadlines"
);
native_case!(
    native_deadline_panic_reaches_stopping_once,
    "panic_deadline"
);
native_case!(native_drop_detaches_callback_context, "drop");
native_case!(native_nested_delivery_resumes_after_handler, "nested");
native_case!(native_menu_delivers_wakes_and_deadlines, "menu_healthy");
native_case!(native_menu_normal_shutdown_finishes, "menu_stop");
native_case!(
    native_menu_callback_panic_finishes,
    "menu_panic_wake",
    "menu_panic_deadline"
);
