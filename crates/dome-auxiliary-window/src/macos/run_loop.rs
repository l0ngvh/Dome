use std::ffi::c_void;

use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;
use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext};

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

/// Holds `MainThreadMarker`, so it is neither `Send` nor callable from a foreign thread.
#[derive(Clone, Copy)]
pub struct LoopHandle {
    mtm: MainThreadMarker,
}

impl LoopHandle {
    pub(crate) fn new(mtm: MainThreadMarker) -> Self {
        Self { mtm }
    }

    pub fn terminate(&self) {
        NSApplication::sharedApplication(self.mtm).terminate(None);
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
