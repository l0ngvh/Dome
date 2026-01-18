use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use anyhow::Result;
use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace,
    NSWorkspaceApplicationKey, NSWorkspaceDidActivateApplicationNotification,
    NSWorkspaceDidLaunchApplicationNotification, NSWorkspaceDidTerminateApplicationNotification,
    NSWorkspaceScreensDidSleepNotification, NSWorkspaceWillSleepNotification,
};
use objc2_application_services::{AXObserver, AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFArray, CFBoolean, CFDictionary, CFEqual, CFNumber, CFRetained,
    CFRunLoop, CFRunLoopTimer, CFRunLoopTimerContext, CFString, CFType, CGPoint, CGSize,
    kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{CGWindowID, CGWindowListCopyWindowInfo, CGWindowListOption};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSObjectProtocol, NSOperationQueue, NSString,
};

use super::hub::{HubEvent, WindowInfo};
use super::objc2_wrapper::{
    add_observer_notification, create_observer, get_attribute, get_cg_window_id, get_pid,
    is_attribute_settable, kAXApplicationHiddenNotification, kAXApplicationShownNotification,
    kAXFocusedWindowAttribute, kAXFocusedWindowChangedNotification, kAXFullScreenAttribute,
    kAXMainAttribute, kAXMinimizedAttribute, kAXMovedNotification, kAXParentAttribute,
    kAXPositionAttribute, kAXResizedNotification, kAXRoleAttribute, kAXSizeAttribute,
    kAXStandardWindowSubrole, kAXSubroleAttribute, kAXTitleAttribute, kAXTitleChangedNotification,
    kAXUIElementDestroyedNotification, kAXWindowCreatedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification, kAXWindowRole,
    kAXWindowsAttribute,
};
use super::recovery;
use super::throttle::Throttle;
use super::window::{AXRegistry, AXWindow};
use crate::core::Dimension;
use crate::platform::macos::objc2_wrapper::kCGWindowNumber;

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
    resize_throttle: ResizeThrottle,
    title_throttle: TitleThrottle,
    screen: Dimension,
    ax_registry: Rc<RefCell<AXRegistry>>,
    hub_sender: Sender<HubEvent>,
}

pub(super) struct EventListener {
    #[expect(dead_code, reason = "owns lifetime for raw pointer used in callbacks")]
    ctx: Box<ListenerCtx>,
    sync_timer: Option<CFRetained<CFRunLoopTimer>>,
    workspace_observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    distributed_observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

type Observers = Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>;
type FocusThrottle = Pin<Box<Throttle<CFRetained<AXUIElement>>>>;
type ResizeThrottle = Pin<Box<Throttle<CFRetained<AXUIElement>>>>;
type TitleThrottle = Pin<Box<Throttle<CFRetained<AXUIElement>>>>;

impl EventListener {
    pub(super) fn new(
        screen: Dimension,
        ax_registry: Rc<RefCell<AXRegistry>>,
        hub_sender: Sender<HubEvent>,
        is_suspended: Rc<Cell<bool>>,
    ) -> Self {
        let (focus_throttle, resize_throttle, title_throttle) =
            setup_throttles(screen, ax_registry.clone(), hub_sender.clone());

        let mut ctx = Box::new(ListenerCtx {
            is_suspended,
            observers: Rc::new(RefCell::new(HashMap::new())),
            focus_throttle,
            resize_throttle,
            title_throttle,
            screen,
            ax_registry,
            hub_sender,
        });

        let (workspace_observers, distributed_observers) = setup_app_observers(&mut ctx);
        let sync_timer = schedule_sync_timer(&ctx);

        Self {
            ctx,
            sync_timer,
            workspace_observers,
            distributed_observers,
        }
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
    }
}

fn send_event(hub_sender: &Sender<HubEvent>, event: HubEvent) {
    if hub_sender.send(event).is_err() {
        tracing::error!("Hub thread died, shutting down");
        let mtm = MainThreadMarker::new().unwrap();
        NSApplication::sharedApplication(mtm).terminate(None);
    }
}

type WorkspaceObservers = Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>;
type DistributedObservers = Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>;

fn setup_app_observers(ctx: &mut ListenerCtx) -> (WorkspaceObservers, DistributedObservers) {
    sync_all_windows(ctx);
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
                sync_all_windows(&mut *ctx_ptr);
            }),
        )
    });

    (workspace_observers, distributed_observers)
}

