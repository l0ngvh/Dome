use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::{Rc, Weak};
use std::sync::mpsc::Sender;
use std::time::Instant;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, KillTimer, PostMessageW, PostQuitMessage, SetTimer, USER_TIMER_MAXIMUM,
    USER_TIMER_MINIMUM,
};

use super::WM_APP_WAKE;
use crate::delivery::{Callbacks, WakeDelivery};

type PanicPayload = Box<dyn Any + Send>;

pub(super) trait NativeCallbacks: Callbacks {
    fn should_stop(&self) -> bool;
    fn dispatch_enabled(&self) -> bool;
    fn record_panic(&self, payload: PanicPayload);
    fn timer_fired(&self, identifier: usize, now: Instant);
}

thread_local! {
    static CURRENT_APP: RefCell<Option<Weak<dyn NativeCallbacks>>> = RefCell::new(None);
}

pub(super) fn current_app() -> Option<Rc<dyn NativeCallbacks>> {
    CURRENT_APP.with(|slot| slot.borrow().as_ref().and_then(Weak::upgrade))
}

pub(super) struct Registration(Weak<dyn NativeCallbacks>);

impl Registration {
    pub(super) fn new(callbacks: Rc<dyn NativeCallbacks>) -> anyhow::Result<Self> {
        CURRENT_APP.with(|slot| {
            let mut slot = slot.borrow_mut();
            anyhow::ensure!(
                slot.as_ref().and_then(Weak::upgrade).is_none(),
                "an app is already live on this thread"
            );
            let weak = Rc::downgrade(&callbacks);
            *slot = Some(weak.clone());
            Ok(Self(weak))
        })
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        CURRENT_APP.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.as_ref().is_some_and(|current| current.ptr_eq(&self.0)) {
                *slot = None;
            }
        });
    }
}

/// Includes context lookup inside the unwind boundary. Secondary payloads must not run
/// arbitrary destructors while returning through a foreign callback.
pub(super) fn guard_callback<R: Default>(body: impl FnOnce() -> R) -> R {
    let result = contain_callback(body);
    #[cfg(test)]
    contain_callback(super::tests::observe_guard_return);
    result
}

fn contain_callback<R: Default>(body: impl FnOnce() -> R) -> R {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(value) => value,
        Err(payload) => {
            let mut payload = Some(payload);
            let recovery = catch_unwind(AssertUnwindSafe(|| {
                if let Some(callbacks) = current_app() {
                    callbacks.record_panic(payload.take().unwrap());
                }
            }));
            if let Some(payload) = payload {
                std::mem::forget(payload);
            }
            if let Err(payload) = recovery {
                std::mem::forget(payload);
            }
            unsafe { PostQuitMessage(0) };
            R::default()
        }
    }
}

pub(super) fn callbacks_allowed() -> bool {
    current_app().is_none_or(|callbacks| callbacks.dispatch_enabled())
}

#[derive(Clone)]
pub(crate) struct WakeTarget {
    pub(super) window: isize,
    #[cfg(test)]
    pub(super) post: Option<std::sync::Arc<dyn Fn(isize) -> bool + Send + Sync>>,
}

impl WakeTarget {
    pub(super) fn new(window: HWND) -> Self {
        Self {
            window: window.0 as isize,
            #[cfg(test)]
            post: None,
        }
    }

    pub(crate) fn signal_nonblocking(&self) {
        #[cfg(test)]
        if let Some(post) = &self.post {
            let _ = post(self.window);
            return;
        }
        // The integer retains no window. A send accepted before shutdown may post
        // after destruction, including to a recycled handle. No retry is owed.
        unsafe {
            PostMessageW(
                Some(HWND(self.window as *mut _)),
                WM_APP_WAKE,
                WPARAM(0),
                LPARAM(0),
            )
            .ok();
        }
    }
}

