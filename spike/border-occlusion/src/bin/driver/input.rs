//! Posted input, the idle gate, and the checks that keep posted input inside
//! the fixture windows.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::{CFRetained, CFString, CFType, CGPoint, kCFBooleanTrue};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    CGEventType, CGMouseButton,
};
use spike::now_ms;
use spike::windows::{WinInfo, query_windows};

pub const KEY_ESCAPE: u16 = 53;
pub const KEY_SPACE: u16 = 49;

static LEFT_DOWN: AtomicBool = AtomicBool::new(false);
static RIGHT_DOWN: AtomicBool = AtomicBool::new(false);
static TAP: AtomicU32 = AtomicU32::new(CGEventTapLocation::SessionEventTap.0);
static LAST_POST_MS: AtomicU64 = AtomicU64::new(0);

/// `kCGAnyInputEventType`.
const ANY_INPUT: CGEventType = CGEventType(u32::MAX);

pub fn idle_seconds() -> f64 {
    CGEventSource::seconds_since_last_event_type(CGEventSourceStateID::HIDSystemState, ANY_INPUT)
}

/// Seconds since the driver last posted an event, or `None` before the first.
pub fn seconds_since_own_post() -> Option<f64> {
    let last = LAST_POST_MS.load(Ordering::SeqCst);
    (last > 0).then(|| now_ms().saturating_sub(last) as f64 / 1000.0)
}

pub fn set_tap(tap: CGEventTapLocation) {
    TAP.store(tap.0, Ordering::SeqCst);
}

pub fn tap_name(tap: CGEventTapLocation) -> &'static str {
    match tap {
        CGEventTapLocation::HIDEventTap => "hid",
        CGEventTapLocation::SessionEventTap => "session",
        _ => "other",
    }
}

fn post(event: &CGEvent) {
    LAST_POST_MS.store(now_ms(), Ordering::SeqCst);
    CGEvent::post(CGEventTapLocation(TAP.load(Ordering::SeqCst)), Some(event));
}

pub fn mouse(ty: CGEventType, (x, y): (f64, f64), button: CGMouseButton) {
    let Some(event) = CGEvent::new_mouse_event(None, ty, CGPoint { x, y }, button) else {
        return;
    };
    if matches!(
        ty,
        CGEventType::LeftMouseDown
            | CGEventType::LeftMouseUp
            | CGEventType::RightMouseDown
            | CGEventType::RightMouseUp
    ) {
        CGEvent::set_integer_value_field(Some(&event), CGEventField::MouseEventClickState, 1);
    }
    post(&event);
    match ty {
        CGEventType::LeftMouseDown => LEFT_DOWN.store(true, Ordering::SeqCst),
        CGEventType::LeftMouseUp => LEFT_DOWN.store(false, Ordering::SeqCst),
        CGEventType::RightMouseDown => RIGHT_DOWN.store(true, Ordering::SeqCst),
        CGEventType::RightMouseUp => RIGHT_DOWN.store(false, Ordering::SeqCst),
        _ => {}
    }
}

pub fn move_to(p: (f64, f64)) {
    mouse(CGEventType::MouseMoved, p, CGMouseButton::Left);
}

