use std::collections::HashSet;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use anyhow::Result;
use block2::RcBlock;
use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
    NSWorkspaceDidActivateApplicationNotification, NSWorkspaceDidLaunchApplicationNotification,
    NSWorkspaceDidTerminateApplicationNotification, NSWorkspaceScreensDidSleepNotification,
    NSWorkspaceWillSleepNotification,
};
use objc2_application_services::{AXObserver, AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFArray, CFBoolean, CFDictionary, CFEqual, CFMachPort, CFNumber,
    CFRetained, CFRunLoop, CFRunLoopTimer, CFRunLoopTimerContext, CFString, CFType, CGPoint,
    CGSize, kCFAllocatorDefault, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType, CGWindowID, CGWindowListCopyWindowInfo,
    CGWindowListOption,
};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSOperationQueue, NSString,
};

use super::app::AppDelegate;
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
use super::window::AXWindow;
use crate::action::Actions;
use crate::config::{Keymap, Modifiers};
use crate::core::Dimension;
use crate::platform::macos::objc2_wrapper::kCGWindowNumber;

const FRAME_THROTTLE: Duration = Duration::from_millis(16);
const SYNC_INTERVAL: Duration = Duration::from_secs(5);

pub(super) fn setup_app_observers(delegate: &'static AppDelegate) {
    sync_all_windows(delegate);
    schedule_sync_timer(delegate);

    let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();

    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidLaunchApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                handle_app_launched(delegate, notification.as_ref());
            }),
        );
    }

    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidTerminateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                handle_app_terminated(delegate, notification.as_ref());
            }),
        );
    }

    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                handle_app_activated(delegate, notification.as_ref());
            }),
        );
    }

    // Suspend on system sleep, screen sleep, or lock, as AX APIs are unusable while under these
    // conditions
    // Resume ONLY on unlock, as screen can wake while locked, AX APIs are still unusable while
    // locked
    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceWillSleepNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("System will sleep, suspending window management");
                delegate.ivars().is_suspended.set(true);
            }),
        );
    }

    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceScreensDidSleepNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("Screen did sleep, suspending window management");
                delegate.ivars().is_suspended.set(true);
            }),
        );
    }

    let distributed_center = NSDistributedNotificationCenter::defaultCenter();
    let lock_name = NSString::from_str("com.apple.screenIsLocked");
    let unlock_name = NSString::from_str("com.apple.screenIsUnlocked");

    unsafe {
        distributed_center.addObserverForName_object_queue_usingBlock(
            Some(lock_name.as_ref()),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("Screen locked, suspending window management");
                delegate.ivars().is_suspended.set(true);
            }),
        );
    }

    unsafe {
        distributed_center.addObserverForName_object_queue_usingBlock(
            Some(unlock_name.as_ref()),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("Screen unlocked, resuming window management");
                delegate.ivars().is_suspended.set(false);
                sync_all_windows(delegate);
            }),
        );
    }
}

pub(super) fn listen_to_input_devices(delegate: &'static AppDelegate) -> Result<()> {
    let run_loop = CFRunLoop::current().unwrap();
    let event_mask = 1u64 << CGEventType::KeyDown.0;
    let delegate_ptr = delegate as *const AppDelegate as *mut std::ffi::c_void;
    let Some(match_port) = (unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            event_mask,
            Some(event_tap_callback),
            delegate_ptr,
        )
    }) else {
        return Err(anyhow::anyhow!("Failed to create event tap"));
    };

    delegate.ivars().event_tap.set(match_port.clone()).unwrap();

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

fn schedule_sync_timer(delegate: &'static AppDelegate) {
    let delegate_ptr = delegate as *const AppDelegate as *mut c_void;
    let mut context = CFRunLoopTimerContext {
        version: 0,
        info: delegate_ptr,
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
            &mut context,
        )
    };
    if let Some(timer) = timer {
        CFRunLoop::current()
            .unwrap()
            .add_timer(Some(&timer), unsafe { kCFRunLoopDefaultMode });
        let _ = delegate.ivars().sync_timer.set(timer);
    }
}

