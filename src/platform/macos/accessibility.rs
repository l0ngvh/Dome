use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use std::ptr::NonNull;

#[cfg(not(test))]
use objc2_app_kit::NSRunningApplication;
use objc2_application_services::{AXUIElement, AXValue, AXValueType};
#[cfg(not(test))]
use objc2_core_foundation::CFArray;
use objc2_core_foundation::{
    CFBoolean, CFDictionary, CFEqual, CFRetained, CFString, CFType, CGPoint, CGSize,
    kCFBooleanFalse, kCFBooleanTrue,
};
use objc2_core_graphics::{CGSessionCopyCurrentDictionary, CGWindowID};

use crate::platform::macos::dispatcher::DispatcherMarker;
use crate::platform::macos::objc2_wrapper::{
    AXError, get_attribute, is_attribute_settable, kAXEnhancedUserInterfaceAttribute,
    kAXFrontmostAttribute, kAXFullScreenAttribute, kAXMainAttribute, kAXMinimizedAttribute,
    kAXParentAttribute, kAXPositionAttribute, kAXRoleAttribute, kAXSizeAttribute,
    kAXStandardWindowSubrole, kAXSubroleAttribute, kAXTitleAttribute, kAXWindowRole,
    set_attribute_value,
};

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
}

// Safety: AXUIElement operations are IPC calls to the accessibility server,
// safe to use from any thread for manipulating other applications' windows.
unsafe impl Send for AXApp {}
unsafe impl Sync for AXApp {}