fn setup_throttles(
    screen: Dimension,
    ax_registry: Rc<RefCell<AXRegistry>>,
    hub_sender: Sender<HubEvent>,
) -> (FocusThrottle, ResizeThrottle, TitleThrottle) {
    let ax_registry2 = ax_registry.clone();
    let hub_sender2 = hub_sender.clone();
    let hub_sender3 = hub_sender.clone();

    let focus_throttle = Throttle::new(
        Duration::from_millis(500),
        move |element: CFRetained<AXUIElement>| {
            handle_window_focused(screen, &ax_registry, &hub_sender, &element);
        },
    );

    let resize_throttle = Throttle::new(FRAME_THROTTLE, move |element: CFRetained<AXUIElement>| {
        handle_window_resize(screen, &ax_registry2, &hub_sender2, &element);
    });

    let title_throttle = Throttle::new(FRAME_THROTTLE, move |element: CFRetained<AXUIElement>| {
        handle_title_changed(&hub_sender3, &element);
    });

    (focus_throttle, resize_throttle, title_throttle)
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
    sync_all_windows(ctx);
    send_event(&ctx.hub_sender, HubEvent::Sync);
}

fn handle_app_launched(ctx: &ListenerCtx, notification: &NSNotification) {
    let Some(app) = get_app_from_notification(notification) else {
        return;
    };
    let app_name = app.localizedName().map(|n| n.to_string());
    tracing::debug!(app = ?app_name, "App launched");
    try_register_app(ctx, &app);
    sync_app_windows(
        ctx.screen,
        &mut ctx.ax_registry.borrow_mut(),
        &ctx.hub_sender,
        &app,
    );
}

fn handle_app_terminated(ctx: &ListenerCtx, notification: &NSNotification) {
    let Some(app) = get_app_from_notification(notification) else {
        return;
    };
    let pid = app.processIdentifier();
    tracing::debug!(%pid, "App terminated");
    remove_terminated_app(ctx, pid);
}

fn remove_terminated_app(ctx: &ListenerCtx, pid: i32) {
    ctx.observers.borrow_mut().remove(&pid);
    for cg_id in ctx.ax_registry.borrow_mut().remove_by_pid(pid) {
        recovery::untrack(cg_id);
        send_event(&ctx.hub_sender, HubEvent::WindowDestroyed(cg_id));
    }
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
    let ax_app = unsafe { AXUIElement::new_application(pid) };
    if let Ok(focused) = get_attribute::<AXUIElement>(&ax_app, &kAXFocusedWindowAttribute()) {
        ctx.focus_throttle.submit(focused);
    }
    sync_app_windows(
        ctx.screen,
        &mut ctx.ax_registry.borrow_mut(),
        &ctx.hub_sender,
        &app,
    );
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

    let app_name = app
        .localizedName()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    match register_app(pid, ctx) {
        Ok(observer) => {
            tracing::info!(%pid, %app_name, "Registered app");
            observers_ref.insert(pid, observer);
        }
        Err(err) => {
            tracing::warn!(%pid, %app_name, "Can't register app: {err:#}");
        }
    }
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
    tracing::trace!("Received event: {}", (*notification));

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
        handle_window_resize(ctx.screen, &ctx.ax_registry, &ctx.hub_sender, &element);
        return;
    }

    if CFEqual(
        Some(notification),
        Some(&*kAXFocusedWindowChangedNotification()),
    ) {
        ctx.focus_throttle.submit(element);
        return;
    }

    if CFEqual(Some(notification), Some(&*kAXMovedNotification()))
        || CFEqual(Some(notification), Some(&*kAXResizedNotification()))
    {
        ctx.resize_throttle.submit(element);
        return;
    }

    if CFEqual(Some(notification), Some(&*kAXTitleChangedNotification())) {
        ctx.title_throttle.submit(element);
    }
}

fn handle_window_resize(
    screen: Dimension,
    ax_registry: &Rc<RefCell<AXRegistry>>,
    hub_sender: &Sender<HubEvent>,
    element: &AXUIElement,
) {
    let Ok(pid) = get_pid(element) else {
        return;
    };
    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        sync_app_windows(screen, &mut ax_registry.borrow_mut(), hub_sender, &app);
    }
}

fn handle_title_changed(hub_sender: &Sender<HubEvent>, element: &AXUIElement) {
    let Some(cg_id) = get_cg_window_id(element) else {
        return;
    };
    let title = get_attribute::<CFString>(element, &kAXTitleAttribute())
        .map(|s| s.to_string())
        .unwrap_or_default();
    send_event(hub_sender, HubEvent::TitleChanged { cg_id, title });
}

