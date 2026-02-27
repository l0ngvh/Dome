use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSApplicationActivationPolicy, NSApplicationDidChangeScreenParametersNotification,
    NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
    NSWorkspaceDidActivateApplicationNotification, NSWorkspaceDidLaunchApplicationNotification,
    NSWorkspaceDidTerminateApplicationNotification, NSWorkspaceScreensDidSleepNotification,
    NSWorkspaceWillSleepNotification,
};
use objc2_application_services::{AXObserver, AXUIElement};
use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFEqual, CFRetained, CFRunLoop, CFRunLoopTimer,
    CFRunLoopTimerContext, CFString, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSObjectProtocol,
    NSOperationQueue, NSString,
};
use std::cell::RefCell;

use super::app::send_hub_event;
use super::dome::HubEvent;
use super::monitor::get_all_screens;
use super::objc2_wrapper::{
    add_observer_notification, create_observer, get_cg_window_id, get_pid,
    kAXApplicationHiddenNotification, kAXApplicationShownNotification,
    kAXFocusedWindowChangedNotification, kAXMovedNotification, kAXResizedNotification,
    kAXTitleChangedNotification, kAXUIElementDestroyedNotification, kAXWindowCreatedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification,
};
use super::throttle::{Debounce, Throttle};

const FRAME_THROTTLE: Duration = Duration::from_millis(16);
const SYNC_INTERVAL: Duration = Duration::from_secs(5);

struct ListenerCtx {
    is_suspended: Rc<Cell<bool>>,
    observers: Observers,
    // Prevent feedback loop when Mac queue a focus event, but by the time the event is processed
    // the focus have been given to another window. This window then tries to take focus and
    // succeeds, but the focus event for the other window is already queued. The other window will
    // then proceed to take focus when the event is processed, which tries to take focus and
    // succeeds after the focus event for the original window is queued but before it's executed
    // and forms the feedback loop. By throttling the focus events, we can be confident that as
    // long as the processing of each focus event is shorter than the throttle duration, the
    // feedback loop can't be formed.
    focus_throttle: FocusThrottle,
    title_throttle: TitleThrottle,
    // Wait for all moving events to settle before checking placement, to prevent a window from
    // being evaluated while being resize. This is especially necessary since we can't track
    // resize/move finishing on a per window level.
    resize_debounce: ResizeDebounce,
    hub_sender: Sender<HubEvent>,
}

pub(super) struct EventListener {
    ctx: Box<ListenerCtx>,
    sync_timer: Option<CFRetained<CFRunLoopTimer>>,
    workspace_observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    distributed_observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    screen_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

type Observers = Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>;
type FocusThrottle = Pin<Box<Throttle<i32>>>;
type TitleThrottle = Pin<Box<Throttle<CGWindowID>>>;
type ResizeDebounce = Pin<Box<Debounce<i32>>>;

impl EventListener {
    pub(super) fn new(hub_sender: Sender<HubEvent>, is_suspended: Rc<Cell<bool>>) -> Self {
        let (focus_throttle, title_throttle, resize_debounce) = setup_throttles(hub_sender.clone());

        let mut ctx = Box::new(ListenerCtx {
            is_suspended,
            observers: Rc::new(RefCell::new(HashMap::new())),
            focus_throttle,
            title_throttle,
            resize_debounce,
            hub_sender,
        });

        let (workspace_observers, distributed_observers) = setup_app_observers(&mut ctx);
        let screen_observer = setup_screen_observer(&ctx);
        let sync_timer = schedule_sync_timer(&ctx);

        Self {
            ctx,
            sync_timer,
            workspace_observers,
            distributed_observers,
            screen_observer,
        }
    }

