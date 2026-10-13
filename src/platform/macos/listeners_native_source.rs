use super::*;
use crate::platform::macos::window_server::ObserverSource;
use objc2_core_foundation::{
    CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext, kCFRunLoopCommonModes,
    kCFRunLoopDefaultMode,
};
use std::sync::Weak;
use std::thread::{self, ThreadId};

#[derive(Debug)]
pub(in crate::platform::macos) enum SourceCall {
    Created(ThreadId),
    Added(ThreadId),
    Attached(ThreadId),
    Detached(ThreadId),
    CallbackEntered(Instant, ThreadId),
    CallbackExited(Instant, ThreadId),
    Removing(ThreadId),
    Dropped(ThreadId),
}

pub(in crate::platform::macos) struct SourceProbe {
    pub source: ObserverSource,
    pub context: Weak<ObserverContext>,
}
impl SourceProbe {
    pub fn attached(&self) -> bool {
        CFRunLoop::main()
            .unwrap()
            .contains_source(Some(&self.source.0), unsafe { kCFRunLoopCommonModes })
    }
    pub fn signal(&self) {
        self.source.0.signal();
        CFRunLoop::main().unwrap().wake_up();
    }
}

struct SourceState {
    context: Weak<ObserverContext>,
    calls: mpsc::Sender<SourceCall>,
    nested_callback: bool,
    waker: LoopWaker<Wake>,
}
unsafe extern "C-unwind" fn retain(info: *const c_void) -> *const c_void {
    unsafe { Arc::increment_strong_count(info.cast::<SourceState>()) };
    info
}
unsafe extern "C-unwind" fn release(info: *const c_void) {
    unsafe { Arc::decrement_strong_count(info.cast::<SourceState>()) };
}
unsafe extern "C-unwind" fn schedule(info: *mut c_void, _: *mut CFRunLoop, _: *const CFString) {
    let state = unsafe { &*info.cast::<SourceState>() };
    state
        .calls
        .send(SourceCall::Attached(thread::current().id()))
        .unwrap();
}
unsafe extern "C-unwind" fn cancel(info: *mut c_void, _: *mut CFRunLoop, _: *const CFString) {
    let state = unsafe { &*info.cast::<SourceState>() };
    state
        .calls
        .send(SourceCall::Detached(thread::current().id()))
        .unwrap();
}
unsafe extern "C-unwind" fn perform(info: *mut c_void) {
    let state = unsafe { &*info.cast::<SourceState>() };
    let context = state
        .context
        .upgrade()
        .expect("source outlived callback context");
    let pointer = Arc::as_ptr(&context).cast_mut().cast();
    let notification = kAXMovedNotification();
    state
        .calls
        .send(SourceCall::CallbackEntered(
            Instant::now(),
            thread::current().id(),
        ))
        .unwrap();
    unsafe {
        deliver_observer_notification(
            NonNull::dangling(),
            NonNull::from(&*notification),
            pointer,
            || {
                drop(context);
                if state.nested_callback {
                    CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, 0.05, false);
                    assert!(
                        state.context.upgrade().is_some(),
                        "active callback lost context"
                    );
                }
            },
        );
    }
    state
        .calls
        .send(SourceCall::CallbackExited(
            Instant::now(),
            thread::current().id(),
        ))
        .unwrap();
    state.waker.wake(Wake::Ui);
}

struct NativeSource {
    source: ObserverSource,
    context: Arc<ObserverContext>,
    calls: mpsc::Sender<SourceCall>,
    removal: Option<mpsc::Receiver<()>>,
    fail_removal: bool,
}
impl ObserverNative for NativeSource {
    fn source(&self) -> CFRetained<CFRunLoopSource> {
        self.source.0.clone()
    }
    fn context(&self) -> Arc<ObserverContext> {
        self.context.clone()
    }
    fn add(&mut self, _: &DispatcherMarker, _: &CFString) -> Result<(), AXError> {
        self.calls
            .send(SourceCall::Added(thread::current().id()))
            .unwrap();
        Ok(())
    }
    fn remove(&mut self, _: &DispatcherMarker, _: &CFString) -> Result<(), AXError> {
        assert!(
            !CFRunLoop::main()
                .unwrap()
                .contains_source(Some(&self.source.0), unsafe { kCFRunLoopCommonModes }),
            "native removal preceded UI detach"
        );
        self.calls
            .send(SourceCall::Removing(thread::current().id()))
            .unwrap();
        if let Some(removal) = self.removal.take() {
            removal.recv().unwrap();
        }
        if self.fail_removal {
            Err(AXError::InvalidUIElement)
        } else {
            Ok(())
        }
    }
}
impl Drop for NativeSource {
    fn drop(&mut self) {
        let _ = self.calls.send(SourceCall::Dropped(thread::current().id()));
    }
}

pub(in crate::platform::macos) struct SourceControl {
    pub removal: Option<mpsc::Receiver<()>>,
    pub setup: Option<mpsc::Receiver<()>>,
    pub nested_callback: bool,
    pub fail_removal: bool,
}

pub(in crate::platform::macos) fn registration(
    context: Arc<ObserverContext>,
    marker: &DispatcherMarker,
    calls: mpsc::Sender<SourceCall>,
    probes: mpsc::Sender<SourceProbe>,
    control: SourceControl,
) -> RetainedObserverRegistration {
    let SourceControl {
        removal,
        setup,
        nested_callback,
        fail_removal,
    } = control;
    calls
        .send(SourceCall::Created(thread::current().id()))
        .unwrap();
    let state = Arc::new(SourceState {
        context: Arc::downgrade(&context),
        calls: calls.clone(),
        nested_callback,
        waker: context.loop_waker.clone(),
    });
    let mut native = CFRunLoopSourceContext {
        version: 0,
        info: Arc::as_ptr(&state).cast_mut().cast(),
        retain: Some(retain),
        release: Some(release),
        copyDescription: None,
        equal: None,
        hash: None,
        schedule: Some(schedule),
        cancel: Some(cancel),
        perform: Some(perform),
    };
    let source = unsafe { CFRunLoopSource::new(None, 0, &mut native).unwrap() };
    probes
        .send(SourceProbe {
            source: ObserverSource(source.clone()),
            context: Arc::downgrade(&context),
        })
        .unwrap();
    let mut registration = RetainedObserverRegistration {
        native: Some(Box::new(NativeSource {
            source: ObserverSource(source),
            context,
            calls,
            removal,
            fail_removal,
        })),
        registered: Vec::new(),
    };
    registration.repair(marker);
    if let Some(setup) = setup {
        setup.recv().unwrap();
    }
    registration
}
