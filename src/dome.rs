use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
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
    config::{Action, Config, Keymap, Modifier, Target, ToggleTarget},
    objc2_wrapper::{
        add_observer_notification, create_observer, get_attribute, get_pid, kAXMinimizedAttribute,
        kAXRoleAttribute, kAXStandardWindowSubrole, kAXSubroleAttribute,
        kAXWindowCreatedNotification, kAXWindowMiniaturizedNotification, kAXWindowRole,
        kAXWindowsAttribute,
    },
};
use crate::{
    core::{Child, Dimension, Hub, WindowId, WorkspaceId},
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
                    rect.r as CGFloat, rect.g as CGFloat, rect.b as CGFloat, rect.a as CGFloat,
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
    r: f32,
    g: f32,
    b: f32,
    a: f32,
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
        if let Some(window_id) = context.registry.borrow_mut().remove_by_hash(cf_hash) {
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
    _event_type: CGEventType,
    event: NonNull<CGEvent>,
    refcon: *mut std::ffi::c_void,
) -> *mut CGEvent {
    let event = event.as_ptr();
    let flags = CGEvent::flags(Some(unsafe { &*event }));
    let context = unsafe { &mut *(refcon as *mut WindowContext) };
    let key = get_key_from_event(event);

    let mut modifiers = HashSet::new();
    if flags.contains(CGEventFlags::MaskCommand) {
        modifiers.insert(Modifier::Cmd);
    }
    if flags.contains(CGEventFlags::MaskShift) {
        modifiers.insert(Modifier::Shift);
    }
    if flags.contains(CGEventFlags::MaskAlternate) {
        modifiers.insert(Modifier::Alt);
    }
    if flags.contains(CGEventFlags::MaskControl) {
        modifiers.insert(Modifier::Ctrl);
    }

    let keymap = Keymap {
        key: key.clone(),
        modifiers,
    };
    let actions = context.config.get_actions(&keymap);

    if actions.is_empty() {
        return event;
    }

    for action in actions {
        if let Err(e) = execute_action(context, &action) {
            tracing::warn!("Failed to execute action: {e:#}");
        }
    }
    std::ptr::null_mut()
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
            Target::Up => context.hub.focus_up(),
            Target::Down => context.hub.focus_down(),
            Target::Left => context.hub.focus_left(),
            Target::Right => context.hub.focus_right(),
            Target::Parent => context.hub.focus_parent(),
            Target::Workspace(n) => return focus_workspace(context, *n),
        },
        Action::Move(target) => match target {
            Target::Workspace(n) => return move_to_workspace(context, *n),
            _ => {}
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

fn render_workspace(context: &WindowContext, workspace_id: WorkspaceId) -> Result<()> {
    if let Some(root) = context.hub.get_workspace(workspace_id).root() {
        render_child(context, root)?;

        // Update overlay with border rects
        let mut rects = Vec::new();
        collect_border_rects(context, root, &mut rects);
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
        // No windows, clear overlay
        context.overlay_view.set_rects(Vec::new());
    }
    Ok(())
}

fn collect_border_rects(context: &WindowContext, child: Child, rects: &mut Vec<OverlayRect>) {
    const COLOR: (f32, f32, f32, f32) = (0.4, 0.6, 1.0, 1.0); // Light blue

    match child {
        Child::Window(window_id) => {
            let dim = context.hub.get_window(window_id).dimension();
            // Convert from top-left to bottom-left coordinates for NSView
            let screen = context.hub.screen();
            let y = screen.y + screen.height - dim.y - dim.height;
            let border_size = context.config.border_size;

            // Draw border around window
            // Top
            rects.push(OverlayRect {
                x: dim.x - border_size,
                y: y + dim.height,
                width: dim.width + border_size * 2.0,
                height: border_size,
                r: COLOR.0,
                g: COLOR.1,
                b: COLOR.2,
                a: COLOR.3,
            });
            // Bottom
            rects.push(OverlayRect {
                x: dim.x - border_size,
                y: y - border_size,
                width: dim.width + border_size * 2.0,
                height: border_size,
                r: COLOR.0,
                g: COLOR.1,
                b: COLOR.2,
                a: COLOR.3,
            });
            // Left
            rects.push(OverlayRect {
                x: dim.x - border_size,
                y,
                width: border_size,
                height: dim.height,
                r: COLOR.0,
                g: COLOR.1,
                b: COLOR.2,
                a: COLOR.3,
            });
            // Right
            rects.push(OverlayRect {
                x: dim.x + dim.width,
                y,
                width: border_size,
                height: dim.height,
                r: COLOR.0,
                g: COLOR.1,
                b: COLOR.2,
                a: COLOR.3,
            });
        }
        Child::Container(container_id) => {
            // If container is focused, draw border inside its dimension
            let workspace = context.hub.get_workspace(context.hub.current_workspace());
            if let Some(Child::Container(focused_id)) = workspace.focused()
                && focused_id == container_id
            {
                let dim = context.hub.get_container(container_id).dimension();
                let screen = context.hub.screen();
                let y = screen.y + screen.height - dim.y - dim.height;
                let border_size = context.config.border_size;

                // Draw border inside container dimension
                // Top
                rects.push(OverlayRect {
                    x: dim.x,
                    y: y + dim.height - border_size,
                    width: dim.width,
                    height: border_size,
                    r: COLOR.0,
                    g: COLOR.1,
                    b: COLOR.2,
                    a: COLOR.3,
                });
                // Bottom
                rects.push(OverlayRect {
                    x: dim.x,
                    y,
                    width: dim.width,
                    height: border_size,
                    r: COLOR.0,
                    g: COLOR.1,
                    b: COLOR.2,
                    a: COLOR.3,
                });
                // Left
                rects.push(OverlayRect {
                    x: dim.x,
                    y: y + border_size,
                    width: border_size,
                    height: dim.height - 2.0 * border_size,
                    r: COLOR.0,
                    g: COLOR.1,
                    b: COLOR.2,
                    a: COLOR.3,
                });
                // Right
                rects.push(OverlayRect {
                    x: dim.x + dim.width - border_size,
                    y: y + border_size,
                    width: border_size,
                    height: dim.height - 2.0 * border_size,
                    r: COLOR.0,
                    g: COLOR.1,
                    b: COLOR.2,
                    a: COLOR.3,
                });
            }

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
                os_window.show()?;
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
    context.hub.move_focused_to_workspace(name);
    render_workspace(context, current_workspace)
}

fn hide_child(context: &WindowContext, child: Child) -> Result<()> {
    match child {
        Child::Window(window_id) => {
            if let Some(window) = context.registry.borrow().get(window_id) {
                window.hide()?;
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
