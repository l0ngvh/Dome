use super::*;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use objc2_core_foundation::CFRunLoop;

use crate::LoopWaker;
use crate::macos::run_loop::{CallbackContext, WakeTarget, create_wake_source};

thread_local! {
    static ON_DROP: RefCell<Option<Box<dyn Fn()>>> = RefCell::new(None);
}

#[derive(Clone)]
struct Value {
    id: char,
    drops: Arc<AtomicUsize>,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
impl Drop for Value {
    fn drop(&mut self) {
        ON_DROP.with(|callback| {
            if let Some(callback) = callback.borrow().as_ref() {
                callback();
            }
        });
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

struct Handler;
impl AppHandler for Handler {
    type Wake = Value;
    fn on_wake(&mut self, _: &Shell, _: Value) {
        panic!("closed wake must not deliver");
    }
}

fn target(recorder: impl Fn() + Send + Sync + 'static) -> WakeTarget {
    let context = Arc::new(CallbackContext::new());
    WakeTarget {
        source: create_wake_source(&context),
        run_loop: CFRunLoop::current().unwrap(),
        recorder: Some(Arc::new(recorder)),
    }
}

#[test]
fn send_typed_wakes_preserves_duplicates_and_after_disconnect_discards_values() {
    let (sender, receiver) = mpsc::channel();
    let received = Arc::new(Mutex::new(Vec::new()));
    let receiver = Arc::new(Mutex::new(Some(receiver)));
    let waker = LoopWaker {
        sender,
        target: target({
            let received = received.clone();
            let receiver = receiver.clone();
            move || {
                let value = receiver
                    .lock()
                    .unwrap()
                    .as_ref()
                    .expect("no signal after disconnect")
                    .try_recv()
                    .expect("enqueue must precede signal");
                received.lock().unwrap().push(value);
            }
        }),
    };
    for value in ['A', 'B', 'A'] {
        waker.wake(value);
    }
    assert_eq!(*received.lock().unwrap(), vec!['A', 'B', 'A']);
    drop(receiver.lock().unwrap().take());
    waker.wake('C');

    let drops = Arc::new(AtomicUsize::new(0));
    let signals = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = mpsc::channel();
    let delivery = Rc::new(WakeDelivery::new(receiver, Box::new(Handler)));
    ON_DROP.with(|slot| {
        let weak = Rc::downgrade(&delivery);
        *slot.borrow_mut() = Some(Box::new(move || {
            let delivery = weak.upgrade().unwrap();
            assert!(delivery.receiver.try_borrow_mut().is_ok());
            assert!(delivery.pending.try_borrow_mut().is_ok());
        }));
    });
    let waker = LoopWaker {
        sender,
        target: target({
            let signals = signals.clone();
            move || {
                signals.fetch_add(1, Ordering::Relaxed);
            }
        }),
    };
    waker.wake(Value {
        id: 'A',
        drops: drops.clone(),
    });
    delivery
        .pending
        .borrow_mut()
        .push_back(Callback::Wake(Value {
            id: 'B',
            drops: drops.clone(),
        }));
    delivery.close();
    assert_eq!(drops.load(Ordering::Relaxed), 2);
    waker.wake(Value {
        id: 'C',
        drops: drops.clone(),
    });
    assert_eq!(drops.load(Ordering::Relaxed), 3);
    assert_eq!(signals.load(Ordering::Relaxed), 1);
    ON_DROP.with(|slot| {
        slot.borrow_mut().take();
    });
}

#[test]
fn stop_delivery_with_accepted_wake_discards_without_waiting_for_signal() {
    let drops = Arc::new(AtomicUsize::new(0));
    let completed = Arc::new(AtomicBool::new(false));
    let (entered, accepted) = mpsc::channel();
    let (release, held) = mpsc::channel();
    let held = Mutex::new(held);
    let (sender, receiver) = mpsc::channel();
    let delivery = WakeDelivery::new(receiver, Box::new(Handler));
    let waker = LoopWaker {
        sender,
        target: target(move || {
            entered.send(()).unwrap();
            held.lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }),
    };
    let value = Value {
        id: 'A',
        drops: drops.clone(),
    };
    let done = completed.clone();
    let worker = std::thread::spawn(move || {
        waker.wake(value);
        done.store(true, Ordering::Release);
    });
    accepted.recv_timeout(Duration::from_secs(5)).unwrap();
    delivery.close();
    assert!(delivery.receiver.borrow().is_none());
    assert_eq!(drops.load(Ordering::Relaxed), 1);
    assert!(!completed.load(Ordering::Acquire));
    release.send(()).unwrap();
    worker.join().unwrap();
    delivery.wake();
    assert!(completed.load(Ordering::Acquire));
}
