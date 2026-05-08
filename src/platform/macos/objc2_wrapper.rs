use std::fmt;
use std::ptr::NonNull;

use objc2_application_services::{AXObserver, AXObserverCallback, AXUIElement};
use objc2_core_foundation::{CFRetained, CFString, CFType, CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::core::{Dimension, Length, Logical};

type RawAXError = objc2_application_services::AXError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AXError {
    InvalidUIElement,
    NullCgWindowId,
    Other(RawAXError),
}

impl From<RawAXError> for AXError {
    fn from(err: RawAXError) -> Self {
        if err == RawAXError::InvalidUIElement {
            AXError::InvalidUIElement
        } else {
            AXError::Other(err)
        }
    }
}

impl fmt::Display for AXError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AXError::InvalidUIElement => {
                write!(
                    f,
                    "The AXUIElementRef passed to the function is invalid (code: {})",
                    RawAXError::InvalidUIElement.0
                )
            }
            AXError::NullCgWindowId => {
                write!(f, "CGWindowID associated with this AXUIElementRef is null")
            }
            AXError::Other(err) => write!(f, "{}", decorate_raw_ax_error(*err)),
        }
    }
}

impl std::error::Error for AXError {}

pub(crate) fn get_attribute<T: objc2_core_foundation::Type>(
    element: &AXUIElement,
    attribute: &CFString,
) -> Result<CFRetained<T>, AXError> {
    let mut value: *const CFType = std::ptr::null();
    let value_ptr = NonNull::new(&mut value as *mut *const CFType).unwrap();

    let res = unsafe { element.copy_attribute_value(attribute, value_ptr) };
    // TODO: return no value error as None
    if res != RawAXError::Success {
        return Err(res.into());
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
) -> Result<(), AXError> {
    let res = unsafe { element.set_attribute_value(attribute, value) };
    if res != RawAXError::Success {
        return Err(res.into());
    }
    Ok(())
}

pub(crate) fn get_pid(element: &AXUIElement) -> Result<i32, AXError> {
    let mut pid = 0;
    let value_ptr = NonNull::new(&mut pid as *mut i32).unwrap();
    let res = unsafe { element.pid(value_ptr) };
    if res != RawAXError::Success {
        return Err(res.into());
    }
    Ok(pid)
}

pub(crate) fn create_observer(
    pid: i32,
    callback: AXObserverCallback,
) -> Result<CFRetained<AXObserver>, AXError> {
    let mut observer: *mut AXObserver = std::ptr::null_mut();
    let observer_ptr = NonNull::new(&mut observer as *mut *mut AXObserver).unwrap();
    let res = unsafe { AXObserver::create(pid, callback, observer_ptr) };
    if res != RawAXError::Success {
        return Err(res.into());
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
) -> Result<(), AXError> {
    let res = unsafe { observer.add_notification(element, notification, refcon) };
    if res != RawAXError::Success {
        return Err(res.into());
    }
    Ok(())
}

pub(crate) fn remove_observer_notification(
    observer: &AXObserver,
    element: &AXUIElement,
    notification: &CFString,
) -> Result<(), AXError> {
    let res = unsafe { observer.remove_notification(element, notification) };
    if res != RawAXError::Success {
        return Err(res.into());
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
pub(crate) fn kAXFocusedWindowChangedNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXFocusedWindowChanged")
}

#[allow(non_snake_case)]
pub(crate) fn kAXWindowDeminiaturizedNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXWindowDeminiaturized")
}

#[allow(non_snake_case)]
pub(crate) fn kAXApplicationHiddenNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXApplicationHidden")
}

#[allow(non_snake_case)]
pub(crate) fn kAXTitleChangedNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXTitleChanged")
}

#[allow(non_snake_case)]
pub(crate) fn kAXApplicationShownNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXApplicationShown")
}

#[allow(non_snake_case)]
pub(crate) fn kAXMovedNotification() -> CFRetained<CFString> {
    CFString::from_static_str("AXMoved")
}

#[allow(non_snake_case)]
pub(crate) fn kAXFocusedWindowAttribute() -> CFRetained<CFString> {
    CFString::from_static_str("AXFocusedWindow")
}

#[allow(non_snake_case)]
pub(crate) fn kCGWindowNumber() -> CFRetained<CFString> {
    CFString::from_static_str("kCGWindowNumber")
}

pub(crate) fn is_attribute_settable(element: &AXUIElement, attribute: &CFString) -> bool {
    let mut settable: u8 = 0;
    let settable_ptr = NonNull::new(&mut settable as *mut u8).unwrap();
    let res = unsafe { element.is_attribute_settable(attribute, settable_ptr) };
    res == RawAXError::Success && settable != 0
}

