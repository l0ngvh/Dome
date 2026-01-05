use std::{collections::HashMap, ptr::NonNull, time::Duration, time::Instant};

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
use objc2_application_services::{AXObserver, AXUIElement};
use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFArray, CFHash, CFMachPort, CFRetained, CFRunLoop, CFRunLoopTimer,
    CFRunLoopTimerContext, CFString, kCFAllocatorDefault, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType,
};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSOperationQueue, NSString,
};

use super::app::AppDelegate;
use super::context::{RemovedWindow, WindowRegistry};
use super::handler::{apply_layout, execute_actions, focus_window, render_workspace};
use super::objc2_wrapper::{
    add_observer_notification, create_observer, get_attribute, get_pid,
    kAXApplicationHiddenNotification, kAXApplicationShownNotification, kAXFocusedWindowAttribute,
    kAXFocusedWindowChangedNotification, kAXResizedNotification, kAXTitleChangedNotification,
    kAXUIElementDestroyedNotification, kAXWindowCreatedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification, kAXWindowsAttribute,
};
use super::window::MacWindow;
use crate::config::{Keymap, Modifiers, WindowRule};
use crate::core::Hub;

const THROTTLE_DURATION: Duration = Duration::from_millis(20);

pub(super) fn setup_app_observers(delegate: &'static AppDelegate) {
    let mut observers = HashMap::new();
    for pid in running_app_pids() {
        let app_name = get_app_name(pid);
        match register_app(pid, delegate) {
            Ok(observer) => {
                tracing::info!(%pid, %app_name, "Registered app on startup");
                observers.insert(pid, observer);
            }
            Err(e) => {
                tracing::warn!(%pid, %app_name, "Can't register app on startup: {e:#}");
            }
        }
    }

    *delegate.ivars().observers.borrow_mut() = observers;
    let apps = delegate.ivars().observers.clone();
    let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();

    unsafe {
        let apps = apps.clone();
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidLaunchApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                let Some(pid) = get_pid_from_notification(notification) else {
                    tracing::trace!("Launched application doesn't have a pid");
                    return;
                };
                let app_name = get_app_name(pid);
                tracing::trace!(%pid, %app_name, "App launched");
                let observer = match register_app(pid, delegate) {
                    Ok(observer) => {
                        tracing::info!(%pid, %app_name, "Registered app on launch");
                        observer
                    }
                    Err(e) => {
                        tracing::warn!(%pid, %app_name, "Can't track application: {e:#}");
                        return;
                    }
                };
                if let Err(e) = render_workspace(delegate) {
                    tracing::warn!("Failed to render workspace after app launch: {e:#}");
                }
                apps.borrow_mut().insert(pid, observer);
            }),
        );
    }

    unsafe {
        let apps = apps.clone();
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidTerminateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                let Some(pid) = get_pid_from_notification(notification) else {
                    tracing::trace!("Terminated application doesn't have a pid");
                    return;
                };
                apps.borrow_mut().remove(&pid);
                let (tiling_windows, float_windows) =
                    delegate.ivars().registry.borrow_mut().remove_by_pid(pid);
                let mut hub = delegate.ivars().hub.borrow_mut();
                for (id, window) in &tiling_windows {
                    let title = window.title();
                    let app_name = window.app_name();
                    let _span = tracing::debug_span!(
                        "app_terminated_delete_tiling",
                        %pid,
                        %id,
                        %title,
                        %app_name
                    )
                    .entered();
                    hub.delete_window(*id);
                    tracing::debug!("Tiling window deleted");
                }
                for (id, window) in &float_windows {
                    let title = window.title();
                    let app_name = window.app_name();
                    let _span = tracing::debug_span!(
                        "app_terminated_delete_float",
                        %pid,
                        %id,
                        %title,
                        %app_name
                    )
                    .entered();
                    hub.delete_float(*id);
                    tracing::debug!("Float window deleted");
                }
                drop(hub);
                if (!tiling_windows.is_empty() || !float_windows.is_empty())
                    && let Err(e) = render_workspace(delegate)
                {
                    tracing::warn!("Failed to render workspace after terminating app: {e:#}");
                }
            }),
        );
    }

    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                let Some(pid) = get_pid_from_notification(notification) else {
                    return;
                };
                let app_name = get_app_name(pid);
                tracing::trace!(%pid, %app_name, "App activated");
                let app = AXUIElement::new_application(pid);
                let Ok(focused_window) =
                    get_attribute::<AXUIElement>(&app, &kAXFocusedWindowAttribute())
                else {
                    return;
                };
                let cf_hash = CFHash(Some(&focused_window));
                let registry = delegate.ivars().registry.borrow();
                let mut hub = delegate.ivars().hub.borrow_mut();
                if let Some(window_id) = registry.get_tiling_by_hash(cf_hash) {
                    if !hub.is_focusing(window_id) {
                        drop(registry);
                        hub.set_focus(window_id);
                        drop(hub);
                        if let Err(e) = render_workspace(delegate) {
                            tracing::warn!("Failed to render workspace: {e:#}");
                        }
                    }
                } else if let Some(float_id) = registry.get_float_by_hash(cf_hash) {
                    drop(registry);
                    hub.set_float_focus(float_id);
                    drop(hub);
                    if let Err(e) = render_workspace(delegate) {
                        tracing::warn!("Failed to render workspace: {e:#}");
                    }
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
        let apps = apps.clone();
        distributed_center.addObserverForName_object_queue_usingBlock(
            Some(unlock_name.as_ref()),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("Screen unlocked, resuming window management");
                delegate.ivars().is_suspended.set(false);

                let mut apps = apps.borrow_mut();
                for pid in running_app_pids() {
                    if let std::collections::hash_map::Entry::Vacant(e) = apps.entry(pid) {
                        let app_name = get_app_name(pid);
                        match register_app(pid, delegate) {
                            Ok(observer) => {
                                tracing::info!(%pid, %app_name, "Registered app on unlock");
                                e.insert(observer);
                            }
                            Err(err) => {
                                tracing::warn!(%pid, %app_name, "Can't register app on unlock: {err:#}");
                            }
                        }
                    } else {
                        let ax_app = AXUIElement::new_application(pid);
                        sync_windows(pid, &ax_app, delegate);
                    }
                }
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

    let is_focus_change = *notification == *kAXFocusedWindowChangedNotification();

    let now = Instant::now();
    let mut throttle = ivars.throttle.borrow_mut();
    let should_execute = throttle
        .last_execution
        .map(|last| now.duration_since(last) >= THROTTLE_DURATION)
        .unwrap_or(true);

    if should_execute {
        throttle.reset();
        drop(throttle);
        let ax_app = unsafe { AXUIElement::new_application(pid) };
        // AX notifications are unreliable, when new windows are being rapidly created and deleted,
        // macOS may decide skip sending notifications.
        // So we are basically polling as much as possible to keep the state in sync
        // https://github.com/nikitabobko/AeroSpace/issues/445
        sync_windows(pid, &ax_app, delegate);

        let mut hub = ivars.hub.borrow_mut();
        let registry = ivars.registry.borrow();
        if is_focus_change {
            sync_focus(&ax_app, &mut hub, &registry);
        } else if let Err(e) = focus_window(&hub, &registry) {
            tracing::warn!("Failed to focus window: {e:#}");
        }

        let mut displayed_windows = ivars.displayed_windows.borrow_mut();
        let tiling_overlay = ivars.tiling_overlay.get().unwrap();
        let float_overlay = ivars.float_overlay.get().unwrap();
        if let Err(e) = apply_layout(
            &hub,
            &registry,
            &ivars.config,
            &mut displayed_windows,
            tiling_overlay,
            float_overlay,
        ) {
            tracing::warn!("Failed to apply layout: {e:#}");
        }
    } else {
        throttle.pending_pids.insert(pid);
        if is_focus_change {
            throttle.pending_focus_sync = true;
        }
        if throttle.timer.is_none() {
            drop(throttle);
            schedule_throttle_timer(delegate, THROTTLE_DURATION);
        }
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
    // Similar to observer callback, we should not call render_workspace here, as this is just the
    // throttling version of observer callback
    // Safety: AppDelegate lives until the end of the app
    let delegate: &'static AppDelegate = unsafe { &*(info as *const AppDelegate) };
    let ivars = delegate.ivars();
    let mut throttle = ivars.throttle.borrow_mut();
    throttle.timer = None;

    // Skip processing when suspended (sleep/lock)
    if ivars.is_suspended.get() {
        throttle.pending_pids.clear();
        throttle.pending_focus_sync = false;
        return;
    }

    throttle.last_execution = Some(Instant::now());

    let pids: Vec<_> = throttle.pending_pids.drain().collect();
    let pending_focus_sync = std::mem::take(&mut throttle.pending_focus_sync);
    drop(throttle);

    for pid in pids {
        let ax_app = unsafe { AXUIElement::new_application(pid) };
        sync_windows(pid, &ax_app, delegate);
        if pending_focus_sync {
            let mut hub = ivars.hub.borrow_mut();
            let registry = ivars.registry.borrow();
            sync_focus(&ax_app, &mut hub, &registry);
        }
    }

    let hub = ivars.hub.borrow();
    let registry = ivars.registry.borrow();
    let mut displayed_windows = ivars.displayed_windows.borrow_mut();
    let tiling_overlay = ivars.tiling_overlay.get().unwrap();
    let float_overlay = ivars.float_overlay.get().unwrap();
    if let Err(e) = apply_layout(
        &hub,
        &registry,
        &ivars.config,
        &mut displayed_windows,
        tiling_overlay,
        float_overlay,
    ) {
        tracing::warn!("Failed to apply layout: {e:#}");
    }

    if !pending_focus_sync && let Err(e) = focus_window(&hub, &registry) {
        tracing::warn!("Failed to focus window: {e:#}");
    }
}

fn sync_windows(pid: i32, app: &CFRetained<AXUIElement>, delegate: &'static AppDelegate) {
    let Ok(windows) = get_windows(app) else {
        tracing::warn!("Failed to get windows");
        return;
    };
    let hub = delegate.ivars().hub.borrow();
    let screen = hub.screen();
    drop(hub);
    let rules = &delegate.ivars().config.window_rules;
    let active_windows: Vec<_> = windows
        .into_iter()
        .filter_map(|w| MacWindow::new(w.clone(), app.clone(), pid, screen))
        .filter(|w| should_manage(w, rules))
        .collect();
    let active_hashes: Vec<_> = active_windows.iter().map(|w| w.cf_hash()).collect();

    let mut registry = delegate.ivars().registry.borrow_mut();
    let tracked_hashes = registry.hashes_for_pid(pid);

    let mut hub = delegate.ivars().hub.borrow_mut();
    for h in tracked_hashes {
        if !active_hashes.contains(&h) {
            match registry.remove_by_hash(h) {
                Some(RemovedWindow::Tiling(id, window)) => {
                    let _span =
                        tracing::info_span!("sync_windows", %id, window = %window).entered();
                    hub.delete_window(id);
                    tracing::info!("Tiling window deleted");
                }
                Some(RemovedWindow::Float(id, window)) => {
                    let _span =
                        tracing::info_span!("sync_windows", %id, window = %window).entered();
                    hub.delete_float(id);
                    tracing::info!("Float window deleted");
                }
                None => {}
            }
        } else {
            registry.update_title(h);
        }
    }

    let new_windows: Vec<_> = active_windows
        .into_iter()
        .filter(|w| !registry.contains(w))
        .collect();

    for mac_window in new_windows {
        let rule = match_rule(&mac_window, &delegate.ivars().config.window_rules);
        if mac_window.should_tile() {
            let id = hub.insert_tiling();
            let _span = tracing::info_span!("sync_windows", %id, window = %mac_window).entered();
            tracing::info!("New tiling window");
            registry.insert_tiling(id, mac_window);
        } else {
            let dim = mac_window.dimension();
            let id = hub.insert_float(dim);
            let _span = tracing::info_span!("sync_windows", %id, window = %mac_window).entered();
            tracing::info!("New float window");
            registry.insert_float(id, mac_window);
        }
        if let Some(r) = rule {
            execute_actions(&mut hub, &mut registry, &r.run);
        }
    }
}

fn sync_focus(app: &CFRetained<AXUIElement>, hub: &mut Hub, registry: &WindowRegistry) {
    let Ok(focused) = get_attribute::<AXUIElement>(app, &kAXFocusedWindowAttribute()) else {
        return;
    };
    let h = CFHash(Some(&focused));
    if let Some(id) = registry.get_tiling_by_hash(h) {
        if !hub.is_focusing(id) {
            let title = registry
                .get_tiling(id)
                .map(|w| w.to_string())
                .unwrap_or_default();
            tracing::debug!(%id, %title, "Focus changed to tiling window");
            hub.set_focus(id);
        }
    } else if let Some(id) = registry.get_float_by_hash(h) {
        let title = registry
            .get_float(id)
            .map(|w| w.to_string())
            .unwrap_or_default();
        tracing::debug!(%id, %title, "Focus changed to float window");
        hub.set_float_focus(id);
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
    let actions = delegate.ivars().config.get_actions(&keymap);

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

fn get_pid_from_notification(notification: NonNull<NSNotification>) -> Option<i32> {
    let notification = unsafe { &*notification.as_ptr() };
    let dict = notification.userInfo()?;
    let app = dict.valueForKey(unsafe { NSWorkspaceApplicationKey })?;
    let app = unsafe { Retained::cast_unchecked::<NSRunningApplication>(app) };
    Some(app.processIdentifier())
}

fn get_windows(app: &AXUIElement) -> Result<CFRetained<CFArray<AXUIElement>>> {
    get_attribute(app, &kAXWindowsAttribute())
}

fn register_app(pid: i32, delegate: &'static AppDelegate) -> Result<CFRetained<AXObserver>> {
    let hub = delegate.ivars().hub.borrow();
    let screen = hub.screen();
    drop(hub);
    let app = unsafe { AXUIElement::new_application(pid) };
    let rules = &delegate.ivars().config.window_rules;

    if let Ok(windows) = get_windows(&app) {
        for window in windows {
            let Some(mac_window) = MacWindow::new(window.clone(), app.clone(), pid, screen) else {
                continue;
            };
            if !should_manage(&mac_window, rules) {
                continue;
            }
            let rule = match_rule(&mac_window, rules);
            let mut registry = delegate.ivars().registry.borrow_mut();
            let mut hub = delegate.ivars().hub.borrow_mut();
            if mac_window.should_tile() {
                let window_id = hub.insert_tiling();
                tracing::debug!(%window_id, window = %mac_window, "Managing as tiling");
                registry.insert_tiling(window_id, mac_window);
            } else {
                let dim = mac_window.dimension();
                let float_id = hub.insert_float(dim);
                tracing::debug!(%float_id, window = %mac_window, "Managing as float");
                registry.insert_float(float_id, mac_window);
            }
            if let Some(r) = rule {
                execute_actions(&mut hub, &mut registry, &r.run);
            }
        }
    }

    let run_loop = CFRunLoop::current().unwrap();
    let observer = create_observer(pid, Some(observer_callback))?;
    let source = unsafe { observer.run_loop_source() };
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

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
        add_observer_notification(&observer, &app, &notification, delegate_ptr)?;
    }

    Ok(observer)
}

fn running_app_pids() -> impl Iterator<Item = i32> {
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .into_iter()
        .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
        .map(|app| app.processIdentifier())
        .filter(|&pid| pid != -1)
}

fn match_rule<'a>(window: &MacWindow, rules: &'a [WindowRule]) -> Option<&'a WindowRule> {
    for rule in rules {
        if let Some(app) = &rule.app
            && !pattern_matches(app, window.app_name())
        {
            continue;
        }
        if let Some(b) = &rule.bundle_id
            && window.bundle_id() != Some(b.as_str())
        {
            continue;
        }
        if let Some(t) = &rule.title
            && !pattern_matches(t, window.title())
        {
            continue;
        }
        if rule.app.is_some() || rule.bundle_id.is_some() || rule.title.is_some() {
            return Some(rule);
        }
    }
    None
}

fn should_manage(window: &MacWindow, rules: &[WindowRule]) -> bool {
    match_rule(window, rules).map_or_else(|| window.is_manageable(), |r| r.manage)
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
