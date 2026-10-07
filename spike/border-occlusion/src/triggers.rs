//! Accessibility and NSWorkspace observers. Every callback reports a trigger
//! kind, a pid, and for an AX notification its element to `crate::on_trigger`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSApplicationActivationPolicy, NSApplicationDidChangeScreenParametersNotification,
    NSRunningApplication, NSWorkspace, NSWorkspaceActiveSpaceDidChangeNotification,
    NSWorkspaceApplicationKey, NSWorkspaceDidActivateApplicationNotification,
    NSWorkspaceDidLaunchApplicationNotification, NSWorkspaceDidTerminateApplicationNotification,
};
use objc2_application_services::{AXError, AXObserver, AXUIElement};
use objc2_core_foundation::{CFRetained, CFRunLoop, CFString, kCFRunLoopCommonModes};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSNotificationName, NSObjectProtocol, NSOperationQueue,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TriggerKind {
    Moved,
    Resized,
    FocusedWindowChanged,
    WindowCreated,
    Destroyed,
    Miniaturized,
    Deminiaturized,
    AppHidden,
    AppShown,
    MenuOpened,
    MenuClosed,
    Activated,
    Launched,
    Terminated,
    SpaceChanged,
    ScreenChanged,
    Safety,
    Start,
}

const AX_NOTIFICATIONS: [(&str, TriggerKind); 11] = [
    ("AXMoved", TriggerKind::Moved),
    ("AXResized", TriggerKind::Resized),
    ("AXFocusedWindowChanged", TriggerKind::FocusedWindowChanged),
    ("AXWindowCreated", TriggerKind::WindowCreated),
    ("AXUIElementDestroyed", TriggerKind::Destroyed),
    ("AXWindowMiniaturized", TriggerKind::Miniaturized),
    ("AXWindowDeminiaturized", TriggerKind::Deminiaturized),
    ("AXApplicationHidden", TriggerKind::AppHidden),
    ("AXApplicationShown", TriggerKind::AppShown),
    ("AXMenuOpened", TriggerKind::MenuOpened),
    ("AXMenuClosed", TriggerKind::MenuClosed),
];

impl TriggerKind {
    pub fn name(self) -> &'static str {
        match self {
            TriggerKind::Moved => "moved",
            TriggerKind::Resized => "resized",
            TriggerKind::FocusedWindowChanged => "focused_window",
            TriggerKind::WindowCreated => "window_created",
            TriggerKind::Destroyed => "destroyed",
            TriggerKind::Miniaturized => "miniaturized",
            TriggerKind::Deminiaturized => "deminiaturized",
            TriggerKind::AppHidden => "app_hidden",
            TriggerKind::AppShown => "app_shown",
            TriggerKind::MenuOpened => "menu_opened",
            TriggerKind::MenuClosed => "menu_closed",
            TriggerKind::Activated => "activated",
            TriggerKind::Launched => "launched",
            TriggerKind::Terminated => "terminated",
            TriggerKind::SpaceChanged => "space",
            TriggerKind::ScreenChanged => "screen",
            TriggerKind::Safety => "safety",
            TriggerKind::Start => "start",
        }
    }
}

struct Registered {
    observer: CFRetained<AXObserver>,
    app: CFRetained<AXUIElement>,
    notifications: Vec<CFRetained<CFString>>,
}

impl Drop for Registered {
    fn drop(&mut self) {
        for n in &self.notifications {
            unsafe { self.observer.remove_notification(&self.app, n) };
        }
        let source = unsafe { self.observer.run_loop_source() };
        if let Some(run_loop) = CFRunLoop::main() {
            run_loop.remove_source(Some(&source), unsafe { kCFRunLoopCommonModes });
        }
    }
}

type Token = Retained<ProtocolObject<dyn NSObjectProtocol>>;

