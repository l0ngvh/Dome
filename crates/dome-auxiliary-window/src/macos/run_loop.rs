#[cfg(test)]
#[path = "run_loop_tests.rs"]
mod tests;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::hash::Hash;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Instant;

use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;
use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext,
    CFRunLoopTimer, CFRunLoopTimerContext, kCFRunLoopCommonModes,
};

use crate::delivery::{Callbacks, WakeDelivery};
use crate::{LoopWaker, MenuEntry, Shell};

#[derive(Clone)]
pub(crate) struct WakeTarget {
    pub(crate) source: CFRetained<CFRunLoopSource>,
    pub(crate) run_loop: CFRetained<CFRunLoop>,
    #[cfg(test)]
    pub(crate) recorder: Option<Arc<dyn Fn() + Send + Sync>>,
}

// Core Foundation permits cross-thread signal, wake, retain, and release.
unsafe impl Send for WakeTarget {}
unsafe impl Sync for WakeTarget {}

impl WakeTarget {
    pub(crate) fn signal_nonblocking(&self) {
        #[cfg(test)]
        if let Some(recorder) = &self.recorder {
            recorder();
        }
        self.source.signal();
        self.run_loop.wake_up();
    }
}

pub(crate) trait NativeDispatch: Callbacks {
    fn timer(&self, timer: &CFRunLoopTimer);
    fn close(&self);
}

pub(crate) struct DispatchBridge {
    pub(crate) dispatch: Rc<dyn NativeDispatch>,
}

pub(crate) struct CallbackContext {
    pub(crate) dispatch: AtomicPtr<DispatchBridge>,
    #[cfg(test)]
    pub(crate) released: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl CallbackContext {
    pub(crate) fn new() -> Self {
        Self {
            dispatch: AtomicPtr::new(std::ptr::null_mut()),
            #[cfg(test)]
            released: None,
        }
    }

    pub(crate) fn clear(&self) {
        self.dispatch.store(std::ptr::null_mut(), Ordering::Release);
    }

    pub(crate) fn dispatch(&self) -> Option<Rc<dyn NativeDispatch>> {
        // Only UI callbacks dereference this pointer. Off-thread owners only retain it.
        let _main = MainThreadMarker::new().expect("native delivery must run on main");
        let pointer = self.dispatch.load(Ordering::Acquire);
        unsafe { pointer.as_ref().map(|bridge| bridge.dispatch.clone()) }
    }
}

#[cfg(test)]
impl Drop for CallbackContext {
    fn drop(&mut self) {
        if let Some(released) = &self.released {
            released.store(true, Ordering::Release);
        }
    }
}

unsafe extern "C-unwind" fn retain_context(info: *const c_void) -> *const c_void {
    unsafe { Arc::increment_strong_count(info.cast::<CallbackContext>()) };
    info
}

unsafe extern "C-unwind" fn release_context(info: *const c_void) {
    unsafe { Arc::decrement_strong_count(info.cast::<CallbackContext>()) };
}

unsafe extern "C-unwind" fn wake_callback(info: *mut c_void) {
    let context = unsafe { &*info.cast::<CallbackContext>() };
    if let Some(dispatch) = context.dispatch() {
        dispatch.wake();
    }
}

unsafe extern "C-unwind" fn timer_callback(timer: *mut CFRunLoopTimer, info: *mut c_void) {
    let context = unsafe { &*info.cast::<CallbackContext>() };
    if let Some(dispatch) = context.dispatch() {
        dispatch.timer(unsafe { &*timer });
    }
}

pub(crate) fn create_wake_source(context: &Arc<CallbackContext>) -> CFRetained<CFRunLoopSource> {
    let mut native = CFRunLoopSourceContext {
        version: 0,
        info: Arc::as_ptr(context).cast_mut().cast(),
        retain: Some(retain_context),
        release: Some(release_context),
        copyDescription: None,
        equal: None,
        hash: None,
        schedule: None,
        cancel: None,
        perform: Some(wake_callback),
    };
    unsafe { CFRunLoopSource::new(None, 0, &mut native).expect("create wake source") }
}

struct Deadline {
    at: Instant,
    timer: Option<CFRetained<CFRunLoopTimer>>,
}

struct Deadlines<W> {
    entries: HashMap<W, Deadline>,
    context: Arc<CallbackContext>,
    run_loop: CFRetained<CFRunLoop>,
    ready: bool,
    closed: bool,
}

impl<W: Clone + Eq + Hash + Send + 'static> Deadlines<W> {
    fn new(context: Arc<CallbackContext>, run_loop: CFRetained<CFRunLoop>) -> Self {
        Self {
            entries: HashMap::new(),
            context,
            run_loop,
            ready: false,
            closed: false,
        }
    }