fn decorate_raw_ax_error(error: RawAXError) -> String {
    let description = match error {
        RawAXError::Success => "No error occurred",
        RawAXError::Failure => "A system error occurred, such as the failure to allocate an object",
        RawAXError::IllegalArgument => "An illegal argument was passed to the function",
        RawAXError::InvalidUIElement => "The AXUIElementRef passed to the function is invalid",
        RawAXError::InvalidUIElementObserver => {
            "The AXObserverRef passed to the function is not a valid observer"
        }
        RawAXError::CannotComplete => {
            "The function cannot complete because messaging failed or the application is busy/unresponsive"
        }
        RawAXError::AttributeUnsupported => "The attribute is not supported by the AXUIElementRef",
        RawAXError::ActionUnsupported => "The action is not supported by the AXUIElementRef",
        RawAXError::NotificationUnsupported => {
            "The notification is not supported by the AXUIElementRef"
        }
        RawAXError::NotImplemented => "The function or method is not implemented",
        RawAXError::NotificationAlreadyRegistered => {
            "This notification has already been registered for"
        }
        RawAXError::NotificationNotRegistered => "The notification is not registered yet",
        RawAXError::APIDisabled => "The accessibility API is disabled",
        RawAXError::NoValue => "The requested value or AXUIElementRef does not exist",
        RawAXError::ParameterizedAttributeUnsupported => {
            "The parameterized attribute is not supported by the AXUIElementRef"
        }
        RawAXError::NotEnoughPrecision => "Not enough precision",
        _ => "Unknown AXError",
    };
    format!("{} (code: {})", description, error.0)
}

/// Single site for the Y-flip from Quartz (top-left origin) to AppKit/Cocoa
/// (bottom-left origin). All `Dimension -> NSRect` crossings in the platform
/// layer go through here so the flip logic exists exactly once.
pub(super) fn dimension_to_ns_rect_cocoa(
    primary_full_height: Length<Logical>,
    dim: Dimension<Logical>,
) -> NSRect {
    // AppKit uses f64; cast from f32 happens here at the wrapper boundary.
    NSRect::new(
        NSPoint::new(
            dim.x.value() as f64,
            (primary_full_height - dim.y - dim.height).value() as f64,
        ),
        NSSize::new(dim.width.value() as f64, dim.height.value() as f64),
    )
}

/// Direct `Dimension -> CGRect` mapping (Quartz top-left origin, no Y-flip).
/// Used for ScreenCaptureKit sourceRect and similar Quartz-native APIs.
pub(super) fn dimension_to_cg_rect(dim: Dimension<Logical>) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: dim.x.value() as f64,
            y: dim.y.value() as f64,
        },
        size: CGSize {
            width: dim.width.value() as f64,
            height: dim.height.value() as f64,
        },
    }
}

pub(crate) fn get_cg_window_id(element: &AXUIElement) -> Result<CGWindowID, AXError> {
    unsafe extern "C" {
        fn _AXUIElementGetWindow(element: &AXUIElement, out: *mut CGWindowID) -> RawAXError;
    }
    let mut window_id: CGWindowID = 0;
    let res = unsafe { _AXUIElementGetWindow(element, &mut window_id) };
    // 0 is kCGNullWindowID
    if res == RawAXError::Success && window_id != 0 {
        Ok(window_id)
    } else if window_id == 0 {
        Err(AXError::NullCgWindowId)
    } else {
        Err(res.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_to_ns_rect_cocoa_flips_y() {
        let dim = Dimension::new(
            Length::new(10.0),
            Length::new(20.0),
            Length::new(100.0),
            Length::new(50.0),
        );
        let primary_full_height = Length::new(1000.0);
        let rect = dimension_to_ns_rect_cocoa(primary_full_height, dim);
        // Y-flip: cocoa_y = primary_height - y - height = 1000 - 20 - 50 = 930
        assert_eq!(rect.origin.x, 10.0);
        assert_eq!(rect.origin.y, 930.0);
        assert_eq!(rect.size.width, 100.0);
        assert_eq!(rect.size.height, 50.0);
    }

    #[test]
    fn dimension_to_cg_rect_no_flip() {
        let dim = Dimension::new(
            Length::new(10.0),
            Length::new(20.0),
            Length::new(100.0),
            Length::new(50.0),
        );
        let rect = dimension_to_cg_rect(dim);
        // No Y-flip: origin passes through unchanged
        assert_eq!(rect.origin.x, 10.0);
        assert_eq!(rect.origin.y, 20.0);
        assert_eq!(rect.size.width, 100.0);
        assert_eq!(rect.size.height, 50.0);
    }
}
