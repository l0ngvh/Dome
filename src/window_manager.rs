use std::{cell::RefCell, collections::HashMap, ptr::NonNull, rc::Rc};

use anyhow::{Context, Result};

use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, sel};
use objc2_app_kit::{
    NSApplicationActivationPolicy, NSRunningApplication, NSScreen, NSWorkspace,
    NSWorkspaceApplicationKey, NSWorkspaceDidLaunchApplicationNotification,
    NSWorkspaceDidTerminateApplicationNotification,
};
use objc2_application_services::{
    AXError, AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXObserver, AXUIElement, AXValue,
    AXValueType,
};
use objc2_core_foundation::{
    CFArray, CFHash, CFMachPort, CFRetained, CFRunLoop, CFString, CFType, CGPoint, CGSize,
    kCFAllocatorDefault, kCFBooleanFalse, kCFBooleanTrue, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType,
};
use objc2_foundation::{NSNotification, NSOperationQueue};

use crate::window::MacWindow;
use crate::workspace::{Hub, Screen};

pub struct WindowContext {
    pub hub: Hub,
    pub pid_to_window_ids: Rc<RefCell<HashMap<i32, Vec<usize>>>>,
    pub window_mapping: Rc<RefCell<HashMap<usize, usize>>>,
}

#[derive(Debug)]
pub struct WindowManager;

impl WindowManager {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    #[tracing::instrument]
    pub fn list_windows(&self) -> Result<()> {
        use objc2_application_services::kAXTrustedCheckOptionPrompt;
        use objc2_core_foundation::CFDictionary;

        tracing::debug!("Accessibility: {}", unsafe {
            AXIsProcessTrustedWithOptions(Some(
                CFDictionary::from_slices(
                    &[kAXTrustedCheckOptionPrompt],
                    &[kCFBooleanTrue.unwrap()],
                )
                .as_opaque(),
            ))
        });

        let mut all_windows = Vec::new();
        let pids = list_apps();
        let mut pids_window_ids = HashMap::new();

        for pid in pids.iter() {
            let app = unsafe { AXUIElement::new_application(*pid) };

            let windows = match get_windows(&app) {
                Ok(windows) => windows,
                Err(e) => {
                    tracing::info!("{e:#}");
                    continue;
                }
            };
            let mut window_ids = Vec::new();
            for window in windows {
                // TODO: don't tile window, but still manage it as floating
                if is_standard_window(&window) {
                    all_windows.push(window.clone());
                    window_ids.push(CFHash(Some(&window)));
                }
            }
            pids_window_ids.insert(*pid, window_ids);
        }

        let screen = get_main_screen();
        let mut hub = Hub::new(screen);

        // Track CFHash -> NodeId mapping for window deletion
        let mut window_mapping = HashMap::new();
        for window in all_windows.iter() {
            let window_id = hub.insert_window(MacWindow(window.clone()));
            let cf_hash = CFHash(Some(window));
            window_mapping.insert(cf_hash, window_id);
        }

        let context = Box::new(WindowContext {
            hub,
            pid_to_window_ids: Rc::new(RefCell::new(pids_window_ids)),
            window_mapping: Rc::new(RefCell::new(window_mapping)),
        });

        let context_ptr = Box::into_raw(context);

        listen_to_keyboard(context_ptr);

        let mut observers = Vec::new();
        for pid in pids {
            let observer = match create_observer(pid, context_ptr) {
                Ok(observer) => observer,
                Err(e) => {
                    tracing::info!("Can't create observer for application {pid}: {e:#}");
                    continue;
                }
            };
            observers.push((pid, observer));
        }

        let apps = Rc::new(RefCell::new(observers));

        listen_to_launching_app(context_ptr, apps.clone());
        listen_to_terminating_app(context_ptr, apps);

        CFRunLoop::run();
        let _ = unsafe { Box::from_raw(context_ptr) };

        Ok(())
    }
}

fn list_apps() -> Vec<i32> {
    let mut apps = Vec::new();

    for app in NSWorkspace::sharedWorkspace().runningApplications() {
        if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
            continue;
        }
        tracing::debug!("Working on app: {:?}", app.localizedName());
        let pid = app.processIdentifier();
        // Some applications may not have any PID, for some reasons
        if pid == -1 {
            continue;
        }
        apps.push(pid);
    }
    apps
}

fn get_main_screen() -> Screen {
    let mtm = MainThreadMarker::new().unwrap();
    let main_screen = NSScreen::mainScreen(mtm).unwrap();
    let frame = main_screen.frame();
    Screen {
        x: frame.origin.x as f32,
        y: frame.origin.y as f32,
        width: frame.size.width as f32,
        height: frame.size.height as f32,
    }
}