    pub(super) fn register_app(&self, app: &NSRunningApplication) {
        try_register_app(&self.ctx, app);
    }
}

impl Drop for EventListener {
    fn drop(&mut self) {
        if let Some(ref timer) = self.sync_timer {
            CFRunLoopTimer::invalidate(timer);
        }

        if let Some(run_loop) = CFRunLoop::current() {
            for observer in self.ctx.observers.borrow().values() {
                let source = unsafe { observer.run_loop_source() };
                run_loop.remove_source(Some(&source), unsafe { kCFRunLoopDefaultMode });
            }
        }

        let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();
        for observer in &self.workspace_observers {
            unsafe { notification_center.removeObserver(ProtocolObject::as_ref(observer)) };
        }

        let distributed_center = NSDistributedNotificationCenter::defaultCenter();
        for observer in &self.distributed_observers {
            unsafe { distributed_center.removeObserver(ProtocolObject::as_ref(observer)) };
        }

        let default_center = NSNotificationCenter::defaultCenter();
        unsafe { default_center.removeObserver(ProtocolObject::as_ref(&self.screen_observer)) };
    }
}

type WorkspaceObservers = Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>;
type DistributedObservers = Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>;
type ScreenObserver = Retained<ProtocolObject<dyn NSObjectProtocol>>;

fn setup_app_observers(ctx: &mut ListenerCtx) -> (WorkspaceObservers, DistributedObservers) {
    // To bypass FnMut and lifetime requirement of block2. ctx will outlive these callbacks as
    // these callbacks are removed on EventListener drop
    let ctx_ptr = ctx as *mut ListenerCtx;

    let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();
    let mut workspace_observers = Vec::new();

    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidLaunchApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                handle_app_launched(&*ctx_ptr, notification.as_ref());
            }),
        )
    });

    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidTerminateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                handle_app_terminated(&*ctx_ptr, notification.as_ref());
            }),
        )
    });

    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                handle_app_activated(&mut *ctx_ptr, notification.as_ref());
            }),
        )
    });

    // Suspend on system sleep, screen sleep, or lock, as AX APIs are unusable while under these
    // conditions
    // Resume ONLY on unlock, as screen can wake while locked, AX APIs are still unusable while
    // locked
    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceWillSleepNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("System will sleep, suspending window management");
                (*ctx_ptr).is_suspended.set(true);
            }),
        )
    });

    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceScreensDidSleepNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("Screen did sleep, suspending window management");
                (*ctx_ptr).is_suspended.set(true);
            }),
        )
    });

    let distributed_center = NSDistributedNotificationCenter::defaultCenter();
    let lock_name = NSString::from_str("com.apple.screenIsLocked");
    let unlock_name = NSString::from_str("com.apple.screenIsUnlocked");
    let mut distributed_observers = Vec::new();

    distributed_observers.push(unsafe {
        distributed_center.addObserverForName_object_queue_usingBlock(
            Some(lock_name.as_ref()),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("Screen locked, suspending window management");
                (*ctx_ptr).is_suspended.set(true);
            }),
        )
    });

    distributed_observers.push(unsafe {
        distributed_center.addObserverForName_object_queue_usingBlock(
            Some(unlock_name.as_ref()),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("Screen unlocked, resuming window management");
                (*ctx_ptr).is_suspended.set(false);
                send_hub_event(&(*ctx_ptr).hub_sender, HubEvent::Sync);
            }),
        )
    });

    (workspace_observers, distributed_observers)
}

fn setup_screen_observer(ctx: &ListenerCtx) -> ScreenObserver {
    let ctx_ptr = ctx as *const ListenerCtx as *mut ListenerCtx;
    let default_center = NSNotificationCenter::defaultCenter();
    unsafe {
        default_center.addObserverForName_object_queue_usingBlock(
            Some(NSApplicationDidChangeScreenParametersNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                let mtm = MainThreadMarker::new().unwrap();
                let screens = get_all_screens(mtm);
                send_hub_event(&(*ctx_ptr).hub_sender, HubEvent::ScreensChanged(screens));
            }),
        )
    }
}

fn setup_throttles(hub_sender: Sender<HubEvent>) -> (FocusThrottle, TitleThrottle, ResizeDebounce) {
    let hub_sender2 = hub_sender.clone();
    let hub_sender3 = hub_sender.clone();

    let focus_throttle = Throttle::new(Duration::from_millis(500), move |pid: i32| {
        send_hub_event(&hub_sender, HubEvent::SyncFocus { pid });
    });

    let title_throttle = Throttle::new(FRAME_THROTTLE, move |cg_id: CGWindowID| {
        send_hub_event(&hub_sender2, HubEvent::TitleChanged(cg_id));
    });

    let resize_debounce = Debounce::new(Duration::from_millis(100), move |pid: i32| {
        send_hub_event(&hub_sender3, HubEvent::WindowMovedOrResized { pid });
    });

    (focus_throttle, title_throttle, resize_debounce)
}

fn schedule_sync_timer(ctx: &ListenerCtx) -> Option<CFRetained<CFRunLoopTimer>> {
    let ctx_ptr = ctx as *const ListenerCtx as *mut c_void;
    let mut timer_context = CFRunLoopTimerContext {
        version: 0,
        info: ctx_ptr,
        retain: None,
        release: None,
        copyDescription: None,
    };
    let timer = unsafe {
        CFRunLoopTimer::new(
            None,
            CFAbsoluteTimeGetCurrent() + SYNC_INTERVAL.as_secs_f64(),
            SYNC_INTERVAL.as_secs_f64(),
            0,
            0,
            Some(sync_timer_callback),
            &mut timer_context,
        )
    };
    if let Some(ref timer) = timer {
        CFRunLoop::current()
            .unwrap()
            .add_timer(Some(timer), unsafe { kCFRunLoopDefaultMode });
    }
    timer
}

unsafe extern "C-unwind" fn sync_timer_callback(_timer: *mut CFRunLoopTimer, info: *mut c_void) {
    let ctx: &mut ListenerCtx = unsafe { &mut *(info as *mut ListenerCtx) };
    if ctx.is_suspended.get() {
        return;
    }
    send_hub_event(&ctx.hub_sender, HubEvent::Sync);
}

fn handle_app_launched(ctx: &ListenerCtx, notification: &NSNotification) {
    let Some(app) = get_app_from_notification(notification) else {
        return;
    };
    let app_name = app.localizedName().map(|n| n.to_string());
    tracing::debug!(app = ?app_name, "App launched");
    try_register_app(ctx, &app);
    send_hub_event(
        &ctx.hub_sender,
        HubEvent::VisibleWindowsChanged {
            pid: app.processIdentifier(),
        },
    );
}

