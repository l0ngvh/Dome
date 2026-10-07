//! AX reads and writes of a window's frame, and the title match that finds a
//! window's AX element without the private `_AXUIElementGetWindow`.

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_application_services::{AXError, AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{CFArray, CFRetained, CFString, CFType, CGPoint, CGSize};

use crate::geometry::Rect;

fn copy(element: &AXUIElement, name: &'static str) -> Option<CFRetained<CFType>> {
    let mut value: *const CFType = std::ptr::null();
    let result = unsafe {
        element.copy_attribute_value(&CFString::from_static_str(name), NonNull::from(&mut value))
    };
    let value = NonNull::new(value as *mut CFType)?;
    let value = unsafe { CFRetained::from_raw(value) };
    (result == AXError::Success).then_some(value)
}

/// The window of `pid` whose `AXTitle` equals `title`, with its messaging
/// timeout set to `timeout_secs`.
pub fn window_with_title(
    pid: i32,
    title: &str,
    timeout_secs: f32,
) -> Option<CFRetained<AXUIElement>> {
    let app = unsafe { AXUIElement::new_application(pid) };
    unsafe { app.set_messaging_timeout(timeout_secs) };
    let windows = copy(&app, "AXWindows")?.downcast::<CFArray>().ok()?;
    let windows: CFRetained<CFArray<AXUIElement>> = unsafe { CFRetained::cast_unchecked(windows) };
    windows.iter().find_map(|window| {
        unsafe { window.set_messaging_timeout(timeout_secs) };
        let found = copy(&window, "AXTitle")?.downcast::<CFString>().ok()?;
        (found.to_string() == title).then_some(window)
    })
}

fn value<T>(element: &AXUIElement, name: &'static str, ty: AXValueType, zero: T) -> Option<T> {
    let v = copy(element, name)?.downcast::<AXValue>().ok()?;
    let mut out = zero;
    let ok = unsafe { v.value(ty, NonNull::from(&mut out).cast::<c_void>()) };
    ok.then_some(out)
}

/// Global coordinates, origin top-left, as in CGWindowList.
pub fn frame(element: &AXUIElement) -> Option<Rect> {
    let origin = value(element, "AXPosition", AXValueType::CGPoint, CGPoint::new(0.0, 0.0))?;
    let size = value(element, "AXSize", AXValueType::CGSize, CGSize::new(0.0, 0.0))?;
    Some(Rect::new(origin.x, origin.y, size.width, size.height))
}

fn set<T>(element: &AXUIElement, name: &'static str, ty: AXValueType, mut v: T) -> bool {
    let Some(v) = (unsafe { AXValue::new(ty, NonNull::from(&mut v).cast::<c_void>()) }) else {
        return false;
    };
    let value: &CFType = &v;
    let result = unsafe { element.set_attribute_value(&CFString::from_static_str(name), value) };
    result == AXError::Success
}

/// True does not mean the window now has the frame `r`. The app may clamp
/// the size, for example to its minimum content size.
pub fn set_frame(element: &AXUIElement, r: &Rect) -> bool {
    let moved = set_position(element, r.x, r.y);
    let sized = set(element, "AXSize", AXValueType::CGSize, CGSize::new(r.w, r.h));
    moved && sized
}

pub fn set_position(element: &AXUIElement, x: f64, y: f64) -> bool {
    set(element, "AXPosition", AXValueType::CGPoint, CGPoint::new(x, y))
}

/// The caller must also make the window's app frontmost, or the window stays
/// behind the frontmost app's windows.
pub fn raise_as_main(element: &AXUIElement) -> bool {
    let Some(yes) = (unsafe { objc2_core_foundation::kCFBooleanTrue }) else {
        return false;
    };
    let value: &CFType = yes;
    let main = unsafe { element.set_attribute_value(&CFString::from_static_str("AXMain"), value) };
    let raised = unsafe { element.perform_action(&CFString::from_static_str("AXRaise")) };
    main == AXError::Success && raised == AXError::Success
}
