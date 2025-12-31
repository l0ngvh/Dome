use std::{
    cell::RefCell, collections::HashMap, collections::HashSet, ptr::NonNull, rc::Rc,
    time::Duration, time::Instant,
};

use anyhow::Result;
use block2::RcBlock;
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

use super::context::{Observers, RemovedWindow, WindowContext};
use super::objc2_wrapper::{
    add_observer_notification, create_observer, get_attribute, get_pid,
    kAXApplicationHiddenNotification, kAXApplicationShownNotification, kAXFocusedWindowAttribute,
    kAXFocusedWindowChangedNotification, kAXResizedNotification, kAXTitleChangedNotification,
    kAXUIElementDestroyedNotification, kAXWindowCreatedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification, kAXWindowsAttribute,
};
use super::overlay::collect_overlays;
use super::window::MacWindow;
use crate::config::{Action, FocusTarget, Keymap, Modifiers, MoveTarget, ToggleTarget};
use crate::core::{Child, Focus};

const THROTTLE_DURATION: Duration = Duration::from_millis(20);

pub(super) fn setup_app_observers(context_ptr: *mut WindowContext) -> Observers {
    let mut observers = HashMap::new();
    for app in NSWorkspace::sharedWorkspace().runningApplications() {
        if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
            continue;
        }
        let pid = app.processIdentifier();
        if pid == -1 {
            continue;
        }
        match register_app(pid, context_ptr) {
            Ok(observer) => {
                observers.insert(pid, observer);
            }
            Err(e) => {
                let app_name = get_app_name(pid);
                tracing::warn!(%pid, %app_name, "Can't create observer: {e:#}");
            }
        }
    }

    let apps = Rc::new(RefCell::new(observers));
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
                let observer = match register_app(pid, context_ptr) {
                    Ok(observer) => observer,
                    Err(e) => {
                        tracing::warn!(%pid, %app_name, "Can't track application: {e:#}");
                        return;
                    }
                };
                let context = &mut *context_ptr;
                if let Err(e) = render_workspace(context) {
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
                let app_name = get_app_name(pid);
                tracing::trace!(%pid, %app_name, "App terminated");
                apps.borrow_mut().remove(&pid);
                let context = &mut *context_ptr;
                let (window_ids, float_ids) = context.registry.borrow_mut().remove_by_pid(pid);
                for window_id in &window_ids {
                    context.hub.delete_window(*window_id);
                    tracing::debug!(%window_id, %app_name, "Tiling window deleted");
                }
                for float_id in &float_ids {
                    context.hub.delete_float(*float_id);
                    tracing::debug!(%float_id, %app_name, "Float window deleted");
                }
                if (!window_ids.is_empty() || !float_ids.is_empty())
                    && let Err(e) = render_workspace(context)
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
                let context = &mut *context_ptr;
                let registry = context.registry.borrow();
                if let Some(window_id) = registry.get_tiling_by_hash(cf_hash) {
                    if !context.hub.is_focusing(Child::Window(window_id)) {
                        drop(registry);
                        context.hub.set_focus(window_id);
                        if let Err(e) = render_workspace(context) {
                            tracing::warn!("Failed to render workspace: {e:#}");
                        }
                    }
                } else if let Some(float_id) = registry.get_float_by_hash(cf_hash) {
                    drop(registry);
                    context.hub.set_float_focus(float_id);
                    if let Err(e) = render_workspace(context) {
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
                let context = &mut *context_ptr;
                context.is_suspended = true;
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
                let context = &mut *context_ptr;
                context.is_suspended = true;
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
                let context = &mut *context_ptr;
                context.is_suspended = true;
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
                let context = &mut *context_ptr;
                context.is_suspended = false;
            }),
        );
    }

    apps
}

