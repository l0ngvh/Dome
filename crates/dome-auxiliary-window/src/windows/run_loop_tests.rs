use super::*;
use crate::{AppHandler, LoopWaker, MenuEntry, MenuItem, Shell};
use std::collections::VecDeque;
use std::sync::{Arc, mpsc};
use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
pub(in crate::windows) enum Event {
    Wake(u8),
    Menu,
    Selected(u32),
    Display,
    Stopping,
}
#[derive(Debug, PartialEq, Eq)]
enum TimerCall {
    Set(usize, u32),
    Kill(usize),
}
#[derive(Default)]
struct RecordingTimer {
    returns: RefCell<VecDeque<usize>>,
    calls: RefCell<Vec<TimerCall>>,
}
impl DeadlineTimer for Rc<RecordingTimer> {
    fn set(&self, previous: usize, delay_ms: u32) -> usize {
        self.calls
            .borrow_mut()
            .push(TimerCall::Set(previous, delay_ms));
        self.returns
            .borrow_mut()
            .pop_front()
            .expect("missing timer result")
    }
    fn kill(&self, identifier: usize) {
        self.calls.borrow_mut().push(TimerCall::Kill(identifier));
    }
}
type WakeHook = Rc<RefCell<Option<Box<dyn Fn(u8)>>>>;
struct Handler {
    events: Rc<RefCell<Vec<Event>>>,
    hook: WakeHook,
    depth: Rc<Cell<usize>>,
}
impl AppHandler for Handler {
    type Wake = u8;
    fn on_wake(&mut self, _: &Shell, wake: u8) {
        assert_eq!(self.depth.replace(1), 0);
        self.events.borrow_mut().push(Event::Wake(wake));
        if let Some(hook) = &*self.hook.borrow() {
            hook(wake);
        }
        self.depth.set(0);
    }
    fn menu(&mut self) -> Vec<MenuEntry> {
        assert_eq!(self.depth.get(), 0);
        self.events.borrow_mut().push(Event::Menu);
        vec![MenuEntry::Item(MenuItem {
            id: 7,
            label: "Choice".into(),
            checked: false,
        })]
    }
    fn on_menu_selected(&mut self, id: u32) {
        self.events.borrow_mut().push(Event::Selected(id));
    }
    fn on_display_changed(&mut self) {
        self.events.borrow_mut().push(Event::Display);
    }
    fn on_stopping(&mut self) {
        self.events.borrow_mut().push(Event::Stopping);
    }
}
pub(in crate::windows) struct Fixture {
    pub(in crate::windows) state: Rc<LoopState<u8>>,
    pub(in crate::windows) events: Rc<RefCell<Vec<Event>>>,
    pub(in crate::windows) waker: LoopWaker<u8>,
    hook: WakeHook,
    timer: Rc<RecordingTimer>,
    _registration: Registration,
}
impl Fixture {
    pub(in crate::windows) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let events = Rc::new(RefCell::new(Vec::new()));
        let hook = Rc::new(RefCell::new(None));
        let delivery = Rc::new(WakeDelivery::new(
            receiver,
            Box::new(Handler {
                events: events.clone(),
                hook: hook.clone(),
                depth: Rc::new(Cell::new(0)),
            }),
        ));
        let timer = Rc::new(RecordingTimer::default());
        let mut state = LoopState::new(delivery, sender.clone());
        state.timer = Box::new(timer.clone());
        let state = Rc::new(state);
        let registration = Registration::new(state.clone()).unwrap();
        state.started(Shell::new(super::super::app::Shell::for_test()));
        Self {
            state,
            events,
            hook,
            timer,
            _registration: registration,
            waker: LoopWaker {
                sender,
                target: WakeTarget {
                    window: 0,
                    post: Some(Arc::new(|_| false)),
                },
            },
        }
    }
    fn returns(&self, identifiers: &[usize]) {
        self.timer.returns.borrow_mut().extend(identifiers);
    }
    fn arm(&self, wake: u8, at: Instant, now: Instant) {
        self.state.replace_deadline(wake, Some(at), now);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.state.stop_wakes();
        use windows::Win32::UI::WindowsAndMessaging::{MSG, PM_REMOVE, PeekMessageW, WM_QUIT};
        unsafe {
            let _ = PeekMessageW(&mut MSG::default(), None, WM_QUIT, WM_QUIT, PM_REMOVE);
        }
    }
}

