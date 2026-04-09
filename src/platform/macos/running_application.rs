#[cfg(not(test))]
mod real {
    use std::fmt;
    use std::sync::Arc;

    use objc2::rc::Retained;
    use objc2_app_kit::{
        NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
    };
    use objc2_application_services::AXUIElement;
    use objc2_core_foundation::CFArray;
    use objc2_foundation::NSNotification;

    use super::super::accessibility::{AXApp, AXWindow};
    use super::super::dispatcher::DispatcherMarker;
    use super::super::objc2_wrapper::{
        get_attribute, get_cg_window_id, kAXFocusedWindowAttribute, kAXWindowsAttribute,
    };

    #[derive(Clone)]
    pub(in crate::platform::macos) struct RunningApp(Retained<NSRunningApplication>);

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

        /// Blocking AX IPC — creates an AXUIElement for the app process.
        pub(in crate::platform::macos) fn ax_app(&self, _marker: &DispatcherMarker) -> Arc<AXApp> {
            Arc::new(AXApp::new(&self.0))
        }

        /// Blocking AX IPC — queries `kAXWindowsAttribute` on the target process.
        pub(in crate::platform::macos) fn ax_windows(
            &self,
            marker: &DispatcherMarker,
        ) -> Vec<AXWindow> {
            let ax_app = self.ax_app(marker);
            let Ok(windows) =
                get_attribute::<CFArray<AXUIElement>>(&ax_app.element, &kAXWindowsAttribute())
            else {
                return Vec::new();
            };
            windows
                .into_iter()
                .filter_map(|w| {
                    let cg_id = get_cg_window_id(&w)?;
                    Some(AXWindow::new(w, cg_id, ax_app.clone()))
                })
                .collect()
        }

        /// Blocking AX IPC — queries `kAXFocusedWindowAttribute` on the target process.
        pub(in crate::platform::macos) fn focused_window(
            &self,
            marker: &DispatcherMarker,
        ) -> Option<AXWindow> {
            let ax_app = self.ax_app(marker);
            let focused =
                get_attribute::<AXUIElement>(&ax_app.element, &kAXFocusedWindowAttribute()).ok()?;
            let cg_id = get_cg_window_id(&focused)?;
            Some(AXWindow::new(focused, cg_id, ax_app))
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
}

#[cfg(not(test))]
pub(super) use real::RunningApp;

#[cfg(test)]
#[expect(
    clippy::items_after_test_module,
    reason = "re-export must follow module definition"
)]
mod mock {
    use std::fmt;

    use objc2::rc::Retained;
    use objc2_app_kit::{NSRunningApplication, NSWorkspaceApplicationKey};
    use objc2_foundation::NSNotification;

    use super::super::dispatcher::DispatcherMarker;

    #[derive(Clone)]
    pub(in crate::platform::macos) struct RunningApp {
        pid: i32,
        hidden: bool,
        active: bool,
    }

    impl RunningApp {
        pub(in crate::platform::macos) fn new(pid: i32) -> Option<Self> {
            Some(Self {
                pid,
                hidden: false,
                active: true,
            })
        }

        pub(in crate::platform::macos) fn from_notification(
            notification: &NSNotification,
        ) -> Option<Self> {
            let user_info = notification.userInfo()?;
            let obj = unsafe { user_info.objectForKey(NSWorkspaceApplicationKey)? };
            let app: Retained<NSRunningApplication> = unsafe { Retained::cast_unchecked(obj) };
            let pid = app.processIdentifier();
            Some(Self {
                pid,
                hidden: false,
                active: true,
            })
        }

        pub(in crate::platform::macos) fn pid(&self) -> i32 {
            self.pid
        }

        pub(in crate::platform::macos) fn is_hidden(&self) -> bool {
            self.hidden
        }

        pub(in crate::platform::macos) fn is_active(&self) -> bool {
            self.active
        }

        pub(in crate::platform::macos) fn ax_windows(
            &self,
            _marker: &DispatcherMarker,
        ) -> Vec<super::super::accessibility::AXWindow> {
            Vec::new()
        }

        pub(in crate::platform::macos) fn focused_window(
            &self,
            _marker: &DispatcherMarker,
        ) -> Option<super::super::accessibility::AXWindow> {
            None
        }

        pub(in crate::platform::macos) fn all() -> impl Iterator<Item = RunningApp> {
            std::iter::empty()
        }
    }

    impl From<Retained<NSRunningApplication>> for RunningApp {
        fn from(app: Retained<NSRunningApplication>) -> Self {
            Self {
                pid: app.processIdentifier(),
                hidden: false,
                active: true,
            }
        }
    }

    impl fmt::Display for RunningApp {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "MockApp(pid={})", self.pid)
        }
    }
}

#[cfg(test)]
pub(super) use mock::RunningApp;
