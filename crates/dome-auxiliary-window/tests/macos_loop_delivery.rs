#[cfg(not(target_os = "macos"))]
fn main() {
    println!("UNAVAILABLE macos_loop_delivery requires an interactive macOS desktop");
}

#[cfg(target_os = "macos")]
fn main() {
    native::main();
}

#[cfg(target_os = "macos")]
mod native {
    use std::cell::{Cell, OnceCell, RefCell};
    use std::ffi::c_void;
    use std::process::{Command, Stdio};
    use std::rc::Rc;
    use std::thread;
    use std::time::{Duration, Instant};

    use dome_auxiliary_window::{
        App, AppHandler, Icon, LoopHandle, LoopWaker, MenuEntry, MenuItem, Shell,
    };
    use objc2::rc::Retained;
    use objc2::{MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
    use objc2_app_kit::{
        NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSMenu,
        NSMenuDidBeginTrackingNotification, NSStatusBarButton, NSView,
    };
    use objc2_core_foundation::{
        CFAbsoluteTimeGetCurrent, CFRetained, CFRunLoop, CFRunLoopTimer, CFRunLoopTimerContext,
        kCFRunLoopCommonModes, kCFRunLoopDefaultMode,
    };
    use objc2_foundation::{
        NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSPoint,
    };

    const CASES: &[&str] = &[
        "keyed_deadlines_and_duplicate_wakes",
        "deadline_rearm_from_handler",
        "deadline_before_launch",
        "wake_panic_reaches_stopping",
        "deadline_panic_reaches_stopping",
        "drop_disarms_retained_handles",
        "menu_open_wake_and_deadline",
        "nested_native_delivery_is_serialized",
        "terminate_from_deadline_callback",
    ];
    const ICON: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0,
        1, 165, 246, 69, 64, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum TestWake {
        A,
        B,
        C,
        D,
        Rearmed,
        Other,
        BeforeLaunch,
        Control,
        PanicWorker,
        PanicDeadline,
        OpenNested,
        NestedWorker,
        NestedDeadline,
        MenuWorker,
        MenuDeadline,
        Terminate,
        Late,
    }

    struct ControlTimer {
        timer: CFRetained<CFRunLoopTimer>,
        _callback: Box<Box<dyn FnMut()>>,
    }
    impl Drop for ControlTimer {
        fn drop(&mut self) {
            self.timer.invalidate();
        }
    }
    unsafe extern "C-unwind" fn control_callback(_: *mut CFRunLoopTimer, info: *mut c_void) {
        unsafe { (&mut *info.cast::<Box<dyn FnMut()>>())() };
    }

    struct State {
        case: String,
        handle: OnceCell<LoopHandle<TestWake>>,
        waker: OnceCell<LoopWaker<TestWake>>,
        traces: RefCell<Vec<String>>,
        wakes: RefCell<Vec<TestWake>>,
        timers: RefCell<Vec<ControlTimer>>,
        workers: RefCell<Vec<thread::JoinHandle<()>>>,
        depth: Cell<usize>,
        started: Cell<bool>,
        stopped: Cell<usize>,
        destroyed: Cell<usize>,
        watchdog: Cell<bool>,
        tracking: Cell<bool>,
        worker_before_menu_deadline: Cell<bool>,
        menu: RefCell<Option<Retained<NSMenu>>>,
        menu_deliveries: RefCell<Vec<TestWake>>,
        nested_exit: Cell<bool>,
        rearm_count: Cell<usize>,
        replacement_due: Cell<Option<Instant>>,
    }
    thread_local! { static STATE: OnceCell<Rc<State>> = const { OnceCell::new() }; }

    impl State {
        fn new(case: &str) -> Rc<Self> {
            Rc::new(Self {
                case: case.into(),
                handle: OnceCell::new(),
                waker: OnceCell::new(),
                traces: RefCell::new(Vec::new()),
                wakes: RefCell::new(Vec::new()),
                timers: RefCell::new(Vec::new()),
                workers: RefCell::new(Vec::new()),
                depth: Cell::new(0),
                started: Cell::new(false),
                stopped: Cell::new(0),
                destroyed: Cell::new(0),
                watchdog: Cell::new(false),
                tracking: Cell::new(false),
                worker_before_menu_deadline: Cell::new(false),
                menu: RefCell::new(None),
                menu_deliveries: RefCell::new(Vec::new()),
                nested_exit: Cell::new(false),
                rearm_count: Cell::new(0),
                replacement_due: Cell::new(None),
            })
        }
        fn trace(&self, text: impl Into<String>) {
            let text = text.into();
            println!("TRACE {} {text}", self.case);
            self.traces.borrow_mut().push(text);
        }
        fn handle(&self) -> &LoopHandle<TestWake> {
            self.handle.get().unwrap()
        }
        fn arm(&self, wake: TestWake, milliseconds: u64) {
            self.handle().wake_at(
                wake,
                Some(Instant::now() + Duration::from_millis(milliseconds)),
            );
        }
        fn after(self: &Rc<Self>, milliseconds: u64, callback: impl FnMut() + 'static) {
            let mut callback: Box<Box<dyn FnMut()>> = Box::new(Box::new(callback));
            let mut context = CFRunLoopTimerContext {
                version: 0,
                info: (&mut *callback as *mut Box<dyn FnMut()>).cast(),
                retain: None,
                release: None,
                copyDescription: None,
            };
            let timer = unsafe {
                CFRunLoopTimer::new(
                    None,
                    CFAbsoluteTimeGetCurrent() + milliseconds as f64 / 1000.0,
                    0.0,
                    0,
                    0,
                    Some(control_callback),
                    &mut context,
                )
                .unwrap()
            };
            CFRunLoop::main()
                .unwrap()
                .add_timer(Some(&timer), unsafe { kCFRunLoopCommonModes });
            self.timers.borrow_mut().push(ControlTimer {
                timer,
                _callback: callback,
            });
        }
        fn worker(self: &Rc<Self>, wake: TestWake, milliseconds: u64) {
            let waker = self.waker.get().unwrap().clone();
            self.workers.borrow_mut().push(thread::spawn(move || {
                thread::sleep(Duration::from_millis(milliseconds));
                waker.wake(wake);
            }));
        }
        fn finish_after(self: &Rc<Self>, milliseconds: u64) {
            self.after(milliseconds, stop_run);
        }
        fn count(&self, wake: TestWake) -> usize {
            self.wakes
                .borrow()
                .iter()
                .filter(|value| **value == wake)
                .count()
        }
        fn validate(&self) {
            assert!(!self.watchdog.get(), "watchdog is not expected delivery");
            assert_eq!(self.stopped.get(), 1, "once-only stopping");
            assert_eq!(
                self.destroyed.get(),
                1,
                "handler destruction after stopping"
            );
            let traces = self.traces.borrow();
            let stopping = traces.iter().position(|value| value == "stopping").unwrap();
            let destruction = traces
                .iter()
                .position(|value| value == "handler-drop")
                .unwrap();
            assert!(
                stopping < destruction,
                "stopping precedes handler destruction"
            );
            match self.case.as_str() {
                "keyed_deadlines_and_duplicate_wakes" => assert_eq!(
                    *self.wakes.borrow(),
                    [TestWake::A, TestWake::B, TestWake::A, TestWake::C]
                ),
                "deadline_rearm_from_handler" => {
                    assert_eq!(self.count(TestWake::Rearmed), 2);
                    assert_eq!(self.count(TestWake::Other), 1);
                    assert_eq!(self.wakes.borrow().len(), 3);
                }
                "deadline_before_launch" => {
                    assert_eq!(
                        *self.wakes.borrow(),
                        [TestWake::BeforeLaunch, TestWake::Control]
                    )
                }
                "wake_panic_reaches_stopping" | "drop_disarms_retained_handles" => {
                    assert_eq!(*self.wakes.borrow(), [TestWake::PanicWorker])
                }
                "deadline_panic_reaches_stopping" => {
                    assert_eq!(*self.wakes.borrow(), [TestWake::PanicDeadline])
                }
                "menu_open_wake_and_deadline" => {
                    assert!(
                        self.worker_before_menu_deadline.get(),
                        "worker source must deliver in-menu before the deadline can drain its queue"
                    );
                    assert_eq!(
                        *self.menu_deliveries.borrow(),
                        [TestWake::MenuWorker, TestWake::MenuDeadline],
                        "both values must arrive while real menu tracks"
                    );
                    assert!(!self.tracking.get());
                }
                "nested_native_delivery_is_serialized" => assert_eq!(
                    *self.wakes.borrow(),
                    [
                        TestWake::OpenNested,
                        TestWake::NestedWorker,
                        TestWake::NestedDeadline
                    ]
                ),
                _ => panic!("unknown validation case"),
            }
        }
    }

    fn application() -> Retained<NSApplication> {
        NSApplication::sharedApplication(MainThreadMarker::new().unwrap())
    }
    fn stop_run() {
        let application = application();
        application.stop(None);
        let event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
            NSEventType::ApplicationDefined, NSPoint::new(0.0, 0.0), NSEventModifierFlags::empty(), 0.0, 0, None, 0, 0, 0).unwrap();
        application.postEvent_atStart(&event, true);
    }
    fn pump(milliseconds: u64) {
        let until = Instant::now() + Duration::from_millis(milliseconds);
        while Instant::now() < until {
            CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, 0.01, false);
        }
    }

    struct HandlerAccess<'a>(&'a Cell<usize>);
    impl Drop for HandlerAccess<'_> {
        fn drop(&mut self) {
            self.0.set(0);
        }
    }

    struct Handler(Rc<State>);
    impl Drop for Handler {
        fn drop(&mut self) {
            self.0.destroyed.set(self.0.destroyed.get() + 1);
            self.0.trace("handler-drop");
        }
    }
    impl AppHandler for Handler {
        type Wake = TestWake;
        fn on_started(&mut self, shell: &Shell) {
            let state = &self.0;
            assert!(!state.started.replace(true));
            state.trace("started");
            shell.set_tooltip("Dome loop delivery test child");
            let watchdog = state.clone();
            state.after(5000, move || {
                watchdog.watchdog.set(true);
                watchdog.trace("WATCHDOG");
                stop_run();
            });
            match state.case.as_str() {
                "keyed_deadlines_and_duplicate_wakes" => {
                    state.arm(TestWake::C, 80);
                    state.arm(TestWake::D, 120);
                    let replacement = Instant::now() + Duration::from_millis(300);
                    state.replacement_due.set(Some(replacement));
                    state.handle().wake_at(TestWake::C, Some(replacement));
                    state.handle().wake_at(TestWake::D, None);
                    let control = state.clone();
                    state.after(200, move || {
                        assert_eq!(
                            *control.wakes.borrow(),
                            [TestWake::A, TestWake::B, TestWake::A],
                            "original deadline controls"
                        );
                        control.trace("original-times-passed");
                    });
                    state.finish_after(450);
                }
                "deadline_rearm_from_handler" => {
                    state.arm(TestWake::Rearmed, 40);
                    state.finish_after(350);
                }
                "deadline_before_launch" => {
                    state.worker(TestWake::Control, 170);
                    pump(100);
                    state.trace("launch-expired");
                    state.finish_after(250);
                }
                "wake_panic_reaches_stopping" | "drop_disarms_retained_handles" => {
                    state.arm(TestWake::Late, 300);
                    state.worker(TestWake::PanicWorker, 50);
                }
                "deadline_panic_reaches_stopping" => state.arm(TestWake::PanicDeadline, 40),
                "menu_open_wake_and_deadline" => {
                    state.after(50, open_status_menu);
                    state.finish_after(900);
                }
                "nested_native_delivery_is_serialized" => {
                    state.arm(TestWake::OpenNested, 30);
                    state.finish_after(450);
                }
                "terminate_from_deadline_callback" => state.arm(TestWake::Terminate, 40),
                _ => unreachable!(),
            }
            state.trace("started-return");
        }
        fn on_wake(&mut self, _: &Shell, wake: TestWake) {
            let state = &self.0;
            assert!(state.started.get());
            assert_eq!(state.depth.replace(1), 0, "handler callback reentered");
            let access = HandlerAccess(&state.depth);
            state.trace(format!("wake-{wake:?}"));
            state.wakes.borrow_mut().push(wake);
            match wake {
                TestWake::PanicWorker | TestWake::PanicDeadline => {
                    state.trace("intentional-panic");
                    panic!("intentional wake-route panic");
                }
                TestWake::C => assert!(
                    Instant::now() >= state.replacement_due.get().unwrap(),
                    "replacement deadline fired early"
                ),
                TestWake::Rearmed => {
                    let count = state.rearm_count.get() + 1;
                    state.rearm_count.set(count);
                    if count == 1 {
                        state.arm(TestWake::Rearmed, 60);
                        state.arm(TestWake::Other, 100);
                    }
                }
                TestWake::BeforeLaunch => assert!(
                    state
                        .traces
                        .borrow()
                        .iter()
                        .any(|value| value == "started-return"),
                    "deadline entered inline during startup"
                ),
                TestWake::OpenNested => {
                    state.worker(TestWake::NestedWorker, 30);
                    state.arm(TestWake::NestedDeadline, 70);
                    pump(180);
                    assert_eq!(
                        *state.wakes.borrow(),
                        [TestWake::OpenNested],
                        "nested native arrivals must defer"
                    );
                    state.trace("nested-exit");
                    state.nested_exit.set(true);
                }
                TestWake::NestedWorker | TestWake::NestedDeadline => {
                    assert!(state.nested_exit.get());
                    state.trace("deferred-after-exit");
                }
                TestWake::MenuWorker | TestWake::MenuDeadline => {
                    let mode = CFRunLoop::main()
                        .unwrap()
                        .current_mode()
                        .unwrap()
                        .to_string();
                    state.trace(format!("menu-mode-{wake:?}-{mode}"));
                    assert!(state.tracking.get(), "value arrived after menu closure");
                    assert_eq!(mode, "NSEventTrackingRunLoopMode");
                    state.menu_deliveries.borrow_mut().push(wake);
                }
                TestWake::Terminate => {
                    state.handle().terminate();
                    state.trace("terminate-return");
                }
                _ => {}
            }
            drop(access);
            state.trace(format!("callback-return-{wake:?}"));
        }
        fn on_stopping(&mut self) {
            let state = &self.0;
            assert_eq!(state.depth.get(), 0);
            state.stopped.set(state.stopped.get() + 1);
            state.trace("stopping");
            assert_eq!(state.stopped.get(), 1);
            if state.case == "terminate_from_deadline_callback" {
                assert_eq!(*state.wakes.borrow(), [TestWake::Terminate]);
                assert!(
                    state
                        .traces
                        .borrow()
                        .iter()
                        .any(|value| value == "callback-return-Terminate")
                );
                println!("PASS {}", state.case);
            }
        }
        fn menu(&mut self) -> Vec<MenuEntry> {
            self.0.trace("menu-built");
            vec![MenuEntry::Item(MenuItem {
                label: "Loop delivery test (automatic close)".into(),
                id: 1,
                checked: false,
            })]
        }
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        struct MenuObserver;
        unsafe impl NSObjectProtocol for MenuObserver {}
        impl MenuObserver {
            #[unsafe(method(menuTracking:))]
            fn menu_tracking(&self, notification: &NSNotification) {
                STATE.with(|cell| {
                    let state = cell.get().unwrap();
                    let object = notification.object().unwrap();
                    let menu = object.downcast::<NSMenu>().expect("menu tracking object");
                    assert!(!state.tracking.replace(true));
                    state.trace("menu-open");
                    *state.menu.borrow_mut() = Some(menu);
                    state.worker(TestWake::MenuWorker, 80);
                    state.arm(TestWake::MenuDeadline, 240);
                    let source_control = state.clone();
                    state.after(160, move || {
                        source_control.worker_before_menu_deadline.set(source_control.count(TestWake::MenuWorker) == 1);
                        source_control.trace(format!("worker-before-deadline-{}", source_control.worker_before_menu_deadline.get()));
                    });
                    let control = state.clone();
                    state.after(400, move || {
                        control.trace("menu-close");
                        control.tracking.set(false);
                        control.menu.borrow().as_ref().unwrap().cancelTrackingWithoutAnimation();
                    });
                });
            }
        }
    );

    fn status_button(view: &NSView) -> Option<Retained<NSStatusBarButton>> {
        if let Some(button) = view.downcast_ref::<NSStatusBarButton>() {
            return Some(button.retain());
        }
        view.subviews().iter().find_map(|view| status_button(&view))
    }
    fn open_status_menu() {
        let button = application()
            .windows()
            .iter()
            .filter_map(|window| window.contentView())
            .find_map(|view| status_button(&view))
            .expect("child owns a status button");
        unsafe { button.performClick(None) };
        STATE.with(|cell| cell.get().unwrap().trace("menu-click-return"));
    }

    fn child(case: &str) {
        let state = State::new(case);
        STATE.with(|cell| assert!(cell.set(state.clone()).is_ok()));
        let main = MainThreadMarker::new().expect("fresh process main thread");
        let observer: Retained<MenuObserver> =
            unsafe { msg_send![MenuObserver::alloc(main), init] };
        if case == "menu_open_wake_and_deadline" {
            unsafe {
                NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                    &observer,
                    sel!(menuTracking:),
                    Some(NSMenuDidBeginTrackingNotification),
                    None,
                )
            };
        }
        objc2::rc::autoreleasepool(|_| {
            let app = App::new(
                Icon::from_png(ICON).unwrap(),
                Box::new(Handler(state.clone())),
            )
            .unwrap();
            assert!(state.handle.set(app.handle()).is_ok());
            assert!(state.waker.set(app.waker()).is_ok());
            if case == "keyed_deadlines_and_duplicate_wakes" {
                for wake in [TestWake::A, TestWake::B, TestWake::A] {
                    app.waker().wake(wake);
                }
            }
            if case == "deadline_before_launch" {
                state.arm(TestWake::BeforeLaunch, 30);
            }
            state.trace("run-enter");
            app.run();
        });
        state.trace("run-return");
        assert!(
            application().delegate().is_none(),
            "AppKit delegate detached"
        );
        assert!(
            application()
                .windows()
                .iter()
                .filter(|window| window.isVisible())
                .filter_map(|window| window.contentView())
                .all(|view| status_button(&view).is_none()),
            "status item removed"
        );
        if case == "drop_disarms_retained_handles" {
            let count = state.traces.borrow().len();
            state.waker.get().unwrap().wake(TestWake::Late);
            state.arm(TestWake::Late, 20);
            state.handle().stop_wakes();
            state.handle().terminate();
            pump(400);
            assert_eq!(
                state.traces.borrow().len(),
                count,
                "dropped handler received activity"
            );
            state.trace("retained-handles-disarmed");
            let never = State::new("never-run");
            let app = App::new(
                Icon::from_png(ICON).unwrap(),
                Box::new(Handler(never.clone())),
            )
            .unwrap();
            let handle = app.handle();
            let waker = app.waker();
            handle.wake_at(
                TestWake::Late,
                Some(Instant::now() + Duration::from_millis(20)),
            );
            drop(app);
            waker.wake(TestWake::Late);
            handle.wake_at(TestWake::Late, Some(Instant::now()));
            handle.stop_wakes();
            handle.terminate();
            pump(80);
            assert_eq!(
                never.stopped.get(),
                0,
                "never-run drop must not call stopping"
            );
            assert_eq!(never.destroyed.get(), 1);
            assert!(never.wakes.borrow().is_empty());
            assert!(application().delegate().is_none());
            drop((handle, waker));
            state.trace("never-run-disarmed");
        }
        for worker in state.workers.borrow_mut().drain(..) {
            worker.join().unwrap();
        }
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(&observer) };
        if let Some(menu) = state.menu.borrow_mut().take() {
            assert!(menu.delegate().is_none(), "menu delegate detached");
            assert_eq!(menu.numberOfItems(), 0, "menu targets detached");
        }
        state.timers.borrow_mut().clear();
        state.validate();
        println!("PASS {case}");
    }

    pub fn main() {
        let arguments: Vec<_> = std::env::args().collect();
        if arguments.iter().any(|value| value == "--list") {
            for case in CASES {
                println!("{case}: test");
            }
            return;
        }
        if let Some(index) = arguments.iter().position(|value| value == "--child") {
            child(&arguments[index + 1]);
            return;
        }
        let selected = arguments
            .iter()
            .position(|value| value == "--case")
            .map(|index| arguments[index + 1].as_str());
        if let Some(case) = selected {
            assert!(CASES.contains(&case), "unknown case {case}");
        }
        for case in CASES
            .iter()
            .copied()
            .filter(|case| selected.is_none_or(|selected| selected == *case))
        {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--child", case])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let limit = Instant::now() + Duration::from_secs(15);
            loop {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                if Instant::now() >= limit {
                    child.kill().unwrap();
                    let output = child.wait_with_output().unwrap();
                    panic!(
                        "TIMEOUT {case}\n{}\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                thread::sleep(Duration::from_millis(10));
            }
            let output = child.wait_with_output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            print!("{stdout}");
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            assert!(
                output.status.success(),
                "child failed {case}: {}",
                output.status
            );
            assert!(
                stdout.lines().any(|line| line == format!("PASS {case}")),
                "missing case-specific success trace for {case}"
            );
        }
    }
}