#[test]
fn panic_in_callback_blocks_later_handlers() {
    for panic in [true, false] {
        let fixture = Fixture::new();
        if panic {
            guard_callback::<()>(|| panic!("first callback"));
            guard_callback::<()>(|| panic!("second callback"));
            assert!(fixture.state.panicked.get());
            let payload = fixture.state.take_panic().unwrap();
            assert_eq!(payload.downcast_ref::<&str>(), Some(&"first callback"));
        } else {
            fixture.state.terminate();
            assert!(!fixture.state.panicked.get());
        }
        assert!(fixture.state.should_stop());
        guard_callback(|| fixture.state.display_changed());
        fixture.waker.wake(1);
        guard_callback(|| fixture.state.wake());
        fixture.state.finish();
        fixture.state.finish();
        assert_eq!(*fixture.events.borrow(), [Event::Stopping]);
    }
}

#[test]
fn defer_callbacks_resumes_without_another_message() {
    let fixture = Fixture::new();
    let state = Rc::downgrade(&fixture.state);
    let waker = fixture.waker.clone();
    *fixture.hook.borrow_mut() = Some(Box::new(move |wake| {
        if wake == 1 {
            waker.wake(2);
            guard_callback(|| state.upgrade().unwrap().wake());
        }
    }));
    fixture.waker.wake(1);
    guard_callback(|| fixture.state.wake());
    assert_eq!(*fixture.events.borrow(), [Event::Wake(1), Event::Wake(2)]);
}

#[test]
fn manage_keyed_deadlines_keeps_independent_native_identifiers() {
    let fixture = Fixture::new();
    let now = Instant::now();
    let at = now + Duration::from_millis(100);
    fixture.returns(&[11, 22, 11]);
    fixture.arm(1, at, now);
    fixture.arm(2, at, now);
    fixture.arm(1, at + Duration::from_millis(100), now);
    fixture.waker.wake(2);
    fixture.state.replace_deadline(2, None, now);
    assert_eq!(
        *fixture.timer.calls.borrow(),
        [
            TimerCall::Set(0, 100),
            TimerCall::Set(0, 100),
            TimerCall::Set(11, 200),
            TimerCall::Kill(22),
        ]
    );
    fixture
        .state
        .timer_fired(11, at + Duration::from_millis(100));
    fixture
        .state
        .timer_fired(11, at + Duration::from_millis(100));
    assert_eq!(*fixture.events.borrow(), [Event::Wake(2), Event::Wake(1)]);
    assert_eq!(
        fixture.timer.calls.borrow().last(),
        Some(&TimerCall::Kill(11))
    );
}

#[test]
fn fire_timer_with_reused_identifier_waits_for_new_deadline() {
    let fixture = Fixture::new();
    let now = Instant::now();
    let old = now + Duration::from_millis(100);
    let replacement = old + Duration::from_millis(100);
    fixture.returns(&[11, 11, 11]);
    fixture.arm(1, old, now);
    fixture.arm(1, replacement, now);
    fixture.state.timer_fired(11, old);
    assert!(fixture.events.borrow().is_empty());
    assert_eq!(
        fixture.timer.calls.borrow().last(),
        Some(&TimerCall::Set(11, 100))
    );
    fixture.state.timer_fired(11, replacement);
    fixture.state.timer_fired(11, replacement);
    assert_eq!(*fixture.events.borrow(), [Event::Wake(1)]);
}

#[test]
fn cancel_deadline_rejects_queued_callback() {
    let fixture = Fixture::new();
    let now = Instant::now();
    let at = now + Duration::from_millis(100);
    fixture.returns(&[11, 22]);
    fixture.arm(1, at, now);
    fixture.arm(2, at, now);
    fixture.state.replace_deadline(1, None, now);
    let calls = fixture.timer.calls.borrow().len();
    fixture.state.timer_fired(11, at);
    fixture.state.timer_fired(999, at);
    assert_eq!(fixture.timer.calls.borrow().len(), calls);
    assert!(fixture.events.borrow().is_empty());
    assert_eq!(
        fixture.state.deadlines.borrow().get(&2).unwrap().timer_id,
        22
    );
    fixture.state.timer_fired(22, at);
    assert_eq!(*fixture.events.borrow(), [Event::Wake(2)]);
}

