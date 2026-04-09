use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::marker::PhantomData;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::{Duration, Instant};

use calloop::channel::Sender as CalloopSender;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSWorkspace,
    NSWorkspaceActiveSpaceDidChangeNotification, NSWorkspaceDidActivateApplicationNotification,
    NSWorkspaceDidLaunchApplicationNotification, NSWorkspaceDidTerminateApplicationNotification,
    NSWorkspaceScreensDidSleepNotification, NSWorkspaceWillSleepNotification,
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

use super::dome::HubEvent;
use super::get_all_screens;
use super::objc2_wrapper::{
    add_observer_notification, create_observer, get_cg_window_id, get_pid,
    kAXApplicationHiddenNotification, kAXApplicationShownNotification,
    kAXFocusedWindowChangedNotification, kAXMovedNotification, kAXResizedNotification,
    kAXTitleChangedNotification, kAXUIElementDestroyedNotification, kAXWindowCreatedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification,
    remove_observer_notification,
};
use super::running_application::RunningApp;
use super::send_hub_event;
use super::throttle::Throttle;

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
    hub_sender: CalloopSender<HubEvent>,
}

pub(super) struct EventListener {
    ctx: Box<ListenerCtx>,
    sync_timer: Option<CFRetained<CFRunLoopTimer>>,
    workspace_observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    distributed_observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    screen_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

/// Wraps an `AXObserver` added to a run loop. Tracks registered notifications
/// so `Drop` can remove them, then removes the run loop source.
/// `PhantomData<*const ()>` makes the type `!Send`/`!Sync`.
struct RegisteredObserver {
    observer: CFRetained<AXObserver>,
    element: CFRetained<AXUIElement>,
    notifications: Vec<CFRetained<CFString>>,
    run_loop: CFRetained<CFRunLoop>,
    _bound_to_thread: PhantomData<*const ()>,
}

impl RegisteredObserver {
    fn add_notification(
        &mut self,
        notification: CFRetained<CFString>,
        refcon: *mut std::ffi::c_void,
    ) -> Result<(), crate::platform::macos::objc2_wrapper::AXError> {
        add_observer_notification(&self.observer, &self.element, &notification, refcon)?;
        self.notifications.push(notification);
        Ok(())
    }
}

impl Drop for RegisteredObserver {
    fn drop(&mut self) {
        for notification in &self.notifications {
            if let Err(err) =
                remove_observer_notification(&self.observer, &self.element, notification)
            {
                tracing::trace!("Failed to remove notification: {err:#}");
            }
        }
        let source = unsafe { self.observer.run_loop_source() };
        self.run_loop
            .remove_source(Some(&source), unsafe { kCFRunLoopDefaultMode });
    }
}

type Observers = Rc<RefCell<HashMap<i32, RegisteredObserver>>>;
type FocusThrottle = Pin<Box<Throttle<i32>>>;
type TitleThrottle = Pin<Box<Throttle<CGWindowID>>>;

impl EventListener {
    pub(super) fn new(hub_sender: CalloopSender<HubEvent>, is_suspended: Rc<Cell<bool>>) -> Self {
        let (focus_throttle, title_throttle) = setup_throttles(hub_sender.clone());

        let mut ctx = Box::new(ListenerCtx {
            is_suspended,
            observers: Rc::new(RefCell::new(HashMap::new())),
            focus_throttle,
            title_throttle,
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

    /// Tears down all observers and re-registers from scratch. Sends
    /// `ObservedPidsRefreshed` back to the hub thread with the full set of
    /// successfully registered PIDs. Must be called on the main thread.
    pub(super) fn refresh_all_observers(&self) {
        self.ctx.observers.borrow_mut().clear();
        for app in RunningApp::all() {
            try_register_app(&self.ctx, &app);
        }
        let pids: HashSet<i32> = self.ctx.observers.borrow().keys().copied().collect();
        send_hub_event(&self.ctx.hub_sender, HubEvent::ObservedPidsRefreshed(pids));
    }
}

impl Drop for EventListener {
    fn drop(&mut self) {
        if let Some(ref timer) = self.sync_timer {
            CFRunLoopTimer::invalidate(timer);
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

    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceActiveSpaceDidChangeNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                send_hub_event(&(*ctx_ptr).hub_sender, HubEvent::SpaceChanged);
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

fn setup_throttles(hub_sender: CalloopSender<HubEvent>) -> (FocusThrottle, TitleThrottle) {
    let hub_sender2 = hub_sender.clone();

    let focus_throttle = Throttle::new(Duration::from_millis(500), move |pid: i32| {
        send_hub_event(&hub_sender, HubEvent::SyncFocus { pid });
    });

    let title_throttle = Throttle::new(FRAME_THROTTLE, move |cg_id: CGWindowID| {
        send_hub_event(&hub_sender2, HubEvent::TitleChanged(cg_id));
    });

    (focus_throttle, title_throttle)
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
    let Some(app) = RunningApp::from_notification(notification) else {
        return;
    };
    tracing::debug!(%app, "App launched");
    if try_register_app(ctx, &app) {
        tracing::debug!(%app, "Registered app");
    }
    if ctx.observers.borrow().contains_key(&app.pid()) {
        send_hub_event(&ctx.hub_sender, HubEvent::PidObserved { pid: app.pid() });
    }
    send_hub_event(
        &ctx.hub_sender,
        HubEvent::VisibleWindowsChanged { pid: app.pid() },
    );
}

fn handle_app_terminated(ctx: &ListenerCtx, notification: &NSNotification) {
    let Some(app) = RunningApp::from_notification(notification) else {
        return;
    };
    let pid = app.pid();
    tracing::debug!(%app, "App terminated");
    ctx.observers.borrow_mut().remove(&pid);
    send_hub_event(&ctx.hub_sender, HubEvent::AppTerminated { pid });
}

fn handle_app_activated(ctx: &mut ListenerCtx, notification: &NSNotification) {
    if ctx.is_suspended.get() {
        return;
    }
    let Some(app) = RunningApp::from_notification(notification) else {
        return;
    };

    // This can happen when Mac queue an event for an activated application, but by the time this
    // callback is run the focus have been given to another app. See focus_throttle for more detail
    if !app.is_active() {
        return;
    }

    tracing::debug!(%app, "App activated");
    ctx.focus_throttle.submit(app.pid());
}

fn try_register_app(ctx: &ListenerCtx, app: &RunningApp) -> bool {
    let pid = app.pid();

    let mut observers_ref = ctx.observers.borrow_mut();
    if observers_ref.contains_key(&pid) {
        return false;
    }

    let run_loop = CFRunLoop::current().unwrap();
    let observer = match create_observer(pid, Some(observer_callback)) {
        Ok(o) => o,
        Err(err) => {
            tracing::debug!(%pid, "Can't create observer: {err:#}");
            return false;
        }
    };
    let source = unsafe { observer.run_loop_source() };
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    let ax_app = unsafe { AXUIElement::new_application(pid) };
    let mut registered = RegisteredObserver {
        observer,
        element: ax_app,
        notifications: Vec::new(),
        run_loop: run_loop.clone(),
        _bound_to_thread: PhantomData,
    };

    let context_ptr = ctx as *const ListenerCtx as *mut std::ffi::c_void;
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
        if let Err(err) = registered.add_notification(notification, context_ptr) {
            tracing::debug!(%pid, "Can't add notification: {err:#}");
        }
    }

    observers_ref.insert(pid, registered);
    true
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
            send_hub_event(
                &ctx.hub_sender,
                HubEvent::WindowMovedOrResized {
                    pid,
                    observed_at: Instant::now(),
                },
            );
        }
        return;
    }

    if CFEqual(Some(notification), Some(&*kAXTitleChangedNotification())) {
        if let Some(cg_id) = get_cg_window_id(&element) {
            ctx.title_throttle.submit(cg_id);
        }
        return;
    }

    tracing::trace!(%notification, "unexpected AX event");
}
