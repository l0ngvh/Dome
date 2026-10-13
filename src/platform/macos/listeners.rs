use super::Wake;
use super::accessibility::AXApp;
use super::dispatcher::DispatcherMarker;
use super::objc2_wrapper::*;
use super::running_application::RunningApp;
use super::window_server::{ApplicationEvent, NativeEvent, ObserverContext, TitleElement};
use block2::RcBlock;
use dome_auxiliary_window::LoopWaker;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceActiveSpaceDidChangeNotification,
    NSWorkspaceDidActivateApplicationNotification, NSWorkspaceDidLaunchApplicationNotification,
    NSWorkspaceDidTerminateApplicationNotification, NSWorkspaceScreensDidSleepNotification,
    NSWorkspaceWillSleepNotification,
};
use objc2_application_services::{AXObserver, AXUIElement};
use objc2_core_foundation::{CFEqual, CFRetained, CFString};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSObjectProtocol, NSOperationQueue, NSString,
};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "listeners_tests.rs"]
pub(super) mod tests;

#[cfg(feature = "native-ui-test")]
#[path = "listeners_native_source.rs"]
pub(super) mod native_source;

pub(super) const SYNC_INTERVAL: Duration = Duration::from_secs(5);
pub(super) const TITLE_INTERVAL: Duration = Duration::from_millis(16);

pub(super) struct RetainedObserverRegistration {
    native: Option<Box<dyn ObserverNative>>,
    registered: Vec<CFRetained<CFString>>,
}

// Notification names are immutable and registration state stays on one worker.
unsafe impl Send for RetainedObserverRegistration {}

trait ObserverNative: Send {
    fn source(&self) -> CFRetained<objc2_core_foundation::CFRunLoopSource>;
    fn context(&self) -> Arc<ObserverContext>;
    fn add(&mut self, marker: &DispatcherMarker, notification: &CFString) -> Result<(), AXError>;
    fn remove(&mut self, marker: &DispatcherMarker, notification: &CFString)
    -> Result<(), AXError>;
}

struct NativeObserver {
    observer: CFRetained<AXObserver>,
    app: Arc<AXApp>,
    context: Arc<ObserverContext>,
}

// AX observer registration is transferred to one serial worker and never messaged elsewhere.
unsafe impl Send for NativeObserver {}

impl ObserverNative for NativeObserver {
    fn source(&self) -> CFRetained<objc2_core_foundation::CFRunLoopSource> {
        unsafe { self.observer.run_loop_source() }
    }

    fn context(&self) -> Arc<ObserverContext> {
        self.context.clone()
    }

    fn add(&mut self, marker: &DispatcherMarker, notification: &CFString) -> Result<(), AXError> {
        add_observer_notification(
            marker,
            &self.observer,
            self.app.element(),
            notification,
            Arc::as_ptr(&self.context) as *mut c_void,
        )
    }

    fn remove(
        &mut self,
        marker: &DispatcherMarker,
        notification: &CFString,
    ) -> Result<(), AXError> {
        remove_observer_notification(marker, &self.observer, self.app.element(), notification)
    }
}

impl RetainedObserverRegistration {
    pub(super) fn new(
        app: Arc<AXApp>,
        context: Arc<ObserverContext>,
        marker: &DispatcherMarker,
    ) -> Result<Self, AXError> {
        let observer = create_observer(marker, app.pid(), Some(observer_callback))?;
        let mut registration = Self {
            native: Some(Box::new(NativeObserver {
                observer,
                app,
                context,
            })),
            registered: Vec::new(),
        };
        registration.repair(marker);
        Ok(registration)
    }

    pub(super) fn source(&self) -> CFRetained<objc2_core_foundation::CFRunLoopSource> {
        self.native.as_ref().unwrap().source()
    }

    pub(super) fn context(&self) -> Arc<ObserverContext> {
        self.native.as_ref().unwrap().context()
    }

    pub(super) fn repair(&mut self, marker: &DispatcherMarker) {
        for notification in [
            kAXWindowCreatedNotification(),
            kAXWindowMiniaturizedNotification(),
            kAXWindowDeminiaturizedNotification(),
            kAXMovedNotification(),
            kAXResizedNotification(),
            kAXUIElementDestroyedNotification(),
            kAXFocusedWindowChangedNotification(),
            kAXApplicationHiddenNotification(),
            kAXApplicationShownNotification(),
            kAXTitleChangedNotification(),
        ] {
            if self
                .registered
                .iter()
                .any(|registered| CFEqual(Some(&**registered), Some(&*notification)))
            {
                continue;
            }
            if self
                .native
                .as_mut()
                .unwrap()
                .add(marker, &notification)
                .is_ok()
            {
                self.registered.push(notification);
            }
        }
    }

    pub(super) fn remove(mut self, marker: &DispatcherMarker) {
        let native = self.native.as_mut().unwrap();
        self.registered
            .retain(|notification| native.remove(marker, notification).is_err());
    }
}

impl Drop for RetainedObserverRegistration {
    fn drop(&mut self) {
        if !self.registered.is_empty() {
            // Failed removal or unwind leaves callback ownership uncertain.
            std::mem::forget(self.native.take());
        }
    }
}

