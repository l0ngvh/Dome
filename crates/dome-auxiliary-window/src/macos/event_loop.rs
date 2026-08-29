use std::cell::RefCell;
use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use objc2_core_foundation::{
    CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext, kCFRunLoopDefaultMode,
};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol};

use crate::AuxiliaryLoopHandler;

struct AuxiliaryDelegateIvars {
    handler: RefCell<Box<dyn AuxiliaryLoopHandler>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AuxiliaryDelegateIvars]
    struct AuxiliaryDelegate;

    unsafe impl NSObjectProtocol for AuxiliaryDelegate {}

    unsafe impl NSApplicationDelegate for AuxiliaryDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            self.ivars().handler.borrow_mut().on_started();
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            self.ivars().handler.borrow_mut().on_stopping();
        }
    }
);

impl AuxiliaryDelegate {
    fn new(mtm: MainThreadMarker, handler: Box<dyn AuxiliaryLoopHandler>) -> Retained<Self> {
        let ivars = AuxiliaryDelegateIvars {
            handler: RefCell::new(handler),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
}

// Keeps the `C-unwind` ABI so a panic unwinds through the CoreFoundation frame instead
// of aborting.
unsafe extern "C-unwind" fn frame_callback(info: *mut c_void) {
    let delegate: &AuxiliaryDelegate = unsafe { &*(info as *const AuxiliaryDelegate) };
    delegate.ivars().handler.borrow_mut().on_wake();
}

fn create_frame_source(delegate: &Retained<AuxiliaryDelegate>) -> CFRetained<CFRunLoopSource> {
    let mut context = CFRunLoopSourceContext {
        version: 0,
        info: Retained::as_ptr(delegate) as *mut c_void,
        retain: None,
        release: None,
        copyDescription: None,
        equal: None,
        hash: None,
        schedule: None,
        cancel: None,
        perform: Some(frame_callback),
    };
    unsafe { CFRunLoopSource::new(None, 0, &mut context).unwrap() }
}

pub struct EventLoop {
    app: Retained<NSApplication>,
    delegate: Retained<AuxiliaryDelegate>,
    source: CFRetained<CFRunLoopSource>,
    run_loop: CFRetained<CFRunLoop>,
    mtm: MainThreadMarker,
}

impl EventLoop {
    pub fn new(handler: Box<dyn AuxiliaryLoopHandler>) -> Self {
        let mtm = MainThreadMarker::new().expect("EventLoop::new must run on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        let delegate = AuxiliaryDelegate::new(mtm, handler);
        let source = create_frame_source(&delegate);
        let run_loop = CFRunLoop::main().unwrap();
        run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

        Self {
            app,
            delegate,
            source,
            run_loop,
            mtm,
        }
    }

    pub fn waker(&self) -> LoopWaker {
        LoopWaker {
            source: self.source.clone(),
            run_loop: self.run_loop.clone(),
        }
    }

    pub fn handle(&self) -> LoopHandle {
        LoopHandle { mtm: self.mtm }
    }

    pub fn run(self) {
        self.app
            .setDelegate(Some(ProtocolObject::from_ref(&*self.delegate)));

        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.app.run())).is_err() {
            self.delegate.ivars().handler.borrow_mut().on_stopping();
        }
    }
}

/// Holds `MainThreadMarker`, so it is neither `Send` nor callable from a foreign thread.
#[derive(Clone, Copy)]
pub struct LoopHandle {
    mtm: MainThreadMarker,
}

impl LoopHandle {
    pub fn terminate(&self) {
        NSApplication::sharedApplication(self.mtm).terminate(None);
    }
}

/// Wakes the loop from any thread. Carries no payload, so the consumer owns its own queue.
#[derive(Clone)]
pub struct LoopWaker {
    source: CFRetained<CFRunLoopSource>,
    run_loop: CFRetained<CFRunLoop>,
}

// The signal and wake_up operations on CFRunLoopSource and CFRunLoop are thread-safe.
unsafe impl Send for LoopWaker {}

impl LoopWaker {
    pub fn wake(&self) {
        self.source.signal();
        self.run_loop.wake_up();
    }
}
