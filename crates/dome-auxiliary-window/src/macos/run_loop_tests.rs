use super::*;
use std::cell::Cell;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Duration;

use crate::AppHandler;

#[derive(Default)]
struct Handler {
    trace: Rc<RefCell<Vec<char>>>,
    nested: Option<Weak<LoopState<char>>>,
    depth: Rc<Cell<usize>>,
}

impl AppHandler for Handler {
    type Wake = char;

    fn on_wake(&mut self, _shell: &Shell, wake: char) {
        assert_eq!(self.depth.replace(1), 0);
        self.trace.borrow_mut().push(wake);
        if wake == 'A'
            && let Some(state) = self.nested.as_ref().and_then(Weak::upgrade)
        {
            let bridge = DispatchBridge {
                dispatch: state.clone(),
            };
            state.waker.wake('B');
            bridge.dispatch.wake();
            state
                .deadlines
                .borrow_mut()
                .replace('C', Some(Instant::now()));
            let timer = state.deadlines.borrow().entries[&'C']
                .timer
                .clone()
                .unwrap();
            bridge.dispatch.timer(&timer);
            bridge
                .dispatch
                .menu(Box::new(|entries| assert!(entries.is_empty())));
            bridge.dispatch.selected(1);
            assert_eq!(&*self.trace.borrow(), &['A']);
            self.trace.borrow_mut().push('X');
        }
        self.depth.set(0);
    }

    fn menu(&mut self) -> Vec<MenuEntry> {
        assert_eq!(self.depth.get(), 0);
        self.trace.borrow_mut().push('M');
        Vec::new()
    }

    fn on_menu_selected(&mut self, _id: u32) {
        assert_eq!(self.depth.get(), 0);
        self.trace.borrow_mut().push('S');
    }
}

fn state(handler: Handler) -> Rc<LoopState<char>> {
    let context = Arc::new(CallbackContext::new());
    let (waker, receiver) = test_waker(&context);
    Rc::new(LoopState::new(
        WakeDelivery::new(receiver, Box::new(handler)),
        context,
        waker,
    ))
}

fn test_waker<W>(context: &Arc<CallbackContext>) -> (LoopWaker<W>, mpsc::Receiver<W>) {
    let (sender, receiver) = mpsc::channel();
    (
        LoopWaker {
            sender,
            target: WakeTarget {
                source: create_wake_source(context),
                run_loop: CFRunLoop::current().unwrap(),
                recorder: None,
            },
        },
        receiver,
    )
}

fn shell() -> Shell {
    Shell::new(crate::macos::app::Shell::Test)
}

#[test]
fn deliver_nested_work_after_callback_exit_without_another_signal() {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let context = Arc::new(CallbackContext::new());
    let (waker, receiver) = test_waker(&context);
    let state = Rc::new_cyclic(|weak| {
        LoopState::new(
            WakeDelivery::new(
                receiver,
                Box::new(Handler {
                    trace: trace.clone(),
                    nested: Some(weak.clone()),
                    ..Default::default()
                }),
            ),
            context,
            waker,
        )
    });
    state.started(shell());
    state.waker.wake('A');
    DispatchBridge {
        dispatch: state.clone(),
    }
    .dispatch
    .wake();
    assert_eq!(&*trace.borrow(), &['A', 'X', 'B', 'C', 'M', 'S']);
    state.close();
}

#[test]
fn replace_and_cancel_deadlines_independently_of_other_keys_and_queued_wakes() {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let state = state(Handler {
        trace: trace.clone(),
        ..Default::default()
    });
    state.started(shell());
    let now = Instant::now();
    let first = now + Duration::from_secs(10);
    let second = first + Duration::from_secs(10);
    let (old_a, old_b);
    {
        let mut deadlines = state.deadlines.borrow_mut();
        deadlines.replace('A', Some(first));
        deadlines.replace('B', Some(first));
        old_a = deadlines.entries[&'A'].timer.clone().unwrap();
        old_b = deadlines.entries[&'B'].timer.clone().unwrap();
        deadlines.replace('A', Some(second));
        assert!(!old_a.is_valid());
        assert!(old_b.is_valid());
        assert_eq!(deadlines.entries[&'B'].at, first);
        state.waker.wake('B');
        deadlines.replace('B', None);
        assert!(!old_b.is_valid());
        assert!(deadlines.take_due(first).is_empty());
        assert_eq!(deadlines.take_due(second), vec!['A']);
        assert!(deadlines.take_due(second).is_empty());
        deadlines.replace('B', Some(second));
        assert_eq!(deadlines.take_due(second), vec!['B']);
    }
    state.wake();
    assert_eq!(&*trace.borrow(), &['B']);
    state.close();
}

