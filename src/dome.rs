use std::{
    cell::RefCell,
    collections::HashMap,
    ptr::NonNull,
    rc::Rc,
};

use anyhow::Result;

use block2::RcBlock;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSBezierPath, NSColor, NSEvent, NSNormalWindowLevel, NSResponder, NSRunningApplication,
    NSScreen, NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask, NSWorkspace,
    NSWorkspaceApplicationKey, NSWorkspaceDidLaunchApplicationNotification,
    NSWorkspaceDidTerminateApplicationNotification,
};
use objc2_application_services::{AXIsProcessTrustedWithOptions, AXObserver, AXUIElement};
use objc2_core_foundation::{
    CFArray, CFHash, CFMachPort, CFRetained, CFRunLoop, CFString, CGFloat, kCFAllocatorDefault,
    kCFBooleanTrue, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSOperationQueue, NSPoint, NSRect, NSSize,
};

use crate::{
    config::{Action, Color, Config, FocusTarget, Keymap, Modifiers, MoveTarget, ToggleTarget},
    objc2_wrapper::{
        add_observer_notification, create_observer, get_attribute, get_pid, kAXMinimizedAttribute,
        kAXRoleAttribute, kAXStandardWindowSubrole, kAXSubroleAttribute,
        kAXWindowCreatedNotification, kAXWindowMiniaturizedNotification, kAXWindowRole,
        kAXWindowsAttribute,
    },
};
use crate::{
    core::{Child, Dimension, Direction, Hub, WindowId, WorkspaceId},
    objc2_wrapper::kAXResizedNotification,
};
use crate::{objc2_wrapper::kAXUIElementDestroyedNotification, window::MacWindow};

pub fn run_app() {
    let mtm = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let delegate = AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    app.run();
}

pub fn check_accessibility() {
    use objc2_application_services::kAXTrustedCheckOptionPrompt;
    use objc2_core_foundation::CFDictionary;

    tracing::debug!("Accessibility: {}", unsafe {
        AXIsProcessTrustedWithOptions(Some(
            CFDictionary::from_slices(&[kAXTrustedCheckOptionPrompt], &[kCFBooleanTrue.unwrap()])
                .as_opaque(),
        ))
    });
}

#[derive(Default)]
struct AppDelegateIvars {
    context: std::cell::OnceCell<*mut WindowContext>,
    observers: std::cell::OnceCell<Observers>,
    overlay_window: std::cell::OnceCell<Retained<NSWindow>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppDelegateIvars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            tracing::info!("Application did finish launching");
            let mtm = self.mtm();

            let screen = get_main_screen();
            let frame = NSRect::new(
                NSPoint::new(screen.x as f64, 0.0),
                NSSize::new(screen.width as f64, screen.height as f64),
            );

            let overlay_window = unsafe {
                NSWindow::initWithContentRect_styleMask_backing_defer(
                    NSWindow::alloc(mtm),
                    frame,
                    NSWindowStyleMask::Borderless,
                    NSBackingStoreType::Buffered,
                    false,
                )
            };

            overlay_window.setBackgroundColor(Some(&NSColor::clearColor()));
            overlay_window.setOpaque(false);
            overlay_window.setLevel(NSNormalWindowLevel - 1);
            overlay_window.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::Stationary,
            );
            unsafe { overlay_window.setReleasedWhenClosed(false) };

            let overlay_view = OverlayView::new(mtm, frame);
            overlay_window.setContentView(Some(&overlay_view));
            overlay_window.makeKeyAndOrderFront(None);

            let pids = list_apps();
            let context = WindowContext::new(&pids, overlay_view);

            let workspace_id = context.hub.current_workspace();
            if let Err(e) = render_workspace(&context, workspace_id) {
                tracing::warn!("Failed to render workspace after initialization: {e:#}");
            }

            let context = Box::new(context);
            let context_ptr = Box::into_raw(context);

            if let Err(e) = listen_to_keyboard(context_ptr) {
                tracing::error!("Failed to setup keyboard listener: {e:#}");
            }

            let mut observers = HashMap::new();
            for pid in pids {
                match register_observer(pid, context_ptr) {
                    Ok(observer) => {
                        observers.insert(pid, observer);
                    }
                    Err(e) => {
                        tracing::info!("Can't create observer for application {pid}: {e:#}");
                    }
                }
            }

            let apps: Observers = Rc::new(RefCell::new(observers));
            listen_to_launching_app(context_ptr, apps.clone());
            listen_to_terminating_app(context_ptr, apps.clone());

            self.ivars().context.set(context_ptr).unwrap();
            self.ivars().observers.set(apps).unwrap();
            self.ivars().overlay_window.set(overlay_window).unwrap();
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            if let Some(&context_ptr) = self.ivars().context.get() {
                let _ = unsafe { Box::from_raw(context_ptr) };
            }
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppDelegateIvars::default());
        unsafe { msg_send![super(this), init] }
    }
}

