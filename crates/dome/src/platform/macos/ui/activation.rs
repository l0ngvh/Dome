use crate::platform::macos::objc2_wrapper::{kAXFrontmostAttribute, set_attribute_value};
use objc2_application_services::AXUIElement;
use objc2_core_foundation::kCFBooleanTrue;

/// macOS 14+ "cooperative activation" silently ignores `NSApplication::activate()` for
/// self-activation.
/// Without this, `makeKeyAndOrderFront` only makes a Dome-owned window key inside Dome's
/// AppKit context while the OS-level foreground app stays elsewhere.
pub(super) fn activate_self() {
    let pid = std::process::id() as i32;
    let ax_app = unsafe { AXUIElement::new_application(pid) };
    set_attribute_value(&ax_app, &kAXFrontmostAttribute(), unsafe {
        kCFBooleanTrue.unwrap()
    })
    .ok();
}
