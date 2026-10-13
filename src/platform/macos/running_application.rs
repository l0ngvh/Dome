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
        Self::accepts_process(pid, app.activationPolicy()).then_some(Self(app))
    }

    pub(in crate::platform::macos) fn process_id_from_notification(
        notification: &NSNotification,
    ) -> Option<i32> {
        let user_info = notification.userInfo()?;
        let obj = unsafe { user_info.objectForKey(NSWorkspaceApplicationKey)? };
        let app: Retained<NSRunningApplication> = unsafe { Retained::cast_unchecked(obj) };
        Some(app.processIdentifier())
    }

    pub(in crate::platform::macos) fn is_active_process(pid: i32) -> bool {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .is_some_and(|app| app.isActive())
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

    pub(in crate::platform::macos) fn accepts_process(
        pid: i32,
        activation_policy: NSApplicationActivationPolicy,
    ) -> bool {
        is_valid_pid(pid) && activation_policy == NSApplicationActivationPolicy::Regular
    }

    pub(in crate::platform::macos) fn all() -> impl Iterator<Item = RunningApp> {
        NSWorkspace::sharedWorkspace()
            .runningApplications()
            .into_iter()
            .filter(|app| Self::accepts_process(app.processIdentifier(), app.activationPolicy()))
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