// OverlayView for drawing borders
#[derive(Default)]
struct OverlayViewIvars {
    rects: RefCell<Vec<OverlayRect>>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = OverlayViewIvars]
    struct OverlayView;

    unsafe impl NSObjectProtocol for OverlayView {}

    impl OverlayView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            for rect in self.ivars().rects.borrow().iter() {
                let color = NSColor::colorWithSRGBRed_green_blue_alpha(
                    rect.color.r as CGFloat, rect.color.g as CGFloat, rect.color.b as CGFloat, rect.color.a as CGFloat,
                );
                color.setFill();
                NSBezierPath::fillRect(NSRect::new(
                    NSPoint::new(rect.x as CGFloat, rect.y as CGFloat),
                    NSSize::new(rect.width as CGFloat, rect.height as CGFloat),
                ));
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let location = event.locationInWindow();
            tracing::debug!("Overlay clicked at: ({}, {})", location.x, location.y);
        }
    }
);

impl OverlayView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(OverlayViewIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn set_rects(&self, rects: Vec<OverlayRect>) {
        *self.ivars().rects.borrow_mut() = rects;
        self.setNeedsDisplay(true);
    }
}

#[derive(Clone)]
struct OverlayRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Color,
}

struct WindowRegistry {
    pid_to_hashes: HashMap<i32, Vec<usize>>,
    hash_to_pid: HashMap<usize, i32>,
    hash_to_id: HashMap<usize, WindowId>,
    id_to_hash: HashMap<WindowId, usize>,
    id_to_window: HashMap<WindowId, MacWindow>,
}

impl WindowRegistry {
    fn new() -> Self {
        Self {
            pid_to_hashes: HashMap::new(),
            hash_to_pid: HashMap::new(),
            hash_to_id: HashMap::new(),
            id_to_hash: HashMap::new(),
            id_to_window: HashMap::new(),
        }
    }

    fn insert(&mut self, window_id: WindowId, window: MacWindow) {
        let cf_hash = window.cf_hash();
        let pid = window.pid();
        self.pid_to_hashes.entry(pid).or_default().push(cf_hash);
        self.hash_to_pid.insert(cf_hash, pid);
        self.hash_to_id.insert(cf_hash, window_id);
        self.id_to_hash.insert(window_id, cf_hash);
        self.id_to_window.insert(window_id, window);
    }

    fn remove_by_hash(&mut self, cf_hash: usize) -> Option<WindowId> {
        let window_id = self.hash_to_id.remove(&cf_hash)?;
        self.id_to_hash.remove(&window_id);
        self.id_to_window.remove(&window_id);
        if let Some(pid) = self.hash_to_pid.remove(&cf_hash)
            && let Some(hashes) = self.pid_to_hashes.get_mut(&pid)
        {
            hashes.retain(|&h| h != cf_hash);
        }
        Some(window_id)
    }

    fn remove_by_pid(&mut self, pid: i32) -> Vec<WindowId> {
        let Some(hashes) = self.pid_to_hashes.remove(&pid) else {
            return Vec::new();
        };
        let mut removed = Vec::new();
        for cf_hash in hashes {
            self.hash_to_pid.remove(&cf_hash);
            if let Some(window_id) = self.hash_to_id.remove(&cf_hash) {
                self.id_to_hash.remove(&window_id);
                self.id_to_window.remove(&window_id);
                removed.push(window_id);
            }
        }
        removed
    }

