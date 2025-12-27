use std::ptr::NonNull;

use anyhow::Result;
use objc2_application_services::{AXError, AXObserver, AXObserverCallback, AXUIElement};
use objc2_core_foundation::{CFRetained, CFString, CFType};

pub(crate) fn get_attribute<T: objc2_core_foundation::Type>(
    element: &AXUIElement,
    attribute: &CFString,
) -> Result<CFRetained<T>> {
    let mut value: *const CFType = std::ptr::null();
    let value_ptr = NonNull::new(&mut value as *mut *const CFType).unwrap();

    let res = unsafe { element.copy_attribute_value(attribute, value_ptr) };
    // TODO: return no value error as None
    if res != AXError::Success {
        return Err(anyhow::anyhow!(
            "Failed to get value for attribute {}: {}",
            attribute,
            decorate_ax_error_message(res)
        ));
    }
    let value = unsafe { *value_ptr.as_ptr() as *mut T };
    // Safety: value shouldn't be null as copy attribute call success
    let value = NonNull::new(value).unwrap();
    let value = unsafe { CFRetained::from_raw(value) };
    Ok(value)
}

pub(crate) fn set_attribute_value(
    element: &AXUIElement,
    attribute: &CFString,
    value: &CFType,
) -> Result<()> {
    let res = unsafe { element.set_attribute_value(attribute, value) };
    if res != AXError::Success {
        return Err(anyhow::anyhow!(
            "Failed to set attribute {}: {}",
            attribute,
            decorate_ax_error_message(res)
        ));
    }
    Ok(())
}

pub(crate) fn get_pid(element: &AXUIElement) -> Result<i32> {
    let mut pid = 0;
    let value_ptr = NonNull::new(&mut pid as *mut i32).unwrap();
    let res = unsafe { element.pid(value_ptr) };
    if res != AXError::Success {
        return Err(anyhow::anyhow!(
            "Failed to get pid for element: {}",
            decorate_ax_error_message(res)
        ));
    }
    Ok(pid)
}

pub(crate) fn create_observer(
    pid: i32,
    callback: AXObserverCallback,
) -> Result<CFRetained<AXObserver>> {
    let mut observer: *mut AXObserver = std::ptr::null_mut();
    let observer_ptr = NonNull::new(&mut observer as *mut *mut AXObserver).unwrap();
    let res = unsafe { AXObserver::create(pid, callback, observer_ptr) };
    if res != AXError::Success {
        return Err(anyhow::anyhow!(
            "Failed to create of server for pid {pid}: {}",
            decorate_ax_error_message(res)
        ));
    }
    let observer = unsafe { *observer_ptr.as_ptr() };
    // Safety: value shouldn't be null as copy attribute call success
    let observer = NonNull::new(observer).unwrap();
    let observer = unsafe { CFRetained::from_raw(observer) };
    Ok(observer)
}

pub(crate) fn add_observer_notification(
    observer: &AXObserver,
    element: &AXUIElement,
    notification: &CFString,
    refcon: *mut std::ffi::c_void,
) -> Result<()> {
    let res = unsafe { observer.add_notification(element, notification, refcon) };
    if res != AXError::Success {
        return Err(anyhow::anyhow!(
            "Failed to add {} notification: {}",
            notification,
            decorate_ax_error_message(res)
        ));
    }
    Ok(())
}

#[allow(non_snake_case)]
pub(crate) fn kAXPositionAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXPosition")
}

#[allow(non_snake_case)]
pub(crate) fn kAXSizeAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXSize")
}

#[allow(non_snake_case)]
pub(crate) fn kAXMinimizedAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXMinimized")
}

#[allow(non_snake_case)]
pub(crate) fn kAXFrontmostAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXFrontmost")
}

#[allow(non_snake_case)]
pub(crate) fn kAXMainAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXMain")
}

#[allow(non_snake_case)]
pub(crate) fn kAXTitleAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXTitle")
}