unsafe extern "C-unwind" fn sync_timer_callback(_timer: *mut CFRunLoopTimer, info: *mut c_void) {
    let delegate: &'static AppDelegate = unsafe { &*(info as *const AppDelegate) };
    sync_all_windows(delegate);
}

fn handle_app_launched(delegate: &'static AppDelegate, notification: &NSNotification) {
    let Some(app) = get_app_from_notification(notification) else {
        return;
    };
    let app_name = app.localizedName().map(|n| n.to_string());
    tracing::debug!(app = ?app_name, "App launched");
    try_register_app(delegate, &app);
    sync_app_windows(delegate, &app);
}

fn handle_app_terminated(delegate: &'static AppDelegate, notification: &NSNotification) {
    let Some(app) = get_app_from_notification(notification) else {
        return;
    };
    let pid = app.processIdentifier();
    tracing::debug!(%pid, "App terminated");
    remove_terminated_app(delegate, pid);
}

fn remove_terminated_app(delegate: &AppDelegate, pid: i32) {
    delegate.ivars().observers.borrow_mut().remove(&pid);
    for cg_id in delegate.ivars().ax_registry.borrow_mut().remove_by_pid(pid) {
        delegate.send_event(HubEvent::WindowDestroyed(cg_id));
    }
}

fn handle_app_activated(delegate: &'static AppDelegate, notification: &NSNotification) {
    if delegate.ivars().is_suspended.get() {
        return;
    }
    let Some(app) = get_app_from_notification(notification) else {
        return;
    };
    if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
        return;
    }

    let app_name = app.localizedName().map(|n| n.to_string());
    tracing::debug!(app = ?app_name, "App activated");
    sync_app_windows(delegate, &app);
    sync_focused_window(delegate, &app);
}

fn get_app_from_notification(
    notification: &NSNotification,
) -> Option<Retained<NSRunningApplication>> {
    let user_info = notification.userInfo()?;
    let app = unsafe { user_info.objectForKey(NSWorkspaceApplicationKey)? };
    Some(unsafe { Retained::cast_unchecked(app) })
}

fn try_register_app(delegate: &'static AppDelegate, app: &NSRunningApplication) {
    let pid = app.processIdentifier();
    if pid == -1 || app.activationPolicy() != NSApplicationActivationPolicy::Regular {
        return;
    }

    let mut observers = delegate.ivars().observers.borrow_mut();
    if observers.contains_key(&pid) {
        return;
    }

    let app_name = app
        .localizedName()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    match register_app(pid, delegate) {
        Ok(observer) => {
            tracing::info!(%pid, %app_name, "Registered app");
            observers.insert(pid, observer);
        }
        Err(err) => {
            tracing::warn!(%pid, %app_name, "Can't register app: {err:#}");
        }
    }
}

fn sync_focused_window(delegate: &AppDelegate, app: &NSRunningApplication) {
    let pid = app.processIdentifier();
    let ax_app = unsafe { AXUIElement::new_application(pid) };
    if let Ok(focused) = get_attribute::<AXUIElement>(&ax_app, &kAXFocusedWindowAttribute())
        && let Some(cg_id) = get_cg_window_id(&focused)
        && delegate.ivars().ax_registry.borrow().contains(cg_id)
    {
        delegate.send_event(HubEvent::WindowFocused(cg_id));
    }
}