pub(super) fn listen_to_input_devices(context_ptr: *mut WindowContext) -> Result<()> {
    let run_loop = CFRunLoop::current().unwrap();
    let event_mask = 1u64 << CGEventType::KeyDown.0;
    let Some(match_port) = (unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            event_mask,
            Some(event_tap_callback),
            context_ptr as *mut std::ffi::c_void,
        )
    }) else {
        return Err(anyhow::anyhow!("Failed to create event tap"));
    };

    let context = unsafe { &mut *context_ptr };
    context.event_tap = Some(match_port.clone());

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
    let context = unsafe { &mut *(refcon as *mut WindowContext) };

    // Skip processing when suspended (sleep/lock)
    if context.is_suspended {
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
    let should_execute = context
        .throttle
        .last_execution
        .map(|last| now.duration_since(last) >= THROTTLE_DURATION)
        .unwrap_or(true);

    if should_execute {
        context.throttle.reset();
        let app = unsafe { AXUIElement::new_application(pid) };
        // AX notifications are unreliable, when new windows are being rapidly created and deleted,
        // macOS may decide skip sending notifications.
        // So we are basically polling as much as possible to keep the state in sync
        // https://github.com/nikitabobko/AeroSpace/issues/445
        sync_windows(pid, &app, context);
        if is_focus_change {
            sync_focus(&app, context);
        } else if let Err(e) = focus_window(context) {
            tracing::warn!("Failed to focus window: {e:#}");
        }

        if let Err(e) = apply_layout(context) {
            tracing::warn!("Failed to apply layout: {e:#}");
        }
    } else {
        context.throttle.pending_pids.insert(pid);
        if is_focus_change {
            context.throttle.pending_focus_sync = true;
        }
        if context.throttle.timer.is_none() {
            schedule_throttle_timer(context, THROTTLE_DURATION);
        }
    }
}

fn schedule_throttle_timer(context: &mut WindowContext, delay: Duration) {
    let context_ptr = context as *mut WindowContext;
    let fire_time = CFAbsoluteTimeGetCurrent() + delay.as_secs_f64();
    let mut timer_context = CFRunLoopTimerContext {
        version: 0,
        info: context_ptr as *mut std::ffi::c_void,
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
        context.throttle.timer = Some(timer);
    }
}

unsafe extern "C-unwind" fn throttle_timer_callback(
    _timer: *mut CFRunLoopTimer,
    info: *mut std::ffi::c_void,
) {
    // Similar to observer callback, we should not call render_workspace here, as this is just the
    // throttling version of observer callback
    let context = unsafe { &mut *(info as *mut WindowContext) };
    context.throttle.timer = None;

    // Skip processing when suspended (sleep/lock)
    if context.is_suspended {
        context.throttle.pending_pids.clear();
        context.throttle.pending_focus_sync = false;
        return;
    }

    context.throttle.last_execution = Some(Instant::now());

    let pids: Vec<_> = context.throttle.pending_pids.drain().collect();
    let pending_focus_sync = std::mem::take(&mut context.throttle.pending_focus_sync);

    for pid in pids {
        let app = unsafe { AXUIElement::new_application(pid) };
        sync_windows(pid, &app, context);
        if pending_focus_sync {
            sync_focus(&app, context);
        }
    }

    if let Err(e) = apply_layout(context) {
        tracing::warn!("Failed to apply layout: {e:#}");
    }

    if !pending_focus_sync && let Err(e) = focus_window(context) {
        tracing::warn!("Failed to focus window: {e:#}");
    }
}

#[tracing::instrument(skip_all, fields(app_name = get_app_name(pid)))]
fn sync_windows(pid: i32, app: &CFRetained<AXUIElement>, context: &mut WindowContext) {
    let Ok(windows) = get_windows(app) else {
        tracing::warn!("Failed to get windows");
        return;
    };
    let screen = context.hub.screen();
    let active_windows: Vec<_> = windows
        .into_iter()
        .filter_map(|w| MacWindow::new(w.clone(), app.clone(), pid, screen))
        .filter(|w| w.is_manageable())
        .collect();
    let active_hashes: Vec<_> = active_windows.iter().map(|w| w.cf_hash()).collect();

    let mut registry = context.registry.borrow_mut();
    let tracked_hashes = registry.hashes_for_pid(pid);

    for h in tracked_hashes {
        if !active_hashes.contains(&h) {
            match registry.remove_by_hash(h) {
                Some(RemovedWindow::Tiling(id, window)) => {
                    let title = window.title();
                    context.hub.delete_window(id);
                    tracing::info!(%id, %title, "Tiling window deleted");
                }
                Some(RemovedWindow::Float(id, window)) => {
                    let title = window.title();
                    context.hub.delete_float(id);
                    tracing::info!(%id, %title, "Float window deleted");
                }
                None => {}
            }
        } else {
            registry.update_title(h);
        }
    }

    for mac_window in active_windows {
        if registry.contains(&mac_window) {
            continue;
        }
        let title = mac_window.title();
        if mac_window.should_tile() {
            let id = context.hub.insert_tiling();
            tracing::info!(%id, %title, "New tiling window");
            registry.insert_tiling(id, mac_window);
        } else {
            let dim = mac_window.dimension();
            let id = context.hub.insert_float(dim);
            tracing::info!(%id, %title, "New float window");
            registry.insert_float(id, mac_window);
        }
    }
}