fn handle_window_focused(
    screen: Dimension,
    ax_registry: &Rc<RefCell<AXRegistry>>,
    hub_sender: &Sender<HubEvent>,
    element: &AXUIElement,
) {
    let Ok(pid) = get_pid(element) else {
        return;
    };
    let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
        return;
    };
    sync_app_windows(screen, &mut ax_registry.borrow_mut(), hub_sender, &app);
    if let Some(cg_id) = get_cg_window_id(element)
        && ax_registry.borrow().contains(cg_id)
    {
        send_event(hub_sender, HubEvent::WindowFocused(cg_id));
    }
}

fn sync_app_windows(
    screen: Dimension,
    ax_registry: &mut AXRegistry,
    hub_sender: &Sender<HubEvent>,
    app: &NSRunningApplication,
) {
    let pid = app.processIdentifier();
    let ax_app = unsafe { AXUIElement::new_application(pid) };
    let Ok(windows) = get_windows(&ax_app) else {
        return;
    };

    let cg_window_ids = list_cg_window_ids();

    let tracked_cg_ids = ax_registry.cg_ids_for_pid(pid);
    if app.isHidden() {
        for cg_id in tracked_cg_ids {
            ax_registry.remove(cg_id);
            recovery::untrack(cg_id);
            send_event(hub_sender, HubEvent::WindowDestroyed(cg_id));
        }
        return;
    }
    for cg_id in tracked_cg_ids {
        if cg_window_ids.contains(&cg_id) && ax_registry.is_valid(cg_id) {
            continue;
        }
        ax_registry.remove(cg_id);
        recovery::untrack(cg_id);
        send_event(hub_sender, HubEvent::WindowDestroyed(cg_id));
    }

    let app_name = app
        .localizedName()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let bundle_id = app.bundleIdentifier().map(|b| b.to_string());

    for ax_window in windows {
        let Some(cg_id) = get_cg_window_id(&ax_window) else {
            continue;
        };
        if ax_registry.contains(cg_id) {
            continue;
        }

        let title = get_attribute::<CFString>(&ax_window, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok();

        if !is_manageable(
            &ax_window,
            &ax_app,
            title.as_deref(),
            &app_name,
            bundle_id.as_deref(),
        ) {
            continue;
        }

        let dimension = get_ax_dimension(&ax_window);
        let ax_win = AXWindow::new(
            ax_window.clone(),
            ax_app.clone(),
            pid,
            screen,
            app_name.clone(),
            title.clone(),
        );
        recovery::track(cg_id, ax_win.clone(), dimension, screen);
        ax_registry.insert(cg_id, ax_win);

        let info = WindowInfo {
            cg_id,
            title,
            app_name: app_name.clone(),
            bundle_id: bundle_id.clone(),
            should_tile: should_tile(&ax_window),
            dimension,
        };
        send_event(hub_sender, HubEvent::WindowCreated(info));
    }
}

// AX notifications are unreliable, when new windows are being rapidly created and deleted,
// macOS may decide skip sending notifications.
// So we poll periodically to keep the state in sync.
// https://github.com/nikitabobko/AeroSpace/issues/445
fn sync_all_windows(ctx: &mut ListenerCtx) {
    if ctx.is_suspended.get() {
        return;
    }
    tracing::trace!("Periodic sync every {}s", SYNC_INTERVAL.as_secs());

    let running_apps: Vec<_> = running_apps().collect();
    let running: HashSet<i32> = running_apps
        .iter()
        .map(|app| app.processIdentifier())
        .collect();

    let terminated_pids: Vec<_> = ctx
        .observers
        .borrow()
        .keys()
        .filter(|pid| !running.contains(pid))
        .copied()
        .collect();
    for pid in terminated_pids {
        remove_terminated_app(ctx, pid);
    }

    let mut ax_registry = ctx.ax_registry.borrow_mut();
    for running_app in &running_apps {
        try_register_app(ctx, running_app);
        sync_app_windows(ctx.screen, &mut ax_registry, &ctx.hub_sender, running_app);
    }
}

fn get_windows(app: &AXUIElement) -> anyhow::Result<CFRetained<CFArray<AXUIElement>>> {
    Ok(get_attribute(app, &kAXWindowsAttribute())?)
}

fn register_app(pid: i32, ctx: &ListenerCtx) -> Result<CFRetained<AXObserver>> {
    let run_loop = CFRunLoop::current().unwrap();
    let observer = create_observer(pid, Some(observer_callback))?;
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
        add_observer_notification(&observer, &ax_app, &notification, context_ptr)?;
    }

    Ok(observer)
}

fn running_apps() -> impl Iterator<Item = Retained<NSRunningApplication>> {
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .into_iter()
        .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
        .filter(|app| app.processIdentifier() != -1)
}