#[tracing::instrument(skip_all)]
unsafe extern "C-unwind" fn observer_callback(
    _observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    refcon: *mut std::ffi::c_void,
) {
    let delegate: &'static AppDelegate = unsafe { &*(refcon as *const AppDelegate) };
    if delegate.ivars().is_suspended.get() {
        return;
    }

    let notification = unsafe { notification.as_ref() };
    let element = unsafe { element.as_ref() };
    tracing::trace!("Received event: {}", (*notification));

    if CFEqual(Some(notification), Some(&*kAXWindowCreatedNotification()))
        || CFEqual(Some(notification), Some(&*kAXUIElementDestroyedNotification()))
        || CFEqual(Some(notification), Some(&*kAXWindowMiniaturizedNotification()))
        || CFEqual(Some(notification), Some(&*kAXWindowDeminiaturizedNotification()))
    {
        handle_window_event(delegate, element);
        return;
    }
    if CFEqual(Some(notification), Some(&*kAXFocusedWindowChangedNotification())) {
        handle_window_focused(delegate, element);
        return;
    }

    let should_throttle = CFEqual(Some(notification), Some(&*kAXMovedNotification()))
        || CFEqual(Some(notification), Some(&*kAXResizedNotification()))
        || CFEqual(Some(notification), Some(&*kAXTitleChangedNotification()));

    if should_throttle {
        let now = Instant::now();
        let mut throttle = delegate.ivars().throttle.borrow_mut();
        let should_execute = throttle
            .last_execution
            .map(|last| now.duration_since(last) >= FRAME_THROTTLE)
            .unwrap_or(true);

        if should_execute {
            throttle.reset();
            drop(throttle);
            handle_frame_event(delegate, notification, element);
        } else if throttle.timer.is_none() {
            drop(throttle);
            schedule_throttle_timer(delegate, FRAME_THROTTLE);
        }
    }
}

fn handle_window_event(delegate: &AppDelegate, element: &AXUIElement) {
    let Ok(pid) = get_pid(element) else {
        return;
    };
    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        sync_app_windows(delegate, &app);
    }
}

fn handle_window_focused(delegate: &AppDelegate, element: &AXUIElement) {
    let Ok(pid) = get_pid(element) else {
        return;
    };
    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        sync_app_windows(delegate, &app);
        sync_focused_window(delegate, &app);
    }
}

fn handle_frame_event(delegate: &AppDelegate, notification: &CFString, element: &AXUIElement) {
    let Some(cg_id) = get_cg_window_id(element) else {
        return;
    };

    if CFEqual(Some(&**notification), Some(&*kAXTitleChangedNotification()))
        && let Ok(title) = get_attribute::<CFString>(element, &kAXTitleAttribute())
    {
        delegate.send_event(HubEvent::TitleChanged {
            cg_id,
            title: title.to_string(),
        });
    }
}

fn schedule_throttle_timer(delegate: &'static AppDelegate, delay: Duration) {
    let delegate_ptr = delegate as *const AppDelegate as *mut std::ffi::c_void;
    let fire_time = CFAbsoluteTimeGetCurrent() + delay.as_secs_f64();
    let mut timer_context = CFRunLoopTimerContext {
        version: 0,
        info: delegate_ptr,
        retain: None,
        release: None,
        copyDescription: None,
    };
    let timer = unsafe {
        CFRunLoopTimer::new(
            None,
            fire_time,
            0.0,
            0,
            0,
            Some(throttle_timer_callback),
            &mut timer_context,
        )
    };
    if let Some(timer) = timer {
        CFRunLoop::current()
            .unwrap()
            .add_timer(Some(&timer), unsafe { kCFRunLoopDefaultMode });
        delegate.ivars().throttle.borrow_mut().timer = Some(timer);
    }
}

unsafe extern "C-unwind" fn throttle_timer_callback(
    _timer: *mut CFRunLoopTimer,
    info: *mut std::ffi::c_void,
) {
    // Safety: AppDelegate lives until the end of the app
    let delegate: &'static AppDelegate = unsafe { &*(info as *const AppDelegate) };
    let ivars = delegate.ivars();

    let mut throttle = ivars.throttle.borrow_mut();
    throttle.timer = None;

    if ivars.is_suspended.get() {
        return;
    }

    throttle.last_execution = Some(Instant::now());
}

