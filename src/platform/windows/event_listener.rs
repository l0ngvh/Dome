use std::cell::{Cell, OnceCell};
use std::time::Instant;

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{GetLastError, HWND};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_CLOAKED, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW,
    EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZESTART, OBJID_WINDOW,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

use super::external::HwndId;
use super::window_server::{NativeEvent, NativeEventSender};

thread_local! {
    static SENDER: OnceCell<NativeEventSender> = const { OnceCell::new() };
    static PANICKED: Cell<bool> = const { Cell::new(false) };
}

pub(super) struct EventHooks(Vec<HWINEVENTHOOK>);

impl Drop for EventHooks {
    fn drop(&mut self) {
        for hook in &self.0 {
            if !unsafe { UnhookWinEvent(*hook) }.as_bool() {
                tracing::warn!("Failed to unhook event hook");
            }
        }
    }
}

pub(super) fn install_event_hooks(sender: NativeEventSender) -> Result<EventHooks> {
    SENDER.with(|s| s.set(sender).ok());

    let ranges = [
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
        (EVENT_SYSTEM_MOVESIZESTART, EVENT_SYSTEM_MOVESIZEEND),
        (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
        (EVENT_OBJECT_CREATE, EVENT_OBJECT_NAMECHANGE),
        (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
    ];

    let mut hooks = Vec::with_capacity(ranges.len());

    for (min, max) in ranges {
        let flags = WINEVENT_OUTOFCONTEXT
            | if min == EVENT_SYSTEM_FOREGROUND {
                0
            } else {
                WINEVENT_SKIPOWNPROCESS
            };
        let hook = unsafe { SetWinEventHook(min, max, None, Some(event_hook_proc), 0, 0, flags) };
        if hook.is_invalid() {
            let err = unsafe { GetLastError() };
            for h in hooks {
                let _ = unsafe { UnhookWinEvent(h) };
            }
            return Err(anyhow!(
                "Failed to install event hook for range {min}-{max}: {err:?}"
            ));
        }
        hooks.push(hook);
    }

    Ok(EventHooks(hooks))
}

unsafe extern "system" fn event_hook_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _event_time: u32,
) {
    if PANICKED.get() {
        return;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch_event(event, hwnd, id_object);
    }));
    if result.is_err() {
        PANICKED.set(true);
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(1);
        }
    }
}

fn dispatch_event(event: u32, hwnd: HWND, id_object: i32) {
    if id_object != OBJID_WINDOW.0 {
        return;
    }

    SENDER.with(|s| {
        let Some(sender) = s.get() else { return };
        let hwnd_id = HwndId::from(hwnd);
        match event {
            EVENT_OBJECT_CREATE | EVENT_OBJECT_SHOW | EVENT_OBJECT_UNCLOAKED => {
                sender.send(NativeEvent::Discover(hwnd_id));
            }
            EVENT_SYSTEM_MINIMIZEEND => {
                sender.send(NativeEvent::Restored {
                    window: hwnd_id,
                    observed_at: Instant::now(),
                });
            }
            EVENT_OBJECT_NAMECHANGE => {
                sender.send(NativeEvent::Title(hwnd_id));
            }
            EVENT_OBJECT_DESTROY | EVENT_OBJECT_HIDE | EVENT_OBJECT_CLOAKED => {
                sender.send(NativeEvent::Gone(hwnd_id));
            }
            EVENT_SYSTEM_MINIMIZESTART => {
                sender.send(NativeEvent::Minimized(hwnd_id));
            }
            EVENT_SYSTEM_FOREGROUND => sender.observe_foreground(Some(hwnd_id)),
            EVENT_SYSTEM_MOVESIZESTART => {
                sender.send(NativeEvent::MoveStarted {
                    window: hwnd_id,
                    observed_at: Instant::now(),
                });
            }
            EVENT_SYSTEM_MOVESIZEEND => {
                sender.send(NativeEvent::MoveEnded {
                    window: hwnd_id,
                    observed_at: Instant::now(),
                });
            }
            EVENT_OBJECT_LOCATIONCHANGE => {
                sender.send(NativeEvent::Moved {
                    window: hwnd_id,
                    observed_at: Instant::now(),
                });
            }
            _ => {
                tracing::trace!(event, ?hwnd_id, "unexpected WinEvent");
            }
        }
    });
}
