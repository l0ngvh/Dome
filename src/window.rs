use anyhow::Result;
use std::ptr::NonNull;

use objc2_application_services::{AXError, AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFRetained, CFString, CGPoint, CGSize, kCFBooleanFalse, kCFBooleanTrue,
};

#[derive(Debug)]
pub(crate) struct MacWindow(pub(crate) CFRetained<AXUIElement>);

impl MacWindow {
    #[tracing::instrument]
    pub(crate) fn set_position(&self, x: f32, y: f32) -> Result<()> {
        let window = &self.0;
        let pos_ptr: *mut CGPoint = &mut CGPoint::new(x as f64, y as f64);
        let pos_ptr = NonNull::new(pos_ptr.cast()).unwrap();
        let pos_ptr = unsafe { AXValue::new(AXValueType::CGPoint, pos_ptr) }.unwrap();
        let res = unsafe {
            window.set_attribute_value(&CFString::from_static_str("AXPosition"), &pos_ptr)
        };
        if res != AXError::Success {
            Err(anyhow::anyhow!(
                "Failed to set attribute. Error code: {res:?}"
            ))
        } else {
            Ok(())
        }
    }

    #[tracing::instrument]
    pub(crate) fn set_size(&self, width: f32, height: f32) -> Result<()> {
        let window = &self.0;
        let size_ptr: *mut CGSize = &mut CGSize::new(width as f64, height as f64);
        let size = NonNull::new(size_ptr.cast()).unwrap();
        let size = unsafe { AXValue::new(AXValueType::CGSize, size) }.unwrap();
        let res =
            unsafe { window.set_attribute_value(&CFString::from_static_str("AXSize"), &size) };
        if res != AXError::Success {
            Err(anyhow::anyhow!("Failed to set size. Error code: {res:?}"))
        } else {
            Ok(())
        }
    }

    // Should introduce a new flag, hidden, in the window, as depend on OS we might want to emulate
    // how windows are resized. For example, if we don't want to use minimize api and rather move
    // it offscreen, it should be os dependent and abstracted away
    // e.g. if it's hidden, then update the stored position + size, but don't call the modify api.
    // Then when it's show, call modify api to sync the position + size
    pub(crate) fn hide(&self) -> Result<()> {
        let res = unsafe {
            self.0.set_attribute_value(
                &CFString::from_static_str("AXMinimized"),
                kCFBooleanTrue.unwrap(),
            )
        };
        if res != AXError::Success {
            Err(anyhow::anyhow!(
                "Failed to set attribute. Error code: {res:?}"
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn show(&self) -> Result<()> {
        let res = unsafe {
            self.0.set_attribute_value(
                &CFString::from_static_str("AXMinimized"),
                kCFBooleanFalse.unwrap(),
            )
        };
        if res != AXError::Success {
            Err(anyhow::anyhow!(
                "Failed to set attribute. Error code: {res:?}"
            ))
        } else {
            Ok(())
        }
    }
}
