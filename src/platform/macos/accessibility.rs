use anyhow::{Context, Result};
use std::ptr::NonNull;

use objc2_app_kit::NSRunningApplication;
use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFBoolean, CFDictionary, CFEqual, CFRetained, CFString, CFType, CGPoint, CGSize,
    kCFBooleanFalse, kCFBooleanTrue,
};
use objc2_core_graphics::{CGSessionCopyCurrentDictionary, CGWindowID};

use crate::core::Dimension;

use super::objc2_wrapper::{
    AXError, get_attribute, is_attribute_settable, kAXEnhancedUserInterfaceAttribute,
    kAXFrontmostAttribute, kAXFullScreenAttribute, kAXMainAttribute, kAXMinimizedAttribute,
    kAXParentAttribute, kAXPositionAttribute, kAXRoleAttribute, kAXSizeAttribute,
    kAXStandardWindowSubrole, kAXSubroleAttribute, kAXTitleAttribute, kAXWindowRole,
    set_attribute_value,
};

#[derive(Clone)]
pub(super) struct AXWindow {
    element: CFRetained<AXUIElement>,
    app: CFRetained<AXUIElement>,
    cg_id: CGWindowID,
    pid: i32,
    app_name: Option<String>,
    bundle_id: Option<String>,
    title: Option<String>,
}

// Safety: AXUIElement operations are IPC calls to the accessibility server,
// safe to use from any thread for manipulating other applications' windows.
unsafe impl Send for AXWindow {}

impl std::fmt::Display for AXWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}:{}] {}", self.pid, self.cg_id, self.app_name_())?;
        if let Some(bundle_id) = &self.bundle_id {
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
        app: &NSRunningApplication,
    ) -> Self {
        let pid = app.processIdentifier();
        let ax_app = unsafe { AXUIElement::new_application(pid) };
        let app_name = app.localizedName().map(|n| n.to_string());
        let bundle_id = app.bundleIdentifier().map(|b| b.to_string());
        let title = get_attribute::<CFString>(&element, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok();

        Self {
            element,
            app: ax_app,
            cg_id,
            pid,
            app_name,
            bundle_id,
            title,
        }
    }

    pub(super) fn cg_id(&self) -> CGWindowID {
        self.cg_id
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

    pub(super) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(super) fn update_title(&mut self) {
        self.title = get_attribute::<CFString>(&self.element, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok();
    }

    pub(super) fn should_tile(&self) -> bool {
        !self.is_native_fullscreen()
    }

    pub(super) fn is_native_fullscreen(&self) -> bool {
        get_attribute::<CFBoolean>(&self.element, &kAXFullScreenAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false)
    }

    pub(super) fn is_mock_fullscreen(&self, monitor: &Dimension) -> bool {
        let Ok((x, y)) = self.get_position() else {
            return false;
        };
        let Ok((w, h)) = self.get_size() else {
            return false;
        };
        let tolerance = 2;
        (x - monitor.x as i32).abs() <= tolerance
            && (y - monitor.y as i32).abs() <= tolerance
            && (w - monitor.width as i32).abs() <= tolerance
            && (h - monitor.height as i32).abs() <= tolerance
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
            tracing::trace!(app = %self.app_name_(), title = ?self.title, "not valid: window is deleted");
            return false;
        }
        let is_minimized = get_attribute::<CFBoolean>(&self.element, &kAXMinimizedAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if is_minimized {
            tracing::trace!(app = %self.app_name_(), title = ?self.title, "not valid: window is minimized");
            return false;
        }
        true
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
        let is_frontmost = get_attribute::<CFBoolean>(&self.app, &kAXFrontmostAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !is_frontmost {
            set_attribute_value(&self.app, &kAXFrontmostAttribute(), unsafe {
                kCFBooleanTrue.unwrap()
            })
            .with_context(|| format!("focus for {self}"))?;
        }
        let is_main = get_attribute::<CFBoolean>(&self.element, &kAXMainAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !is_main {
            set_attribute_value(&self.element, &kAXMainAttribute(), unsafe {
                kCFBooleanTrue.unwrap()
            })
            .with_context(|| format!("focus for {self}"))?;
        }
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

        let is_root = match get_attribute::<AXUIElement>(&self.element, &kAXParentAttribute()) {
            Err(_) => true,
            Ok(parent) => CFEqual(Some(&*parent), Some(&*self.app)),
        };
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
        let was_enabled =
            get_attribute::<CFBoolean>(&self.app, &kAXEnhancedUserInterfaceAttribute()).ok();
        let _ = set_attribute_value(&self.app, &kAXEnhancedUserInterfaceAttribute(), unsafe {
            kCFBooleanFalse.unwrap()
        });
        let result = f();
        if was_enabled.is_some() {
            let _ = set_attribute_value(&self.app, &kAXEnhancedUserInterfaceAttribute(), unsafe {
                kCFBooleanTrue.unwrap()
            });
        }
        result
    }

    fn set_position(&self, x: i32, y: i32) -> Result<()> {
        if self.get_position().ok() == Some((x, y)) {
            return Ok(());
        }
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
        if self.get_size().ok() == Some((width, height)) {
            return Ok(());
        }
        let size_ptr: *mut CGSize = &mut CGSize::new(width as f64, height as f64);
        let size = NonNull::new(size_ptr.cast()).unwrap();
        let size = unsafe { AXValue::new(AXValueType::CGSize, size) }.unwrap();
        Ok(set_attribute_value(
            &self.element,
            &kAXSizeAttribute(),
            &size,
        )?)
    }

    fn app_name_(&self) -> &str {
        self.app_name.as_deref().unwrap_or("Unknown")
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