fn sync_app_windows(delegate: &AppDelegate, app: &NSRunningApplication) {
    let pid = app.processIdentifier();
    let ax_app = unsafe { AXUIElement::new_application(pid) };
    let Ok(windows) = get_windows(&ax_app) else {
        return;
    };

    let screen = delegate.ivars().screen;
    let cg_window_ids = list_cg_window_ids();
    let mut ax_registry = delegate.ivars().ax_registry.borrow_mut();

    let tracked_cg_ids = ax_registry.cg_ids_for_pid(pid);
    for cg_id in tracked_cg_ids {
        if cg_window_ids.contains(&cg_id) && ax_registry.is_valid(cg_id) {
            continue;
        }
        ax_registry.remove(cg_id);
        delegate.send_event(HubEvent::WindowDestroyed(cg_id));
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

        if !is_manageable(&ax_window, &ax_app, title.as_deref()) {
            continue;
        }

        let ax_win = AXWindow::new(ax_window.clone(), ax_app.clone(), pid, screen);
        ax_registry.insert(cg_id, ax_win);

        let info = WindowInfo {
            cg_id,
            title,
            app_name: app_name.clone(),
            bundle_id: bundle_id.clone(),
            should_tile: should_tile(&ax_window),
            dimension: get_ax_dimension(&ax_window),
        };
        delegate.send_event(HubEvent::WindowCreated(info));
    }
}

// AX notifications are unreliable, when new windows are being rapidly created and deleted,
// macOS may decide skip sending notifications.
// So we poll periodically to keep the state in sync.
// https://github.com/nikitabobko/AeroSpace/issues/445
fn sync_all_windows(delegate: &'static AppDelegate) {
    if delegate.ivars().is_suspended.get() {
        return;
    }
    tracing::trace!("Periodic sync every {}s", SYNC_INTERVAL.as_secs());

    let running_apps: Vec<_> = running_apps().collect();
    let running: HashSet<i32> = running_apps
        .iter()
        .map(|app| app.processIdentifier())
        .collect();

    let terminated_pids: Vec<_> = delegate
        .ivars()
        .observers
        .borrow()
        .keys()
        .filter(|pid| !running.contains(pid))
        .copied()
        .collect();
    for pid in terminated_pids {
        remove_terminated_app(delegate, pid);
    }

    for running_app in &running_apps {
        try_register_app(delegate, running_app);
        sync_app_windows(delegate, running_app);
    }
}

unsafe extern "C-unwind" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    refcon: *mut std::ffi::c_void,
) -> *mut CGEvent {
    // Safety: AppDelegate lives until the end of the app
    let delegate: &'static AppDelegate = unsafe { &*(refcon as *const AppDelegate) };
    let event = event.as_ptr();

    if event_type == CGEventType::TapDisabledByTimeout
        || event_type == CGEventType::TapDisabledByUserInput
    {
        if let Some(tap) = delegate.ivars().event_tap.get() {
            tracing::debug!("Event tap disabled, re-enabling");
            CGEvent::tap_enable(tap, true);
        }
    } else if event_type == CGEventType::KeyDown {
        if handle_keyboard(delegate, event) {
            return std::ptr::null_mut();
        }
    } else {
        tracing::warn!("Unrecognized event type: {:?}", event_type);
    }

    event
}

fn handle_keyboard(delegate: &'static AppDelegate, event: *mut CGEvent) -> bool {
    let flags = CGEvent::flags(Some(unsafe { &*event }));
    let key = get_key_from_event(event);

    let mut modifiers = Modifiers::empty();
    if flags.contains(CGEventFlags::MaskCommand) {
        modifiers |= Modifiers::CMD;
    }
    if flags.contains(CGEventFlags::MaskShift) {
        modifiers |= Modifiers::SHIFT;
    }
    if flags.contains(CGEventFlags::MaskAlternate) {
        modifiers |= Modifiers::ALT;
    }
    if flags.contains(CGEventFlags::MaskControl) {
        modifiers |= Modifiers::CTRL;
    }

    let keymap = Keymap { key, modifiers };
    let actions = delegate.ivars().config.borrow().get_actions(&keymap);

    if actions.is_empty() {
        return false;
    }

    // Event tap is disabled while locked.
    // If we receive hotkeys event, it must be unlocked
    if delegate.ivars().is_suspended.get() {
        tracing::info!("Received keymap action, resuming window management");
        delegate.ivars().is_suspended.set(false);
    }

    handle_actions(delegate, &actions);
    true
}

