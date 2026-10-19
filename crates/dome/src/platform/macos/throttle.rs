use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::platform::macos::timer::Timer;

pub(super) struct Throttle<T: 'static> {
    interval: Duration,
    state: Rc<RefCell<Pending<T>>>,
    callback: Rc<dyn Fn(T)>,
}

impl<T: 'static> Throttle<T> {
    pub(super) fn new(interval: Duration, callback: impl Fn(T) + 'static) -> Self {
        Self {
            interval,
            state: Rc::new(RefCell::new(Pending {
                last_sent: None,
                value: None,
                timer: None,
            })),
            callback: Rc::new(callback),
        }
    }

    pub(super) fn submit(&self, value: T) {
        let now = Instant::now();
        let mut state = self.state.borrow_mut();
        let can_send = state
            .last_sent
            .map(|last| now.duration_since(last) >= self.interval)
            .unwrap_or(true);

        if can_send {
            state.timer.take();
            state.last_sent = Some(now);
            state.value = None;
            drop(state);
            (self.callback)(value);
        } else {
            state.value = Some(value);
            if state.timer.is_none() {
                let delay = self
                    .interval
                    .saturating_sub(state.last_sent.unwrap().elapsed());
                let pending = Rc::downgrade(&self.state);
                let callback = self.callback.clone();
                state.timer = Some(Timer::once(delay, move || {
                    let Some(pending) = pending.upgrade() else {
                        return;
                    };
                    let mut state = pending.borrow_mut();
                    state.timer.take();
                    let value = state.value.take();
                    if value.is_some() {
                        state.last_sent = Some(Instant::now());
                    }
                    drop(state);
                    if let Some(value) = value {
                        callback(value);
                    }
                }));
            }
        }
    }
}

struct Pending<T> {
    last_sent: Option<Instant>,
    value: Option<T>,
    timer: Option<Timer>,
}
