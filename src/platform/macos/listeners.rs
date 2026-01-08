use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use anyhow::Result;
use block2::RcBlock;
use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace,
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
use super::context::WindowRegistry;
use super::handler::{execute_actions, render_workspace};
use super::objc2_wrapper::{
    add_observer_notification, create_observer, get_attribute, get_cg_window_id, get_pid,
    is_attribute_settable, kAXApplicationHiddenNotification, kAXApplicationShownNotification,
    kAXFocusedWindowAttribute, kAXFocusedWindowChangedNotification, kAXFullScreenAttribute,
    kAXMainAttribute, kAXMinimizedAttribute, kAXParentAttribute, kAXPositionAttribute,
    kAXResizedNotification, kAXRoleAttribute, kAXSizeAttribute, kAXStandardWindowSubrole,
    kAXSubroleAttribute, kAXTitleAttribute, kAXTitleChangedNotification,
    kAXUIElementDestroyedNotification, kAXWindowCreatedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification, kAXWindowRole,
    kAXWindowsAttribute,
};
use super::window::{MacWindow, WindowType};
use crate::config::{Keymap, MacosWindowRule, Modifiers};
use crate::core::{Dimension, Hub};
use crate::platform::macos::objc2_wrapper::kCGWindowNumber;

const THROTTLE_DURATION: Duration = Duration::from_millis(20);

pub(super) fn setup_app_observers(delegate: &'static AppDelegate) {
    sync_all_windows(delegate);
    if let Err(e) = render_workspace(delegate) {
        tracing::warn!("Failed to render workspace on startup: {e:#}");
    }

    let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();

    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidLaunchApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                sync_all_windows(delegate);
                if let Err(e) = render_workspace(delegate) {
                    tracing::warn!("Failed to render workspace after app launch: {e:#}");
                }
            }),
        );
    }

    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidTerminateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                sync_all_windows(delegate);
                if let Err(e) = render_workspace(delegate) {
                    tracing::warn!("Failed to render workspace after app termination: {e:#}");
                }
            }),
        );
    }

    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                sync_all_windows(delegate);
                if let Err(e) = render_workspace(delegate) {
                    tracing::warn!("Failed to render workspace: {e:#}");
                }
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
                if let Err(e) = render_workspace(delegate) {
                    tracing::warn!("Failed to render workspace after unlock: {e:#}");
                }
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