pub(super) unsafe extern "system" fn wake_window_proc(
    window: HWND,
    message: u32,
    word: WPARAM,
    long: LPARAM,
) -> LRESULT {
    guard_callback(|| {
        if message == WM_APP_WAKE {
            if let Some(callbacks) = current_app()
                && callbacks.dispatch_enabled()
            {
                callbacks.wake();
            }
            LRESULT(0)
        } else {
            unsafe { DefWindowProcW(window, message, word, long) }
        }
    })
}

unsafe extern "system" fn timer_proc(_: HWND, _: u32, identifier: usize, _: u32) {
    guard_callback(|| {
        if let Some(callbacks) = current_app()
            && callbacks.dispatch_enabled()
        {
            callbacks.timer_fired(identifier, Instant::now());
        }
    });
}

trait DeadlineTimer {
    fn set(&self, previous: usize, delay_ms: u32) -> usize;
    fn kill(&self, identifier: usize);
}

struct NativeTimer;
impl DeadlineTimer for NativeTimer {
    fn set(&self, previous: usize, delay_ms: u32) -> usize {
        unsafe { SetTimer(None, previous, delay_ms, Some(timer_proc)) }
    }
    fn kill(&self, identifier: usize) {
        unsafe {
            KillTimer(None, identifier).ok();
        }
    }
}

struct Deadline {
    at: Instant,
    timer_id: usize,
}

fn native_delay(at: Instant, now: Instant) -> u32 {
    let nanos = at.saturating_duration_since(now).as_nanos();
    nanos
        .div_ceil(1_000_000)
        .clamp(USER_TIMER_MINIMUM as u128, USER_TIMER_MAXIMUM as u128) as u32
}

pub(super) struct LoopState<W> {
    pub(super) delivery: Rc<WakeDelivery<W>>,
    sender: Sender<W>,
    started: Cell<bool>,
    active: Cell<bool>,
    panicked: Cell<bool>,
    termination_requested: Cell<bool>,
    stopping: Cell<bool>,
    panic: RefCell<Option<PanicPayload>>,
    deadlines: RefCell<HashMap<W, Deadline>>,
    timer: Box<dyn DeadlineTimer>,
}

impl<W: Clone + Eq + Hash + Send + 'static> LoopState<W> {
    pub(super) fn new(delivery: Rc<WakeDelivery<W>>, sender: Sender<W>) -> Self {
        Self {
            delivery,
            sender,
            started: Cell::new(false),
            active: Cell::new(true),
            panicked: Cell::new(false),
            termination_requested: Cell::new(false),
            stopping: Cell::new(false),
            panic: RefCell::new(None),
            deadlines: RefCell::new(HashMap::new()),
            timer: Box::new(NativeTimer),
        }
    }

    fn replace_deadline(&self, wake: W, at: Option<Instant>, now: Instant) {
        if !self.dispatch_enabled() {
            return;
        }
        let previous = self
            .deadlines
            .borrow_mut()
            .remove(&wake)
            .map_or(0, |entry| entry.timer_id);
        if let Some(at) = at {
            let identifier = if self.started.get() {
                self.timer.set(previous, native_delay(at, now))
            } else {
                0
            };
            if self.started.get() && identifier == 0 {
                if previous != 0 {
                    self.timer.kill(previous);
                }
                panic!("SetTimer failed");
            }
            self.deadlines.borrow_mut().insert(
                wake,
                Deadline {
                    at,
                    timer_id: identifier,
                },
            );
        } else if previous != 0 {
            self.timer.kill(previous);
        }
    }

    pub(super) fn stop_wakes(&self) {
        self.active.set(false);
        let deadlines = std::mem::take(&mut *self.deadlines.borrow_mut());
        for deadline in deadlines.values() {
            if deadline.timer_id != 0 {
                self.timer.kill(deadline.timer_id);
            }
        }
        self.delivery.close();
        drop(deadlines);
    }

    pub(super) fn terminate(&self) {
        self.termination_requested.set(true);
        unsafe { PostQuitMessage(0) };
        self.stop_wakes();
    }

    pub(super) fn finish(&self) {
        if self.stopping.replace(true) {
            return;
        }
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| self.stop_wakes())) {
            self.record_panic(payload);
        }
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| self.delivery.stopping())) {
            self.record_panic(payload);
        }
    }

    pub(super) fn take_panic(&self) -> Option<PanicPayload> {
        self.panic.borrow_mut().take()
    }
}

