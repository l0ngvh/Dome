use std::collections::HashMap;
use std::ffi::c_void;
use std::hash::Hash;
use std::marker::PhantomData;
use std::pin::Pin;
use std::time::{Duration, Instant};

use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFRetained, CFRunLoop, CFRunLoopTimer, CFRunLoopTimerContext,
    kCFRunLoopDefaultMode,
};

/// The callback must not re-enter `submit` (e.g. by pumping the run loop),
/// as that would create aliasing `&mut` references through the raw self-pointer.
pub(super) struct Throttle<T: 'static> {
    interval: Duration,
    last_sent: Option<Instant>,
    pending: Option<T>,
    timer: Option<CFRetained<CFRunLoopTimer>>,
    run_loop: CFRetained<CFRunLoop>,
    callback: Box<dyn Fn(T)>,
    /// Prevent Send/Sync - bound to creating thread's run loop
    _bound_to_thread: PhantomData<*const ()>,
}

impl<T: 'static> Throttle<T> {
    pub(super) fn new(interval: Duration, callback: impl Fn(T) + 'static) -> Pin<Box<Self>> {
        Box::pin(Self {
            interval,
            last_sent: None,
            pending: None,
            timer: None,
            run_loop: CFRunLoop::current().expect("No run loop on current thread"),
            callback: Box::new(callback),
            _bound_to_thread: PhantomData,
        })
    }

    pub(super) fn submit(self: &mut Pin<Box<Self>>, value: T) {
        let now = Instant::now();
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        let can_send = this
            .last_sent
            .map(|last| now.duration_since(last) >= this.interval)
            .unwrap_or(true);

        if can_send {
            if let Some(timer) = this.timer.take() {
                CFRunLoopTimer::invalidate(&timer);
            }
            this.last_sent = Some(now);
            this.pending = None;
            (this.callback)(value);
        } else {
            this.pending = Some(value);
            if this.timer.is_none() {
                let delay = this
                    .interval
                    .saturating_sub(this.last_sent.unwrap().elapsed());
                self.as_mut().schedule_timer(delay);
            }
        }
    }

    fn schedule_timer(self: Pin<&mut Self>, delay: Duration) {
        let this = unsafe { self.get_unchecked_mut() };
        let ptr = this as *mut Self as *mut c_void;
        let mut context = CFRunLoopTimerContext {
            version: 0,
            info: ptr,
            retain: None,
            release: None,
            copyDescription: None,
        };
        let timer = unsafe {
            CFRunLoopTimer::new(
                None,
                CFAbsoluteTimeGetCurrent() + delay.as_secs_f64(),
                0.0,
                0,
                0,
                Some(throttle_timer_callback::<T>),
                &mut context,
            )
        }
        .expect("Failed to create timer");

        this.run_loop
            .add_timer(Some(&timer), unsafe { kCFRunLoopDefaultMode });
        this.timer = Some(timer);
    }

    fn on_timer(&mut self) {
        self.timer = None;
        if let Some(value) = self.pending.take() {
            self.last_sent = Some(Instant::now());
            (self.callback)(value);
        }
    }
}

impl<T> Drop for Throttle<T> {
    fn drop(&mut self) {
        if let Some(timer) = self.timer.take() {
            CFRunLoopTimer::invalidate(&timer);
        }
    }
}

unsafe extern "C-unwind" fn throttle_timer_callback<T: 'static>(
    _timer: *mut CFRunLoopTimer,
    info: *mut c_void,
) {
    let throttle = unsafe { &mut *(info as *mut Throttle<T>) };
    throttle.on_timer();
}

type TimerContext<T> = *mut (*mut Debounce<T>, T);

/// The callback must not re-enter `submit` (e.g. by pumping the run loop),
/// as that would create aliasing `&mut` references through the raw self-pointer.
pub(super) struct Debounce<T: 'static + Eq + Hash + Copy> {
    interval: Duration,
    pending: HashMap<T, (CFRetained<CFRunLoopTimer>, TimerContext<T>)>,
    run_loop: CFRetained<CFRunLoop>,
    callback: Box<dyn Fn(T)>,
    _bound_to_thread: PhantomData<*const ()>,
}

impl<T: 'static + Eq + Hash + Copy> Debounce<T> {
    pub(super) fn new(interval: Duration, callback: impl Fn(T) + 'static) -> Pin<Box<Self>> {
        Box::pin(Self {
            interval,
            pending: HashMap::new(),
            run_loop: CFRunLoop::current().expect("No run loop on current thread"),
            callback: Box::new(callback),
            _bound_to_thread: PhantomData,
        })
    }

    pub(super) fn submit(self: &mut Pin<Box<Self>>, value: T) {
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        if let Some((timer, ctx_ptr)) = this.pending.remove(&value) {
            CFRunLoopTimer::invalidate(&timer);
            unsafe { drop(Box::from_raw(ctx_ptr)) };
        }
        let ptr = Box::into_raw(Box::new((this as *mut Self, value)));
        let mut context = CFRunLoopTimerContext {
            version: 0,
            info: ptr as *mut c_void,
            retain: None,
            release: None,
            copyDescription: None,
        };
        let timer = unsafe {
            CFRunLoopTimer::new(
                None,
                CFAbsoluteTimeGetCurrent() + this.interval.as_secs_f64(),
                0.0,
                0,
                0,
                Some(debounce_timer_callback::<T>),
                &mut context,
            )
        }
        .expect("Failed to create timer");

        this.run_loop
            .add_timer(Some(&timer), unsafe { kCFRunLoopDefaultMode });
        this.pending.insert(value, (timer, ptr));
    }

    fn on_timer(&mut self, value: T) {
        self.pending.remove(&value);
        (self.callback)(value);
    }
}

impl<T: Eq + Hash + Copy> Drop for Debounce<T> {
    fn drop(&mut self) {
        for (timer, ctx_ptr) in self.pending.values() {
            CFRunLoopTimer::invalidate(timer);
            unsafe { drop(Box::from_raw(*ctx_ptr)) };
        }
    }
}

unsafe extern "C-unwind" fn debounce_timer_callback<T: 'static + Eq + Hash + Copy>(
    _timer: *mut CFRunLoopTimer,
    info: *mut c_void,
) {
    let (debounce_ptr, value) = *unsafe { Box::from_raw(info as *mut (*mut Debounce<T>, T)) };
    let debounce = unsafe { &mut *debounce_ptr };
    debounce.on_timer(value);
}
