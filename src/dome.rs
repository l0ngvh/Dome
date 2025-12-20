use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ptr::NonNull,
    rc::Rc,
};

use anyhow::{Context, Result};

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
use objc2_application_services::{AXError, AXIsProcessTrustedWithOptions, AXObserver, AXUIElement};
use objc2_core_foundation::{
    CFArray, CFHash, CFMachPort, CFRetained, CFRunLoop, CFString, CFType, CGFloat,
    kCFAllocatorDefault, kCFBooleanTrue, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSOperationQueue, NSPoint, NSRect, NSSize,
};

use crate::config::{Action, Config, Keymap, Modifier, Target, ToggleTarget};
use crate::core::{Child, Dimension, Hub, WindowId, WorkspaceId};
use crate::window::MacWindow;

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
                match create_observer(pid, context_ptr) {
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

struct WindowContext {
    hub: Hub,
    overlay_view: Retained<OverlayView>,
    pid_to_window_ids: Rc<RefCell<HashMap<i32, Vec<usize>>>>,
    window_mapping: Rc<RefCell<HashMap<usize, WindowId>>>,
    id_to_window: Rc<RefCell<HashMap<WindowId, MacWindow>>>,
    config: Config,
}

impl WindowContext {
    fn new(pids: &[i32], overlay_view: Retained<OverlayView>) -> Self {
        let mut pids_window_ids = HashMap::new();
        let config = Config::load();

        let screen = get_main_screen();
        tracing::info!("Screen {screen:?}");
        let mut hub = Hub::new(screen, config.border_size);

        // Track CFHash -> WindowId and WindowId -> MacWindow mappings
        let mut window_mapping = HashMap::new();
        let mut id_to_window = HashMap::new();

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
                    let window_id = hub.insert_window();
                    let cf_hash = CFHash(Some(&window));
                    window_mapping.insert(cf_hash, window_id);
                    id_to_window.insert(window_id, MacWindow::new(window.clone(), app.clone()));
                    window_ids.push(cf_hash);
                }
            }
            pids_window_ids.insert(*pid, window_ids);
        }

        Self {
            hub,
            overlay_view,
            pid_to_window_ids: Rc::new(RefCell::new(pids_window_ids)),
            window_mapping: Rc::new(RefCell::new(window_mapping)),
            id_to_window: Rc::new(RefCell::new(id_to_window)),
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
    if res != AXError::Success {
        return Err(anyhow::anyhow!(
            "Failed to add AXWindowCreated notification: {res:?}"
        ));
    }

    let res = unsafe {
        observer.add_notification(
            &app,
            &CFString::from_static_str("AXWindowMiniaturized"),
            context_ptr as *mut std::ffi::c_void,
        )
    };
    if res != AXError::Success {
        return Err(anyhow::anyhow!(
            "Failed to add AXWindowMiniaturized notification: {res:?}"
        ));
    }

    let res = unsafe {
        observer.add_notification(
            &app,
            &CFString::from_static_str("AXResized"),
            context_ptr as *mut std::ffi::c_void,
        )
    };
    if res != AXError::Success {
        return Err(anyhow::anyhow!(
            "Failed to add AXResized notification: {res:?}"
        ));
    }

    let res = unsafe {
        observer.add_notification(
            &app,
            &CFString::from_static_str("AXUIElementDestroyed"),
            context_ptr as *mut std::ffi::c_void,
        )
    };
    if res != AXError::Success {
        return Err(anyhow::anyhow!(
            "Failed to add AXUIElementDestroyed notification: {res:?}"
        ));
    }

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
    tracing::info!("AX Notification: {notification} {element:?}");
    if notification.to_string() == *"AXWindowCreated" && is_standard_window(&element) {
        match get_pid(&element) {
            Ok(pid) => {
                let app = unsafe { AXUIElement::new_application(pid) };
                let window = MacWindow::new(element.clone(), app);
                let window_id = context.hub.insert_window();
                let cf_hash = CFHash(Some(&element));
                context
                    .window_mapping
                    .borrow_mut()
                    .insert(cf_hash, window_id);
                context.id_to_window.borrow_mut().insert(window_id, window);
                // Render the entire workspace since insert_window may have resized other windows
                let workspace_id = context.hub.current_workspace();
                if let Err(e) = render_workspace(context, workspace_id) {
                    tracing::warn!("Failed to render workspace after window insert: {e:#}");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to get PID for window: {e:#}");
            }
        }
    } else if notification.to_string() == *"AXUIElementDestroyed" {
        let cf_hash = CFHash(Some(&element));
        if let Some(window_id) = context.window_mapping.borrow_mut().remove(&cf_hash) {
            let workspace_id = context.hub.delete_window(window_id);
            tracing::info!("Window deleted: {window_id}");
            if workspace_id == context.hub.current_workspace()
                && let Err(e) = render_workspace(context, workspace_id)
            {
                tracing::warn!("Failed to render workspace after window insert: {e:#}");
            }
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
                        let window_id = context.hub.insert_window();
                        let cf_hash = CFHash(Some(&window));
                        context
                            .window_mapping
                            .borrow_mut()
                            .insert(cf_hash, window_id);
                        context
                            .id_to_window
                            .borrow_mut()
                            .insert(window_id, MacWindow::new(window.clone(), app.clone()));
                        window_ids.push(cf_hash);
                    }
                }
                // Render the entire workspace after inserting all windows
                let workspace_id = context.hub.current_workspace();
                if let Err(e) = render_workspace(context, workspace_id) {
                    tracing::warn!("Failed to render workspace after app launch: {e:#}");
                }
                context
                    .pid_to_window_ids
                    .borrow_mut()
                    .insert(pid, window_ids);

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
                    tracing::debug!("Launched application doesn't have a pid");
                    return;
                };
                tracing::trace!("Received notification for terminating app with pid: {pid:?}",);
                apps.borrow_mut().remove(&pid);
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
            && let Some(os_window) = context.id_to_window.borrow().get(&window_id)
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
            if let Some(os_window) = context.id_to_window.borrow().get(&window_id) {
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

fn hide_child(context: &WindowContext, child: Child) -> Result<()> {
    match child {
        Child::Window(window_id) => {
            if let Some(window) = context.id_to_window.borrow().get(&window_id) {
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

fn get_pid(window: &AXUIElement) -> Result<i32> {
    let mut pid = 0;
    let value_ptr = NonNull::new(&mut pid as *mut i32).unwrap();
    let res = unsafe { window.pid(value_ptr) };

    if res != AXError::Success {
        return Err(anyhow::anyhow!("Failed to get pid. Error code: {res:?}"));
    }
    Ok(pid)
}

type Observers = Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>;