fn sync_focus(app: &CFRetained<AXUIElement>, context: &mut WindowContext) {
    let Ok(focused) = get_attribute::<AXUIElement>(app, &kAXFocusedWindowAttribute()) else {
        return;
    };
    let h = CFHash(Some(&focused));
    let registry = context.registry.borrow();
    if let Some(id) = registry.get_tiling_by_hash(h) {
        if !context.hub.is_focusing(Child::Window(id)) {
            let title = registry
                .get_tiling(id)
                .map(|w| w.title().to_owned())
                .unwrap_or_default();
            drop(registry);
            tracing::debug!(%id, %title, "Focus changed to tiling window");
            context.hub.set_focus(id);
        }
    } else if let Some(id) = registry.get_float_by_hash(h) {
        let title = registry
            .get_float(id)
            .map(|w| w.title().to_owned())
            .unwrap_or_default();
        drop(registry);
        tracing::debug!(%id, %title, "Focus changed to float window");
        context.hub.set_float_focus(id);
    }
}

unsafe extern "C-unwind" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    refcon: *mut std::ffi::c_void,
) -> *mut CGEvent {
    let context = unsafe { &mut *(refcon as *mut WindowContext) };
    let event = event.as_ptr();

    if event_type == CGEventType::TapDisabledByTimeout
        || event_type == CGEventType::TapDisabledByUserInput
    {
        if let Some(tap) = &context.event_tap {
            tracing::debug!("Event tap disabled, re-enabling");
            CGEvent::tap_enable(tap, true);
        }
    } else if event_type == CGEventType::KeyDown {
        if handle_keyboard(context, event) {
            return std::ptr::null_mut();
        }
    } else {
        tracing::warn!("Unrecognized event type: {:?}", event_type);
    }

    event
}