#[allow(non_snake_case)]
pub(crate) fn kAXWindowCreatedNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXWindowCreated")
}

#[allow(non_snake_case)]
pub(crate) fn kAXWindowMiniaturizedNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXWindowMiniaturized")
}

#[allow(non_snake_case)]
pub(crate) fn kAXResizedNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXResized")
}

#[allow(non_snake_case)]
pub(crate) fn kAXUIElementDestroyedNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXUIElementDestroyed")
}

#[allow(non_snake_case)]
pub(crate) fn kAXWindowsAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXWindows")
}

#[allow(non_snake_case)]
pub(crate) fn kAXRoleAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXRole")
}

#[allow(non_snake_case)]
pub(crate) fn kAXSubroleAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXSubrole")
}

#[allow(non_snake_case)]
pub(crate) fn kAXWindowRole() -> CFRetained<CFString> {
    CFString::from_static_str("AXWindow")
}

#[allow(non_snake_case)]
pub(crate) fn kAXStandardWindowSubrole() -> CFRetained<CFString> {
    CFString::from_static_str("AXStandardWindow")
}

#[allow(non_snake_case)]
pub(crate) fn kAXEnhancedUserInterfaceAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXEnhancedUserInterface")
}

#[allow(non_snake_case)]
pub(crate) fn kAXParentAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXParent")
}

#[allow(non_snake_case)]
pub(crate) fn kAXFullScreenAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXFullScreen")
}

#[allow(non_snake_case)]
pub(crate) fn kAXDialogSubrole() -> CFRetained<CFString> {
    CFString::from_static_str("AXDialog")
}

#[allow(non_snake_case)]
pub(crate) fn kAXFloatingWindowSubrole() -> CFRetained<CFString> {
    CFString::from_static_str("AXFloatingWindow")
}

#[allow(non_snake_case)]
pub(crate) fn kAXFocusedWindowChangedNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXFocusedWindowChanged")
}

#[allow(non_snake_case)]
pub(crate) fn kAXFocusedWindowAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXFocusedWindow")
}

pub(crate) fn is_attribute_settable(element: &AXUIElement, attribute: &CFString) -> bool {
    let mut settable: u8 = 0;
    let settable_ptr = NonNull::new(&mut settable as *mut u8).unwrap();
    let res = unsafe { element.is_attribute_settable(attribute, settable_ptr) };
    res == AXError::Success && settable != 0
}

pub(crate) fn decorate_ax_error_message(error: AXError) -> String {
    let description = match error {
        AXError::Success => "No error occurred",
        AXError::Failure => "A system error occurred, such as the failure to allocate an object",
        AXError::IllegalArgument => "An illegal argument was passed to the function",
        AXError::InvalidUIElement => "The AXUIElementRef passed to the function is invalid",
        AXError::InvalidUIElementObserver => {
            "The AXObserverRef passed to the function is not a valid observer"
        }
        AXError::CannotComplete => {
            "The function cannot complete because messaging failed or the application is busy/unresponsive"
        }
        AXError::AttributeUnsupported => "The attribute is not supported by the AXUIElementRef",
        AXError::ActionUnsupported => "The action is not supported by the AXUIElementRef",
        AXError::NotificationUnsupported => {
            "The notification is not supported by the AXUIElementRef"
        }
        AXError::NotImplemented => "The function or method is not implemented",
        AXError::NotificationAlreadyRegistered => {
            "This notification has already been registered for"
        }
        AXError::NotificationNotRegistered => "The notification is not registered yet",
        AXError::APIDisabled => "The accessibility API is disabled",
        AXError::NoValue => "The requested value or AXUIElementRef does not exist",
        AXError::ParameterizedAttributeUnsupported => {
            "The parameterized attribute is not supported by the AXUIElementRef"
        }
        AXError::NotEnoughPrecision => "Not enough precision",
        _ => "Unknown AXError",
    };
    format!("{} (code: {})", description, error.0)
}
