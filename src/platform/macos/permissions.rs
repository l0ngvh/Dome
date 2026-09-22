//! One-time permission setup. See `plans/macos-permission-setup-wizard.md`.

use objc2_application_services::AXIsProcessTrusted;
use objc2_core_foundation::{CFRunLoop, kCFRunLoopDefaultMode};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const GRANT_WAIT: Duration = Duration::from_secs(60);

/// Returns false when Dome must exit.
pub(super) fn ensure_permissions() -> bool {
    if unsafe { AXIsProcessTrusted() } {
        tracing::info!("Accessibility already granted");
    } else {
        prompt_for_accessibility();
        tracing::info!("Waiting for the Accessibility grant");
        if !wait_for_accessibility() {
            tracing::error!(
                "Accessibility not granted, exiting. Turn Dome on under Privacy & Security > Accessibility, then open Dome again. If the switch is already on, run: tccutil reset Accessibility com.longvh.dome"
            );
            return false;
        }
        tracing::info!("Accessibility granted");
    }
    true
}

/// Runs the run loop rather than sleeping, because the trust value a sleeping
/// process reads never changes.
fn wait_for_accessibility() -> bool {
    let deadline = Instant::now() + GRANT_WAIT;
    while Instant::now() < deadline {
        unsafe {
            CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, POLL_INTERVAL.as_secs_f64(), false);
        }
        if unsafe { AXIsProcessTrusted() } {
            return true;
        }
    }
    false
}

/// Registers Dome under Privacy & Security > Accessibility, and shows the macOS
/// dialog. `AXIsProcessTrusted` performs the same check without the dialog.
fn prompt_for_accessibility() {
    use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
    use objc2_core_foundation::{CFDictionary, kCFBooleanTrue};

    unsafe {
        AXIsProcessTrustedWithOptions(Some(
            CFDictionary::from_slices(&[kAXTrustedCheckOptionPrompt], &[kCFBooleanTrue.unwrap()])
                .as_opaque(),
        ));
    }
}
