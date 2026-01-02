use anyhow::{Context, Result};
use std::ptr::NonNull;

use objc2_app_kit::NSRunningApplication;
use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFBoolean, CFEqual, CFHash, CFRetained, CFString, CGPoint, CGSize, kCFBooleanFalse,
    kCFBooleanTrue,
};

use super::objc2_wrapper::{
    get_attribute, is_attribute_settable, kAXDialogSubrole, kAXEnhancedUserInterfaceAttribute,
    kAXFloatingWindowSubrole, kAXFrontmostAttribute, kAXFullScreenAttribute, kAXMainAttribute,
    kAXMinimizedAttribute, kAXParentAttribute, kAXPositionAttribute, kAXRoleAttribute,
    kAXSizeAttribute, kAXStandardWindowSubrole, kAXSubroleAttribute, kAXTitleAttribute,
    kAXWindowRole, set_attribute_value,
};
use crate::core::Dimension;

#[derive(Debug)]
pub(crate) struct MacWindow {
    window: CFRetained<AXUIElement>,
    app: CFRetained<AXUIElement>,
    pid: i32,
    running_app: objc2::rc::Retained<NSRunningApplication>,
    screen: Dimension,
    title: String,
    app_name: String,
}

impl MacWindow {
    pub(crate) fn new(
        window: CFRetained<AXUIElement>,
        app: CFRetained<AXUIElement>,
        pid: i32,
        screen: Dimension,
    ) -> Option<Self> {
        let running_app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
        if running_app.isTerminated() {
            return None;
        }

        let title = get_attribute::<CFString>(&window, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .unwrap_or_else(|_| "Unknown".to_string());
        let app_name = running_app
            .localizedName()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        Some(Self {
            window,
            app,
            pid,
            running_app,
            screen,
            title,
            app_name,
        })
    }

    pub(crate) fn cf_hash(&self) -> usize {
        CFHash(Some(&self.window))
    }

    pub(crate) fn pid(&self) -> i32 {
        self.pid
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn set_dimension(&self, dim: Dimension) -> Result<()> {
        self.with_animation_disabled(|| {
            self.set_position(dim.x, dim.y)?;
            self.set_size(dim.width, dim.height)
        })
        .with_context(|| format!("set_dimension for {self}"))
    }

    pub(crate) fn focus(&self) -> Result<()> {
        set_attribute_value(&self.app, &kAXFrontmostAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })?;
        set_attribute_value(&self.window, &kAXMainAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })
        .with_context(|| format!("focus for {self}"))
    }