    fn contains_hash(&self, cf_hash: usize) -> bool {
        self.hash_to_id.contains_key(&cf_hash)
    }

    fn get(&self, window_id: WindowId) -> Option<&MacWindow> {
        self.id_to_window.get(&window_id)
    }
}

struct WindowContext {
    hub: Hub,
    overlay_view: Retained<OverlayView>,
    registry: RefCell<WindowRegistry>,
    config: Config,
    event_tap: Option<CFRetained<CFMachPort>>,
}

impl WindowContext {
    fn new(pids: &[i32], overlay_view: Retained<OverlayView>) -> Self {
        let config = Config::load();

        let screen = get_main_screen();
        tracing::info!("Detected Screen {screen:?}");
        let mut hub = Hub::new(screen, config.border_size);
        let mut registry = WindowRegistry::new();

        for pid in pids.iter() {
            let app = unsafe { AXUIElement::new_application(*pid) };

            let windows = match get_windows(&app) {
                Ok(windows) => windows,
                Err(e) => {
                    tracing::info!("{e:#}");
                    continue;
                }
            };
            for window in windows {
                // TODO: don't tile window, but still manage it as floating
                if is_standard_window(&window) {
                    let window_id = hub.insert_window();
                    let mac_window = MacWindow::new(window.clone(), app.clone(), *pid);
                    registry.insert(window_id, mac_window);
                }
            }
        }

        Self {
            hub,
            overlay_view,
            registry: RefCell::new(registry),
            config,
            event_tap: None,
        }
    }
}

fn list_apps() -> Vec<i32> {
    let mut apps = Vec::new();

    for app in NSWorkspace::sharedWorkspace().runningApplications() {
        if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
            continue;
        }
        let pid = app.processIdentifier();
        // Some applications may not have any PID, for some reasons
        if pid == -1 {
            continue;
        }
        apps.push(pid);
    }
    apps
}

fn get_main_screen() -> Dimension {
    let mtm = MainThreadMarker::new().unwrap();
    let main_screen = NSScreen::mainScreen(mtm).unwrap();
    let frame = main_screen.frame();
    let visible_frame = main_screen.visibleFrame();
    Dimension {
        x: visible_frame.origin.x as f32,
        // Reason: NSScreen returns bottom-left coordinate instead of the usual top left
        // Then subtract to exclude the menu bar
        y: (frame.size.height - visible_frame.size.height) as f32,
        width: visible_frame.size.width as f32,
        height: visible_frame.size.height as f32,
    }
}

fn register_observer(pid: i32, context_ptr: *mut WindowContext) -> Result<CFRetained<AXObserver>> {
    let run_loop = CFRunLoop::current().unwrap();
    let observer = create_observer(pid, Some(observer_callback))?;
    let source = unsafe { observer.run_loop_source() };

    // Swift docs func CFRunLoopGetCurrent() -> CFRunLoop!
    // So it's not nullable
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    let app = unsafe { AXUIElement::new_application(pid) };
    let context_ptr = context_ptr as *mut std::ffi::c_void;
    add_observer_notification(
        &observer,
        &app,
        &kAXWindowCreatedNotification(),
        context_ptr,
    )?;
    add_observer_notification(
        &observer,
        &app,
        &kAXWindowMiniaturizedNotification(),
        context_ptr,
    )?;
    add_observer_notification(&observer, &app, &kAXResizedNotification(), context_ptr)?;
    add_observer_notification(
        &observer,
        &app,
        &kAXUIElementDestroyedNotification(),
        context_ptr,
    )?;

    Ok(observer)
}