fn should_tile(window: &AXUIElement) -> bool {
    let is_fullscreen = get_attribute::<CFBoolean>(window, &kAXFullScreenAttribute())
        .map(|b| b.as_bool())
        .unwrap_or(false);
    !is_fullscreen
}

fn get_ax_dimension(window: &AXUIElement) -> Dimension {
    let (x, y) = get_attribute::<AXValue>(window, &kAXPositionAttribute())
        .map(|v| {
            let mut pos = CGPoint::new(0.0, 0.0);
            let ptr = std::ptr::NonNull::new(&mut pos as *mut _ as *mut _).unwrap();
            unsafe { v.value(AXValueType::CGPoint, ptr) };
            (pos.x as f32, pos.y as f32)
        })
        .unwrap_or((0.0, 0.0));
    let (width, height) = get_attribute::<AXValue>(window, &kAXSizeAttribute())
        .map(|v| {
            let mut size = CGSize::new(0.0, 0.0);
            let ptr = std::ptr::NonNull::new(&mut size as *mut _ as *mut _).unwrap();
            unsafe { v.value(AXValueType::CGSize, ptr) };
            (size.width as f32, size.height as f32)
        })
        .unwrap_or((0.0, 0.0));
    Dimension {
        x,
        y,
        width,
        height,
    }
}

fn is_manageable(
    window: &AXUIElement,
    app: &AXUIElement,
    title: Option<&str>,
    app_name: &str,
    bundle_id: Option<&str>,
) -> bool {
    let Some(title) = title else {
        tracing::trace!(app_name, bundle_id, "not manageable: window has no title");
        return false;
    };

    let role = get_attribute::<CFString>(window, &kAXRoleAttribute()).ok();
    let is_window = role
        .as_ref()
        .map(|r| CFEqual(Some(&**r), Some(&*kAXWindowRole())))
        .unwrap_or(false);
    if !is_window {
        tracing::trace!(
            app_name,
            bundle_id,
            title,
            "not manageable: role is not AXWindow"
        );
        return false;
    }

    let subrole = get_attribute::<CFString>(window, &kAXSubroleAttribute()).ok();
    let is_standard = subrole
        .as_ref()
        .map(|sr| CFEqual(Some(&**sr), Some(&*kAXStandardWindowSubrole())))
        .unwrap_or(false);
    if !is_standard {
        tracing::trace!(
            app_name,
            bundle_id,
            title,
            "not manageable: subrole is not AXStandardWindow"
        );
        return false;
    }

    let is_root = match get_attribute::<AXUIElement>(window, &kAXParentAttribute()) {
        Err(_) => true,
        Ok(parent) => CFEqual(Some(&*parent), Some(app)),
    };
    if !is_root {
        tracing::trace!(
            app_name,
            bundle_id,
            title,
            "not manageable: window is not root"
        );
        return false;
    }

    if !is_attribute_settable(window, &kAXPositionAttribute()) {
        tracing::trace!(
            app_name,
            bundle_id,
            title,
            "not manageable: position is not settable"
        );
        return false;
    }

    if !is_attribute_settable(window, &kAXSizeAttribute()) {
        tracing::trace!(
            app_name,
            bundle_id,
            title,
            "not manageable: size is not settable"
        );
        return false;
    }

    if !is_attribute_settable(window, &kAXMainAttribute()) {
        tracing::trace!(
            app_name,
            bundle_id,
            title,
            "not manageable: main attribute is not settable"
        );
        return false;
    }

    let is_minimized = get_attribute::<CFBoolean>(window, &kAXMinimizedAttribute())
        .map(|b| b.as_bool())
        .unwrap_or(false);
    if is_minimized {
        tracing::trace!(
            app_name,
            bundle_id,
            title,
            "not manageable: window is minimized"
        );
        return false;
    }

    true
}

fn list_cg_window_ids() -> HashSet<CGWindowID> {
    let Some(window_list) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, 0) else {
        tracing::warn!("CGWindowListCopyWindowInfo returned None");
        return HashSet::new();
    };
    let window_list: &CFArray<CFDictionary<CFString, CFType>> =
        unsafe { window_list.cast_unchecked() };

    let mut ids = HashSet::new();
    let key = kCGWindowNumber();
    for dict in window_list {
        // window id is a required attribute
        // https://developer.apple.com/documentation/coregraphics/kcgwindownumber?language=objc
        let id = dict
            .get(&key)
            .unwrap()
            .downcast::<CFNumber>()
            .unwrap()
            .as_i64()
            .unwrap();
        ids.insert(id as CGWindowID);
    }
    ids
}
