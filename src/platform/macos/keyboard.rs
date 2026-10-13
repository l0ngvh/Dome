use crate::platform::keymap::KeymapView;
use std::cell::{OnceCell, RefCell};
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

use objc2_core_foundation::{
    CFMachPort, CFRetained, CFRunLoop, kCFAllocatorDefault, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType,
};

use crate::config::{Keystroke, Modifiers};
use crate::platform::macos::{dome::HubEvent, set_current_thread_qos};

struct KeyboardState {
    keymap_intake: Receiver<KeymapView>,
    keymap: RefCell<KeymapView>,
    is_suspended: Arc<AtomicBool>,
    event_sender: calloop::channel::Sender<HubEvent>,
    event_tap: OnceCell<CFRetained<CFMachPort>>,
}

/// Runs until the process exits. Creation and registration happen here because
/// a run loop's configuration should only be altered from the thread that owns
/// it.
pub(super) fn run_event_tap(
    keymap_intake: Receiver<KeymapView>,
    is_suspended: Arc<AtomicBool>,
    event_sender: calloop::channel::Sender<HubEvent>,
) {
    // Every key press in the session waits on this thread.
    set_current_thread_qos(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE);

    let state = KeyboardState {
        keymap_intake,
        keymap: RefCell::new(KeymapView::new()),
        is_suspended,
        event_sender,
        event_tap: OnceCell::new(),
    };

    let run_loop = CFRunLoop::current().unwrap();
    let event_mask = 1u64 << CGEventType::KeyDown.0;
    let state_pointer = &state as *const KeyboardState as *mut std::ffi::c_void;

    let Some(event_tap) = (unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            event_mask,
            Some(event_tap_callback),
            state_pointer,
        )
    }) else {
        tracing::error!("Failed to create event tap, keymaps are unavailable");
        return;
    };

    let Some(run_loop_source) =
        CFMachPort::new_run_loop_source(unsafe { kCFAllocatorDefault }, Some(&event_tap), 0)
    else {
        tracing::error!("Failed to create event tap run loop source");
        return;
    };
    run_loop.add_source(Some(&run_loop_source), unsafe { kCFRunLoopDefaultMode });

    state.event_tap.set(event_tap).ok();

    CFRunLoop::run();

    tracing::error!("Event tap run loop ended, keymaps are dead");
}

unsafe extern "C-unwind" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    refcon: *mut std::ffi::c_void,
) -> *mut CGEvent {
    let event_ptr = event.as_ptr();

    let state: &KeyboardState = unsafe { &*(refcon as *const KeyboardState) };

    let handled = if event_type == CGEventType::TapDisabledByTimeout
        || event_type == CGEventType::TapDisabledByUserInput
    {
        if let Some(tap) = state.event_tap.get() {
            tracing::warn!(?event_type, "Event tap disabled, re-enabling");
            CGEvent::tap_enable(tap, true);
        }
        false
    } else {
        event_type == CGEventType::KeyDown && handle_keyboard(state, event_ptr)
    };

    if handled {
        std::ptr::null_mut()
    } else {
        event_ptr
    }
}

fn handle_keyboard(state: &KeyboardState, event: *mut CGEvent) -> bool {
    let flags = CGEvent::flags(Some(unsafe { &*event }));
    let key = get_key_from_event(event);

    let mut modifiers = Modifiers::empty();
    if flags.contains(CGEventFlags::MaskCommand) {
        modifiers |= Modifiers::META;
    }
    if flags.contains(CGEventFlags::MaskShift) {
        modifiers |= Modifiers::SHIFT;
    }
    if flags.contains(CGEventFlags::MaskAlternate) {
        modifiers |= Modifiers::ALT;
    }
    if flags.contains(CGEventFlags::MaskControl) {
        modifiers |= Modifiers::CTRL;
    }

    let keystroke = Keystroke { key, modifiers };
    while let Ok(next) = state.keymap_intake.try_recv() {
        *state.keymap.borrow_mut() = next;
    }
    let resolved = state
        .keymap
        .borrow()
        .resolve(&keystroke)
        .map(str::to_string);
    let Some(keymap) = resolved else {
        return false;
    };

    if state.is_suspended.load(Ordering::Relaxed) {
        tracing::info!("Received keymap action, resuming window management");
        state.is_suspended.store(false, Ordering::Relaxed);
    }

    tracing::trace!(%keymap, %keystroke, "Keystroke matched binding");
    if state
        .event_sender
        .send(HubEvent::RunBinding { keymap, keystroke })
        .is_err()
    {
        tracing::warn!("Domain thread unavailable, callback dropped");
    }
    true
}

fn get_key_from_event(event: *mut CGEvent) -> String {
    let keycode =
        CGEvent::integer_value_field(Some(unsafe { &*event }), CGEventField::KeyboardEventKeycode);

    match keycode {
        0x00 => "a",
        0x01 => "s",
        0x02 => "d",
        0x03 => "f",
        0x04 => "h",
        0x05 => "g",
        0x06 => "z",
        0x07 => "x",
        0x08 => "c",
        0x09 => "v",
        0x0B => "b",
        0x0C => "q",
        0x0D => "w",
        0x0E => "e",
        0x0F => "r",
        0x10 => "y",
        0x11 => "t",
        0x12 => "1",
        0x13 => "2",
        0x14 => "3",
        0x15 => "4",
        0x16 => "6",
        0x17 => "5",
        0x18 => "=",
        0x19 => "9",
        0x1A => "7",
        0x1B => "-",
        0x1C => "8",
        0x1D => "0",
        0x1E => "]",
        0x1F => "o",
        0x20 => "u",
        0x21 => "[",
        0x22 => "i",
        0x23 => "p",
        0x25 => "l",
        0x26 => "j",
        0x27 => "'",
        0x28 => "k",
        0x29 => ";",
        0x2A => "\\",
        0x2B => ",",
        0x2C => "/",
        0x2D => "n",
        0x2E => "m",
        0x2F => ".",
        0x32 => "`",
        // Return and Numpad Enter.
        0x24 | 0x4C => "return",
        0x33 => "backspace",
        0x35 => "escape",
        0x30 => "tab",
        0x31 => "space",
        0x7E => "up",
        0x7D => "down",
        0x7B => "left",
        0x7C => "right",
        _ => return format!("keycode_{keycode}"),
    }
    .to_string()
}