// AXObserver callback
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
    if notification.to_string() == *"AXWindowCreated" && is_standard_window(&element) {
        let cf_hash = CFHash(Some(&element));
        if context.registry.borrow().contains_hash(cf_hash) {
            return;
        }
        match get_pid(&element) {
            Ok(pid) => {
                let app = unsafe { AXUIElement::new_application(pid) };
                let window = MacWindow::new(element.clone(), app, pid);
                tracing::debug!("New window created: {window}",);
                let window_id = context.hub.insert_window();
                context.registry.borrow_mut().insert(window_id, window);
                // Render the entire workspace since insert_window may have resized other windows
                let workspace_id = context.hub.current_workspace();
                if let Err(e) = render_workspace(context, workspace_id) {
                    tracing::warn!("Failed to render workspace after window insert: {e:#}");
                }
            }
            Err(e) => {
                tracing::trace!("Failed to get PID for window: {e:#}");
            }
        }
    } else if notification.to_string() == *"AXUIElementDestroyed" {
        let cf_hash = CFHash(Some(&element));
        let removed = context.registry.borrow_mut().remove_by_hash(cf_hash);
        if let Some(window_id) = removed {
            let workspace_id = context.hub.delete_window(window_id);
            tracing::info!("Window deleted: {window_id}");
            if workspace_id == context.hub.current_workspace()
                && let Err(e) = render_workspace(context, workspace_id)
            {
                tracing::warn!("Failed to render workspace after deleting window: {e:#}");
            }
        }
    }
}

fn get_windows(app: &AXUIElement) -> Result<CFRetained<CFArray<AXUIElement>>> {
    // TODO: log more info to know what app is this
    get_attribute(app, &kAXWindowsAttribute())
}

fn is_minimized(window: &AXUIElement) -> bool {
    get_attribute::<objc2_core_foundation::CFBoolean>(window, &kAXMinimizedAttribute())
        .map(|b| b.as_bool())
        .unwrap_or(false)
}

fn is_standard_window(window: &AXUIElement) -> bool {
    let role: CFRetained<CFString> = match get_attribute(window, &kAXRoleAttribute()) {
        Ok(role) => role,
        Err(e) => {
            tracing::trace!("Can't get role for window {window:?}: {e:#}");
            return false;
        }
    };

    let subrole: CFRetained<CFString> = match get_attribute(window, &kAXSubroleAttribute()) {
        Ok(role) => role,
        Err(e) => {
            tracing::trace!("Can't get subrole for window {window:?}: {e:#}");
            return false;
        }
    };

    role == kAXWindowRole() && subrole == kAXStandardWindowSubrole() && !is_minimized(window)
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
        if context
            .hub
            .get_workspace(context.hub.current_workspace())
            .focused()
            != Some(Child::Window(window_id))
        {
            tracing::info!("Mouse click focused {:?}", window_id);
            context.hub.set_focus(window_id);
            // Don't need to focus window as it should already be focused by the act of clicking
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
        },
        Action::Move(target) => match target {
            MoveTarget::Workspace(n) => return move_to_workspace(context, *n),
            MoveTarget::Up => context.hub.move_up(),
            MoveTarget::Down => context.hub.move_down(),
            MoveTarget::Left => context.hub.move_left(),
            MoveTarget::Right => context.hub.move_right(),
        },
        Action::Toggle(target) => match target {
            ToggleTarget::Direction => context.hub.toggle_new_window_direction(),
        },
    }

    let workspace_id = context.hub.current_workspace();
    if let Err(e) = render_workspace(context, workspace_id) {
        tracing::warn!("Failed to render workspace after action: {e:#}");
    }

    Ok(())
}

fn listen_to_launching_app(
    context_ptr: *mut WindowContext,
    apps: Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>,
) {
    let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();
    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidLaunchApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                let Some(pid) = get_pid_from_notification(notification) else {
                    tracing::trace!("Launched application doesn't have a pid");
                    return;
                };

                tracing::trace!("Received notification for launching app with pid: {pid:?}",);
                let observer = match register_observer(pid, context_ptr) {
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
                        tracing::debug!("{e:#}");
                        return;
                    }
                };
                for window in windows {
                    if is_standard_window(&window) {
                        let window_id = context.hub.insert_window();
                        let mac_window = MacWindow::new(window.clone(), app.clone(), pid);
                        context.registry.borrow_mut().insert(window_id, mac_window);
                    }
                }
                // Render the entire workspace after inserting all windows
                let workspace_id = context.hub.current_workspace();
                if let Err(e) = render_workspace(context, workspace_id) {
                    tracing::warn!("Failed to render workspace after app launch: {e:#}");
                }

                apps.borrow_mut().insert(pid, observer);
            }),
        );
    };
}

