use anyhow::Result;
use std::ptr::NonNull;

use objc2_app_kit::NSRunningApplication;
use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFRetained, CFString, CGPoint, CGSize, kCFBooleanFalse, kCFBooleanTrue,
};

use crate::objc2_wrapper::{
    get_attribute, kAXFrontmostAttribute, kAXMainAttribute, kAXMinimizedAttribute,
    kAXPositionAttribute, kAXRoleAttribute, kAXSizeAttribute, kAXTitleAttribute,
    set_attribute_value,
};

#[derive(Debug)]
pub(crate) struct MacWindow {
    window: CFRetained<AXUIElement>,
    app: CFRetained<AXUIElement>,
    pid: i32,
    running_app: objc2::rc::Retained<NSRunningApplication>,
}

impl MacWindow {
    pub(crate) fn new(
        window: CFRetained<AXUIElement>,
        app: CFRetained<AXUIElement>,
        pid: i32,
    ) -> Self {
        let running_app =
            NSRunningApplication::runningApplicationWithProcessIdentifier(pid).unwrap();

        Self {
            window,
            app,
            pid,
            running_app,
        }
    }
    #[tracing::instrument(skip(self))]
    pub(crate) fn set_position(&self, x: f32, y: f32) -> Result<()> {
        let pos_ptr: *mut CGPoint = &mut CGPoint::new(x as f64, y as f64);
        let pos_ptr = NonNull::new(pos_ptr.cast()).unwrap();
        let pos_ptr = unsafe { AXValue::new(AXValueType::CGPoint, pos_ptr) }.unwrap();
        set_attribute_value(&self.window, &kAXPositionAttribute(), &pos_ptr)
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn set_size(&self, width: f32, height: f32) -> Result<()> {
        let size_ptr: *mut CGSize = &mut CGSize::new(width as f64, height as f64);
        let size = NonNull::new(size_ptr.cast()).unwrap();
        let size = unsafe { AXValue::new(AXValueType::CGSize, size) }.unwrap();
        set_attribute_value(&self.window, &kAXSizeAttribute(), &size)
    }

    // Should introduce a new flag, hidden, in the window, as depend on OS we might want to emulate
    // how windows are resized. For example, if we don't want to use minimize api and rather move
    // it offscreen, it should be os dependent and abstracted away
    // e.g. if it's hidden, then update the stored position + size, but don't call the modify api.
    // Then when it's show, call modify api to sync the position + size
    pub(crate) fn hide(&self) -> Result<()> {
        set_attribute_value(&self.window, &kAXMinimizedAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })
    }

    pub(crate) fn show(&self) -> Result<()> {
        set_attribute_value(&self.window, &kAXMinimizedAttribute(), unsafe {
            kCFBooleanFalse.unwrap()
        })
    }

    pub(crate) fn focus(&self) -> Result<()> {
        set_attribute_value(&self.app, &kAXFrontmostAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })?;
        set_attribute_value(&self.window, &kAXMainAttribute(), unsafe {
            kCFBooleanTrue.unwrap()
        })
    }

    pub(crate) fn is_valid(&self) -> bool {
        // Technically we should check for InvalidUIElement (-25202), but if a we could get the window
        // role fine previously, then we can't, then it's most likely that window got deleted
        get_attribute::<CFString>(&self.window, &kAXRoleAttribute()).is_ok()
    }
}

impl std::fmt::Display for MacWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let window_title = get_attribute::<CFString>(&self.window, &kAXTitleAttribute())
            .map(|t| t.to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        let app_name = self
            .running_app
            .localizedName()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        write!(
            f,
            "'{}' from app '{}' (PID: {})",
            window_title, app_name, self.pid
        )
    }
}