unsafe extern "C-unwind" fn observer_callback(
    _observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    refcon: *mut c_void,
) {
    unsafe { deliver_observer_notification(element, notification, refcon, || {}) };
}

unsafe fn deliver_observer_notification(
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    refcon: *mut c_void,
    during_callback: impl FnOnce(),
) {
    let context = refcon.cast::<ObserverContext>();
    unsafe {
        Arc::increment_strong_count(context);
    }
    let context = unsafe { Arc::from_raw(context) };
    let notification = unsafe { notification.as_ref() };
    let is = |name: CFRetained<CFString>| CFEqual(Some(notification), Some(&*name));
    let event = if is(kAXMovedNotification())
        || is(kAXResizedNotification())
        || is(kAXWindowDeminiaturizedNotification())
    {
        ApplicationEvent::MovedOrResized {
            observed_at: Instant::now(),
        }
    } else if is(kAXApplicationHiddenNotification()) {
        ApplicationEvent::Hidden
    } else if is(kAXApplicationShownNotification()) {
        ApplicationEvent::Shown
    } else if is(kAXFocusedWindowChangedNotification()) {
        ApplicationEvent::FocusChanged
    } else if is(kAXTitleChangedNotification()) {
        ApplicationEvent::TitleElementChanged(TitleElement(unsafe { CFRetained::retain(element) }))
    } else if is(kAXWindowCreatedNotification())
        || is(kAXUIElementDestroyedNotification())
        || is(kAXWindowMiniaturizedNotification())
    {
        ApplicationEvent::WindowsChanged
    } else {
        return;
    };
    send_native(
        &context.events,
        &context.loop_waker,
        NativeEvent::Application {
            lifetime: context.lifetime.clone(),
            event,
        },
    );
    during_callback();
}

pub(super) fn send_native(
    sender: &mpsc::Sender<NativeEvent>,
    waker: &LoopWaker<Wake>,
    event: NativeEvent,
) {
    if sender.send(event).is_ok() {
        waker.wake(Wake::WindowServer);
    }
}

type NotificationTokens = Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>;
pub(super) struct EventListener {
    workspace: NotificationTokens,
    distributed: NotificationTokens,
}

impl EventListener {
    pub(super) fn new(
        sender: mpsc::Sender<NativeEvent>,
        waker: LoopWaker<Wake>,
        is_suspended: Arc<AtomicBool>,
    ) -> Self {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let mut workspace = Vec::new();
        for (name, kind) in unsafe {
            [
                (NSWorkspaceDidLaunchApplicationNotification, 0),
                (NSWorkspaceDidTerminateApplicationNotification, 1),
                (NSWorkspaceDidActivateApplicationNotification, 2),
                (NSWorkspaceActiveSpaceDidChangeNotification, 3),
                (NSWorkspaceWillSleepNotification, 4),
                (NSWorkspaceScreensDidSleepNotification, 4),
            ]
        } {
            let is_suspended = is_suspended.clone();
            let sender = sender.clone();
            let waker = waker.clone();
            workspace.push(unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &RcBlock::new(move |notification: NonNull<NSNotification>| {
                        let event = match kind {
                            3 => NativeEvent::SpaceChanged,
                            4 => NativeEvent::Suspended(true),
                            _ => {
                                let Some(process_id) =
                                    RunningApp::process_id_from_notification(notification.as_ref())
                                else {
                                    return;
                                };
                                match kind {
                                    0 => NativeEvent::ApplicationLaunched { process_id },
                                    1 => NativeEvent::ApplicationTerminated { process_id },
                                    _ => NativeEvent::ApplicationActivated { process_id },
                                }
                            }
                        };
                        if let NativeEvent::Suspended(suspended) = &event {
                            is_suspended.store(*suspended, Ordering::Relaxed);
                        }
                        send_native(&sender, &waker, event);
                    }),
                )
            });
        }
        let center = NSDistributedNotificationCenter::defaultCenter();
        let mut distributed = Vec::new();
        for (name, suspended) in [
            ("com.apple.screenIsLocked", true),
            ("com.apple.screenIsUnlocked", false),
        ] {
            let is_suspended = is_suspended.clone();
            let sender = sender.clone();
            let waker = waker.clone();
            distributed.push(unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(&NSString::from_str(name)),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &RcBlock::new(move |_: NonNull<NSNotification>| {
                        is_suspended.store(suspended, Ordering::Relaxed);
                        send_native(&sender, &waker, NativeEvent::Suspended(suspended));
                    }),
                )
            });
        }
        Self {
            workspace,
            distributed,
        }
    }
}

impl Drop for EventListener {
    fn drop(&mut self) {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        for token in &self.workspace {
            unsafe {
                center.removeObserver(ProtocolObject::as_ref(token));
            }
        }
        let center = NSDistributedNotificationCenter::defaultCenter();
        for token in &self.distributed {
            unsafe {
                center.removeObserver(ProtocolObject::as_ref(token));
            }
        }
    }
}