#[test]
fn fire_timer_early_rearms_and_rejects_old_timer_before_delivering_once_at_due() {
    let state = state(Handler::default());
    state.started(shell());
    let due = Instant::now() + Duration::from_secs(60);
    let mut deadlines = state.deadlines.borrow_mut();
    deadlines.replace('A', Some(due));
    let old = deadlines.entries[&'A'].timer.clone().unwrap();
    assert!(
        deadlines
            .fired(&old, due - Duration::from_nanos(1))
            .is_empty()
    );
    assert!(!old.is_valid());
    let current = deadlines.entries[&'A'].timer.clone().unwrap();
    assert!(!std::ptr::eq(&*old, &*current));
    assert!(current.is_valid());
    assert!(deadlines.fired(&old, due).is_empty());
    assert_eq!(deadlines.fired(&current, due), vec!['A']);
    assert!(!current.is_valid());
    assert!(deadlines.fired(&current, due).is_empty());
    drop(deadlines);
    state.close();
}

#[test]
fn rearm_after_closing_deadlines_cannot_recreate_timers() {
    let state = state(Handler::default());
    let now = Instant::now();
    let mut deadlines = state.deadlines.borrow_mut();
    deadlines.replace('A', Some(now));
    deadlines.replace('B', Some(now));
    assert!(
        deadlines
            .entries
            .values()
            .all(|entry| entry.timer.is_none())
    );
    deadlines.start();
    let timers: Vec<_> = deadlines
        .entries
        .values()
        .map(|entry| entry.timer.clone().unwrap())
        .collect();
    deadlines.cancel_all();
    deadlines.cancel_all();
    assert!(timers.iter().all(|timer| !timer.is_valid()));
    deadlines.replace('C', Some(now));
    deadlines.start();
    assert!(deadlines.entries.is_empty());
    assert!(deadlines.take_due(now).is_empty());
    drop(deadlines);

    let not_started = self::state(Handler::default());
    let mut deadlines = not_started.deadlines.borrow_mut();
    deadlines.replace('A', Some(now));
    deadlines.cancel_all();
    deadlines.start();
    deadlines.replace('B', Some(now));
    assert!(deadlines.entries.is_empty());
    assert!(!deadlines.ready);
}

#[test]
fn release_source_owners_keeps_context_alive_until_final_off_thread_release() {
    let released = Arc::new(AtomicBool::new(false));
    let context = Arc::new(CallbackContext {
        dispatch: AtomicPtr::new(std::ptr::null_mut()),
        released: Some(released.clone()),
    });
    let weak = Arc::downgrade(&context);
    let (waker, receiver) = test_waker(&context);
    let source = waker.target.source.clone();
    let target = waker.target.clone();
    let state = Rc::new(LoopState::new(
        WakeDelivery::new(receiver, Box::new(Handler::default())),
        context.clone(),
        waker,
    ));
    let mut bridge = Box::new(DispatchBridge {
        dispatch: state.clone(),
    });
    context.dispatch.store(&mut *bridge, Ordering::Release);
    context.clear();
    state.close();
    source.invalidate();
    drop(bridge);
    drop(state);
    drop(source);
    drop(context);
    assert_eq!(weak.strong_count(), 1);
    assert!(!released.load(Ordering::Acquire));
    std::thread::spawn(move || {
        target.signal_nonblocking();
        assert!(!target.source.is_valid());
        drop(target);
    })
    .join()
    .unwrap();
    assert!(released.load(Ordering::Acquire));
    assert!(weak.upgrade().is_none());
}
