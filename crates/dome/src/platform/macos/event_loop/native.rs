use std::cell::RefCell;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};

use futures::channel::mpsc;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSApplicationDidChangeScreenParametersNotification, NSApplicationTerminateReply, NSEvent,
    NSEventModifierFlags, NSEventType, NSImage,
};
use objc2_core_foundation::{CFRunLoop, kCFRunLoopDefaultMode};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSPoint};

use super::super::waker::{LoopWaker, create_wake_source};
use super::{CURRENT, EventSender, NativeEvent, RunLoop, Shutdown};
use crate::action::Action;
use crate::platform::macos::menu_bar::NativeMenuBar;

impl RunLoop {
    pub(crate) fn new(
        icon: Retained<NSImage>,
        on_action: Rc<dyn Fn(Action)>,
    ) -> anyhow::Result<(Rc<Self>, mpsc::UnboundedReceiver<NativeEvent>)> {
        CURRENT.with(|current| {
            anyhow::ensure!(current.borrow().is_none(), "native loop already exists");
            Ok(())
        })?;
        let marker = MainThreadMarker::new()
            .ok_or_else(|| anyhow::anyhow!("RunLoop::new must run on the main thread"))?;
        let app = NSApplication::sharedApplication(marker);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        let stopped = Arc::new(AtomicBool::new(false));
        let waker = Arc::new(OnceLock::new());
        let shutdown = Shutdown {
            stopped: stopped.clone(),
            waker: waker.clone(),
        };
        let signal_shutdown = shutdown.clone();
        let signals = super::super::signals::StopSignals::new(move || signal_shutdown.terminate())?;
        let (sender, events) = mpsc::unbounded();
        let delegate = AppDelegate::new(
            marker,
            EventSender {
                sender,
                waker: waker.clone(),
            },
            shutdown,
        );
        let source = create_wake_source(Retained::as_ptr(&delegate) as *mut c_void, wake_callback);
        let native_run_loop = CFRunLoop::main().unwrap();
        let menu_bar = catch_unwind(AssertUnwindSafe(|| {
            native_run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });
            assert!(
                waker
                    .set(Mutex::new(LoopWaker::new(
                        source.clone(),
                        native_run_loop.clone()
                    )))
                    .is_ok()
            );
            app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            app.finishLaunching();
            Rc::new(NativeMenuBar::new(&icon, on_action))
        }));
        let menu_bar = match menu_bar {
            Ok(menu_bar) => menu_bar,
            Err(panic) => {
                source.invalidate();
                native_run_loop.remove_source(Some(&source), unsafe { kCFRunLoopDefaultMode });
                app.setDelegate(None);
                unsafe { NSNotificationCenter::defaultCenter().removeObserver(&delegate) };
                resume_unwind(panic);
            }
        };
        let owner = Rc::new(Self {
            dome: RefCell::new(None),
            submitted: RefCell::default(),
            tasks: RefCell::default(),
            stopped,
            waker,
            menu_bar,
            app,
            delegate,
            source,
            native_run_loop,
            _signals: signals,
        });
        CURRENT.with(|current| *current.borrow_mut() = Some(Rc::downgrade(&owner)));
        unsafe {
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &owner.delegate,
                sel!(screenParametersChanged:),
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
            );
        }
        Ok((owner, events))
    }
}

pub(super) struct AppDelegateIvars {
    events: EventSender<NativeEvent>,
    shutdown: Shutdown,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DomeAppDelegate"]
    #[ivars = AppDelegateIvars]
    pub(super) struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}
    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationShouldTerminate:))]
        fn should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            self.ivars().shutdown.terminate();
            NSApplicationTerminateReply::TerminateCancel
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            self.ivars().shutdown.terminate();
        }
    }

    impl AppDelegate {
        #[unsafe(method(screenParametersChanged:))]
        fn screen_parameters_changed(&self, _notification: &NSNotification) {
            let _ = self.ivars().events.send(NativeEvent::DisplayChanged);
        }
    }
);

impl AppDelegate {
    fn new(
        marker: MainThreadMarker,
        events: EventSender<NativeEvent>,
        shutdown: Shutdown,
    ) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(AppDelegateIvars { events, shutdown });
        unsafe { msg_send![super(this), init] }
    }

    fn poll(&self) {
        let Some(owner) = RunLoop::try_current() else {
            return;
        };
        owner.poll_tasks();
        if owner.is_stopped() {
            owner.app.stop(None);
            // AppKit may already be waiting for its next event when a run-loop source stops it.
            if let Some(event) = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                NSEventType::ApplicationDefined, NSPoint::new(0.0, 0.0), NSEventModifierFlags::empty(), 0.0, 0, None, 0, 0, 0,
            ) { owner.app.postEvent_atStart(&event, true); }
        }
    }
}

unsafe extern "C-unwind" fn wake_callback(info: *mut c_void) {
    let delegate = unsafe { &*info.cast::<AppDelegate>() };
    delegate.poll();
}