fn create_observer(pid: i32, context_ptr: *mut WindowContext) -> Result<CFRetained<AXObserver>> {
    let run_loop = CFRunLoop::current().unwrap();

    let mut observer: *mut AXObserver = std::ptr::null_mut();
    let observer_ptr = NonNull::new(&mut observer as *mut *mut AXObserver).unwrap();
    let res = unsafe { AXObserver::create(pid, Some(observer_callback), observer_ptr) };
    if res != AXError::Success {
        return Err(anyhow::anyhow!("Failed to set size. Error code: {res:?}"));
    }
    let observer = unsafe { *observer_ptr.as_ptr() };
    // Safety: value shouldn't be null as copy attribute call success
    let observer = NonNull::new(observer).unwrap();
    let observer = unsafe { CFRetained::from_raw(observer) };

    let source = unsafe { observer.run_loop_source() };

    // Swift docs func CFRunLoopGetCurrent() -> CFRunLoop!
    // So it's not nullable
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    let app = unsafe { AXUIElement::new_application(pid) };
    let res = unsafe {
        observer.add_notification(
            &app,
            &CFString::from_static_str("AXWindowCreated"),
            context_ptr as *mut std::ffi::c_void,
        )
    };

    unsafe {
        observer.add_notification(
            &app,
            &CFString::from_static_str("AXWindowMiniaturized"),
            context_ptr as *mut std::ffi::c_void,
        )
    };

    unsafe {
        observer.add_notification(
            &app,
            &CFString::from_static_str("AXResized"),
            context_ptr as *mut std::ffi::c_void,
        )
    };

    unsafe {
        observer.add_notification(
            &app,
            &CFString::from_static_str("AXUIElementDestroyed"),
            context_ptr as *mut std::ffi::c_void,
        )
    };

    Ok(observer)
}

// AXObserver callback
#[tracing::instrument]
unsafe extern "C-unwind" fn observer_callback(
    _observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    refcon: *mut std::ffi::c_void,
) {
    let notification = unsafe { &*notification.as_ptr() };
    let context = unsafe { &mut *(refcon as *mut WindowContext) };
    let element = unsafe { CFRetained::retain(element) };
    tracing::info!("AX Notification: {notification} {element:?}");
    if notification.to_string() == *"AXWindowCreated" && is_standard_window(&element) {
        let window = MacWindow(element.clone());
        let window_id = context.hub.insert_window(window);
        let cf_hash = CFHash(Some(&element));
        context
            .window_mapping
            .borrow_mut()
            .insert(cf_hash, window_id);
    } else if notification.to_string() == *"AXUIElementDestroyed" {
        let cf_hash = CFHash(Some(&element));
        if let Some(window_id) = context.window_mapping.borrow_mut().remove(&cf_hash) {
            context.hub.delete_window(window_id);
            tracing::info!("Window deleted: {window_id}");
        }
    }
}

fn get_windows(app: &AXUIElement) -> Result<CFRetained<CFArray<AXUIElement>>> {
    // TODO: log more info to know what app is this
    get_attribute(app, &CFString::from_static_str("AXWindows")).context("Getting windows from app")
}

fn get_role(window: &AXUIElement) -> Result<CFRetained<CFString>> {
    get_attribute(window, &CFString::from_static_str("AXRole")).context("Getting role from window")
}

fn get_subrole(window: &AXUIElement) -> Result<CFRetained<CFString>> {
    get_attribute(window, &CFString::from_static_str("AXSubrole"))
        .context("Getting sub role from window")
}

fn is_minimized(window: &AXUIElement) -> bool {
    get_attribute::<objc2_core_foundation::CFBoolean>(
        window,
        &CFString::from_static_str("AXMinimized"),
    )
    .map(|b| b.as_bool())
    .unwrap_or(false)
}

fn get_attribute<T: objc2_core_foundation::Type>(
    element: &AXUIElement,
    attribute: &CFString,
) -> Result<CFRetained<T>> {
    let mut value: *const CFType = std::ptr::null();
    let value_ptr = NonNull::new(&mut value as *mut *const CFType).unwrap();

    let res = unsafe { element.copy_attribute_value(attribute, value_ptr) };
    // TODO: return no value error as None
    if res != AXError::Success {
        return Err(anyhow::anyhow!(
            "Failed to get attribute value. Error code: {res:?}"
        ));
    }
    let value = unsafe { *value_ptr.as_ptr() as *mut T };
    // Safety: value shouldn't be null as copy attribute call success
    let value = NonNull::new(value).unwrap();
    let value = unsafe { CFRetained::from_raw(value) };
    Ok(value)
}

fn is_standard_window(window: &AXUIElement) -> bool {
    let role = match get_role(window) {
        Ok(role) => role,
        Err(e) => {
            tracing::debug!("Can't get role for window {window:?}: {e:#}");
            return false;
        }
    };

    let subrole = match get_subrole(window) {
        Ok(role) => role,
        Err(e) => {
            tracing::debug!("Can't get subrole for window {window:?}: {e:#}");
            return false;
        }
    };

    role == CFString::from_static_str("AXWindow")
        && subrole == CFString::from_static_str("AXStandardWindow")
        && !is_minimized(window)
}

