use anyhow::{Context, Result};
use std::{collections::HashMap, ptr::NonNull};

use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFBoolean, CFDictionary, CFRetained, CFString, CFType, CGPoint, CGSize, kCFBooleanFalse,
    kCFBooleanTrue,
};
use objc2_core_graphics::{CGSessionCopyCurrentDictionary, CGWindowID};

use super::objc2_wrapper::{
    AXError, get_attribute, kAXEnhancedUserInterfaceAttribute, kAXFrontmostAttribute,
    kAXMainAttribute, kAXMinimizedAttribute, kAXPositionAttribute, kAXRoleAttribute,
    kAXSizeAttribute, set_attribute_value,
};
use crate::core::Dimension;

#[derive(Clone)]
pub(super) struct AXWindow {
    window: CFRetained<AXUIElement>,
    app: CFRetained<AXUIElement>,
    pid: i32,
    screen: Dimension,
    app_name: String,
    title: Option<String>,
}

impl std::fmt::Display for AXWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.title {
            Some(title) => write!(f, "AXWindow(app={}, title={})", self.app_name, title),
            None => write!(f, "AXWindow(app={})", self.app_name),
        }
    }
}

impl AXWindow {
    pub(super) fn new(
        window: CFRetained<AXUIElement>,
        app: CFRetained<AXUIElement>,
        pid: i32,
        screen: Dimension,
        app_name: String,
        title: Option<String>,
    ) -> Self {
        Self {
            window,
            app,
            pid,
            screen,
            app_name,
            title,
        }
    }

    pub(super) fn pid(&self) -> i32 {
        self.pid
    }

    /// As we're tracking windows with CGWindowID, we have to check whether a window is still valid
    /// as macOS can reuse CGWindowID of deleted windows.
    pub(super) fn is_valid(&self) -> bool {
        if is_screen_locked() {
            return true;
        }
        let is_deleted = matches!(
            get_attribute::<CFString>(&self.window, &kAXRoleAttribute()),
            Err(AXError::InvalidUIElement)
        );
        if is_deleted {
            tracing::trace!(app = %self.app_name, title = ?self.title, "not valid: window is deleted");
            return false;
        }
        let is_minimized = get_attribute::<CFBoolean>(&self.window, &kAXMinimizedAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if is_minimized {
            tracing::trace!(app = %self.app_name, title = ?self.title, "not valid: window is minimized");
            return false;
        }
        true
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn set_dimension(&self, dim: Dimension) -> Result<()> {
        self.with_animation_disabled(|| {
            self.set_position(dim.x, dim.y)?;
            self.set_size(dim.width, dim.height)
        })
        .with_context(|| format!("set_dimension for {} {:?}", self.app_name, self.title))
    }

    pub(super) fn focus(&self) -> Result<()> {
        let is_frontmost = get_attribute::<CFBoolean>(&self.app, &kAXFrontmostAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !is_frontmost {
            set_attribute_value(&self.app, &kAXFrontmostAttribute(), unsafe {
                kCFBooleanTrue.unwrap()
            })
            .with_context(|| format!("focus for {} {:?}", self.app_name, self.title))?;
        }
        let is_main = get_attribute::<CFBoolean>(&self.window, &kAXMainAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !is_main {
            set_attribute_value(&self.window, &kAXMainAttribute(), unsafe {
                kCFBooleanTrue.unwrap()
            })
            .with_context(|| format!("focus for {} {:?}", self.app_name, self.title))?;
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
        .with_context(|| format!("hide for {} {:?}", self.app_name, self.title))
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

    pub(super) fn get_size(&self) -> Result<(f32, f32)> {
        let size = get_attribute::<AXValue>(&self.window, &kAXSizeAttribute())
            .with_context(|| format!("get_size for {} {:?}", self.app_name, self.title))?;
        let mut cg_size = CGSize::new(0.0, 0.0);
        let ptr = NonNull::new((&mut cg_size as *mut CGSize).cast()).unwrap();
        unsafe { size.value(AXValueType::CGSize, ptr) };
        Ok((cg_size.width as f32, cg_size.height as f32))
    }
}

pub(super) struct AXRegistry {
    windows: HashMap<CGWindowID, AXWindow>,
    pid_to_cg: HashMap<i32, Vec<CGWindowID>>,
}

impl AXRegistry {
    pub(super) fn new() -> Self {
        Self {
            windows: HashMap::new(),
            pid_to_cg: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, cg_id: CGWindowID, window: AXWindow) {
        let pid = window.pid();
        self.pid_to_cg.entry(pid).or_default().push(cg_id);
        self.windows.insert(cg_id, window);
    }

    pub(super) fn remove(&mut self, cg_id: CGWindowID) -> Option<AXWindow> {
        let window = self.windows.remove(&cg_id)?;
        if let Some(ids) = self.pid_to_cg.get_mut(&window.pid()) {
            ids.retain(|&id| id != cg_id);
        }
        Some(window)
    }

    pub(super) fn get(&self, cg_id: CGWindowID) -> Option<&AXWindow> {
        self.windows.get(&cg_id)
    }

    pub(super) fn contains(&self, cg_id: CGWindowID) -> bool {
        self.windows.contains_key(&cg_id)
    }

    pub(super) fn cg_ids_for_pid(&self, pid: i32) -> Vec<CGWindowID> {
        self.pid_to_cg.get(&pid).cloned().unwrap_or_default()
    }

    pub(super) fn remove_by_pid(&mut self, pid: i32) -> Vec<CGWindowID> {
        let Some(cg_ids) = self.pid_to_cg.remove(&pid) else {
            return Vec::new();
        };
        for &cg_id in &cg_ids {
            self.windows.remove(&cg_id);
        }
        cg_ids
    }

    pub(super) fn is_valid(&self, cg_id: CGWindowID) -> bool {
        self.windows.get(&cg_id).is_some_and(|w| w.is_valid())
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (CGWindowID, &AXWindow)> {
        self.windows.iter().map(|(&id, w)| (id, w))
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