#[test]
fn fire_deadline_removes_entry_before_handler_rearm() {
    let fixture = Fixture::new();
    let now = Instant::now();
    let at = now + Duration::from_millis(100);
    let later = at + Duration::from_millis(100);
    fixture.returns(&[11, 11, 11]);
    fixture.arm(1, at, now);
    let state = Rc::downgrade(&fixture.state);
    let timer = fixture.timer.clone();
    *fixture.hook.borrow_mut() = Some(Box::new(move |_| {
        let state = state.upgrade().unwrap();
        assert!(!state.deadlines.borrow().contains_key(&1));
        assert_eq!(timer.calls.borrow().last(), Some(&TimerCall::Kill(11)));
        state.replace_deadline(1, Some(later), at);
    }));
    fixture.state.timer_fired(11, at);
    fixture.state.timer_fired(11, at);
    assert_eq!(*fixture.events.borrow(), [Event::Wake(1)]);
    assert_eq!(fixture.state.deadlines.borrow().get(&1).unwrap().at, later);
    *fixture.hook.borrow_mut() = None;
    fixture.state.timer_fired(11, later);
    assert_eq!(*fixture.events.borrow(), [Event::Wake(1), Event::Wake(1)]);
}

#[test]
fn arm_deadline_on_native_failure_stops_loop_with_bounded_delay() {
    let now = Instant::now();
    assert_eq!(
        native_delay(now - Duration::from_millis(1), now),
        USER_TIMER_MINIMUM
    );
    assert_eq!(native_delay(now + Duration::from_micros(10_001), now), 11);
    assert_eq!(
        native_delay(now + Duration::from_secs(u32::MAX as u64), now),
        USER_TIMER_MAXIMUM
    );
    for replacing in [false, true] {
        let fixture = Fixture::new();
        if replacing {
            fixture.returns(&[11]);
            fixture.arm(1, now + Duration::from_secs(1), now);
        }
        fixture.returns(&[0]);
        guard_callback(|| fixture.arm(1, now + Duration::from_secs(2), now));
        assert!(fixture.state.should_stop());
        assert!(fixture.state.deadlines.borrow().is_empty());
        assert_eq!(
            fixture.timer.calls.borrow().contains(&TimerCall::Kill(11)),
            replacing
        );
        fixture.state.finish();
        assert_eq!(*fixture.events.borrow(), [Event::Stopping]);
    }
}

#[test]
fn wake_accepted_before_target_destruction_may_post_afterward() {
    let fixture = Fixture::new();
    let mut window = super::super::window::OwnedHwnd::message_only().unwrap();
    let shutdown = Rc::new(RefCell::new(Vec::new()));
    let destroyed = shutdown.clone();
    window.on_destroyed = Some(Box::new(move || {
        destroyed.borrow_mut().push("window destroyed")
    }));
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (attempt_sender, attempt_receiver) = mpsc::channel();
    let release_receiver = std::sync::Mutex::new(release_receiver);
    let waker = LoopWaker {
        sender: fixture.waker.sender.clone(),
        target: WakeTarget {
            window: window.hwnd().0 as isize,
            post: Some(Arc::new(move |identifier| {
                entered_sender.send(()).unwrap();
                release_receiver
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
                attempt_sender.send((identifier, false)).unwrap();
                false
            })),
        },
    };
    let retained = waker.clone();
    let worker = std::thread::spawn(move || waker.wake(1));
    entered_receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    fixture.state.stop_wakes();
    assert!(retained.sender.send(2).is_err());
    shutdown.borrow_mut().push("receiver dropped");
    drop(window);
    assert_eq!(*shutdown.borrow(), ["receiver dropped", "window destroyed"]);
    release_sender.send(()).unwrap();
    assert!(
        !attempt_receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .1
    );
    worker.join().unwrap();
    retained.wake(3);
    assert!(attempt_receiver.try_recv().is_err());
    assert!(entered_receiver.try_recv().is_err());
}