    fn arm(&self, at: Instant) -> CFRetained<CFRunLoopTimer> {
        let mut native = CFRunLoopTimerContext {
            version: 0,
            info: Arc::as_ptr(&self.context).cast_mut().cast(),
            retain: Some(retain_context),
            release: Some(release_context),
            copyDescription: None,
        };
        let fire =
            CFAbsoluteTimeGetCurrent() + at.saturating_duration_since(Instant::now()).as_secs_f64();
        let timer = unsafe {
            CFRunLoopTimer::new(None, fire, 0.0, 0, 0, Some(timer_callback), &mut native)
                .expect("create deadline timer")
        };
        self.run_loop
            .add_timer(Some(&timer), unsafe { kCFRunLoopCommonModes });
        timer
    }

    fn replace(&mut self, wake: W, deadline: Option<Instant>) {
        if self.closed {
            return;
        }
        if let Some(old) = self.entries.remove(&wake)
            && let Some(timer) = old.timer
        {
            timer.invalidate();
        }
        if let Some(at) = deadline {
            let timer = self.ready.then(|| self.arm(at));
            self.entries.insert(wake, Deadline { at, timer });
        }
    }

    fn start(&mut self) {
        if self.closed || self.ready {
            return;
        }
        self.ready = true;
        let entries: Vec<_> = self
            .entries
            .iter()
            .map(|(wake, entry)| (wake.clone(), entry.at))
            .collect();
        for (wake, at) in entries {
            self.replace(wake, Some(at));
        }
    }

    fn fired(&mut self, timer: &CFRunLoopTimer, now: Instant) -> Vec<W> {
        let current = self.entries.iter().find_map(|(wake, entry)| {
            entry
                .timer
                .as_ref()
                .filter(|owned| std::ptr::eq(&***owned, timer))
                .map(|_| (wake.clone(), entry.at))
        });
        let Some((wake, at)) = current else {
            return Vec::new();
        };
        if at > now {
            self.replace(wake, Some(at));
            return Vec::new();
        }
        self.take_due(now)
    }

    fn take_due(&mut self, now: Instant) -> Vec<W> {
        let due: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.at <= now)
            .map(|(wake, _)| wake.clone())
            .collect();
        for wake in &due {
            if let Some(entry) = self.entries.remove(wake)
                && let Some(timer) = entry.timer
            {
                timer.invalidate();
            }
        }
        due
    }

    fn cancel_all(&mut self) {
        self.closed = true;
        for (_, entry) in self.entries.drain() {
            if let Some(timer) = entry.timer {
                timer.invalidate();
            }
        }
    }
}

pub(crate) struct LoopState<W> {
    pub(crate) delivery: WakeDelivery<W>,
    deadlines: RefCell<Deadlines<W>>,
    waker: LoopWaker<W>,
}

impl<W: Clone + Eq + Hash + Send + 'static> LoopState<W> {
    pub(crate) fn new(
        delivery: WakeDelivery<W>,
        context: Arc<CallbackContext>,
        waker: LoopWaker<W>,
    ) -> Self {
        Self {
            deadlines: RefCell::new(Deadlines::new(context, waker.target.run_loop.clone())),
            delivery,
            waker,
        }
    }
}

impl<W: Clone + Eq + Hash + Send + 'static> Callbacks for LoopState<W> {
    fn started(&self, shell: Shell) {
        self.delivery.started(shell);
        self.deadlines.borrow_mut().start();
    }
    fn wake(&self) {
        self.delivery.wake();
    }
    fn menu(&self, apply: Box<dyn FnOnce(Vec<MenuEntry>)>) {
        self.delivery.menu(apply);
    }
    fn selected(&self, id: u32) {
        self.delivery.selected(id);
    }
    fn display_changed(&self) {
        self.delivery.display_changed();
    }
    fn stopping(&self) {
        self.close();
        self.delivery.stopping();
    }
    fn active(&self) -> bool {
        self.delivery.active()
    }
}

impl<W: Clone + Eq + Hash + Send + 'static> NativeDispatch for LoopState<W> {
    fn timer(&self, timer: &CFRunLoopTimer) {
        let due = self.deadlines.borrow_mut().fired(timer, Instant::now());
        for wake in due {
            self.waker.wake(wake);
        }
        self.delivery.wake();
    }
    fn close(&self) {
        self.delivery.close();
        self.deadlines.borrow_mut().cancel_all();
    }
}

#[derive(Clone)]
pub(crate) struct LoopHandle<W> {
    state: Weak<LoopState<W>>,
    main: MainThreadMarker,
}

impl<W: Clone + Eq + Hash + Send + 'static> LoopHandle<W> {
    pub(crate) fn new(state: &Rc<LoopState<W>>, main: MainThreadMarker) -> Self {
        Self {
            state: Rc::downgrade(state),
            main,
        }
    }
    pub(crate) fn wake_at(&self, wake: W, deadline: Option<Instant>) {
        if let Some(state) = self.state.upgrade()
            && !state.delivery.is_closed()
        {
            state.deadlines.borrow_mut().replace(wake, deadline);
        }
    }
    pub(crate) fn stop_wakes(&self) {
        if let Some(state) = self.state.upgrade() {
            state.close();
        }
    }
    pub(crate) fn terminate(&self) {
        if self.state.upgrade().is_some() {
            NSApplication::sharedApplication(self.main).terminate(None);
        }
    }
}