thread_local! {
    static OBSERVERS: RefCell<HashMap<i32, Registered>> = RefCell::new(HashMap::new());
    static TOKENS: RefCell<Vec<Token>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "C-unwind" fn ax_callback(
    _observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    refcon: *mut c_void,
) {
    let name = unsafe { notification.as_ref() }.to_string();
    // The refcon carries the pid itself, not a pointer.
    let pid = refcon as usize as i32;
    if let Some((_, kind)) = AX_NOTIFICATIONS.iter().find(|(n, _)| *n == name) {
        crate::on_trigger(*kind, pid, Some(unsafe { element.as_ref() }));
    }
}

fn observed_policy(app: &NSRunningApplication) -> bool {
    matches!(
        app.activationPolicy(),
        NSApplicationActivationPolicy::Regular | NSApplicationActivationPolicy::Accessory
    )
}

/// Returns how many AX notifications were added. Zero means the app refused
/// the observer, for example while it is still launching.
fn register(pid: i32) -> usize {
    if pid == std::process::id() as i32 || OBSERVERS.with(|o| o.borrow().contains_key(&pid)) {
        return 0;
    }
    let mut raw: *mut AXObserver = std::ptr::null_mut();
    let result = unsafe { AXObserver::create(pid, Some(ax_callback), NonNull::from(&mut raw)) };
    let Some(raw) = NonNull::new(raw).filter(|_| result == AXError::Success) else {
        return 0;
    };
    let observer = unsafe { CFRetained::from_raw(raw) };
    let app = unsafe { AXUIElement::new_application(pid) };
    // An app that stops answering would otherwise block the main thread for
    // the system default of several seconds per call.
    unsafe { app.set_messaging_timeout(0.5) };
    let mut notifications = Vec::new();
    for (name, _) in AX_NOTIFICATIONS {
        let cf = CFString::from_static_str(name);
        let refcon = pid as usize as *mut c_void;
        if unsafe { observer.add_notification(&app, &cf, refcon) } == AXError::Success {
            notifications.push(cf);
        }
    }
    if notifications.is_empty() {
        return 0;
    }
    let source = unsafe { observer.run_loop_source() };
    if let Some(run_loop) = CFRunLoop::main() {
        run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
    }
    let added = notifications.len();
    OBSERVERS.with(|o| {
        o.borrow_mut().insert(
            pid,
            Registered {
                observer,
                app,
                notifications,
            },
        )
    });
    added
}

fn app_from(notification: &NSNotification) -> Option<Retained<NSRunningApplication>> {
    let info = notification.userInfo()?;
    let obj = info.objectForKey(unsafe { NSWorkspaceApplicationKey })?;
    obj.downcast::<NSRunningApplication>().ok()
}

fn observe(
    center: &NSNotificationCenter,
    name: &NSNotificationName,
    handler: impl Fn(&NSNotification) + 'static,
) {
    let block = RcBlock::new(move |n: NonNull<NSNotification>| handler(unsafe { n.as_ref() }));
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(name),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &block,
        )
    };
    TOKENS.with(|t| t.borrow_mut().push(token));
}

/// Returns (apps observed, apps that refused).
pub fn install() -> (usize, usize) {
    let workspace = NSWorkspace::sharedWorkspace();
    let mut observed = 0;
    let mut refused = 0;
    for app in workspace.runningApplications().iter() {
        if !observed_policy(&app) || app.processIdentifier() == std::process::id() as i32 {
            continue;
        }
        if register(app.processIdentifier()) > 0 {
            observed += 1;
        } else {
            refused += 1;
        }
    }

    let center = workspace.notificationCenter();
    observe(
        &center,
        unsafe { NSWorkspaceDidLaunchApplicationNotification },
        |n| {
            let Some(app) = app_from(n) else { return };
            let pid = app.processIdentifier();
            if observed_policy(&app) {
                register(pid);
            }
            crate::on_trigger(TriggerKind::Launched, pid, None);
        },
    );
    observe(
        &center,
        unsafe { NSWorkspaceDidTerminateApplicationNotification },
        |n| {
            let Some(app) = app_from(n) else { return };
            let pid = app.processIdentifier();
            OBSERVERS.with(|o| o.borrow_mut().remove(&pid));
            crate::on_trigger(TriggerKind::Terminated, pid, None);
        },
    );
    observe(
        &center,
        unsafe { NSWorkspaceDidActivateApplicationNotification },
        |n| {
            let pid = app_from(n).map_or(0, |a| a.processIdentifier());
            crate::on_trigger(TriggerKind::Activated, pid, None);
        },
    );
    observe(
        &center,
        unsafe { NSWorkspaceActiveSpaceDidChangeNotification },
        |_| crate::on_trigger(TriggerKind::SpaceChanged, 0, None),
    );
    observe(
        &NSNotificationCenter::defaultCenter(),
        unsafe { NSApplicationDidChangeScreenParametersNotification },
        |_| crate::on_trigger(TriggerKind::ScreenChanged, 0, None),
    );
    (observed, refused)
}
