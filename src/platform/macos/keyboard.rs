use std::cell::{Cell, OnceCell};
use std::collections::HashMap;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use objc2_core_foundation::{
    CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource, kCFAllocatorDefault, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType,
};

use super::app::send_hub_event;
use super::dome::HubEvent;
use crate::action::Actions;
use crate::config::{Keymap, Modifiers};

pub(super) type Keymaps = Arc<RwLock<HashMap<Keymap, Actions>>>;

struct KeyboardCtx {
    keymaps: Keymaps,
    is_suspended: Rc<Cell<bool>>,
    hub_sender: Sender<HubEvent>,
    event_tap: OnceCell<CFRetained<CFMachPort>>,
}

pub(super) struct KeyboardListener {
    #[expect(dead_code, reason = "prevent finalizer running")]
    ctx: Box<KeyboardCtx>,
    run_loop_source: CFRetained<CFRunLoopSource>,
}

impl Drop for KeyboardListener {
    fn drop(&mut self) {
        CFRunLoop::current()
            .unwrap()
            .remove_source(Some(&self.run_loop_source), unsafe {
                kCFRunLoopDefaultMode
            });
    }
}

impl KeyboardListener {
    pub(super) fn new(
        keymaps: Keymaps,
        is_suspended: Rc<Cell<bool>>,
        hub_sender: Sender<HubEvent>,
    ) -> Result<Self> {
        let ctx = Box::new(KeyboardCtx {
            keymaps,
            is_suspended,
            hub_sender,
            event_tap: OnceCell::new(),
        });

        let run_loop = CFRunLoop::current().unwrap();
        let event_mask = 1u64 << CGEventType::KeyDown.0;
        let ctx_ptr = &*ctx as *const KeyboardCtx as *mut std::ffi::c_void;

        let Some(event_tap) = (unsafe {
            CGEvent::tap_create(
                CGEventTapLocation::SessionEventTap,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                event_mask,
                Some(event_tap_callback),
                ctx_ptr,
            )
        }) else {
            return Err(anyhow::anyhow!("Failed to create event tap"));
        };

        let Some(run_loop_source) =
            CFMachPort::new_run_loop_source(unsafe { kCFAllocatorDefault }, Some(&event_tap), 0)
        else {
            return Err(anyhow::anyhow!("Failed to create run loop source"));
        };
        run_loop.add_source(Some(&run_loop_source), unsafe { kCFRunLoopDefaultMode });

        ctx.event_tap.set(event_tap).ok();

        Ok(Self {
            ctx,
            run_loop_source,
        })
    }
}

unsafe extern "C-unwind" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    refcon: *mut std::ffi::c_void,
) -> *mut CGEvent {
    let ctx: &KeyboardCtx = unsafe { &*(refcon as *const KeyboardCtx) };
    let event_ptr = event.as_ptr();

    if event_type == CGEventType::TapDisabledByTimeout
        || event_type == CGEventType::TapDisabledByUserInput
    {
        if let Some(tap) = ctx.event_tap.get() {
            tracing::debug!("Event tap disabled, re-enabling");
            CGEvent::tap_enable(tap, true);
        }
    } else if event_type == CGEventType::KeyDown && handle_keyboard(ctx, event_ptr) {
        return std::ptr::null_mut();
    }

    event_ptr
}

fn handle_keyboard(ctx: &KeyboardCtx, event: *mut CGEvent) -> bool {
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
    let actions = ctx
        .keymaps
        .read()
        .unwrap()
        .get(&keymap)
        .cloned()
        .unwrap_or_default();

    if actions.is_empty() {
        return false;
    }

    tracing::trace!(?keymap, %actions, "Keymap matched");

    if ctx.is_suspended.get() {
        tracing::info!("Received keymap action, resuming window management");
        ctx.is_suspended.set(false);
    }

    send_hub_event(&ctx.hub_sender, HubEvent::Action(actions));
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
        0x24 => "return",
        0x4C => "enter",
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
