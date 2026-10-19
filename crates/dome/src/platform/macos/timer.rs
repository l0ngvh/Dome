use std::cell::RefCell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Duration;

use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFRetained, CFRunLoop, CFRunLoopTimer, CFRunLoopTimerContext,
    kCFRunLoopDefaultMode,
};

/// A timer on the current thread's default native run-loop mode.
/// Dropping the timer cancels it. Callbacks run on its creating thread.
#[must_use = "dropping the timer cancels its callback"]
pub struct Timer {
    timer: CFRetained<CFRunLoopTimer>,
    _bound_to_thread: PhantomData<Rc<()>>,
}

impl Timer {
    pub fn once(delay: Duration, callback: impl FnOnce() + 'static) -> Self {
        let callback = RefCell::new(Some(callback));
        Self::new(delay, Duration::ZERO, move || {
            let callback = callback.borrow_mut().take();
            if let Some(callback) = callback {
                callback();
            }
        })
    }

    pub fn repeating(interval: Duration, callback: impl Fn() + 'static) -> Self {
        assert!(
            !interval.is_zero(),
            "repeating timer interval must be positive"
        );
        Self::new(interval, interval, callback)
    }

    fn new(delay: Duration, interval: Duration, callback: impl Fn() + 'static) -> Self {
        let callback: Rc<Callback> = Rc::new(Box::new(callback));
        let mut context = CFRunLoopTimerContext {
            version: 0,
            info: Rc::as_ptr(&callback).cast_mut().cast(),
            retain: Some(retain_callback),
            release: Some(release_callback),
            copyDescription: None,
        };
        let timer = unsafe {
            CFRunLoopTimer::new(
                None,
                CFAbsoluteTimeGetCurrent() + delay.as_secs_f64(),
                interval.as_secs_f64(),
                0,
                0,
                Some(fire),
                &mut context,
            )
        }
        .expect("failed to create native timer");
        CFRunLoop::current()
            .expect("current thread has a run loop")
            .add_timer(Some(&timer), unsafe { kCFRunLoopDefaultMode });
        Self {
            timer,
            _bound_to_thread: PhantomData,
        }
    }
}

type Callback = Box<dyn Fn()>;

impl Drop for Timer {
    fn drop(&mut self) {
        self.timer.invalidate();
    }
}

unsafe extern "C-unwind" fn retain_callback(info: *const c_void) -> *const c_void {
    unsafe { Rc::increment_strong_count(info.cast::<Callback>()) };
    info
}

unsafe extern "C-unwind" fn release_callback(info: *const c_void) {
    unsafe { Rc::decrement_strong_count(info.cast::<Callback>()) };
}

unsafe extern "C-unwind" fn fire(_timer: *mut CFRunLoopTimer, info: *mut c_void) {
    // A callback may drop its own timer, so retain it across the call.
    let callback = unsafe {
        Rc::increment_strong_count(info.cast::<Callback>());
        Rc::from_raw(info.cast::<Callback>())
    };
    callback();
}
