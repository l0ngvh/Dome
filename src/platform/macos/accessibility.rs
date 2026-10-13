use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use objc2_app_kit::NSRunningApplication;
use std::ptr::NonNull;

use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFEqual, CFRetained, CFString, CFType, CGPoint, CGSize,
    kCFBooleanFalse, kCFBooleanTrue,
};
use objc2_core_graphics::{CGSessionCopyCurrentDictionary, CGWindowID};

use crate::core::{Dimension, Length, Logical, PixelRect};
use crate::platform::macos::dispatcher::DispatcherMarker;
use crate::platform::macos::objc2_wrapper::{
    AXError, get_attribute, get_cg_window_id, is_attribute_settable, kAXCloseButtonAttribute,
    kAXEnhancedUserInterfaceAttribute, kAXFocusedWindowAttribute, kAXFrontmostAttribute,
    kAXFullScreenAttribute, kAXMainAttribute, kAXMinimizedAttribute, kAXParentAttribute,
    kAXPositionAttribute, kAXPressAction, kAXRoleAttribute, kAXSizeAttribute,
    kAXStandardWindowSubrole, kAXSubroleAttribute, kAXTitleAttribute, kAXWindowRole,
    kAXWindowsAttribute, perform_action, set_attribute_value,
};

#[cfg(test)]
type EnhancedUiSetter = Box<dyn Fn(bool) -> Result<(), AXError> + Send + Sync>;

/// Per-app accessibility state shared across all windows of the same application.
///
/// Holds the AXUIElement for the app process and cached app metadata (name, bundle ID).
/// Always used behind `Arc<AXApp>` so multiple `AXWindow`s from the same app share one instance.
pub(super) struct AXApp {
    element: CFRetained<AXUIElement>,
    pid: i32,
    app_name: Option<String>,
    bundle_id: Option<String>,
    /// Whether `kAXEnhancedUserInterfaceAttribute` is settable on this app.
    /// Used by `with_animation_disabled` to skip set calls on apps that don't support it.
    /// Refreshed periodically via `refresh_enhanced_ui` during reconciliation.
    can_set_enhanced_ui: AtomicBool,
    #[cfg(test)]
    enhanced_ui_setter: Option<EnhancedUiSetter>,
}

// Safety: AXUIElement operations are IPC calls to the accessibility server,
// safe to use from any thread for manipulating other applications' windows.
unsafe impl Send for AXApp {}
unsafe impl Sync for AXApp {}

impl AXApp {
    pub(super) fn new(app: &NSRunningApplication) -> Self {
        let pid = app.processIdentifier();
        let element = unsafe { AXUIElement::new_application(pid) };
        let app_name = app.localizedName().map(|n| n.to_string());
        let bundle_id = app.bundleIdentifier().map(|b| b.to_string());
        let can_set_enhanced_ui = false;
        Self {
            element,
            pid,
            app_name,
            bundle_id,
            can_set_enhanced_ui: AtomicBool::new(can_set_enhanced_ui),
            #[cfg(test)]
            enhanced_ui_setter: None,
        }
    }

    pub(super) fn pid(&self) -> i32 {
        self.pid
    }

    pub(super) fn app_name(&self) -> Option<&str> {
        self.app_name.as_deref()
    }

    pub(super) fn bundle_id(&self) -> Option<&str> {
        self.bundle_id.as_deref()
    }

    /// Whether `kAXEnhancedUserInterfaceAttribute` is settable on this app.
    /// Relaxed ordering suffices for this capability hint.
    pub(super) fn can_set_enhanced_ui(&self) -> bool {
        self.can_set_enhanced_ui.load(Ordering::Relaxed)
    }

    /// Re-probes whether `kAXEnhancedUserInterfaceAttribute` is settable on the
    /// app element and updates the cache. Called during periodic reconciliation.
    pub(super) fn refresh_enhanced_ui(&self, marker: &DispatcherMarker) {
        let settable =
            is_attribute_settable(marker, &self.element, &kAXEnhancedUserInterfaceAttribute());
        self.can_set_enhanced_ui.store(settable, Ordering::Relaxed);
    }

