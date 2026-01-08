use std::cell::OnceCell;
use std::sync::mpsc::Sender;

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_SHOW,
    EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART,
    EVENT_SYSTEM_MOVESIZEEND, OBJID_WINDOW, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

use super::hub::{HubEvent, WindowHandle};
use super::window::is_manageable_window;

thread_local! {
    static SENDER: OnceCell<Sender<HubEvent>> = const { OnceCell::new() };
}

pub(super) fn install_event_hook(sender: Sender<HubEvent>) -> Result<HWINEVENTHOOK> {
    SENDER.with(|s| s.set(sender).ok());

    let hook = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_CREATE,
            EVENT_SYSTEM_MOVESIZEEND,
            None,
            Some(event_hook_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };

    if hook.is_invalid() {
        Err(anyhow!("Failed to install event hook"))
    } else {
        Ok(hook)
    }
}

pub(super) fn uninstall_event_hook(hook: HWINEVENTHOOK) {
    if !unsafe { UnhookWinEvent(hook) }.as_bool() {
        tracing::warn!("UnhookWinEvent failed");
    }
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
        let sender = s.get().unwrap();
        match event {
            EVENT_OBJECT_CREATE | EVENT_OBJECT_SHOW | EVENT_SYSTEM_MINIMIZEEND => {
                if is_manageable_window(hwnd) {
                    sender
                        .send(HubEvent::WindowCreated(WindowHandle(hwnd)))
                        .ok();
                }
            }
            EVENT_OBJECT_DESTROY | EVENT_OBJECT_HIDE | EVENT_SYSTEM_MINIMIZESTART => {
                sender
                    .send(HubEvent::WindowDestroyed(WindowHandle(hwnd)))
                    .ok();
            }
            EVENT_SYSTEM_FOREGROUND => {
                sender
                    .send(HubEvent::WindowFocused(WindowHandle(hwnd)))
                    .ok();
            }
            EVENT_SYSTEM_MOVESIZEEND => {
                sender
                    .send(HubEvent::WindowMovedOrResized(WindowHandle(hwnd)))
                    .ok();
            }
            _ => {}
        }
    });
}
