use std::{cell::RefCell, collections::HashMap, ptr::NonNull, rc::Rc};

use anyhow::Result;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
    NSWorkspaceDidActivateApplicationNotification, NSWorkspaceDidLaunchApplicationNotification,
    NSWorkspaceDidTerminateApplicationNotification,
};
use objc2_application_services::{AXObserver, AXUIElement};
use objc2_core_foundation::{
    CFArray, CFHash, CFMachPort, CFRetained, CFRunLoop, CFString, kCFAllocatorDefault,
    kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType,
};
use objc2_foundation::{NSNotification, NSOperationQueue};

use super::context::{Observers, WindowContext};
use super::objc2_wrapper::{
    add_observer_notification, create_observer, get_attribute, get_pid, kAXFocusedWindowAttribute,
    kAXFocusedWindowChangedNotification, kAXResizedNotification, kAXUIElementDestroyedNotification,
    kAXWindowCreatedNotification, kAXWindowMiniaturizedNotification, kAXWindowsAttribute,
};
use super::overlay::collect_overlays;
use super::window::MacWindow;
use crate::config::{Action, FocusTarget, Keymap, Modifiers, MoveTarget, ToggleTarget};
use crate::core::{Child, Focus, WorkspaceId};

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
                tracing::info!("Can't create observer for application {pid}: {e:#}");
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
                tracing::trace!("Received notification for launching app with pid: {pid:?}");
                let observer = match register_app(pid, context_ptr) {
                    Ok(observer) => observer,
                    Err(e) => {
                        tracing::info!("Can't track application {pid}: {e:#}");
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
                tracing::trace!("Received notification for terminating app with pid: {pid:?}");
                apps.borrow_mut().remove(&pid);
                let context = &mut *context_ptr;
                let (window_ids, float_ids) = context.registry.borrow_mut().remove_by_pid(pid);
                for window_id in &window_ids {
                    context.hub.delete_window(*window_id);
                    tracing::debug!("Window deleted: {window_id}");
                }
                for float_id in &float_ids {
                    context.hub.delete_float(*float_id);
                    tracing::debug!("Float deleted: {float_id}");
                }
                if !window_ids.is_empty() || !float_ids.is_empty() {
                    if let Err(e) = render_workspace(context) {
                        tracing::warn!("Failed to render workspace after terminating app: {e:#}");
                    }
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
                tracing::trace!("App activated with pid: {pid}");
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
                        let old_workspace = context.hub.current_workspace();
                        context.hub.set_focus(window_id);
                        if old_workspace != context.hub.current_workspace() {
                            hide_workspace(context, old_workspace);
                            if let Err(e) = render_workspace(context) {
                                tracing::warn!("Failed to render workspace: {e:#}");
                            }
                        }
                        update_overlay(context);
                        tracing::debug!("Focus changed to tiling window: {window_id}");
                    }
                } else if let Some(float_id) = registry.get_float_by_hash(cf_hash) {
                    drop(registry);
                    let old_workspace = context.hub.current_workspace();
                    context.hub.set_float_focus(float_id);
                    if old_workspace != context.hub.current_workspace() {
                        hide_workspace(context, old_workspace);
                        if let Err(e) = render_workspace(context) {
                            tracing::warn!("Failed to render workspace: {e:#}");
                        }
                    }
                    update_overlay(context);
                    tracing::debug!("Focus changed to float window: {float_id}");
                }
            }),
        );
    }

    apps
}