    pub(super) fn set_frontmost(&self, marker: &DispatcherMarker) -> Result<(), AXError> {
        set_attribute_value(marker, &self.element, &kAXFrontmostAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })
    }

    pub(super) fn set_enhanced_ui(
        &self,
        marker: &DispatcherMarker,
        enabled: bool,
    ) -> Result<(), AXError> {
        #[cfg(test)]
        if let Some(setter) = &self.enhanced_ui_setter {
            return setter(enabled);
        }
        let value = unsafe {
            if enabled {
                kCFBooleanTrue
            } else {
                kCFBooleanFalse
            }
        };
        let Some(value) = value else {
            return Ok(());
        };
        set_attribute_value(
            marker,
            &self.element,
            &kAXEnhancedUserInterfaceAttribute(),
            value,
        )
    }

    /// Returns `true` if the window's parent is the app element itself (i.e. a
    /// top-level window, not a sheet or child panel).
    pub(super) fn is_root_window(
        &self,
        marker: &DispatcherMarker,
        window_element: &AXUIElement,
    ) -> bool {
        match get_attribute::<AXUIElement>(marker, window_element, &kAXParentAttribute()) {
            Err(_) => true,
            Ok(parent) => CFEqual(Some(&*parent), Some(&*self.element)),
        }
    }

    pub(super) fn windows(
        self: Arc<Self>,
        marker: &DispatcherMarker,
    ) -> Result<Vec<AXWindow>, AXError> {
        let windows =
            get_attribute::<CFArray<AXUIElement>>(marker, &self.element, &kAXWindowsAttribute())?;
        let windows = windows
            .into_iter()
            .filter_map(|w| {
                // TODO: figure out how to logs this error, as windows should have cg_id under
                // normal circumstances
                let Ok(cg_id) = get_cg_window_id(marker, &w) else {
                    return None;
                };
                Some(AXWindow::new(w, cg_id, self.clone(), marker))
            })
            .collect();
        Ok(windows)
    }

    pub(super) fn focused_window(
        self: Arc<Self>,
        marker: &DispatcherMarker,
    ) -> Result<AXWindow, AXError> {
        let focused =
            get_attribute::<AXUIElement>(marker, &self.element, &kAXFocusedWindowAttribute())?;
        let cg_id = get_cg_window_id(marker, &focused)?;
        Ok(AXWindow::new(focused, cg_id, self, marker))
    }

    /// Exposes the raw AXUIElement for observer registration (add/remove notification).
    pub(super) fn element(&self) -> &AXUIElement {
        &self.element
    }
}

impl std::fmt::Display for AXApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {}",
            self.pid,
            self.app_name.as_deref().unwrap_or("Unknown")
        )?;
        if let Some(bundle_id) = &self.bundle_id {
            write!(f, " ({bundle_id})")?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct AXWindow {
    element: CFRetained<AXUIElement>,
    app: Arc<AXApp>,
    cg_id: CGWindowID,
    title: Option<String>,
    #[cfg(test)]
    native_focus: Option<Arc<dyn Fn() -> Result<()> + Send + Sync>>,
}

// Safety: AXUIElement operations are IPC calls to the accessibility server,
// safe to use from any thread for manipulating other applications' windows.
unsafe impl Send for AXWindow {}
unsafe impl Sync for AXWindow {}

impl std::fmt::Display for AXWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}:{}] {}",
            self.app.pid(),
            self.cg_id,
            self.app.app_name().unwrap_or("Unknown")
        )?;
        if let Some(bundle_id) = self.app.bundle_id() {
            write!(f, " ({bundle_id})")?;
        }
        if let Some(title) = &self.title {
            write!(f, " - {title}")?;
        }
        Ok(())
    }
}

impl AXWindow {
    #[cfg(test)]
    pub(super) fn test_handle(pid: i32, cg_id: CGWindowID) -> Self {
        let element = unsafe { AXUIElement::new_application(pid) };
        let app = Arc::new(AXApp {
            element: element.clone(),
            pid,
            app_name: Some("test application".into()),
            bundle_id: None,
            can_set_enhanced_ui: AtomicBool::new(false),
            enhanced_ui_setter: None,
        });
        Self {
            element,
            app,
            cg_id,
            title: Some("test window".into()),
            native_focus: None,
        }
    }

