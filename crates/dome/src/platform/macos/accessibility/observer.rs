use std::cell::RefCell;
use std::collections::VecDeque;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use objc2_application_services::{AXObserver, AXUIElement};
use objc2_core_foundation::{CFEqual, CFRetained, CFRunLoop, CFString, kCFRunLoopCommonModes};

use super::AXApp;
use crate::platform::macos::event_loop::EventSender;
use crate::platform::macos::objc2_wrapper::{
    add_observer_notification, create_observer, get_cg_window_id, get_pid,
    kAXApplicationHiddenNotification, kAXApplicationShownNotification,
    kAXFocusedWindowChangedNotification, kAXMovedNotification, kAXResizedNotification,
    kAXTitleChangedNotification, kAXUIElementDestroyedNotification, kAXWindowCreatedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification,
    remove_observer_notification,
};
use crate::platform::macos::waker::LoopWaker;
use crate::platform::macos::window_server::{NativeEvent, send_native_event};

pub(super) struct RegisteredObserver {
    observer: CFRetained<AXObserver>,
    application: Arc<AXApp>,
    notifications: Vec<CFRetained<CFString>>,
    run_loop: CFRetained<CFRunLoop>,
}

pub(super) struct CallbackContext {
    events: RefCell<VecDeque<ObserverEvent>>,
    waker: Option<LoopWaker>,
    sender: EventSender<NativeEvent>,
    suspended: Arc<AtomicBool>,
}

pub(super) struct ObserverEvent {
    element: CFRetained<AXUIElement>,
    notification: CFRetained<CFString>,
    arrived_at: Instant,
}

impl RegisteredObserver {
    pub(super) fn new(application: Arc<AXApp>, context: &CallbackContext) -> Option<Self> {
        let pid = application.pid();
        let observer = match create_observer(pid, Some(observer_callback)) {
            Ok(observer) => observer,
            Err(error) => {
                tracing::warn!(pid, %error, "Failed to create Accessibility observer");
                return None;
            }
        };
        let run_loop = CFRunLoop::current().expect("worker run loop");
        let source = unsafe { observer.run_loop_source() };
        run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
        let mut registered = Self {
            observer,
            application,
            notifications: Vec::new(),
            run_loop,
        };
        let context_pointer = (context as *const CallbackContext).cast_mut().cast();
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
            match add_observer_notification(
                &registered.observer,
                registered.application.element(),
                &notification,
                context_pointer,
            ) {
                Ok(()) => registered.notifications.push(notification),
                Err(error) => {
                    tracing::warn!(pid, %error, "Failed to register Accessibility notification")
                }
            }
        }
        send_native_event(&context.sender, NativeEvent::PidObserved { pid });
        Some(registered)
    }
}

impl Drop for RegisteredObserver {
    fn drop(&mut self) {
        for notification in &self.notifications {
            if let Err(error) = remove_observer_notification(
                &self.observer,
                self.application.element(),
                notification,
            ) {
                tracing::trace!(%error, "Failed to remove Accessibility notification");
            }
        }
        let source = unsafe { self.observer.run_loop_source() };
        self.run_loop
            .remove_source(Some(&source), unsafe { kCFRunLoopCommonModes });
    }
}

impl CallbackContext {
    pub(super) fn new(sender: EventSender<NativeEvent>, suspended: Arc<AtomicBool>) -> Self {
        Self {
            events: RefCell::new(VecDeque::new()),
            waker: None,
            sender,
            suspended,
        }
    }
    pub(super) fn set_waker(&mut self, waker: LoopWaker) {
        self.waker = Some(waker);
    }
    pub(super) fn wake(&self) {
        self.waker.as_ref().expect("installed worker waker").wake();
    }
    pub(super) fn pop(&self) -> Option<ObserverEvent> {
        self.events.borrow_mut().pop_front()
    }
    pub(super) fn has_events(&self) -> bool {
        !self.events.borrow().is_empty()
    }
    pub(super) fn decode(&self, event: ObserverEvent) {
        let matches =
            |name: CFRetained<CFString>| CFEqual(Some(&*event.notification), Some(&*name));
        let element = &event.element;
        let native = if matches(kAXWindowCreatedNotification()) {
            get_cg_window_id(element)
                .ok()
                .zip(get_pid(element).ok())
                .map(|(id, pid)| NativeEvent::WindowCreated { id, pid })
        } else if matches(kAXUIElementDestroyedNotification()) {
            get_cg_window_id(element)
                .ok()
                .map(|id| NativeEvent::WindowDestroyed { id })
        } else if matches(kAXApplicationHiddenNotification()) {
            get_pid(element)
                .ok()
                .map(|pid| NativeEvent::AppHidden { pid })
        } else if matches(kAXApplicationShownNotification()) {
            get_pid(element)
                .ok()
                .map(|pid| NativeEvent::AppShown { pid })
        } else if self.suspended.load(Ordering::Relaxed) {
            None
        } else if matches(kAXWindowMiniaturizedNotification()) {
            get_pid(element)
                .ok()
                .map(|pid| NativeEvent::AppWindowsChanged { pid })
        } else if matches(kAXFocusedWindowChangedNotification()) {
            get_pid(element)
                .ok()
                .map(|pid| NativeEvent::FocusObserved { pid })
        } else if matches(kAXMovedNotification())
            || matches(kAXResizedNotification())
            || matches(kAXWindowDeminiaturizedNotification())
        {
            get_pid(element)
                .ok()
                .map(|pid| NativeEvent::WindowMovedOrResized {
                    pid,
                    observed_at: event.arrived_at,
                })
        } else if matches(kAXTitleChangedNotification()) {
            get_cg_window_id(element)
                .ok()
                .map(NativeEvent::TitleObserved)
        } else {
            None
        };
        if let Some(native) = native {
            send_native_event(&self.sender, native);
        }
    }
}

unsafe extern "C-unwind" fn observer_callback(
    _observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    context: *mut std::ffi::c_void,
) {
    let context = unsafe { &*context.cast::<CallbackContext>() };
    context.events.borrow_mut().push_back(ObserverEvent {
        element: unsafe { CFRetained::retain(element) },
        notification: unsafe { CFRetained::retain(notification) },
        arrived_at: Instant::now(),
    });
    context.wake();
}
