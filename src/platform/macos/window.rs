use anyhow::{Context, Result};
use std::ptr::NonNull;

use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFBoolean, CFDictionary, CFRetained, CFString, CFType, CGPoint, CGSize, kCFBooleanFalse,
    kCFBooleanTrue,
};
use objc2_core_graphics::CGSessionCopyCurrentDictionary;

use super::objc2_wrapper::{
    AXError, get_attribute, kAXEnhancedUserInterfaceAttribute, kAXFrontmostAttribute,
    kAXMainAttribute, kAXPositionAttribute, kAXRoleAttribute, kAXSizeAttribute, kAXTitleAttribute,
    set_attribute_value,
};
use crate::core::Dimension;

pub(super) struct AXWindow {
    window: CFRetained<AXUIElement>,
    app: CFRetained<AXUIElement>,
    pid: i32,
    screen: Dimension,
}

impl AXWindow {
    pub(super) fn new(
        window: CFRetained<AXUIElement>,
        app: CFRetained<AXUIElement>,
        pid: i32,
        screen: Dimension,
    ) -> Self {
        Self {
            window,
            app,
            pid,
            screen,
        }
    }

    pub(super) fn pid(&self) -> i32 {
        self.pid
    }

    pub(super) fn title(&self) -> Option<String> {
        get_attribute::<CFString>(&self.window, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .ok()
    }

    /// As we're tracking windows with CGWindowID, we have to check whether a window is still valid
    /// as macOS can reuse CGWindowID of deleted windows.
    pub(super) fn is_valid(&self) -> bool {
        if is_screen_locked() {
            return true;
        }
        !matches!(
            get_attribute::<CFString>(&self.window, &kAXRoleAttribute()),
            Err(AXError::InvalidUIElement)
        )
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn set_dimension(&self, dim: Dimension) -> Result<()> {
        self.with_animation_disabled(|| {
            self.set_position(dim.x, dim.y)?;
            self.set_size(dim.width, dim.height)
        })
        .with_context(|| format!("set_dimension for pid {}", self.pid))
    }

    pub(super) fn focus(&self) -> Result<()> {
        let is_frontmost = get_attribute::<CFBoolean>(&self.app, &kAXFrontmostAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !is_frontmost {
            set_attribute_value(&self.app, &kAXFrontmostAttribute(), unsafe {
                kCFBooleanTrue.unwrap()
            })
            .with_context(|| format!("focus for pid {}", self.pid))?;
        }
        let is_main = get_attribute::<CFBoolean>(&self.window, &kAXMainAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !is_main {
            set_attribute_value(&self.window, &kAXMainAttribute(), unsafe {
                kCFBooleanTrue.unwrap()
            })
            .with_context(|| format!("focus for pid {}", self.pid))?;
        }
        Ok(())
    }

    /// Hide the window by moving it offscreen
    /// We don't minimize windows as there is no way to disable minimizing animation. When hiding
    /// multiple windows, it gets triggered in a staggered manner, which is extremely slow, and
    /// causes event tap to be timed out
    pub(super) fn hide(&self) -> Result<()> {
        // MacOS doesn't allow completely set windows offscreen, so we need to leave at
        // least one pixel left
        // https://nikitabobko.github.io/AeroSpace/guide#emulation-of-virtual-workspaces
        self.with_animation_disabled(|| {
            self.set_position(
                self.screen.x + self.screen.width - 1.0,
                self.screen.y + self.screen.height - 1.0,
            )
        })
        .with_context(|| format!("hide for pid {}", self.pid))
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

    fn set_position(&self, x: f32, y: f32) -> anyhow::Result<()> {
        let pos_ptr: *mut CGPoint = &mut CGPoint::new(x as f64, y as f64);
        let pos_ptr = NonNull::new(pos_ptr.cast()).unwrap();
        let pos_ptr = unsafe { AXValue::new(AXValueType::CGPoint, pos_ptr) }.unwrap();
        Ok(set_attribute_value(
            &self.window,
            &kAXPositionAttribute(),
            &pos_ptr,
        )?)
    }

    fn set_size(&self, width: f32, height: f32) -> anyhow::Result<()> {
        let size_ptr: *mut CGSize = &mut CGSize::new(width as f64, height as f64);
        let size = NonNull::new(size_ptr.cast()).unwrap();
        let size = unsafe { AXValue::new(AXValueType::CGSize, size) }.unwrap();
        Ok(set_attribute_value(
            &self.window,
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