#[tracing::instrument(skip_all)]
unsafe extern "C-unwind" fn observer_callback(
    _observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    refcon: *mut std::ffi::c_void,
) {
    // Should not call render_workspace here, as that also set focused window. This callback
    // can be fired when focused change, so calling render_workspace can cause an infinite loop
    // of focus changes
    // Safety: AppDelegate lives until the end of the app
    let delegate: &'static AppDelegate = unsafe { &*(refcon as *const AppDelegate) };
    let ivars = delegate.ivars();

    // Skip processing when suspended (sleep/lock)
    if ivars.is_suspended.get() {
        return;
    }

    let element = unsafe { CFRetained::retain(element) };
    let Ok(pid) = get_pid(&element) else {
        return;
    };
    let notification = unsafe { CFRetained::retain(notification) };
    let app_name = get_app_name(pid);
    tracing::trace!(
        "[{app_name}] (pid: {pid}) Received event: {}",
        (*notification)
    );

    let now = Instant::now();
    let mut throttle = ivars.throttle.borrow_mut();
    let should_execute = throttle
        .last_execution
        .map(|last| now.duration_since(last) >= THROTTLE_DURATION)
        .unwrap_or(true);

    if should_execute {
        throttle.reset();
        drop(throttle);
        // AX notifications are unreliable, when new windows are being rapidly created and deleted,
        // macOS may decide skip sending notifications.
        // So we are basically polling as much as possible to keep the state in sync
        // https://github.com/nikitabobko/AeroSpace/issues/445
        sync_all_windows(delegate);
        if let Err(e) = render_workspace(delegate) {
            tracing::warn!("Failed to render workspace: {e:#}");
        }
    } else if throttle.timer.is_none() {
        drop(throttle);
        schedule_throttle_timer(delegate, THROTTLE_DURATION);
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

    // Skip processing when suspended (sleep/lock)
    if ivars.is_suspended.get() {
        return;
    }

    throttle.last_execution = Some(Instant::now());
    drop(throttle);

    sync_all_windows(delegate);
    if let Err(e) = render_workspace(delegate) {
        tracing::warn!("Failed to render workspace: {e:#}");
    }
}

#[tracing::instrument(skip_all)]
fn sync_all_windows(delegate: &'static AppDelegate) {
    if delegate.ivars().is_suspended.get() {
        return;
    }

    let mut observers = delegate.ivars().observers.borrow_mut();
    let mut registry = delegate.ivars().registry.borrow_mut();
    let mut hub = delegate.ivars().hub.borrow_mut();
    let config = delegate.ivars().config.borrow();
    let cg_window_ids = list_cg_window_ids();
    let running_apps: Vec<_> = running_apps().collect();

    let running: HashSet<i32> = running_apps
        .iter()
        .map(|app| app.processIdentifier())
        .collect();

    // Remove terminated apps
    let terminated_pids: Vec<_> = observers
        .keys()
        .filter(|pid| !running.contains(pid))
        .copied()
        .collect();
    for pid in terminated_pids {
        observers.remove(&pid);
        for window in registry.remove_by_pid(pid) {
            match window.window_type() {
                WindowType::Tiling(id) => {
                    let _span =
                        tracing::info_span!("sync_terminated", %id, window = %window).entered();
                    hub.delete_window(id);
                    tracing::info!("Tiling window deleted");
                }
                WindowType::Float(id) => {
                    let _span =
                        tracing::info_span!("sync_terminated", %id, window = %window).entered();
                    hub.delete_float(id);
                    tracing::info!("Float window deleted");
                }
                WindowType::Popup => {}
            }
        }
    }

    for running_app in running_apps {
        let pid = running_app.processIdentifier();

        // Register app if not already registered
        if let Entry::Vacant(e) = observers.entry(pid) {
            let app_name = get_app_name(pid);
            match register_app(pid, delegate) {
                Ok(observer) => {
                    tracing::info!(%pid, %app_name, "Registered app");
                    e.insert(observer);
                }
                Err(err) => {
                    tracing::warn!(%pid, %app_name, "Can't register app: {err:#}");
                }
            }
        }

        let ax_app = unsafe { AXUIElement::new_application(pid) };
        let Ok(windows) = get_windows(&ax_app) else {
            continue;
        };

        let tracked_cg_ids = registry.cg_ids_for_pid(pid);
        for cg_id in tracked_cg_ids {
            if cg_window_ids.contains(&cg_id) && registry.get(cg_id).is_some_and(|w| w.is_valid()) {
                registry.update_title(cg_id);
                continue;
            }
            if let Some(window) = registry.remove(cg_id) {
                match window.window_type() {
                    WindowType::Tiling(id) => {
                        let _span =
                            tracing::info_span!("sync_windows", %id, window = %window).entered();
                        hub.delete_window(id);
                        tracing::info!("Tiling window deleted");
                    }
                    WindowType::Float(id) => {
                        let _span =
                            tracing::info_span!("sync_windows", %id, window = %window).entered();
                        hub.delete_float(id);
                        tracing::info!("Float window deleted");
                    }
                    WindowType::Popup => {}
                }
            }
        }

        let app_name = running_app
            .localizedName()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let bundle_id = running_app.bundleIdentifier().map(|b| b.to_string());

        for ax_window in windows {
            process_new_window(
                ax_window,
                ax_app.clone(),
                pid,
                &app_name,
                bundle_id.as_deref(),
                &config.macos.window_rules,
                &mut hub,
                &mut registry,
            );
        }

        if !running_app.isActive() {
            continue;
        }
        if let Ok(focused) = get_attribute::<AXUIElement>(&ax_app, &kAXFocusedWindowAttribute())
            && let Some(cg_id) = get_cg_window_id(&focused)
            && let Some(window) = registry.get(cg_id)
        {
            let last_focused = delegate.ivars().last_focused.get();
            if last_focused == Some((pid, cg_id)) {
                continue;
            }
            delegate.ivars().last_focused.set(Some((pid, cg_id)));
            match window.window_type() {
                WindowType::Tiling(tiling_id) => {
                    hub.set_focus(tiling_id);
                }
                WindowType::Float(float_id) => {
                    hub.set_float_focus(float_id);
                }
                WindowType::Popup => {}
            }
        }
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

    execute_actions(
        &mut delegate.ivars().hub.borrow_mut(),
        &mut delegate.ivars().registry.borrow_mut(),
        &actions,
    );
    if let Err(e) = render_workspace(delegate) {
        tracing::warn!("Failed to render workspace: {e:#}");
    }
    true
}

fn get_key_from_event(event: *mut CGEvent) -> String {
    // Get virtual keycode for special keys
    let keycode =
        CGEvent::integer_value_field(Some(unsafe { &*event }), CGEventField::KeyboardEventKeycode);

    // Map special keys to names
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

    // For regular keys, get unicode character
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

fn get_app_name(pid: i32) -> String {
    NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .and_then(|app| app.localizedName())
        .map(|name| name.to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

/// Returns list of windows for this app in current space
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

fn match_rule<'a>(
    app_name: &str,
    bundle_id: Option<&str>,
    title: Option<&str>,
    rules: &'a [MacosWindowRule],
) -> Option<&'a MacosWindowRule> {
    for rule in rules {
        if let Some(app) = &rule.app
            && !pattern_matches(app, app_name)
        {
            continue;
        }
        if let Some(b) = &rule.bundle_id
            && bundle_id != Some(b.as_str())
        {
            continue;
        }
        if let Some(t) = &rule.title
            && !title.is_some_and(|title| pattern_matches(t, title))
        {
            continue;
        }
        if rule.app.is_some() || rule.bundle_id.is_some() || rule.title.is_some() {
            return Some(rule);
        }
    }
    None
}

fn should_manage(
    app_name: &str,
    bundle_id: Option<&str>,
    title: Option<&str>,
    rules: &[MacosWindowRule],
) -> bool {
    match_rule(app_name, bundle_id, title, rules).is_none_or(|r| r.manage)
}

fn process_new_window(
    ax_window: CFRetained<AXUIElement>,
    ax_app: CFRetained<AXUIElement>,
    pid: i32,
    app_name: &str,
    bundle_id: Option<&str>,
    rules: &[MacosWindowRule],
    hub: &mut Hub,
    registry: &mut WindowRegistry,
) {
    let Some(cg_id) = get_cg_window_id(&ax_window) else {
        return;
    };
    if registry.contains(cg_id) {
        return;
    }
    let screen = hub.screen();
    let title = get_attribute::<CFString>(&ax_window, &kAXTitleAttribute())
        .map(|t| t.to_string())
        .ok();

    let manageable = is_manageable(&ax_window, &ax_app, title.as_deref());
    let dominated = should_manage(app_name, bundle_id, title.as_deref(), rules);

    if !manageable || !dominated {
        let mac_window = MacWindow::new(
            ax_window,
            ax_app.clone(),
            cg_id,
            pid,
            screen,
            title,
            app_name.to_string(),
            WindowType::Popup,
        );
        tracing::trace!(window = %mac_window, "New popup window");
        registry.insert(mac_window);
        return;
    }

    let rule = match_rule(app_name, bundle_id, title.as_deref(), rules);
    let window_type = if should_tile(&ax_window) {
        WindowType::Tiling(hub.insert_tiling())
    } else {
        WindowType::Float(hub.insert_float(get_ax_dimension(&ax_window)))
    };
    let mac_window = MacWindow::new(
        ax_window,
        ax_app.clone(),
        cg_id,
        pid,
        screen,
        title,
        app_name.to_string(),
        window_type,
    );
    tracing::info!(window = %mac_window, "New window");
    registry.insert(mac_window);
    if let Some(r) = rule {
        execute_actions(hub, registry, &r.run);
    }
}

/// Returns true if this window should be tiled (not floated)
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

/// Returns true if this is a "real" window worth managing (tile or float)
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

fn pattern_matches(pattern: &str, text: &str) -> bool {
    if let Some(regex_pattern) = pattern.strip_prefix('/').and_then(|p| p.strip_suffix('/')) {
        regex::Regex::new(regex_pattern)
            .map(|r| r.is_match(text))
            .unwrap_or(false)
    } else {
        pattern == text
    }
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
