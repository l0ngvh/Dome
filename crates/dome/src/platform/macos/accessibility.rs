mod application;
mod observer;
mod queue;
mod reads;

pub(super) use application::{AccessibilityApplication, FocusState};

use dome_core::FractionalCoordinate;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use std::ptr::NonNull;

use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFEqual, CFRetained, CFString, CFType, CGPoint, CGSize,
    kCFBooleanFalse, kCFBooleanTrue,
};
use objc2_core_graphics::{CGSessionCopyCurrentDictionary, CGWindowID};

use crate::platform::macos::objc2_wrapper::{
    AXError, get_attribute, get_cg_window_id, is_attribute_settable, kAXCloseButtonAttribute,
    kAXEnhancedUserInterfaceAttribute, kAXFocusedWindowAttribute, kAXFrontmostAttribute,
    kAXFullScreenAttribute, kAXMainAttribute, kAXMinimizedAttribute, kAXParentAttribute,
    kAXPositionAttribute, kAXPressAction, kAXRoleAttribute, kAXSizeAttribute, kAXSubroleAttribute,
    kAXTitleAttribute, kAXWindowsAttribute, perform_action, set_attribute_value,
};
use dome_core::{Dimension, FractionalLength, Logical, PixelRect};

/// Per-app accessibility state shared across all windows of the same application.
///
/// Holds the AXUIElement for the app process and cached app metadata (name, bundle ID).
/// Always used behind `Arc<AXApp>` so multiple `AXWindow`s from the same app share one instance.
struct AXApp {
    element: CFRetained<AXUIElement>,
    pid: i32,
    app_name: Option<String>,
    bundle_id: Option<String>,
    /// Whether `kAXEnhancedUserInterfaceAttribute` is settable on this app.
    can_set_enhanced_ui: AtomicBool,
}

// Safety: AXUIElement operations are IPC calls to the accessibility server,
// safe to use from any thread for manipulating other applications' windows.
unsafe impl Send for AXApp {}
unsafe impl Sync for AXApp {}

impl AXApp {
    pub(super) fn pid(&self) -> i32 {
        self.pid
    }

    pub(super) fn element(&self) -> &AXUIElement {
        &self.element
    }

    pub(super) fn app_name(&self) -> Option<&str> {
        self.app_name.as_deref()
    }

    pub(super) fn bundle_id(&self) -> Option<&str> {
        self.bundle_id.as_deref()
    }

    /// Whether `kAXEnhancedUserInterfaceAttribute` is settable on this app.
    /// Uses a cached value. `Relaxed` ordering is fine since this is a
    /// performance hint, not a synchronization primitive.
    pub(super) fn can_set_enhanced_ui(&self) -> bool {
        self.can_set_enhanced_ui.load(Ordering::Relaxed)
    }