pub fn key(code: u16, flags: CGEventFlags) {
    for down in [true, false] {
        let Some(event) = CGEvent::new_keyboard_event(None, code, down) else {
            return;
        };
        CGEvent::set_flags(Some(&event), flags);
        post(&event);
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn release_buttons() {
    let at = mouse_location();
    if LEFT_DOWN.load(Ordering::SeqCst) {
        mouse(CGEventType::LeftMouseUp, at, CGMouseButton::Left);
    }
    if RIGHT_DOWN.load(Ordering::SeqCst) {
        mouse(CGEventType::RightMouseUp, at, CGMouseButton::Right);
    }
}

pub fn mouse_location() -> (f64, f64) {
    let event = CGEvent::new(None);
    let p = CGEvent::location(event.as_deref());
    (p.x, p.y)
}

pub fn front_window_at(p: (f64, f64), skip_pid: Option<i32>) -> Option<WinInfo> {
    query_windows(skip_pid.map(i64::from))
        .into_iter()
        .find(|w| w.rect.x <= p.0 && p.0 < w.rect.right() && w.rect.y <= p.1 && p.1 < w.rect.bottom())
}

pub fn assert_fixture_at(p: (f64, f64), fixtures: &[i32], overlay: Option<i32>) {
    let front = front_window_at(p, overlay);
    let ok = front.is_some_and(|w| fixtures.contains(&(w.pid as i32)));
    assert!(
        ok,
        "run safety: the frontmost window at {p:?} is {front:?}, not a fixture window"
    );
    let hit = pid_at(p);
    assert!(
        hit.is_some_and(|pid| fixtures.contains(&pid)),
        "run safety: the AX hit test at {p:?} returned pid {hit:?}, not a fixture"
    );
}

fn pid_at((x, y): (f64, f64)) -> Option<i32> {
    let system = unsafe { AXUIElement::new_system_wide() };
    let mut found: *const AXUIElement = std::ptr::null();
    let result = unsafe {
        system.copy_element_at_position(x as f32, y as f32, std::ptr::NonNull::from(&mut found))
    };
    let found = std::ptr::NonNull::new(found as *mut AXUIElement)?;
    let element: CFRetained<AXUIElement> = unsafe { CFRetained::from_raw(found) };
    if result != AXError::Success {
        return None;
    }
    let mut pid: libc::pid_t = 0;
    let result = unsafe { element.pid(std::ptr::NonNull::from(&mut pid)) };
    (result == AXError::Success).then_some(pid)
}

/// Returns the time of the press, in Unix-epoch milliseconds.
pub fn click(p: (f64, f64), fixtures: &[i32], overlay: Option<i32>) -> u64 {
    move_to(p);
    std::thread::sleep(Duration::from_millis(30));
    assert_fixture_at(p, fixtures, overlay);
    let at = now_ms();
    mouse(CGEventType::LeftMouseDown, p, CGMouseButton::Left);
    std::thread::sleep(Duration::from_millis(40));
    mouse(CGEventType::LeftMouseUp, p, CGMouseButton::Left);
    at
}

/// Returns the time of the press, in Unix-epoch milliseconds.
pub fn right_click(p: (f64, f64), fixtures: &[i32], overlay: Option<i32>) -> u64 {
    move_to(p);
    std::thread::sleep(Duration::from_millis(30));
    assert_fixture_at(p, fixtures, overlay);
    let at = now_ms();
    mouse(CGEventType::RightMouseDown, p, CGMouseButton::Right);
    std::thread::sleep(Duration::from_millis(40));
    mouse(CGEventType::RightMouseUp, p, CGMouseButton::Right);
    at
}

/// `path` maps seconds since the press to a cursor position. Returns the time
/// of the release, in Unix-epoch milliseconds.
pub fn drag(
    from: (f64, f64),
    duration: Duration,
    fixtures: &[i32],
    overlay: Option<i32>,
    path: impl Fn(f64) -> (f64, f64),
) -> u64 {
    press(from, fixtures, overlay);
    drag_pressed(from, duration, path)
}

pub fn press(from: (f64, f64), fixtures: &[i32], overlay: Option<i32>) {
    move_to(from);
    std::thread::sleep(Duration::from_millis(50));
    assert_fixture_at(from, fixtures, overlay);
    mouse(CGEventType::LeftMouseDown, from, CGMouseButton::Left);
    std::thread::sleep(Duration::from_millis(100));
}

/// Drags with the left button already held since `press`, then releases.
/// Returns the time of the release, in Unix-epoch milliseconds.
pub fn drag_pressed(
    from: (f64, f64),
    duration: Duration,
    path: impl Fn(f64) -> (f64, f64),
) -> u64 {
    let start = Instant::now();
    let frame = Duration::from_micros(16_667);
    let mut next = start;
    let mut last = from;
    while start.elapsed() < duration {
        last = path(start.elapsed().as_secs_f64());
        mouse(CGEventType::LeftMouseDragged, last, CGMouseButton::Left);
        next += frame;
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
    let at = now_ms();
    mouse(CGEventType::LeftMouseUp, last, CGMouseButton::Left);
    at
}

pub fn escape() {
    key(KEY_ESCAPE, CGEventFlags(0));
}

pub fn cmd_space() {
    key(KEY_SPACE, CGEventFlags::MaskCommand);
}

fn cf_str(s: &'static str) -> CFRetained<CFString> {
    CFString::from_static_str(s)
}

/// False when the AX write fails.
pub fn activate(pid: i32) -> bool {
    let app = unsafe { AXUIElement::new_application(pid) };
    let Some(yes) = (unsafe { kCFBooleanTrue }) else {
        return false;
    };
    let value: &CFType = yes;
    let result = unsafe { app.set_attribute_value(&cf_str("AXFrontmost"), value) };
    result == AXError::Success
}

pub fn app_name(pid: i32) -> String {
    objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .and_then(|app| app.localizedName())
        .map_or_else(String::new, |name| name.to_string())
}

pub fn focused_app_pid() -> Option<i32> {
    let system = unsafe { AXUIElement::new_system_wide() };
    let mut value: *const CFType = std::ptr::null();
    let result = unsafe {
        system.copy_attribute_value(
            &cf_str("AXFocusedApplication"),
            std::ptr::NonNull::from(&mut value),
        )
    };
    if result != AXError::Success || value.is_null() {
        return None;
    }
    let element: CFRetained<AXUIElement> = unsafe {
        CFRetained::from_raw(std::ptr::NonNull::new_unchecked(value as *mut AXUIElement))
    };
    let mut pid: libc::pid_t = 0;
    let result = unsafe { element.pid(std::ptr::NonNull::from(&mut pid)) };
    (result == AXError::Success).then_some(pid)
}
