use crate::platform::keymap::KeymapView;
use std::cell::{OnceCell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;

use block2::StackBlock;
use objc2_core_foundation::{
    CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource, kCFAllocatorDefault, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType,
};

use crate::config::{Keystroke, Modifiers};
use crate::platform::macos::{dome::HubEvent, set_current_thread_qos};

pub(super) struct EventTap {
    shutdown: EventTapShutdown,
    worker: Option<JoinHandle<()>>,
}

impl EventTap {
    pub(super) fn new(shutdown: EventTapShutdown, worker: JoinHandle<()>) -> Self {
        Self {
            shutdown,
            worker: Some(worker),
        }
    }
}

impl Drop for EventTap {
    fn drop(&mut self) {
        self.shutdown.request();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::error!("Event tap thread panicked");
        }
    }
}

pub(super) struct EventTapShutdown(CFRetained<CFRunLoop>);

// Cross-thread use is limited to self.0.perform_block, self.0.wake_up and releasing
// the retained loop. CoreFoundation supports these operations across threads.
unsafe impl Send for EventTapShutdown {}

pub(super) fn run_event_tap(
    keymap_rx: Receiver<KeymapView>,
    is_suspended: Arc<AtomicBool>,
    event_sender: crate::platform::macos::event_loop::EventSender<super::HubEvent>,
    ready: mpsc::SyncSender<anyhow::Result<EventTapShutdown>>,
) {
    // Every key press in the session waits on this thread.
    set_current_thread_qos(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE);

    let context = KeyboardContext {
        keymap_rx,
        keymap: RefCell::new(KeymapView::new()),
        is_suspended,
        event_sender,
        run_loop: CFRunLoop::current().unwrap(),
        event_tap: OnceCell::new(),
        run_loop_source: OnceCell::new(),
    };

    let event_mask = 1u64 << CGEventType::KeyDown.0;
    let context_pointer = &context as *const KeyboardContext as *mut std::ffi::c_void;

    let Some(event_tap) = (unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            event_mask,
            Some(event_tap_callback),
            context_pointer,
        )
    }) else {
        let _ = ready.send(Err(anyhow::anyhow!("Failed to create event tap")));
        return;
    };

    let event_tap = context.event_tap.get_or_init(|| event_tap);
    let Some(run_loop_source) =
        CFMachPort::new_run_loop_source(unsafe { kCFAllocatorDefault }, Some(event_tap), 0)
    else {
        let _ = ready.send(Err(anyhow::anyhow!(
            "Failed to create event tap run loop source"
        )));
        return;
    };
    let run_loop_source = context.run_loop_source.get_or_init(|| run_loop_source);
    context
        .run_loop
        .add_source(Some(run_loop_source), unsafe { kCFRunLoopDefaultMode });

    if ready
        .send(Ok(EventTapShutdown(context.run_loop.clone())))
        .is_err()
    {
        return;
    }

    CFRunLoop::run();

    tracing::debug!("Event tap run loop ended");
}

impl EventTapShutdown {
    fn request(&self) {
        let stop = StackBlock::new(|| {
            if let Some(run_loop) = CFRunLoop::current() {
                run_loop.stop();
            }
        });
        // CoreFoundation copies the block into its queue, even before CFRunLoop::run().
        // Stop therefore executes inside the worker's active loop, not before loop entry.
        unsafe {
            self.0
                .perform_block(kCFRunLoopDefaultMode.map(|mode| mode.as_ref()), Some(&stop));
        }
        self.0.wake_up();
    }
}

struct KeyboardContext {
    keymap_rx: Receiver<KeymapView>,
    keymap: RefCell<KeymapView>,
    is_suspended: Arc<AtomicBool>,
    event_sender: crate::platform::macos::event_loop::EventSender<super::HubEvent>,
    run_loop: CFRetained<CFRunLoop>,
    event_tap: OnceCell<CFRetained<CFMachPort>>,
    run_loop_source: OnceCell<CFRetained<CFRunLoopSource>>,
}

impl KeyboardContext {
    fn stop(&self) {
        if let Some(tap) = self.event_tap.get() {
            CGEvent::tap_enable(tap, false);
        }
        self.run_loop.stop();
    }
}

impl Drop for KeyboardContext {
    fn drop(&mut self) {
        // Remove native references before this stack-backed callback context goes away,
        // including when source creation fails or worker setup unwinds.
        if let Some(tap) = self.event_tap.get() {
            CGEvent::tap_enable(tap, false);
            tap.invalidate();
        }
        if let Some(source) = self.run_loop_source.get() {
            self.run_loop
                .remove_source(Some(source), unsafe { kCFRunLoopDefaultMode });
            source.invalidate();
        }
    }
}

unsafe extern "C-unwind" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    refcon: *mut std::ffi::c_void,
) -> *mut CGEvent {
    let event_ptr = event.as_ptr();
    let context: &KeyboardContext = unsafe { &*(refcon as *const KeyboardContext) };

    let handled = catch_unwind(AssertUnwindSafe(|| {
        if event_type == CGEventType::TapDisabledByTimeout
            || event_type == CGEventType::TapDisabledByUserInput
        {
            if let Some(tap) = context.event_tap.get() {
                tracing::warn!(?event_type, "Event tap disabled, re-enabling");
                CGEvent::tap_enable(tap, true);
            }
            false
        } else {
            event_type == CGEventType::KeyDown && handle_keyboard(context, event_ptr)
        }
    }))
    .unwrap_or_else(|_| {
        tracing::error!("Event tap callback panicked, stopping event tap");
        context.stop();
        false
    });

    if handled {
        std::ptr::null_mut()
    } else {
        event_ptr
    }
}

fn handle_keyboard(context: &KeyboardContext, event: *mut CGEvent) -> bool {
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
    while let Ok(next) = context.keymap_rx.try_recv() {
        *context.keymap.borrow_mut() = next;
    }
    let resolved = context
        .keymap
        .borrow()
        .resolve(&keystroke)
        .map(str::to_string);
    let Some(keymap) = resolved else {
        return false;
    };

    if context.is_suspended.load(Ordering::Relaxed) {
        tracing::info!("Received keymap action, resuming window management");
        context.is_suspended.store(false, Ordering::Relaxed);
    }

    tracing::trace!(%keymap, %keystroke, "Keystroke matched binding");
    if context
        .event_sender
        .send(HubEvent::RunBinding { keymap, keystroke })
        .is_err()
    {
        tracing::warn!("Owner loop unavailable, stopping event tap");
        context.stop();
        return false;
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