fn handle_keyboard(context: &mut WindowContext, event: *mut CGEvent) -> bool {
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
    let actions = context.config.get_actions(&keymap);

    if actions.is_empty() {
        return false;
    }

    // Event tap is disabled while locked.
    // If we receive hotkeys event, it must be unlocked
    if context.is_suspended {
        tracing::info!("Received keymap action, resuming window management");
        context.is_suspended = false;
    }

    for action in actions {
        if let Err(e) = execute_action(context, &action) {
            tracing::warn!("Failed to execute action: {e:#}");
        }
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

fn execute_action(context: &mut WindowContext, action: &Action) -> Result<()> {
    tracing::debug!(?action, "Executing action");
    match action {
        Action::Focus(target) => match target {
            FocusTarget::Up => context.hub.focus_up(),
            FocusTarget::Down => context.hub.focus_down(),
            FocusTarget::Left => context.hub.focus_left(),
            FocusTarget::Right => context.hub.focus_right(),
            FocusTarget::Parent => context.hub.focus_parent(),
            FocusTarget::Workspace(n) => context.hub.focus_workspace(*n),
            FocusTarget::NextTab => context.hub.focus_next_tab(),
            FocusTarget::PrevTab => context.hub.focus_prev_tab(),
        },
        Action::Move(target) => match target {
            MoveTarget::Workspace(n) => context.hub.move_focused_to_workspace(*n),
            MoveTarget::Up => context.hub.move_up(),
            MoveTarget::Down => context.hub.move_down(),
            MoveTarget::Left => context.hub.move_left(),
            MoveTarget::Right => context.hub.move_right(),
        },
        Action::Toggle(target) => match target {
            ToggleTarget::SpawnDirection => context.hub.toggle_spawn_direction(),
            ToggleTarget::Direction => context.hub.toggle_direction(),
            ToggleTarget::Layout => context.hub.toggle_container_layout(),
            ToggleTarget::Float => {
                if let Some((window_id, float_id)) = context.hub.toggle_float() {
                    context
                        .registry
                        .borrow_mut()
                        .toggle_float(window_id, float_id);
                }
            }
        },
    }

    if let Err(e) = render_workspace(context) {
        tracing::warn!("Failed to render workspace after action: {e:#}");
    }

    Ok(())
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

fn register_app(pid: i32, context_ptr: *mut WindowContext) -> Result<CFRetained<AXObserver>> {
    let context = unsafe { &mut *context_ptr };
    let screen = context.hub.screen();
    let app = unsafe { AXUIElement::new_application(pid) };

    if let Ok(windows) = get_windows(&app) {
        for window in windows {
            let Some(mac_window) = MacWindow::new(window.clone(), app.clone(), pid, screen) else {
                continue;
            };
            if mac_window.is_manageable() {
                if mac_window.should_tile() {
                    let window_id = context.hub.insert_tiling();
                    context
                        .registry
                        .borrow_mut()
                        .insert_tiling(window_id, mac_window);
                } else {
                    let dim = mac_window.dimension();
                    let float_id = context.hub.insert_float(dim);
                    context
                        .registry
                        .borrow_mut()
                        .insert_float(float_id, mac_window);
                }
            }
        }
    }

    let run_loop = CFRunLoop::current().unwrap();
    let observer = create_observer(pid, Some(observer_callback))?;
    let source = unsafe { observer.run_loop_source() };
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    let context_ptr = context_ptr as *mut std::ffi::c_void;
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
        add_observer_notification(&observer, &app, &notification, context_ptr)?;
    }

    Ok(observer)
}

pub(super) fn render_workspace(context: &mut WindowContext) -> Result<()> {
    apply_layout(context)?;
    focus_window(context)?;
    Ok(())
}

fn apply_layout(context: &mut WindowContext) -> Result<()> {
    let workspace_id = context.hub.current_workspace();
    let workspace = context.hub.get_workspace(workspace_id);
    let registry = context.registry.borrow();

    // Collect all windows that should be displayed with their dimensions
    let mut workspace_windows = HashSet::new();
    let mut tiling_layouts = Vec::new();
    let mut float_layouts = Vec::new();

    let mut stack: Vec<Child> = workspace.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(window_id) => {
                if let Some(os_window) = registry.get_tiling(window_id) {
                    workspace_windows.insert(os_window.cf_hash());
                    let dim = context.hub.get_window(window_id).dimension();
                    tiling_layouts.push((window_id, dim));
                }
            }
            Child::Container(container_id) => {
                let container = context.hub.get_container(container_id);
                if container.is_tabbed() {
                    if let Some(&active_child) = container.children().get(container.active_tab()) {
                        stack.push(active_child);
                    }
                } else {
                    for &c in container.children() {
                        stack.push(c);
                    }
                }
            }
        }
    }
    for &float_id in workspace.float_windows() {
        if let Some(os_window) = registry.get_float(float_id) {
            workspace_windows.insert(os_window.cf_hash());
            let dim = context.hub.get_float(float_id).dimension();
            float_layouts.push((float_id, dim));
        }
    }

    // Hide windows that shouldn't be displayed
    let to_hide: Vec<usize> = context
        .displayed_windows
        .difference(&workspace_windows)
        .copied()
        .collect();

    for cf_hash in to_hide {
        if let Some(window_id) = registry.get_tiling_by_hash(cf_hash) {
            if let Some(os_window) = registry.get_tiling(window_id)
                && let Err(e) = os_window.hide()
            {
                tracing::warn!("Failed to hide tiling window {window_id}: {e:#}");
            }
        } else if let Some(float_id) = registry.get_float_by_hash(cf_hash)
            && let Some(os_window) = registry.get_float(float_id)
            && let Err(e) = os_window.hide()
        {
            tracing::warn!("Failed to hide float window {float_id}: {e:#}");
        }
    }

    // Apply layout
    for (window_id, dim) in tiling_layouts {
        if let Some(os_window) = registry.get_tiling(window_id) {
            os_window.set_dimension(dim)?;
        }
    }
    for (float_id, dim) in float_layouts {
        if let Some(os_window) = registry.get_float(float_id) {
            os_window.set_dimension(dim)?;
        }
    }

    let overlays = collect_overlays(&context.hub, &context.config, workspace_id, &registry);

    context
        .tiling_overlay
        .set_rects(overlays.tiling_rects, overlays.tiling_labels);
    context
        .float_overlay
        .set_rects(overlays.float_rects, vec![]);

    // Update displayed set
    context.displayed_windows = workspace_windows;

    Ok(())
}

fn focus_window(context: &WindowContext) -> Result<()> {
    let workspace_id = context.hub.current_workspace();
    let workspace = context.hub.get_workspace(workspace_id);

    match workspace.focused() {
        Some(Focus::Tiling(Child::Window(window_id))) => {
            if let Some(os_window) = context.registry.borrow().get_tiling(window_id) {
                os_window.focus()?;
            }
        }
        Some(Focus::Float(float_id)) => {
            if let Some(os_window) = context.registry.borrow().get_float(float_id) {
                os_window.focus()?;
            }
        }
        _ => {}
    }

    Ok(())
}
