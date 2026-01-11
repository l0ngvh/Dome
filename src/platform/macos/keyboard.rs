use std::ptr::NonNull;

use anyhow::Result;
use objc2::DefinedClass;
use objc2_core_foundation::{CFMachPort, CFRunLoop, kCFAllocatorDefault, kCFRunLoopDefaultMode};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType,
};

use super::app::AppDelegate;
use super::hub::HubEvent;
use crate::config::{Keymap, Modifiers};

pub(super) fn listen_to_input_devices(delegate: &'static AppDelegate) -> Result<()> {
    let run_loop = CFRunLoop::current().unwrap();
    let event_mask = 1u64 << CGEventType::KeyDown.0;
    let delegate_ptr = delegate as *const AppDelegate as *mut std::ffi::c_void;

    let Some(event_tap) = (unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            event_mask,
            Some(event_tap_callback),
            delegate_ptr,
        )
    }) else {
        return Err(anyhow::anyhow!("Failed to create event tap"));
    };

    delegate.ivars().event_tap.set(event_tap.clone()).ok();

    let Some(run_loop_source) =
        CFMachPort::new_run_loop_source(unsafe { kCFAllocatorDefault }, Some(&event_tap), 0)
    else {
        return Err(anyhow::anyhow!("Failed to create run loop source"));
    };
    run_loop.add_source(Some(&run_loop_source), unsafe { kCFRunLoopDefaultMode });
    Ok(())
}

unsafe extern "C-unwind" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    refcon: *mut std::ffi::c_void,
) -> *mut CGEvent {
    let delegate: &'static AppDelegate = unsafe { &*(refcon as *const AppDelegate) };
    let event_ptr = event.as_ptr();

    if event_type == CGEventType::TapDisabledByTimeout
        || event_type == CGEventType::TapDisabledByUserInput
    {
        if let Some(tap) = delegate.ivars().event_tap.get() {
            tracing::debug!("Event tap disabled, re-enabling");
            CGEvent::tap_enable(tap, true);
        }
    } else if event_type == CGEventType::KeyDown {
        if handle_keyboard(delegate, event_ptr) {
            return std::ptr::null_mut();
        }
    }

    event_ptr
}

fn handle_keyboard(delegate: &AppDelegate, event: *mut CGEvent) -> bool {
    let flags = CGEvent::flags(Some(unsafe { &*event }));
    let key = get_key_from_event(event);

    let mut modifiers = Modifiers::empty();
    if flags.contains(CGEventFlags::MaskCommand) {
        modifiers |= Modifiers::CMD;
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

    let keymap = Keymap { key, modifiers };
    let actions = delegate.ivars().config.read().unwrap().get_actions(&keymap);

    if actions.is_empty() {
        return false;
    }

    if delegate.ivars().is_suspended.get() {
        tracing::info!("Received keymap action, resuming window management");
        delegate.ivars().is_suspended.set(false);
    }

    delegate.send_event(HubEvent::Action(actions));
    true
}

fn get_key_from_event(event: *mut CGEvent) -> String {
    let keycode =
        CGEvent::integer_value_field(Some(unsafe { &*event }), CGEventField::KeyboardEventKeycode);

    match keycode {
        0x24 => return "return".to_string(),
        0x4C => return "enter".to_string(),
        0x33 => return "backspace".to_string(),
        0x35 => return "escape".to_string(),
        0x30 => return "tab".to_string(),
        0x31 => return "space".to_string(),
        0x7E => return "up".to_string(),
        0x7D => return "down".to_string(),
        0x7B => return "left".to_string(),
        0x7C => return "right".to_string(),
        _ => {}
    }

    let max_len: usize = 256;
    let mut buffer: Vec<u16> = vec![0; max_len];
    let mut actual_len: std::ffi::c_ulong = 0;
    unsafe {
        CGEvent::keyboard_get_unicode_string(
            Some(&*event),
            max_len as std::ffi::c_ulong,
            &mut actual_len as *mut std::ffi::c_ulong,
            buffer.as_mut_ptr(),
        )
    };
    String::from_utf16(&buffer[..actual_len as usize]).unwrap()
}