pub(super) fn listen_to_input_devices(context_ptr: *mut WindowContext) -> Result<()> {
    let run_loop = CFRunLoop::current().unwrap();
    let event_mask = (1u64 << CGEventType::KeyDown.0) | (1u64 << CGEventType::LeftMouseDown.0);
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
    let notification = unsafe { &*notification.as_ptr() };
    let context = unsafe { &mut *(refcon as *mut WindowContext) };
    let element = unsafe { CFRetained::retain(element) };
    if notification.to_string() == *"AXWindowCreated" {
        match get_pid(&element) {
            Ok(pid) => {
                let app = unsafe { AXUIElement::new_application(pid) };
                let screen = context.hub.screen();
                let window = MacWindow::new(element.clone(), app, pid, screen);
                if !window.is_manageable() {
                    return;
                }
                if context.registry.borrow().contains(&window) {
                    return;
                }
                tracing::debug!("New window created: {window}",);
                if window.should_tile() {
                    let window_id = context.hub.insert_tiling(window.title());
                    context
                        .registry
                        .borrow_mut()
                        .insert_tiling(window_id, window);
                } else {
                    let dim = window.dimension();
                    let float_id = context.hub.insert_float(dim, window.title());
                    context.registry.borrow_mut().insert_float(float_id, window);
                }
                if let Err(e) = render_workspace(context) {
                    tracing::warn!("Failed to render workspace after window insert: {e:#}");
                }
            }
            Err(e) => {
                tracing::trace!("Failed to get PID for window: {e:#}");
            }
        }
    } else if notification.to_string() == *"AXUIElementDestroyed" {
        let cf_hash = CFHash(Some(&element));
        let removed_tiling = context.registry.borrow_mut().remove_tiling_by_hash(cf_hash);
        let removed_float = if removed_tiling.is_none() {
            context.registry.borrow_mut().remove_float_by_hash(cf_hash)
        } else {
            None
        };
        if let Some(window_id) = removed_tiling {
            context.hub.delete_window(window_id);
            tracing::info!("Window deleted: {window_id}");
        } else if let Some(float_id) = removed_float {
            context.hub.delete_float(float_id);
            tracing::info!("Float window deleted: {float_id}");
        }
        if removed_tiling.is_some() || removed_float.is_some() {
            if let Err(e) = render_workspace(context) {
                tracing::warn!("Failed to render workspace after deleting window: {e:#}");
            }
        }
    } else if notification.to_string() == *"AXFocusedWindowChanged" {
        tracing::info!("App  focus changed");
        let Ok(pid) = get_pid(&element) else {
            return;
        };
        let app = unsafe { AXUIElement::new_application(pid) };
        tracing::info!("App {pid}'s focus changed");
        let Ok(focused_window) = get_attribute::<AXUIElement>(&app, &kAXFocusedWindowAttribute())
        else {
            return;
        };
        let cf_hash = CFHash(Some(&focused_window));
        let registry = context.registry.borrow();
        if let Some(window_id) = registry.get_tiling_by_hash(cf_hash) {
            if !context.hub.is_focusing(Child::Window(window_id)) {
                drop(registry);
                context.hub.set_focus(window_id);
                update_overlay(context);
                tracing::debug!("Focus changed to tiling window: {window_id}");
            }
        } else if let Some(float_id) = registry.get_float_by_hash(cf_hash) {
            drop(registry);
            context.hub.set_float_focus(float_id);
            update_overlay(context);
            tracing::debug!("Focus changed to float window: {float_id}");
        }
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
    } else if event_type == CGEventType::LeftMouseDown {
        handle_mouse_down(context, event);
    } else if event_type == CGEventType::KeyDown {
        if handle_keyboard(context, event) {
            return std::ptr::null_mut();
        }
    } else {
        tracing::warn!("Unrecognized event type: {:?}", event_type);
    }

    event
}