    #[cfg(test)]
    pub(super) fn with_native_focus(
        mut self,
        focus: impl Fn() -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        self.native_focus = Some(Arc::new(focus));
        self
    }

    #[cfg(test)]
    pub(super) fn test_app(&self) -> Arc<AXApp> {
        self.app.clone()
    }

    #[cfg(test)]
    pub(super) fn test_element_address(&self) -> usize {
        (&*self.element as *const AXUIElement) as usize
    }

    pub(super) fn new(
        element: CFRetained<AXUIElement>,
        cg_id: CGWindowID,
        app: Arc<AXApp>,
        marker: &DispatcherMarker,
    ) -> Self {
        let title = get_attribute::<CFString>(marker, &element, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok();

        Self {
            element,
            app,
            cg_id,
            title,
            #[cfg(test)]
            native_focus: None,
        }
    }

    pub(super) fn cg_id(&self) -> CGWindowID {
        self.cg_id
    }

    pub(super) fn pid(&self) -> i32 {
        self.app.pid()
    }

    pub(super) fn app_name(&self) -> Option<&str> {
        self.app.app_name()
    }

    pub(super) fn bundle_id(&self) -> Option<&str> {
        self.app.bundle_id()
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(super) fn is_native_fullscreen(&self, marker: &DispatcherMarker) -> Result<bool, AXError> {
        match get_attribute::<CFBoolean>(marker, &self.element, &kAXFullScreenAttribute()) {
            Ok(value) => Ok(value.as_bool()),
            Err(AXError::Other(error))
                if error == objc2_application_services::AXError::AttributeUnsupported =>
            {
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn get_position(
        &self,
        marker: &DispatcherMarker,
    ) -> Result<(Length<Logical>, Length<Logical>)> {
        let pos = get_attribute::<AXValue>(marker, &self.element, &kAXPositionAttribute())
            .with_context(|| format!("get_position for {self}"))?;
        let mut cg_pos = CGPoint::new(0.0, 0.0);
        let ptr = NonNull::new((&mut cg_pos as *mut CGPoint).cast()).unwrap();
        if !unsafe { pos.value(AXValueType::CGPoint, ptr) } {
            anyhow::bail!("get_position for {self}: AXValue is not a CGPoint");
        }
        Ok((Length::new(cg_pos.x as f32), Length::new(cg_pos.y as f32)))
    }

    /// As we're tracking windows with CGWindowID, we have to check whether a window is still valid
    /// as macOS can reuse CGWindowID of deleted windows.
    pub(super) fn is_valid(&self, marker: &DispatcherMarker) -> bool {
        if is_screen_locked() {
            return true;
        }
        let is_deleted = matches!(
            get_attribute::<CFString>(marker, &self.element, &kAXRoleAttribute()),
            Err(AXError::InvalidUIElement)
        );
        if is_deleted {
            tracing::trace!(app = %self.app.app_name().unwrap_or("Unknown"), title = ?self.title, "not valid: window is deleted");
            return false;
        }
        true
    }

    pub(super) fn is_minimized(&self, marker: &DispatcherMarker) -> Result<bool, AXError> {
        get_attribute::<CFBoolean>(marker, &self.element, &kAXMinimizedAttribute())
            .map(|b| b.as_bool())
    }

    #[tracing::instrument(skip(self, marker))]
    pub(super) fn set_frame(
        &self,
        marker: &DispatcherMarker,
        rect: PixelRect<Logical>,
    ) -> Result<()> {
        let dim = rect.to_dimension();
        self.with_animation_disabled(marker, || {
            self.set_position(marker, dim.x, dim.y)?;
            self.set_size(marker, dim.width, dim.height)
        })
        .with_context(|| format!("set_frame for {self}"))
    }

    pub(super) fn focus(&self, marker: &DispatcherMarker) -> Result<()> {
        #[cfg(test)]
        if let Some(focus) = &self.native_focus {
            return focus();
        }
        self.app
            .set_frontmost(marker)
            .with_context(|| format!("focus for {self}"))?;
        set_attribute_value(marker, &self.element, &kAXMainAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })
        .with_context(|| format!("focus for {self}"))?;
        Ok(())
    }

    /// Hide the window by moving it offscreen.
    /// We don't minimize windows as there is no way to disable minimizing animation. When hiding
    /// multiple windows, it gets triggered in a staggered manner, which is extremely slow.
    pub(super) fn move_offscreen(
        &self,
        marker: &DispatcherMarker,
        x: Length<Logical>,
        y: Length<Logical>,
    ) -> Result<()> {
        self.with_animation_disabled(marker, || self.set_position(marker, x, y))
            .with_context(|| format!("hide for {self}"))
    }

    pub(super) fn minimize(&self, marker: &DispatcherMarker) -> Result<()> {
        set_attribute_value(marker, &self.element, &kAXMinimizedAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })
        .with_context(|| format!("minimize for {self}"))
    }

    pub(super) fn unminimize(&self, marker: &DispatcherMarker) -> Result<()> {
        set_attribute_value(marker, &self.element, &kAXMinimizedAttribute(), unsafe {
            kCFBooleanFalse.unwrap()
        })
        .with_context(|| format!("unminimize for {self}"))
    }

    pub(super) fn close(&self, marker: &DispatcherMarker) -> Result<()> {
        let button =
            match get_attribute::<AXUIElement>(marker, &self.element, &kAXCloseButtonAttribute()) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(window = %self, "close button missing: {e}");
                    return Ok(());
                }
            };
        perform_action(marker, &button, &kAXPressAction())
            .with_context(|| format!("close for {self}"))
    }

    pub(super) fn get_size(
        &self,
        marker: &DispatcherMarker,
    ) -> Result<(Length<Logical>, Length<Logical>)> {
        let size = get_attribute::<AXValue>(marker, &self.element, &kAXSizeAttribute())
            .with_context(|| format!("get_size for {self}"))?;
        let mut cg_size = CGSize::new(0.0, 0.0);
        let ptr = NonNull::new((&mut cg_size as *mut CGSize).cast()).unwrap();
        if !unsafe { size.value(AXValueType::CGSize, ptr) } {
            anyhow::bail!("get_size for {self}: AXValue is not a CGSize");
        }
        Ok((
            Length::new(cg_size.width as f32),
            Length::new(cg_size.height as f32),
        ))
    }

    pub(super) fn check_unmanageable(&self, marker: &DispatcherMarker) -> bool {
        let cg_id = self.cg_id();
        let pid = self.pid();
        if self.title.is_none() {
            crate::trace_once!(
                key: (cg_id, pid),
                window = %self, "not manageable: no title"
            );
            return true;
        }

        let role = get_attribute::<CFString>(marker, &self.element, &kAXRoleAttribute()).ok();
        let is_window = role
            .as_ref()
            .map(|r| CFEqual(Some(&**r), Some(&*kAXWindowRole())))
            .unwrap_or(false);
        if !is_window {
            crate::trace_once!(
                key: (cg_id, pid),
                window = %self,
                role = ?role.as_ref().map(|r| r.to_string()),
                "not manageable: role is not AXWindow"
            );
            return true;
        }

        let subrole = get_attribute::<CFString>(marker, &self.element, &kAXSubroleAttribute()).ok();
        let is_standard = subrole
            .as_ref()
            .map(|sr| CFEqual(Some(&**sr), Some(&*kAXStandardWindowSubrole())))
            .unwrap_or(false);
        if !is_standard {
            crate::trace_once!(
                key: (cg_id, pid),
                window = %self,
                subrole = ?subrole.as_ref().map(|sr| sr.to_string()),
                "not manageable: subrole is not standard"
            );
            return true;
        }

        let is_root = self.app.is_root_window(marker, &self.element);
        if !is_root {
            crate::trace_once!(
                key: (cg_id, pid),
                window = %self, "not manageable: not a root window"
            );
            return true;
        }

        if !self.is_native_fullscreen(marker).unwrap_or(false)
            && !is_attribute_settable(marker, &self.element, &kAXPositionAttribute())
        {
            crate::trace_once!(
                key: (cg_id, pid),
                window = %self, "not manageable: position not settable"
            );
            return true;
        }

        if !is_attribute_settable(marker, &self.element, &kAXSizeAttribute()) {
            crate::trace_once!(
                key: (cg_id, pid),
                window = %self, "not manageable: size not settable"
            );
            return true;
        }

        if !is_attribute_settable(marker, &self.element, &kAXMainAttribute()) {
            crate::trace_once!(
                key: (cg_id, pid),
                window = %self, "not manageable: main not settable"
            );
            return true;
        }

        false
    }

    fn with_animation_disabled<F>(&self, marker: &DispatcherMarker, operation: F) -> Result<()>
    where
        F: FnOnce() -> Result<()>,
    {
        let restore = self.app.can_set_enhanced_ui().then(|| EnhancedUiRestore {
            app: self.app.clone(),
            marker,
        });
        if restore.is_some() {
            let _ = self.app.set_enhanced_ui(marker, false);
        }
        let result = operation();
        drop(restore);
        result
    }

    /// FFI boundary: packs a logical position into a CGPoint for the AX API.
    fn set_position(
        &self,
        marker: &DispatcherMarker,
        x: Length<Logical>,
        y: Length<Logical>,
    ) -> Result<()> {
        let pos_ptr: *mut CGPoint = &mut CGPoint::new(x.value() as f64, y.value() as f64);
        let pos_ptr = NonNull::new(pos_ptr.cast()).unwrap();
        let pos_ptr = unsafe { AXValue::new(AXValueType::CGPoint, pos_ptr) }.unwrap();
        Ok(set_attribute_value(
            marker,
            &self.element,
            &kAXPositionAttribute(),
            &pos_ptr,
        )?)
    }

    /// FFI boundary: packs a logical size into a CGSize for the AX API.
    fn set_size(
        &self,
        marker: &DispatcherMarker,
        width: Length<Logical>,
        height: Length<Logical>,
    ) -> Result<()> {
        let size_ptr: *mut CGSize = &mut CGSize::new(width.value() as f64, height.value() as f64);
        let size = NonNull::new(size_ptr.cast()).unwrap();
        let size = unsafe { AXValue::new(AXValueType::CGSize, size) }.unwrap();
        Ok(set_attribute_value(
            marker,
            &self.element,
            &kAXSizeAttribute(),
            &size,
        )?)
    }
    pub(super) fn read_title(&self, marker: &DispatcherMarker) -> Option<String> {
        get_attribute::<CFString>(marker, &self.element, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok()
    }
    pub(super) fn read_close_button_frame(
        &self,
        marker: &DispatcherMarker,
    ) -> Option<Dimension<Logical>> {
        let button =
            get_attribute::<AXUIElement>(marker, &self.element, &kAXCloseButtonAttribute()).ok()?;
        let position = get_attribute::<AXValue>(marker, &button, &kAXPositionAttribute()).ok()?;
        let size = get_attribute::<AXValue>(marker, &button, &kAXSizeAttribute()).ok()?;
        let mut origin = CGPoint::new(0.0, 0.0);
        let mut extent = CGSize::new(0.0, 0.0);
        let has_origin =
            unsafe { position.value(AXValueType::CGPoint, NonNull::from(&mut origin).cast()) };
        let has_extent =
            unsafe { size.value(AXValueType::CGSize, NonNull::from(&mut extent).cast()) };
        (has_origin && has_extent).then(|| {
            Dimension::new(
                Length::new(origin.x as f32),
                Length::new(origin.y as f32),
                Length::new(extent.width as f32),
                Length::new(extent.height as f32),
            )
        })
    }
}

fn is_screen_locked() -> bool {
    let Some(dict) = CGSessionCopyCurrentDictionary() else {
        return false;
    };
    let dict: &CFDictionary<CFString, CFType> = unsafe { dict.cast_unchecked() };

    // CGSSessionScreenIsLocked is present when screen is locked
    let locked_key = CFString::from_static_str("CGSSessionScreenIsLocked");
    if dict.contains_key(&locked_key) {
        return true;
    }

    // kCGSSessionOnConsoleKey is false when screen is off/sleeping
    let on_console_key = CFString::from_static_str("kCGSSessionOnConsoleKey");
    if let Some(value) = dict.get(&on_console_key)
        && let Some(b) = value.downcast_ref::<CFBoolean>()
    {
        return !b.as_bool();
    }

    false
}

struct EnhancedUiRestore<'worker> {
    app: Arc<AXApp>,
    marker: &'worker DispatcherMarker,
}

impl Drop for EnhancedUiRestore<'_> {
    fn drop(&mut self) {
        let _ = self.app.set_enhanced_ui(self.marker, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::macos::dispatcher::{
        AccessibilityApplication, ApplicationCommand, ApplicationNative, ApplicationOperation,
    };
    use crate::platform::macos::window_server::ApplicationLifetime;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    include!("accessibility_serialization_tests.rs");

    #[test]
    fn disable_animation_on_disable_or_operation_unwind_restores_enhanced_ui() {
        struct NativeFrame {
            panic_in_operation: bool,
            entered: mpsc::Sender<thread::ThreadId>,
        }
        impl ApplicationNative for NativeFrame {
            fn probe(&mut self, _: &DispatcherMarker) {}
            fn execute(&mut self, operation: ApplicationOperation, marker: &DispatcherMarker) {
                if let ApplicationOperation::SetFrame { window, .. } = operation {
                    let _ = window.with_animation_disabled(marker, || {
                        self.entered.send(thread::current().id()).unwrap();
                        assert!(!self.panic_in_operation, "injected frame panic");
                        Ok(())
                    });
                }
            }
        }
        struct Exit(mpsc::Sender<bool>);
        impl Drop for Exit {
            fn drop(&mut self) {
                let _ = self.0.send(thread::panicking());
            }
        }
        for panic_site in [None, Some("disable"), Some("operation")] {
            let (toggles, calls) = mpsc::channel();
            let (entered, operations) = mpsc::channel();
            let (exit, exited) = mpsc::channel();
            let (restore_release, restore_gate) = mpsc::channel();
            let restore_gate = std::sync::Mutex::new(Some(restore_gate));
            let attempts = std::rc::Rc::new(std::cell::Cell::new(0));
            let attempted = attempts.clone();
            let mut window = AXWindow::test_handle(123, 1);
            let app = Arc::get_mut(&mut window.app).unwrap();
            app.can_set_enhanced_ui.store(true, Ordering::Relaxed);
            app.enhanced_ui_setter = Some(Box::new(move |enabled| {
                toggles.send((enabled, thread::current().id())).unwrap();
                assert!(
                    enabled || panic_site != Some("disable"),
                    "injected disable panic"
                );
                if enabled {
                    restore_gate.lock().unwrap().take().unwrap().recv().unwrap();
                    Err(AXError::InvalidUIElement)
                } else {
                    Ok(())
                }
            }));
            let mut owner = AccessibilityApplication::with_native(
                Arc::new(ApplicationLifetime { pid: 123 }),
                false,
                false,
                Box::new(move |_, _| {
                    Box::new(NativeFrame {
                        panic_in_operation: panic_site == Some("operation"),
                        entered: entered.clone(),
                    })
                }),
                Box::new(move |job| {
                    attempted.set(attempted.get() + 1);
                    let exit = exit.clone();
                    thread::Builder::new().spawn(move || {
                        let _exit = Exit(exit);
                        job();
                    })
                }),
            );
            owner.send(ApplicationCommand::Operation(
                ApplicationOperation::SetFrame {
                    window,
                    frame: PixelRect::from_dimension(Dimension::new(
                        Length::new(0.0),
                        Length::new(0.0),
                        Length::new(80.0),
                        Length::new(60.0),
                    )),
                },
            ));
            let disable = calls.recv_timeout(Duration::from_secs(3)).unwrap();
            let restore = calls.recv_timeout(Duration::from_secs(3)).unwrap();
            assert!(!disable.0);
            assert!(restore.0);
            assert_eq!(disable.1, restore.1);
            assert_ne!(disable.1, thread::current().id());
            owner.send(ApplicationCommand::Read(
                crate::platform::macos::window_server::InspectionKind::Title(1),
            ));
            assert_eq!(
                attempts.get(),
                1,
                "restoration must retain the sole receiver"
            );
            restore_release.send(()).unwrap();
            if panic_site == Some("disable") {
                assert!(operations.try_recv().is_err());
            } else {
                assert_eq!(
                    operations.recv_timeout(Duration::from_secs(3)).unwrap(),
                    restore.1
                );
            }
            drop(owner);
            assert_eq!(
                exited.recv_timeout(Duration::from_secs(3)).unwrap(),
                panic_site.is_some()
            );
            assert!(calls.try_recv().is_err());
        }
    }
}
