use std::cell::OnceCell;

use anyhow::{Result, anyhow};
use calloop::channel::Sender;
use windows::Win32::Foundation::{GetLastError, HWND};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_CLOAKED, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW,
    EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MOVESIZEEND, GetForegroundWindow, OBJID_WINDOW,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

use super::dome::HubEvent;
use super::window::ManagedHwnd;

thread_local! {
    static SENDER: OnceCell<Sender<HubEvent>> = const { OnceCell::new() };
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

pub(super) fn install_event_hooks(sender: Sender<HubEvent>) -> Result<EventHooks> {
    SENDER.with(|s| s.set(sender).ok());

    // We need separate hooks because SetWinEventHook only accepts contiguous
    // event ranges (min <= max). A single hook covering all events would include
    // thousands of irrelevant events between the ranges we care about:
    // - foreground/movesize: 0x0003-0x000B
    // - minimize: 0x0016-0x0017
    // - object create/hide/namechange: 0x8000-0x800C
    // - object cloaked/uncloaked: 0x8017-0x8018 (for UWP apps like Settings)
    // Using a single range like 0x0003-0x8018 would fire our callback for every
    // event in between, wasting CPU. Worse, if min > max (e.g., 0x8000-0x000B),
    // SetWinEventHook fails with ERROR_INVALID_HOOK_FILTER (1426).
    let ranges = [
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MOVESIZEEND),
        (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
        (EVENT_OBJECT_CREATE, EVENT_OBJECT_NAMECHANGE),
        (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
    ];

    let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
    let mut hooks = Vec::with_capacity(ranges.len());

    for (min, max) in ranges {
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
    if id_object != OBJID_WINDOW.0 {
        return;
    }

    SENDER.with(|s| {
        let Some(sender) = s.get() else { return };
        let managed = ManagedHwnd::new(hwnd);
        match event {
            EVENT_OBJECT_CREATE
            | EVENT_OBJECT_SHOW
            | EVENT_SYSTEM_MINIMIZEEND
            | EVENT_OBJECT_UNCLOAKED => {
                sender.send(HubEvent::WindowCreated(managed)).ok();
            }
            EVENT_OBJECT_NAMECHANGE => {
                sender.send(HubEvent::WindowTitleChanged(managed)).ok();
            }
            EVENT_OBJECT_DESTROY | EVENT_OBJECT_HIDE | EVENT_OBJECT_CLOAKED => {
                sender.send(HubEvent::WindowDestroyed(managed)).ok();
            }
            EVENT_SYSTEM_MINIMIZESTART => {
                sender.send(HubEvent::WindowMinimized(managed)).ok();
            }
            EVENT_SYSTEM_FOREGROUND => {
                // This can happen when Windows queue an event for an activated application, but by
                // the time this callback is run the focus have been given to another app. This
                // will cause a feedback loop where this app try to take focus and succeed, but the
                // activation event for the other app is already queued. The other app will then
                // proceed to take focus when the event is processed, but which tries to take focus
                // and forms the feedback loop.
                if unsafe { GetForegroundWindow() } != hwnd {
                    return;
                }
                sender.send(HubEvent::WindowFocused(managed)).ok();
            }
            // MOVESIZEEND fires after user drag/resize. LOCATIONCHANGE catches programmatic
            // resizes (e.g. maximize, restore) which don't trigger the move/size cycle.
            EVENT_SYSTEM_MOVESIZEEND | EVENT_OBJECT_LOCATIONCHANGE => {
                sender.send(HubEvent::WindowMovedOrResized(managed)).ok();
            }
            _ => {}
        }
    });
}