    pub(super) fn set_frontmost(&self) -> Result<(), AXError> {
        set_attribute_value(&self.element, &kAXFrontmostAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })
    }

    pub(super) fn refresh_enhanced_ui(&self) {
        let settable = is_attribute_settable(&self.element, &kAXEnhancedUserInterfaceAttribute());
        self.can_set_enhanced_ui.store(settable, Ordering::Relaxed);
    }

    /// Returns `true` if the window's parent is the app element itself (i.e. a
    /// top-level window, not a sheet or child panel).
    pub(super) fn is_root_window(&self, window_element: &AXUIElement) -> bool {
        match get_attribute::<AXUIElement>(window_element, &kAXParentAttribute()) {
            Err(_) => true,
            Ok(parent) => CFEqual(Some(&*parent), Some(&*self.element)),
        }
    }

    pub(super) fn focused_window(self: Arc<Self>) -> Result<AXWindow, AXError> {
        let focused = get_attribute::<AXUIElement>(&self.element, &kAXFocusedWindowAttribute())?;
        let cg_id = get_cg_window_id(&focused)?;
        Ok(AXWindow::new(focused, cg_id, self))
    }

    pub(super) fn set_enhanced_ui(&self, enabled: bool) -> Result<(), AXError> {
        let value = if enabled {
            unsafe { kCFBooleanTrue.unwrap() }
        } else {
            unsafe { kCFBooleanFalse.unwrap() }
        };
        set_attribute_value(&self.element, &kAXEnhancedUserInterfaceAttribute(), value)
    }

    pub(super) fn windows(self: Arc<Self>) -> Result<Vec<AXWindow>, AXError> {
        let windows = get_attribute::<CFArray<AXUIElement>>(&self.element, &kAXWindowsAttribute())?;
        let windows = windows
            .into_iter()
            .filter_map(|w| {
                // TODO: figure out how to logs this error, as windows should have cg_id under
                // normal circumstances
                let Ok(cg_id) = get_cg_window_id(&w) else {
                    return None;
                };
                Some(AXWindow::new(w, cg_id, self.clone()))
            })
            .collect();
        Ok(windows)
    }

    fn new(pid: i32, app_name: Option<String>, bundle_id: Option<String>) -> Self {
        let element = unsafe { AXUIElement::new_application(pid) };
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
struct AXWindow {
    element: CFRetained<AXUIElement>,
    app: Arc<AXApp>,
    cg_id: CGWindowID,
    title: Option<String>,
    placement: Option<(PixelRect<Logical>, std::time::Instant)>,
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
            placement: None,
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

    pub(super) fn read_native_fullscreen(&self) -> Option<bool> {
        match get_attribute::<CFBoolean>(&self.element, &kAXFullScreenAttribute()) {
            Ok(value) => Some(value.as_bool()),
            Err(AXError::Other(objc2_application_services::AXError::AttributeUnsupported)) => {
                Some(false)
            }
            Err(_) => None,
        }
    }

    pub(super) fn get_position(
        &self,
    ) -> Result<(FractionalCoordinate<Logical>, FractionalCoordinate<Logical>)> {
        let pos = get_attribute::<AXValue>(&self.element, &kAXPositionAttribute())
            .with_context(|| format!("get_position for {self}"))?;
        let mut cg_pos = CGPoint::new(0.0, 0.0);
        let ptr = NonNull::new((&mut cg_pos as *mut CGPoint).cast()).unwrap();
        if !unsafe { pos.value(AXValueType::CGPoint, ptr) } {
            anyhow::bail!("get_position for {self}: AXValue is not a CGPoint");
        }
        anyhow::ensure!(
            cg_pos.x.is_finite() && cg_pos.y.is_finite(),
            "invalid accessibility window position"
        );
        Ok((
            FractionalCoordinate::new(cg_pos.x),
            FractionalCoordinate::new(cg_pos.y),
        ))
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
    pub(super) fn set_frame(&self, rect: PixelRect<Logical>) -> Result<()> {
        let dim = rect.to_dimension();
        self.with_animation_disabled(|| {
            self.set_position(dim.x, dim.y)?;
            self.set_size(dim.width, dim.height)
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

    /// Hide the window by moving it offscreen.
    /// We don't minimize windows as there is no way to disable minimizing animation. When hiding
    /// multiple windows, it gets triggered in a staggered manner, which is extremely slow.
    pub(super) fn hide_at(
        &self,
        x: FractionalCoordinate<Logical>,
        y: FractionalCoordinate<Logical>,
    ) -> Result<()> {
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

    pub(super) fn close(&self) -> Result<()> {
        let button = match get_attribute::<AXUIElement>(&self.element, &kAXCloseButtonAttribute()) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(window = %self, "close button missing: {e}");
                return Ok(());
            }
        };
        perform_action(&button, &kAXPressAction()).with_context(|| format!("close for {self}"))
    }

    pub(super) fn get_size(
        &self,
    ) -> Result<(FractionalLength<Logical>, FractionalLength<Logical>)> {
        let size = get_attribute::<AXValue>(&self.element, &kAXSizeAttribute())
            .with_context(|| format!("get_size for {self}"))?;
        let mut cg_size = CGSize::new(0.0, 0.0);
        let ptr = NonNull::new((&mut cg_size as *mut CGSize).cast()).unwrap();
        if !unsafe { size.value(AXValueType::CGSize, ptr) } {
            anyhow::bail!("get_size for {self}: AXValue is not a CGSize");
        }
        anyhow::ensure!(
            cg_size.width.is_finite()
                && cg_size.height.is_finite()
                && cg_size.width >= 0.0
                && cg_size.height >= 0.0,
            "invalid accessibility window size"
        );
        Ok((
            FractionalLength::new(cg_size.width),
            FractionalLength::new(cg_size.height),
        ))
    }

    pub(super) fn read_role(&self) -> Option<String> {
        get_attribute::<CFString>(&self.element, &kAXRoleAttribute())
            .ok()
            .map(|role| role.to_string())
    }

    pub(super) fn read_subrole(&self) -> Option<String> {
        get_attribute::<CFString>(&self.element, &kAXSubroleAttribute())
            .ok()
            .map(|subrole| subrole.to_string())
    }

    pub(super) fn is_root_window(&self) -> bool {
        self.app.is_root_window(&self.element)
    }

    pub(super) fn is_position_settable(&self) -> bool {
        is_attribute_settable(&self.element, &kAXPositionAttribute())
    }

    pub(super) fn is_size_settable(&self) -> bool {
        is_attribute_settable(&self.element, &kAXSizeAttribute())
    }

    pub(super) fn is_main_settable(&self) -> bool {
        is_attribute_settable(&self.element, &kAXMainAttribute())
    }

    fn with_animation_disabled<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce() -> Result<()>,
    {
        let can_set = self.app.can_set_enhanced_ui();
        let pid = self.app.pid();
        if can_set && let Err(err) = self.app.set_enhanced_ui(false) {
            crate::trace_once!(
                key: pid,
                window = %self, "Failed to disable enhanced UI: {err:#}"
            );
        }
        let result = f();
        if can_set && let Err(err) = self.app.set_enhanced_ui(true) {
            crate::trace_once!(
                key: pid,
                window = %self, "Failed to re-enable enhanced UI: {err:#}"
            );
        }
        result
    }

    fn set_position(
        &self,
        x: FractionalCoordinate<Logical>,
        y: FractionalCoordinate<Logical>,
    ) -> Result<()> {
        let pos_ptr: *mut CGPoint = &mut CGPoint::new(x.value(), y.value());
        let pos_ptr = NonNull::new(pos_ptr.cast()).unwrap();
        let pos_ptr = unsafe { AXValue::new(AXValueType::CGPoint, pos_ptr) }.unwrap();
        Ok(set_attribute_value(
            &self.element,
            &kAXPositionAttribute(),
            &pos_ptr,
        )?)
    }

    fn set_size(
        &self,
        width: FractionalLength<Logical>,
        height: FractionalLength<Logical>,
    ) -> Result<()> {
        let size_ptr: *mut CGSize = &mut CGSize::new(width.value(), height.value());
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

impl AXWindow {
    pub(super) fn read_title(&self) -> Option<String> {
        get_attribute::<CFString>(&self.element, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok()
    }
    pub(super) fn read_close_button_frame(&self) -> Option<Dimension<Logical>> {
        let button =
            get_attribute::<AXUIElement>(&self.element, &kAXCloseButtonAttribute()).ok()?;
        let position = get_attribute::<AXValue>(&button, &kAXPositionAttribute()).ok()?;
        let size = get_attribute::<AXValue>(&button, &kAXSizeAttribute()).ok()?;
        let mut origin = CGPoint::new(0.0, 0.0);
        let mut extent = CGSize::new(0.0, 0.0);
        let has_origin =
            unsafe { position.value(AXValueType::CGPoint, NonNull::from(&mut origin).cast()) };
        let has_extent =
            unsafe { size.value(AXValueType::CGSize, NonNull::from(&mut extent).cast()) };
        (has_origin
            && has_extent
            && extent.width.is_finite()
            && extent.height.is_finite()
            && extent.width >= 0.0
            && extent.height >= 0.0)
            .then(|| {
                Dimension::new(
                    FractionalCoordinate::new(origin.x),
                    FractionalCoordinate::new(origin.y),
                    FractionalLength::new(extent.width),
                    FractionalLength::new(extent.height),
                )
            })
    }
}