fn handle_mouse_down(context: &mut WindowContext, event: *mut CGEvent) {
    let location = CGEvent::location(Some(unsafe { &*event }));
    let screen = context.hub.screen();
    let x = location.x as f32;
    let y = screen.y + location.y as f32;
    tracing::trace!(
        "Mouse down at ({}, {}) -> hub ({}, {})",
        location.x,
        location.y,
        x,
        y
    );
    if let Some(window_id) = context.hub.window_at(x, y) {
        if !context.hub.is_focusing(Child::Window(window_id)) {
            tracing::info!("Mouse click focused {:?}", window_id);
            context.hub.set_focus(window_id);
            update_overlay(context);
        }
    } else {
        tracing::debug!("No window at ({}, {})", x, y);
    }
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

    tracing::trace!("Keypress: {keymap:?}, actions: {actions:?}");

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
    match action {
        Action::Focus(target) => match target {
            FocusTarget::Up => context.hub.focus_up(),
            FocusTarget::Down => context.hub.focus_down(),
            FocusTarget::Left => context.hub.focus_left(),
            FocusTarget::Right => context.hub.focus_right(),
            FocusTarget::Parent => context.hub.focus_parent(),
            FocusTarget::Workspace(n) => return focus_workspace(context, *n),
            FocusTarget::NextTab => context.hub.focus_next_tab(),
            FocusTarget::PrevTab => context.hub.focus_prev_tab(),
        },
        Action::Move(target) => match target {
            MoveTarget::Workspace(n) => return move_to_workspace(context, *n),
            MoveTarget::Up => context.hub.move_up(),
            MoveTarget::Down => context.hub.move_down(),
            MoveTarget::Left => context.hub.move_left(),
            MoveTarget::Right => context.hub.move_right(),
        },
        Action::Toggle(target) => match target {
            ToggleTarget::SpawnDirection => context.hub.toggle_spawn_direction(),
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
            let mac_window = MacWindow::new(window.clone(), app.clone(), pid, screen);
            if mac_window.is_manageable() {
                if mac_window.should_tile() {
                    let window_id = context.hub.insert_tiling(mac_window.title());
                    context
                        .registry
                        .borrow_mut()
                        .insert_tiling(window_id, mac_window);
                } else {
                    let dim = mac_window.dimension();
                    let float_id = context.hub.insert_float(dim, mac_window.title());
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
        kAXResizedNotification(),
        kAXUIElementDestroyedNotification(),
        kAXFocusedWindowChangedNotification(),
    ] {
        add_observer_notification(&observer, &app, &notification, context_ptr)?;
    }

    Ok(observer)
}

fn update_overlay(context: &WindowContext) {
    let workspace_id = context.hub.current_workspace();
    let overlays = collect_overlays(&context.hub, &context.config, workspace_id);
    context
        .tiling_overlay
        .set_rects(overlays.tiling_rects, overlays.tiling_labels);
    context
        .float_overlay
        .set_rects(overlays.float_rects, vec![]);
}

pub(super) fn render_workspace(context: &WindowContext) -> Result<()> {
    let workspace_id = context.hub.current_workspace();
    let workspace = context.hub.get_workspace(workspace_id);

    let mut stack: Vec<Child> = workspace.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(window_id) => {
                if let Some(os_window) = context.registry.borrow().get_tiling(window_id) {
                    let dim = context.hub.get_window(window_id).dimension();
                    os_window.set_dimension(dim)?;
                }
            }
            Child::Container(container_id) => {
                for &c in context.hub.get_container(container_id).children() {
                    stack.push(c);
                }
            }
        }
    }

    for &float_id in workspace.float_windows() {
        if let Some(os_window) = context.registry.borrow().get_float(float_id) {
            let dim = context.hub.get_float(float_id).dimension();
            os_window.set_dimension(dim)?;
        }
    }

    let overlays = collect_overlays(&context.hub, &context.config, workspace_id);
    context
        .tiling_overlay
        .set_rects(overlays.tiling_rects, overlays.tiling_labels);
    context
        .float_overlay
        .set_rects(overlays.float_rects, vec![]);

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

fn hide_workspace(context: &WindowContext, workspace_id: WorkspaceId) {
    let workspace = context.hub.get_workspace(workspace_id);
    let mut stack: Vec<Child> = workspace.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(window_id) => {
                if let Some(os_window) = context.registry.borrow().get_tiling(window_id) {
                    let _ = os_window.hide();
                }
            }
            Child::Container(container_id) => {
                for &c in context.hub.get_container(container_id).children() {
                    stack.push(c);
                }
            }
        }
    }
    for &float_id in workspace.float_windows() {
        if let Some(os_window) = context.registry.borrow().get_float(float_id) {
            let _ = os_window.hide();
        }
    }
}

fn focus_workspace(context: &mut WindowContext, name: usize) -> Result<()> {
    let old_workspace_id = context.hub.current_workspace();
    context.hub.focus_workspace(name);
    let new_workspace_id = context.hub.current_workspace();
    if old_workspace_id == new_workspace_id {
        return Ok(());
    }

    hide_workspace(context, old_workspace_id);
    render_workspace(context)
}

fn move_to_workspace(context: &mut WindowContext, name: usize) -> Result<()> {
    match context.hub.move_focused_to_workspace(name) {
        Some(Focus::Tiling(Child::Window(window_id))) => {
            if let Some(os_window) = context.registry.borrow().get_tiling(window_id) {
                os_window.hide()?;
            }
        }
        Some(Focus::Tiling(Child::Container(container_id))) => {
            let mut stack = vec![Child::Container(container_id)];
            while let Some(child) = stack.pop() {
                match child {
                    Child::Window(window_id) => {
                        if let Some(os_window) = context.registry.borrow().get_tiling(window_id) {
                            os_window.hide()?;
                        }
                    }
                    Child::Container(cid) => {
                        for &c in context.hub.get_container(cid).children() {
                            stack.push(c);
                        }
                    }
                }
            }
        }
        Some(Focus::Float(float_id)) => {
            if let Some(os_window) = context.registry.borrow().get_float(float_id) {
                os_window.hide()?;
            }
        }
        None => {}
    }
    render_workspace(context)
}