    /// Hide the window by moving it offscreen
    /// We don't minimize windows as there is no way to disable minimizing animation. When hiding
    /// multiple windows, it gets triggered in a staggered manner, which is extremely slow, and
    /// causes event tap to be timed out
    pub(crate) fn hide(&self) -> Result<()> {
        // MacOS doesn't allow completely set windows offscreen, so we need to leave at
        // least one pixel left
        // https://nikitabobko.github.io/AeroSpace/guide#emulation-of-virtual-workspaces
        self.with_animation_disabled(|| {
            self.set_position(
                self.screen.x + self.screen.width - 1.0,
                self.screen.y + self.screen.height - 1.0,
            )
        })
        .with_context(|| format!("hide for {self}"))
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

    fn set_position(&self, x: f32, y: f32) -> Result<()> {
        let pos_ptr: *mut CGPoint = &mut CGPoint::new(x as f64, y as f64);
        let pos_ptr = NonNull::new(pos_ptr.cast()).unwrap();
        let pos_ptr = unsafe { AXValue::new(AXValueType::CGPoint, pos_ptr) }.unwrap();
        set_attribute_value(&self.window, &kAXPositionAttribute(), &pos_ptr)
    }

    fn set_size(&self, width: f32, height: f32) -> Result<()> {
        let size_ptr: *mut CGSize = &mut CGSize::new(width as f64, height as f64);
        let size = NonNull::new(size_ptr.cast()).unwrap();
        let size = unsafe { AXValue::new(AXValueType::CGSize, size) }.unwrap();
        set_attribute_value(&self.window, &kAXSizeAttribute(), &size)
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn update_title(&mut self) {
        if let Ok(t) = get_attribute::<CFString>(&self.window, &kAXTitleAttribute()) {
            self.title = t.to_string();
        }
    }

    pub(crate) fn dimension(&self) -> Dimension {
        let (x, y) = self.position();
        let (width, height) = self.size();
        Dimension {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns true if this is a "real" window worth managing (tile or float)
    pub(crate) fn is_manageable(&self) -> bool {
        let role = get_attribute::<CFString>(&self.window, &kAXRoleAttribute()).ok();
        let subrole = get_attribute::<CFString>(&self.window, &kAXSubroleAttribute()).ok();

        let is_window = role
            .as_ref()
            .map(|r| CFEqual(Some(&**r), Some(&*kAXWindowRole())))
            .unwrap_or(false);

        let is_valid_subrole = subrole
            .as_ref()
            .map(|sr| {
                CFEqual(Some(&**sr), Some(&*kAXStandardWindowSubrole()))
                    || CFEqual(Some(&**sr), Some(&*kAXDialogSubrole()))
                    || CFEqual(Some(&**sr), Some(&*kAXFloatingWindowSubrole()))
            })
            .unwrap_or(false);

        is_window
            && is_valid_subrole
            && self.is_root()
            && self.can_move()
            && self.can_focus()
            && !self.is_minimized()
            && !self.is_hidden()
    }

    /// Returns true if this window should be tiled (not floated)
    pub(crate) fn should_tile(&self) -> bool {
        let subrole = get_attribute::<CFString>(&self.window, &kAXSubroleAttribute()).ok();
        let is_standard = subrole
            .as_ref()
            .map(|sr| CFEqual(Some(&**sr), Some(&*kAXStandardWindowSubrole())))
            .unwrap_or(false);

        is_standard && self.can_resize() && !self.is_fullscreen()
    }

    fn is_root(&self) -> bool {
        match get_attribute::<AXUIElement>(&self.window, &kAXParentAttribute()) {
            Err(_) => true,
            Ok(parent) => CFEqual(Some(&*parent), Some(&*self.app)),
        }
    }

    fn can_move(&self) -> bool {
        is_attribute_settable(&self.window, &kAXPositionAttribute())
    }

    fn can_resize(&self) -> bool {
        is_attribute_settable(&self.window, &kAXSizeAttribute())
    }

    fn is_fullscreen(&self) -> bool {
        get_attribute::<CFBoolean>(&self.window, &kAXFullScreenAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false)
    }

    fn is_minimized(&self) -> bool {
        get_attribute::<CFBoolean>(&self.window, &kAXMinimizedAttribute())
            .map(|b| b.as_bool())
            .unwrap_or(false)
    }

    fn is_hidden(&self) -> bool {
        self.running_app.isHidden()
    }

    fn can_focus(&self) -> bool {
        is_attribute_settable(&self.window, &kAXMainAttribute())
    }

    fn position(&self) -> (f32, f32) {
        get_attribute::<AXValue>(&self.window, &kAXPositionAttribute())
            .map(|v| {
                let mut pos = CGPoint::new(0.0, 0.0);
                let ptr = NonNull::new(&mut pos as *mut _ as *mut _).unwrap();
                unsafe { v.value(AXValueType::CGPoint, ptr) };
                (pos.x as f32, pos.y as f32)
            })
            .unwrap_or((0.0, 0.0))
    }

    fn size(&self) -> (f32, f32) {
        get_attribute::<AXValue>(&self.window, &kAXSizeAttribute())
            .map(|v| {
                let mut size = CGSize::new(0.0, 0.0);
                let ptr = NonNull::new(&mut size as *mut _ as *mut _).unwrap();
                unsafe { v.value(AXValueType::CGSize, ptr) };
                (size.width as f32, size.height as f32)
            })
            .unwrap_or((0.0, 0.0))
    }
}

impl std::fmt::Display for MacWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "'{}' from app '{}' (PID: {})",
            self.title, self.app_name, self.pid
        )
    }
}