fn handle_app_terminated(ctx: &ListenerCtx, notification: &NSNotification) {
    let Some(app) = get_app_from_notification(notification) else {
        return;
    };
    let pid = app.processIdentifier();
    tracing::debug!(%pid, "App terminated");
    if let Some(observer) = ctx.observers.borrow_mut().remove(&pid) {
        let source = unsafe { observer.run_loop_source() };
        if let Some(run_loop) = CFRunLoop::current() {
            run_loop.remove_source(Some(&source), unsafe { kCFRunLoopDefaultMode });
        }
    }
    send_hub_event(&ctx.hub_sender, HubEvent::AppTerminated { pid });
}

fn handle_app_activated(ctx: &mut ListenerCtx, notification: &NSNotification) {
    if ctx.is_suspended.get() {
        return;
    }
    let Some(app) = get_app_from_notification(notification) else {
        return;
    };
    if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
        return;
    }

    // This can happen when Mac queue an event for an activated application, but by the time this
    // callback is run the focus have been given to another app. See focus_throttle for more detail
    if !app.isActive() {
        return;
    }

    let app_name = app.localizedName().map(|n| n.to_string());
    tracing::debug!(app = ?app_name, "App activated");
    let pid = app.processIdentifier();
    ctx.focus_throttle.submit(pid);
}

fn get_app_from_notification(
    notification: &NSNotification,
) -> Option<Retained<NSRunningApplication>> {
    let user_info = notification.userInfo()?;
    let app = unsafe { user_info.objectForKey(NSWorkspaceApplicationKey)? };
    Some(unsafe { Retained::cast_unchecked(app) })
}

fn try_register_app(ctx: &ListenerCtx, app: &NSRunningApplication) {
    let pid = app.processIdentifier();
    if pid == -1 || app.activationPolicy() != NSApplicationActivationPolicy::Regular {
        return;
    }

    let mut observers_ref = ctx.observers.borrow_mut();
    if observers_ref.contains_key(&pid) {
        return;
    }

    let run_loop = CFRunLoop::current().unwrap();
    let observer = match create_observer(pid, Some(observer_callback)) {
        Ok(o) => o,
        Err(err) => {
            tracing::debug!(%pid, "Can't create observer: {err:#}");
            return;
        }
    };
    let source = unsafe { observer.run_loop_source() };
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    let context_ptr = ctx as *const ListenerCtx as *mut std::ffi::c_void;
    let ax_app = unsafe { AXUIElement::new_application(pid) };
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
        if let Err(err) = add_observer_notification(&observer, &ax_app, &notification, context_ptr)
        {
            tracing::debug!(%pid, "Can't add notification: {err:#}");
        }
    }

    let app_name = app
        .localizedName()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    tracing::info!(%pid, %app_name, "Registered app");
    observers_ref.insert(pid, observer);
}

#[tracing::instrument(skip_all)]
unsafe extern "C-unwind" fn observer_callback(
    _observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    refcon: *mut std::ffi::c_void,
) {
    let ctx: &mut ListenerCtx = unsafe { &mut *(refcon as *mut ListenerCtx) };
    if ctx.is_suspended.get() {
        return;
    }

    let notification = unsafe { notification.as_ref() };
    let element = unsafe { CFRetained::retain(element) };

    if CFEqual(Some(notification), Some(&*kAXWindowCreatedNotification()))
        || CFEqual(
            Some(notification),
            Some(&*kAXUIElementDestroyedNotification()),
        )
        || CFEqual(
            Some(notification),
            Some(&*kAXWindowMiniaturizedNotification()),
        )
        || CFEqual(
            Some(notification),
            Some(&*kAXWindowDeminiaturizedNotification()),
        )
        || CFEqual(
            Some(notification),
            Some(&*kAXApplicationHiddenNotification()),
        )
        || CFEqual(
            Some(notification),
            Some(&*kAXApplicationShownNotification()),
        )
    {
        if let Ok(pid) = get_pid(&element) {
            send_hub_event(&ctx.hub_sender, HubEvent::VisibleWindowsChanged { pid });
        }
        return;
    }

    if CFEqual(
        Some(notification),
        Some(&*kAXFocusedWindowChangedNotification()),
    ) {
        if let Ok(pid) = get_pid(&element) {
            ctx.focus_throttle.submit(pid);
        }
        return;
    }

    if CFEqual(Some(notification), Some(&*kAXMovedNotification()))
        || CFEqual(Some(notification), Some(&*kAXResizedNotification()))
    {
        if let Ok(pid) = get_pid(&element) {
            ctx.resize_debounce.submit(pid);
        }
        return;
    }

    if CFEqual(Some(notification), Some(&*kAXTitleChangedNotification()))
        && let Some(cg_id) = get_cg_window_id(&element)
    {
        ctx.title_throttle.submit(cg_id);
        return;
    }

    tracing::trace!(%notification, "unexpected AX event");
}