#[tracing::instrument]
unsafe extern "C-unwind" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    refcon: *mut std::ffi::c_void,
) -> *mut CGEvent {
    let event = event.as_ptr();
    let flags = CGEvent::flags(Some(unsafe { &*event }));
    let context = unsafe { &mut *(refcon as *mut WindowContext) };
    // Pick a reasonably large buffer
    let max_len: usize = 256;

    // Create a UTF-16 buffer initialized to 0
    let mut buffer: Vec<u16> = vec![0; max_len];

    // Storage for actual number of UTF-16 code units
    let mut actual_len: std::ffi::c_ulong = 0;

    unsafe {
        CGEvent::keyboard_get_unicode_string(
            Some(&*event),
            max_len as std::ffi::c_ulong,
            &mut actual_len as *mut std::ffi::c_ulong,
            buffer.as_mut_ptr(),
        )
    };
    let slice = &buffer[..actual_len as usize];
    let key = String::from_utf16(slice).unwrap();
    if key == *"0" && flags.contains(CGEventFlags::MaskCommand) {
        context.hub.focus_workspace(0);
        return std::ptr::null_mut();
    } else if key == *"1" && flags.contains(CGEventFlags::MaskCommand) {
        context.hub.focus_workspace(1);
        return std::ptr::null_mut();
    }
    tracing::trace!("Event tap: {event_type:?} {key:?} ",);
    event
}

fn listen_to_launching_app(
    context_ptr: *mut WindowContext,
    apps: Rc<RefCell<Vec<(i32, CFRetained<AXObserver>)>>>,
) {
    let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();
    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidLaunchApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                let Some(pid) = get_pid_from_notification(notification) else {
                    tracing::debug!("Launched application doesn't have a pid");
                    return;
                };

                tracing::trace!("Received notification for launching app with pid: {pid:?}",);
                let observer = match create_observer(pid, context_ptr) {
                    Ok(observer) => observer,
                    Err(e) => {
                        tracing::info!("Can't create observer for application {pid}: {e:#}");
                        return;
                    }
                };

                let context = &mut *context_ptr;
                let app = AXUIElement::new_application(pid);
                let windows = match get_windows(&app) {
                    Ok(windows) => windows,
                    Err(e) => {
                        tracing::info!("{e:#}");
                        return;
                    }
                };
                let mut window_ids = Vec::new();
                for window in windows {
                    if is_standard_window(&window) {
                        let window_id = context.hub.insert_window(MacWindow(window.clone()));
                        let cf_hash = CFHash(Some(&window));
                        context
                            .window_mapping
                            .borrow_mut()
                            .insert(cf_hash, window_id);
                        window_ids.push(cf_hash);
                    }
                }
                context
                    .pid_to_window_ids
                    .borrow_mut()
                    .insert(pid, window_ids);

                apps.borrow_mut().push((pid, observer));
            }),
        );
    };
}

fn listen_to_terminating_app(
    context_ptr: *mut WindowContext,
    apps: Rc<RefCell<Vec<(i32, CFRetained<AXObserver>)>>>,
) {
    let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();
    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidTerminateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                let Some(pid) = get_pid_from_notification(notification) else {
                    tracing::debug!("Launched application doesn't have a pid");
                    return;
                };
                tracing::trace!("Received notification for terminating app with pid: {pid:?}",);
                // How many apps are we talking about, 100, 1000?
                apps.borrow_mut().retain(|(p, _)| *p != pid);
                let context = &mut *context_ptr;
                if let Some(window_ids) = context.pid_to_window_ids.borrow_mut().remove(&pid) {
                    tracing::trace!("Removing window {window_ids:?}");
                    for cf_hash in window_ids {
                        if let Some(window_id) =
                            context.window_mapping.borrow_mut().remove(&cf_hash)
                        {
                            context.hub.delete_window(window_id);
                            tracing::info!("Window deleted: {window_id}");
                        }
                    }
                } else {
                    tracing::debug!("App {pid} doesn't have any windows");
                }
            }),
        );
    };
}

fn get_pid_from_notification(notification: NonNull<NSNotification>) -> Option<i32> {
    let notification = unsafe { &*notification.as_ptr() };
    let dict = notification.userInfo()?;
    let app = dict.valueForKey(unsafe { NSWorkspaceApplicationKey })?;
    let app = unsafe { Retained::cast_unchecked::<NSRunningApplication>(app) };
    let pid = app.processIdentifier();
    Some(pid)
}

fn listen_to_keyboard(context_ptr: *mut WindowContext) -> Result<()> {
    let run_loop = CFRunLoop::current().unwrap();
    let Some(match_port) = (unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            1u64 << CGEventType::KeyDown.0,
            Some(event_tap_callback),
            context_ptr as *mut std::ffi::c_void,
        )
    }) else {
        return Err(anyhow::anyhow!("Failed to create event tap"));
    };

    let Some(run_loop_source) =
        CFMachPort::new_run_loop_source(unsafe { kCFAllocatorDefault }, Some(&match_port), 0)
    else {
        return Err(anyhow::anyhow!(
            "Failed to create match port run loop source"
        ));
    };
    run_loop.add_source(Some(&run_loop_source), unsafe { kCFRunLoopDefaultMode });
    Ok(())
}