impl<W: Clone + Eq + Hash + Send + 'static> NativeCallbacks for LoopState<W> {
    fn should_stop(&self) -> bool {
        self.termination_requested.get() || self.panicked.get()
    }
    fn dispatch_enabled(&self) -> bool {
        self.active.get() && !self.should_stop()
    }
    fn record_panic(&self, payload: PanicPayload) {
        self.panicked.set(true);
        match self.panic.try_borrow_mut() {
            Ok(mut stored) if stored.is_none() => *stored = Some(payload),
            _ => std::mem::forget(payload),
        }
        self.stop_wakes();
    }
    fn timer_fired(&self, identifier: usize, now: Instant) {
        if !self.started.get() || !self.dispatch_enabled() {
            return;
        }
        let wake = self
            .deadlines
            .borrow()
            .iter()
            .find(|(_, entry)| entry.timer_id == identifier)
            .map(|(wake, _)| wake.clone());
        let Some(wake) = wake else {
            return;
        };
        let entry = self.deadlines.borrow_mut().remove(&wake).unwrap();
        if now < entry.at {
            let replacement = self.timer.set(identifier, native_delay(entry.at, now));
            if replacement == 0 {
                self.timer.kill(identifier);
                panic!("SetTimer failed");
            }
            self.deadlines.borrow_mut().insert(
                wake,
                Deadline {
                    at: entry.at,
                    timer_id: replacement,
                },
            );
        } else {
            self.timer.kill(identifier);
            if self.sender.send(wake).is_ok() {
                self.delivery.wake();
            }
        }
    }
}

impl<W: Clone + Eq + Hash + Send + 'static> Callbacks for LoopState<W> {
    fn started(&self, shell: crate::Shell) {
        if !self.dispatch_enabled() {
            return;
        }
        self.delivery.started(shell);
        self.started.set(true);
        if !self.dispatch_enabled() {
            return;
        }
        let pending: Vec<_> = self
            .deadlines
            .borrow()
            .iter()
            .map(|(wake, entry)| (wake.clone(), entry.at))
            .collect();
        for (wake, at) in pending {
            self.replace_deadline(wake, Some(at), Instant::now());
        }
    }
    fn wake(&self) {
        if self.dispatch_enabled() {
            self.delivery.wake();
        }
    }
    fn menu(&self, apply: Box<dyn FnOnce(Vec<crate::MenuEntry>)>) {
        if self.dispatch_enabled() {
            self.delivery.menu(apply);
        }
    }
    fn selected(&self, id: u32) {
        if self.dispatch_enabled() {
            self.delivery.selected(id);
        }
    }
    fn display_changed(&self) {
        if self.dispatch_enabled() {
            self.delivery.display_changed();
        }
    }
    fn work_area_changed(&self) {
        if self.dispatch_enabled() {
            self.delivery.work_area_changed();
        }
    }
    fn stopping(&self) {
        self.finish();
    }
}

#[derive(Clone)]
pub(crate) struct LoopHandle<W> {
    state: Weak<LoopState<W>>,
}
impl<W: Clone + Eq + Hash + Send + 'static> LoopHandle<W> {
    pub(super) fn new(state: &Rc<LoopState<W>>) -> Self {
        Self {
            state: Rc::downgrade(state),
        }
    }
    pub(crate) fn wake_at(&self, wake: W, deadline: Option<Instant>) {
        if let Some(state) = self.state.upgrade() {
            guard_callback(|| state.replace_deadline(wake, deadline, Instant::now()));
        }
    }
    pub(crate) fn stop_wakes(&self) {
        if let Some(state) = self.state.upgrade() {
            state.stop_wakes();
        }
    }
    pub(crate) fn terminate(&self) {
        if let Some(state) = self.state.upgrade() {
            state.terminate();
        }
    }
}

#[cfg(test)]
#[path = "run_loop_tests.rs"]
pub(super) mod tests;