impl AXApp {
    #[cfg(not(test))]
    pub(super) fn new(app: &NSRunningApplication) -> Self {
        let pid = app.processIdentifier();
        let element = unsafe { AXUIElement::new_application(pid) };
        let app_name = app.localizedName().map(|n| n.to_string());
        let bundle_id = app.bundleIdentifier().map(|b| b.to_string());
        let can_set_enhanced_ui =
            is_attribute_settable(&element, &kAXEnhancedUserInterfaceAttribute());
        Self {
            element,
            pid,
            app_name,
            bundle_id,
            can_set_enhanced_ui: AtomicBool::new(can_set_enhanced_ui),
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
    /// Uses a cached value — `Relaxed` ordering is fine since this is a
    /// performance hint, not a synchronization primitive.
    pub(super) fn can_set_enhanced_ui(&self) -> bool {
        self.can_set_enhanced_ui.load(Ordering::Relaxed)
    }

    /// Re-probes whether `kAXEnhancedUserInterfaceAttribute` is settable on the
    /// app element and updates the cache. Called during periodic reconciliation.
    pub(super) fn refresh_enhanced_ui(&self) {
        let settable = is_attribute_settable(&self.element, &kAXEnhancedUserInterfaceAttribute());
        self.can_set_enhanced_ui.store(settable, Ordering::Relaxed);
    }

    pub(super) fn set_frontmost(&self) -> Result<(), AXError> {
        set_attribute_value(&self.element, &kAXFrontmostAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })
    }

    pub(super) fn set_enhanced_ui(&self, enabled: bool) -> Result<(), AXError> {
        let value = if enabled {
            unsafe { kCFBooleanTrue.unwrap() }
        } else {
            unsafe { kCFBooleanFalse.unwrap() }
        };
        set_attribute_value(&self.element, &kAXEnhancedUserInterfaceAttribute(), value)
    }

    /// Returns `true` if the window's parent is the app element itself (i.e. a
    /// top-level window, not a sheet or child panel).
    pub(super) fn is_root_window(&self, window_element: &AXUIElement) -> bool {
        match get_attribute::<AXUIElement>(window_element, &kAXParentAttribute()) {
            Err(_) => true,
            Ok(parent) => CFEqual(Some(&*parent), Some(&*self.element)),
        }
    }

    #[cfg(not(test))]
    pub(super) fn windows(&self) -> Result<CFRetained<CFArray<AXUIElement>>, AXError> {
        get_attribute::<CFArray<AXUIElement>>(
            &self.element,
            &crate::platform::macos::objc2_wrapper::kAXWindowsAttribute(),
        )
    }

    #[cfg(not(test))]
    pub(super) fn focused_window_element(&self) -> Result<CFRetained<AXUIElement>, AXError> {
        get_attribute::<AXUIElement>(
            &self.element,
            &crate::platform::macos::objc2_wrapper::kAXFocusedWindowAttribute(),
        )
    }

    /// Exposes the raw AXUIElement for observer registration (add/remove notification).
    pub(super) fn element(&self) -> &AXUIElement {
        &self.element
    }

    #[cfg(test)]
    pub(super) fn stub(pid: i32) -> Self {
        Self {
            element: unsafe { AXUIElement::new_application(pid) },
            pid,
            app_name: None,
            bundle_id: None,
            can_set_enhanced_ui: AtomicBool::new(false),
        }
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
    #[cfg(not(test))]
    pub(super) fn new(
        element: CFRetained<AXUIElement>,
        cg_id: CGWindowID,
        app: Arc<AXApp>,
    ) -> Self {
        let title = get_attribute::<CFString>(&element, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok();

        Self {
            element,
            app,
            cg_id,
            title,
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

    pub(super) fn is_native_fullscreen(&self) -> bool {
        get_attribute::<CFBoolean>(&self.element, &kAXFullScreenAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false)
    }

    pub(super) fn get_position(&self) -> Result<(i32, i32)> {
        let pos = get_attribute::<AXValue>(&self.element, &kAXPositionAttribute())
            .with_context(|| format!("get_position for {self}"))?;
        let mut cg_pos = CGPoint::new(0.0, 0.0);
        let ptr = NonNull::new((&mut cg_pos as *mut CGPoint).cast()).unwrap();
        unsafe { pos.value(AXValueType::CGPoint, ptr) };
        Ok((cg_pos.x as i32, cg_pos.y as i32))
    }

    /// As we're tracking windows with CGWindowID, we have to check whether a window is still valid
    /// as macOS can reuse CGWindowID of deleted windows.
    pub(super) fn is_valid(&self) -> bool {
        if is_screen_locked() {
            return true;
        }
        let is_deleted = matches!(
            get_attribute::<CFString>(&self.element, &kAXRoleAttribute()),
            Err(AXError::InvalidUIElement)
        );
        if is_deleted {
            tracing::trace!(app = %self.app.app_name().unwrap_or("Unknown"), title = ?self.title, "not valid: window is deleted");
            return false;
        }
        true
    }

    pub(super) fn is_minimized(&self) -> bool {
        get_attribute::<CFBoolean>(&self.element, &kAXMinimizedAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false)
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn set_frame(&self, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
        self.with_animation_disabled(|| {
            self.set_position(x, y)?;
            self.set_size(width, height)
        })
        .with_context(|| format!("set_frame for {self}"))
    }

    pub(super) fn focus(&self) -> Result<()> {
        self.app
            .set_frontmost()
            .with_context(|| format!("focus for {self}"))?;
        set_attribute_value(&self.element, &kAXMainAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })
        .with_context(|| format!("focus for {self}"))?;
        Ok(())
    }

    /// Hide the window by moving it offscreen
    /// We don't minimize windows as there is no way to disable minimizing animation. When hiding
    /// multiple windows, it gets triggered in a staggered manner, which is extremely slow, and
    /// causes event tap to be timed out
    pub(super) fn hide_at(&self, x: i32, y: i32) -> Result<()> {
        self.with_animation_disabled(|| self.set_position(x, y))
            .with_context(|| format!("hide for {self}"))
    }

    pub(super) fn minimize(&self) -> Result<()> {
        set_attribute_value(&self.element, &kAXMinimizedAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })
        .with_context(|| format!("minimize for {self}"))
    }

    pub(super) fn unminimize(&self) -> Result<()> {
        set_attribute_value(&self.element, &kAXMinimizedAttribute(), unsafe {
            kCFBooleanFalse.unwrap()
        })
        .with_context(|| format!("unminimize for {self}"))
    }

    pub(super) fn get_size(&self) -> Result<(i32, i32)> {
        let size = get_attribute::<AXValue>(&self.element, &kAXSizeAttribute())
            .with_context(|| format!("get_size for {self}"))?;
        let mut cg_size = CGSize::new(0.0, 0.0);
        let ptr = NonNull::new((&mut cg_size as *mut CGSize).cast()).unwrap();
        unsafe { size.value(AXValueType::CGSize, ptr) };
        Ok((cg_size.width as i32, cg_size.height as i32))
    }

    pub(super) fn is_manageable(&self) -> bool {
        if self.title.is_none() {
            tracing::trace!(window = %self, "not manageable: window has no title");
            return false;
        };

        let role = get_attribute::<CFString>(&self.element, &kAXRoleAttribute()).ok();
        let is_window = role
            .as_ref()
            .map(|r| CFEqual(Some(&**r), Some(&*kAXWindowRole())))
            .unwrap_or(false);
        if !is_window {
            tracing::trace!(window = %self, "not manageable: role is not AXWindow");
            return false;
        }

        let subrole = get_attribute::<CFString>(&self.element, &kAXSubroleAttribute()).ok();
        let is_standard = subrole
            .as_ref()
            .map(|sr| CFEqual(Some(&**sr), Some(&*kAXStandardWindowSubrole())))
            .unwrap_or(false);
        if !is_standard {
            tracing::trace!(window = %self, "not manageable: subrole is not AXStandardWindow");
            return false;
        }

        let is_root = self.app.is_root_window(&self.element);
        if !is_root {
            tracing::trace!(window = %self, "not manageable: window is not root");
            return false;
        }

        if !is_attribute_settable(&self.element, &kAXPositionAttribute()) {
            tracing::trace!(window = %self, "not manageable: position is not settable");
            return false;
        }

        if !is_attribute_settable(&self.element, &kAXSizeAttribute()) {
            tracing::trace!(window = %self, "not manageable: size is not settable");
            return false;
        }

        if !is_attribute_settable(&self.element, &kAXMainAttribute()) {
            tracing::trace!(window = %self, "not manageable: main attribute is not settable");
            return false;
        }

        let is_minimized = get_attribute::<CFBoolean>(&self.element, &kAXMinimizedAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if is_minimized {
            tracing::trace!(window = %self, "not manageable: window is minimized");
            return false;
        }

        true
    }

    /// Without this the windows move in a janky way
    /// https://github.com/nikitabobko/AeroSpace/issues/51
    fn with_animation_disabled<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce() -> Result<()>,
    {
        let can_set = self.app.can_set_enhanced_ui();
        if can_set && let Err(err) = self.app.set_enhanced_ui(false) {
            tracing::trace!(window = %self, "Failed to disable enhanced UI: {err:#}");
        }
        let result = f();
        if can_set && let Err(err) = self.app.set_enhanced_ui(true) {
            tracing::trace!(window = %self, "Failed to re-enable enhanced UI: {err:#}");
        }
        result
    }

    fn set_position(&self, x: i32, y: i32) -> Result<()> {
        let pos_ptr: *mut CGPoint = &mut CGPoint::new(x as f64, y as f64);
        let pos_ptr = NonNull::new(pos_ptr.cast()).unwrap();
        let pos_ptr = unsafe { AXValue::new(AXValueType::CGPoint, pos_ptr) }.unwrap();
        Ok(set_attribute_value(
            &self.element,
            &kAXPositionAttribute(),
            &pos_ptr,
        )?)
    }

    fn set_size(&self, width: i32, height: i32) -> Result<()> {
        let size_ptr: *mut CGSize = &mut CGSize::new(width as f64, height as f64);
        let size = NonNull::new(size_ptr.cast()).unwrap();
        let size = unsafe { AXValue::new(AXValueType::CGSize, size) }.unwrap();
        Ok(set_attribute_value(
            &self.element,
            &kAXSizeAttribute(),
            &size,
        )?)
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

/// Abstraction over macOS accessibility (AX) window operations.
///
/// Read methods take a `&DispatcherMarker` parameter to enforce at compile time
/// that blocking AX IPC calls only happen on GCD dispatch queues, never on the
/// dome thread. Write methods (which are intentionally dome-thread operations)
/// and pure getters omit the marker.
pub(super) trait AXWindowApi: Send + Sync + std::fmt::Display {
    fn cg_id(&self) -> CGWindowID;
    fn pid(&self) -> i32;
    fn is_native_fullscreen(&self, marker: &DispatcherMarker) -> bool;
    fn get_position(&self, marker: &DispatcherMarker) -> Result<(i32, i32)>;
    fn get_size(&self, marker: &DispatcherMarker) -> Result<(i32, i32)>;
    fn set_frame(&self, x: i32, y: i32, width: i32, height: i32) -> Result<()>;
    fn focus(&self) -> Result<()>;
    fn hide_at(&self, x: i32, y: i32) -> Result<()>;
    fn minimize(&self) -> Result<()>;
    fn unminimize(&self) -> Result<()>;
    fn is_valid(&self, marker: &DispatcherMarker) -> bool;
    fn is_minimized(&self, marker: &DispatcherMarker) -> bool;
    fn read_title(&self, marker: &DispatcherMarker) -> Option<String>;
    /// Refresh the cached `kAXEnhancedUserInterfaceAttribute` probe for this
    /// window's app. Deduplication by PID is the caller's responsibility.
    fn refresh_enhanced_ui(&self, marker: &DispatcherMarker);
}

// The marker proves the caller is on a GCD queue. It isn't forwarded to the
// concrete `AXWindow` methods because those are the underlying implementation
// that the trait delegates to.
impl AXWindowApi for AXWindow {
    fn cg_id(&self) -> CGWindowID {
        self.cg_id()
    }
    fn pid(&self) -> i32 {
        self.pid()
    }
    fn is_native_fullscreen(&self, _marker: &DispatcherMarker) -> bool {
        self.is_native_fullscreen()
    }
    fn get_position(&self, _marker: &DispatcherMarker) -> Result<(i32, i32)> {
        self.get_position()
    }
    fn get_size(&self, _marker: &DispatcherMarker) -> Result<(i32, i32)> {
        self.get_size()
    }
    fn set_frame(&self, x: i32, y: i32, w: i32, h: i32) -> Result<()> {
        self.set_frame(x, y, w, h)
    }
    fn focus(&self) -> Result<()> {
        self.focus()
    }
    fn hide_at(&self, x: i32, y: i32) -> Result<()> {
        self.hide_at(x, y)
    }
    fn minimize(&self) -> Result<()> {
        self.minimize()
    }
    fn unminimize(&self) -> Result<()> {
        self.unminimize()
    }
    fn is_valid(&self, _marker: &DispatcherMarker) -> bool {
        self.is_valid()
    }
    fn is_minimized(&self, _marker: &DispatcherMarker) -> bool {
        self.is_minimized()
    }
    fn read_title(&self, _marker: &DispatcherMarker) -> Option<String> {
        get_attribute::<CFString>(&self.element, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok()
    }
    fn refresh_enhanced_ui(&self, _marker: &DispatcherMarker) {
        self.app.refresh_enhanced_ui();
    }
}
