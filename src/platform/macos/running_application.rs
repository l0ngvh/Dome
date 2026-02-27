use std::fmt;

use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
};
use objc2_application_services::AXUIElement;
use objc2_core_foundation::CFArray;
use objc2_core_graphics::CGWindowID;
use objc2_foundation::NSNotification;

use super::accessibility::AXWindow;
use super::objc2_wrapper::{
    get_attribute, get_cg_window_id, kAXFocusedWindowAttribute, kAXWindowsAttribute,
};

#[derive(Clone)]
pub(super) struct RunningApp(Retained<NSRunningApplication>);

impl RunningApp {
    pub(super) fn new(pid: i32) -> Option<Self> {
        if !is_valid_pid(pid) {
            return None;
        }
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
        (app.activationPolicy() == NSApplicationActivationPolicy::Regular).then_some(Self(app))
    }

    pub(super) fn from_notification(notification: &NSNotification) -> Option<Self> {
        let user_info = notification.userInfo()?;
        let obj = unsafe { user_info.objectForKey(NSWorkspaceApplicationKey)? };
        let app: Retained<NSRunningApplication> = unsafe { Retained::cast_unchecked(obj) };
        if !is_valid_pid(app.processIdentifier()) {
            return None;
        }
        (app.activationPolicy() == NSApplicationActivationPolicy::Regular).then_some(Self(app))
    }

    pub(super) fn pid(&self) -> i32 {
        self.0.processIdentifier()
    }

    pub(super) fn is_hidden(&self) -> bool {
        self.0.isHidden()
    }

    pub(super) fn is_active(&self) -> bool {
        self.0.isActive()
    }

    pub(super) fn ax_windows(&self) -> Vec<AXWindow> {
        let ax_app = unsafe { AXUIElement::new_application(self.pid()) };
        let Ok(windows) = get_attribute::<CFArray<AXUIElement>>(&ax_app, &kAXWindowsAttribute())
        else {
            return Vec::new();
        };
        windows
            .into_iter()
            .filter_map(|w| {
                let cg_id = get_cg_window_id(&w)?;
                Some(AXWindow::new(w, cg_id, &self.0))
            })
            .collect()
    }

    pub(super) fn focused_window_cg_id(&self) -> Option<CGWindowID> {
        let ax_app = unsafe { AXUIElement::new_application(self.pid()) };
        let focused =
            get_attribute::<AXUIElement>(&ax_app, &kAXFocusedWindowAttribute()).ok()?;
        get_cg_window_id(&focused)
    }

    pub(super) fn all() -> impl Iterator<Item = RunningApp> {
        NSWorkspace::sharedWorkspace()
            .runningApplications()
            .into_iter()
            .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
            .filter(|app| is_valid_pid(app.processIdentifier()))
            .map(RunningApp::from)
    }
}

impl From<Retained<NSRunningApplication>> for RunningApp {
    fn from(app: Retained<NSRunningApplication>) -> Self {
        Self(app)
    }
}

impl fmt::Display for RunningApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self
            .0
            .localizedName()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        write!(f, "App:{name}({pid})", pid = self.pid())
    }
}

fn is_valid_pid(pid: i32) -> bool {
    pid != -1 && pid != std::process::id() as i32
}