fn listen_to_terminating_app(
    context_ptr: *mut WindowContext,
    apps: Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>,
) {
    let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();
    unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidTerminateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                let Some(pid) = get_pid_from_notification(notification) else {
                    tracing::trace!("Launched application doesn't have a pid");
                    return;
                };
                tracing::trace!("Received notification for terminating app with pid: {pid:?}",);
                apps.borrow_mut().remove(&pid);
                let context = &mut *context_ptr;
                let window_ids = context.registry.borrow_mut().remove_by_pid(pid);
                for window_id in window_ids {
                    context.hub.delete_window(window_id);
                    tracing::debug!("Window deleted: {window_id}");
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

fn update_overlay(context: &WindowContext) {
    let workspace_id = context.hub.current_workspace();
    if let Some(root) = context.hub.get_workspace(workspace_id).root() {
        let mut rects = Vec::new();
        collect_border_rects(context, root, &mut rects);
        collect_focused_border_rects(context, &mut rects);
        context.overlay_view.set_rects(rects);
    }
}

fn render_workspace(context: &WindowContext, workspace_id: WorkspaceId) -> Result<()> {
    if let Some(root) = context.hub.get_workspace(workspace_id).root() {
        render_child(context, root)?;

        // Update overlay with border rects
        let mut rects = Vec::new();
        collect_border_rects(context, root, &mut rects);
        collect_focused_border_rects(context, &mut rects);
        context.overlay_view.set_rects(rects);

        // Focus the currently focused window in this workspace
        if let Some(focused) = context.hub.get_workspace(workspace_id).focused()
            && let Child::Window(window_id) = focused
            && let Some(os_window) = context.registry.borrow().get(window_id)
            && let Err(e) = os_window.focus()
        {
            tracing::warn!("Failed to focus window {window_id:?}: {e:#}");
        }
    } else {
        // No windows, focus overlay to keep keyboard events working
        context.overlay_view.set_rects(Vec::new());
    }
    Ok(())
}

fn collect_focused_border_rects(context: &WindowContext, rects: &mut Vec<OverlayRect>) {
    let workspace = context.hub.get_workspace(context.hub.current_workspace());
    let Some(focused) = workspace.focused() else {
        return;
    };
    let border_size = context.config.border_size;
    let color = context.config.focused_color.clone();
    let spawn_color = context.config.spawn_indicator_color.clone();
    let screen = context.hub.screen();

    match focused {
        Child::Window(window_id) => {
            let window = context.hub.get_window(window_id);
            let dim = window.dimension();
            let direction = window.new_window_direction();
            let y = screen.y + screen.height - dim.y - dim.height;
            // Top
            rects.push(OverlayRect {
                x: dim.x - border_size,
                y: y + dim.height,
                width: dim.width + border_size * 2.0,
                height: border_size,
                color: color.clone(),
            });
            // Bottom (spawn indicator if Vertical)
            rects.push(OverlayRect {
                x: dim.x - border_size,
                y: y - border_size,
                width: dim.width + border_size * 2.0,
                height: border_size,
                color: if direction == Direction::Vertical { spawn_color.clone() } else { color.clone() },
            });
            // Left
            rects.push(OverlayRect {
                x: dim.x - border_size,
                y,
                width: border_size,
                height: dim.height,
                color: color.clone(),
            });
            // Right (spawn indicator if Horizontal)
            rects.push(OverlayRect {
                x: dim.x + dim.width,
                y,
                width: border_size,
                height: dim.height,
                color: if direction == Direction::Horizontal { spawn_color } else { color },
            });
        }
        Child::Container(container_id) => {
            let container = context.hub.get_container(container_id);
            let dim = container.dimension();
            let direction = container.new_window_direction();
            let y = screen.y + screen.height - dim.y - dim.height;
            // Top
            rects.push(OverlayRect {
                x: dim.x,
                y: y + dim.height - border_size,
                width: dim.width,
                height: border_size,
                color: color.clone(),
            });
            // Bottom (spawn indicator if Vertical)
            rects.push(OverlayRect {
                x: dim.x,
                y,
                width: dim.width,
                height: border_size,
                color: if direction == Direction::Vertical { spawn_color.clone() } else { color.clone() },
            });
            // Left
            rects.push(OverlayRect {
                x: dim.x,
                y: y + border_size,
                width: border_size,
                height: dim.height - 2.0 * border_size,
                color: color.clone(),
            });
            // Right (spawn indicator if Horizontal)
            rects.push(OverlayRect {
                x: dim.x + dim.width - border_size,
                y: y + border_size,
                width: border_size,
                height: dim.height - 2.0 * border_size,
                color: if direction == Direction::Horizontal { spawn_color } else { color },
            });
        }
    }
}

fn collect_border_rects(context: &WindowContext, child: Child, rects: &mut Vec<OverlayRect>) {
    let workspace = context.hub.get_workspace(context.hub.current_workspace());
    let focused = workspace.focused();

    match child {
        Child::Window(window_id) => {
            if focused == Some(Child::Window(window_id)) {
                return;
            }
            let dim = context.hub.get_window(window_id).dimension();
            let screen = context.hub.screen();
            let y = screen.y + screen.height - dim.y - dim.height;
            let border_size = context.config.border_size;
            let color = context.config.border_color.clone();

            rects.push(OverlayRect {
                x: dim.x - border_size,
                y: y + dim.height,
                width: dim.width + border_size * 2.0,
                height: border_size,
                color: color.clone(),
            });
            rects.push(OverlayRect {
                x: dim.x - border_size,
                y: y - border_size,
                width: dim.width + border_size * 2.0,
                height: border_size,
                color: color.clone(),
            });
            rects.push(OverlayRect {
                x: dim.x - border_size,
                y,
                width: border_size,
                height: dim.height,
                color: color.clone(),
            });
            rects.push(OverlayRect {
                x: dim.x + dim.width,
                y,
                width: border_size,
                height: dim.height,
                color,
            });
        }
        Child::Container(container_id) => {
            for child in context.hub.get_container(container_id).children() {
                collect_border_rects(context, *child, rects);
            }
        }
    }
}

fn render_child(context: &WindowContext, child: Child) -> Result<()> {
    match child {
        Child::Window(window_id) => {
            if let Some(os_window) = context.registry.borrow().get(window_id) {
                let window = context.hub.get_window(window_id);
                let dim = window.dimension();
                os_window.set_position(dim.x, dim.y)?;
                os_window.set_size(dim.width, dim.height)?;
            }
            Ok(())
        }
        Child::Container(container_id) => {
            for child in context.hub.get_container(container_id).children() {
                render_child(context, *child)?;
            }
            Ok(())
        }
    }
}

fn focus_workspace(context: &mut WindowContext, name: usize) -> Result<()> {
    let old_workspace = context.hub.current_workspace();
    context.hub.focus_workspace(name);
    let new_workspace = context.hub.current_workspace();
    if old_workspace == new_workspace {
        return Ok(());
    }

    if let Some(root) = context.hub.get_workspace(old_workspace).root() {
        hide_child(context, root)?;
    }

    render_workspace(context, new_workspace)
}

fn move_to_workspace(context: &mut WindowContext, name: usize) -> Result<()> {
    let current_workspace = context.hub.current_workspace();
    if let Some(moved) = context.hub.move_focused_to_workspace(name) {
        hide_child(context, moved)?;
    }
    render_workspace(context, current_workspace)
}

fn hide_child(context: &WindowContext, child: Child) -> Result<()> {
    let screen = context.hub.screen();
    match child {
        Child::Window(window_id) => {
            if let Some(window) = context.registry.borrow().get(window_id) {
                // MacOS doesn't allow completely set windows offscreen, so we need to leave at
                // least one pixel left
                // Taken from https://github.com/nikitabobko/AeroSpace/blob/976b2cf4b04d371143bb31f3b094d04e9e85fdcd/Sources/AppBundle/tree/MacWindow.swift#L144
                window.set_position(
                    screen.x + screen.width - 1.0,
                    screen.y + screen.height - 1.0,
                )?;
            }
        }
        Child::Container(container_id) => {
            for child in context.hub.get_container(container_id).children() {
                hide_child(context, *child)?;
            }
        }
    }
    Ok(())
}

type Observers = Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>;