pub(super) fn handle_actions(delegate: &AppDelegate, actions: &Actions) {
    delegate.send_event(HubEvent::Action(actions.clone()));
}

fn get_key_from_event(event: *mut CGEvent) -> String {
    let keycode =
        CGEvent::integer_value_field(Some(unsafe { &*event }), CGEventField::KeyboardEventKeycode);

    match keycode {
        0x24 => return "return".to_string(),
        0x4C => return "enter".to_string(),
        0x33 => return "backspace".to_string(),
        0x35 => return "escape".to_string(),
        0x30 => return "tab".to_string(),
        0x31 => return "space".to_string(),
        0x7E => return "up".to_string(),
        0x7D => return "down".to_string(),
        0x7B => return "left".to_string(),
        0x7C => return "right".to_string(),
        _ => {}
    }

    let max_len: usize = 256;
    let mut buffer: Vec<u16> = vec![0; max_len];
    let mut actual_len: std::ffi::c_ulong = 0;
    unsafe {
        CGEvent::keyboard_get_unicode_string(
            Some(&*event),
            max_len as std::ffi::c_ulong,
            &mut actual_len as *mut std::ffi::c_ulong,
            buffer.as_mut_ptr(),
        )
    };
    String::from_utf16(&buffer[..actual_len as usize]).unwrap()
}

fn get_windows(app: &AXUIElement) -> anyhow::Result<CFRetained<CFArray<AXUIElement>>> {
    Ok(get_attribute(app, &kAXWindowsAttribute())?)
}

fn register_app(pid: i32, delegate: &'static AppDelegate) -> Result<CFRetained<AXObserver>> {
    let run_loop = CFRunLoop::current().unwrap();
    let observer = create_observer(pid, Some(observer_callback))?;
    let source = unsafe { observer.run_loop_source() };
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    let ax_app = unsafe { AXUIElement::new_application(pid) };
    let delegate_ptr = delegate as *const AppDelegate as *mut std::ffi::c_void;
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
        add_observer_notification(&observer, &ax_app, &notification, delegate_ptr)?;
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

fn is_manageable(window: &AXUIElement, app: &AXUIElement, title: Option<&str>) -> bool {
    let role = get_attribute::<CFString>(window, &kAXRoleAttribute()).ok();
    let subrole = get_attribute::<CFString>(window, &kAXSubroleAttribute()).ok();

    let is_window = role
        .as_ref()
        .map(|r| CFEqual(Some(&**r), Some(&*kAXWindowRole())))
        .unwrap_or(false);

    let is_standard = subrole
        .as_ref()
        .map(|sr| CFEqual(Some(&**sr), Some(&*kAXStandardWindowSubrole())))
        .unwrap_or(false);

    let is_root = match get_attribute::<AXUIElement>(window, &kAXParentAttribute()) {
        Err(_) => true,
        Ok(parent) => CFEqual(Some(&*parent), Some(app)),
    };

    let can_move = is_attribute_settable(window, &kAXPositionAttribute());
    let can_resize = is_attribute_settable(window, &kAXSizeAttribute());
    let can_focus = is_attribute_settable(window, &kAXMainAttribute());

    let is_minimized = get_attribute::<CFBoolean>(window, &kAXMinimizedAttribute())
        .map(|b| b.as_bool())
        .unwrap_or(false);

    is_window
        && is_standard
        && is_root
        && can_move
        && can_resize
        && can_focus
        && !is_minimized
        && title.is_some()
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

pub(super) struct ThrottleState {
    pub(super) last_execution: Option<Instant>,
    pub(super) timer: Option<CFRetained<CFRunLoopTimer>>,
}

impl ThrottleState {
    pub(super) fn new() -> Self {
        Self {
            last_execution: None,
            timer: None,
        }
    }

    fn reset(&mut self) {
        if let Some(timer) = self.timer.take() {
            CFRunLoopTimer::invalidate(&timer);
        }
        self.last_execution = Some(Instant::now());
    }
}
