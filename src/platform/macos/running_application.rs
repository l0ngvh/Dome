use std::fmt;
use std::sync::Arc;

use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
};
use objc2_foundation::NSNotification;

use crate::platform::macos::accessibility::AXApp;

#[derive(Clone)]
pub(super) struct RunningApp(Retained<NSRunningApplication>);

impl RunningApp {
    pub(in crate::platform::macos) fn new(pid: i32) -> Option<Self> {
        if !is_valid_pid(pid) {
            return None;
        }
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
        (app.activationPolicy() == NSApplicationActivationPolicy::Regular).then_some(Self(app))
    }

    pub(in crate::platform::macos) fn from_notification(
        notification: &NSNotification,
    ) -> Option<Self> {
        let user_info = notification.userInfo()?;
        let obj = unsafe { user_info.objectForKey(NSWorkspaceApplicationKey)? };
        let app: Retained<NSRunningApplication> = unsafe { Retained::cast_unchecked(obj) };
        if !is_valid_pid(app.processIdentifier()) {
            return None;
        }
        (app.activationPolicy() == NSApplicationActivationPolicy::Regular).then_some(Self(app))
    }

    pub(in crate::platform::macos) fn pid(&self) -> i32 {
        self.0.processIdentifier()
    }

    pub(in crate::platform::macos) fn is_hidden(&self) -> bool {
        self.0.isHidden()
    }

    pub(in crate::platform::macos) fn is_active(&self) -> bool {
        self.0.isActive()
    }

    pub(in crate::platform::macos) fn ax_app(&self) -> Arc<AXApp> {
        Arc::new(AXApp::new(&self.0))
    }

    pub(in crate::platform::macos) fn all() -> impl Iterator<Item = RunningApp> {
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
