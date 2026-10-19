use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext};
use std::ffi::c_void;

/// Wakes the loop from any thread. Carries no payload, so the consumer owns its own queue.
#[derive(Clone)]
pub struct LoopWaker {
    source: CFRetained<CFRunLoopSource>,
    run_loop: CFRetained<CFRunLoop>,
}

// The signal and wake_up operations on CFRunLoopSource and CFRunLoop are thread-safe.
unsafe impl Send for LoopWaker {}

impl LoopWaker {
    pub(crate) fn new(
        source: CFRetained<CFRunLoopSource>,
        run_loop: CFRetained<CFRunLoop>,
    ) -> Self {
        Self { source, run_loop }
    }

    pub fn wake(&self) {
        self.source.signal();
        self.run_loop.wake_up();
    }
}

/// Builds a run-loop source that calls `perform` with `info` when signaled. `info` must
/// outlive the source. The `C-unwind` ABI lets a panic unwind through the CoreFoundation
/// frame instead of aborting.
pub(crate) fn create_wake_source(
    info: *mut c_void,
    perform: unsafe extern "C-unwind" fn(*mut c_void),
) -> CFRetained<CFRunLoopSource> {
    let mut context = CFRunLoopSourceContext {
        version: 0,
        info,
        retain: None,
        release: None,
        copyDescription: None,
        equal: None,
        hash: None,
        schedule: None,
        cancel: None,
        perform: Some(perform),
    };
    unsafe { CFRunLoopSource::new(None, 0, &mut context).unwrap() }
}

impl LoopWaker {
    pub fn task_waker(&self) -> std::task::Waker {
        struct NativeWake(std::sync::Mutex<LoopWaker>);

        impl std::task::Wake for NativeWake {
            fn wake(self: std::sync::Arc<Self>) {
                self.wake_by_ref();
            }

            fn wake_by_ref(self: &std::sync::Arc<Self>) {
                self.0.lock().unwrap().wake();
            }
        }

        std::task::Waker::from(std::sync::Arc::new(NativeWake(std::sync::Mutex::new(
            self.clone(),
        ))))
    }
}
