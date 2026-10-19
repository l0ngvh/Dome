use std::cell::OnceCell;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EVENT_OBJECT_CLOAKED, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY,
    EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW,
    EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZESTART,
    GetForegroundWindow, GetMessageW, MSG, OBJID_WINDOW, PM_NOREMOVE, PeekMessageW,
    PostThreadMessageW, TranslateMessage, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_QUIT,
};

use super::external::HwndId;
use super::window_server::NativeEvent;
use crate::platform::windows::event_loop::EventSender;
use crate::platform::windows::event_loop::Shutdown;

pub(super) struct EventHookHandle {
    thread_id: u32,
    join_handle: Option<JoinHandle<()>>,
}

impl EventHookHandle {
    pub(super) fn new(thread_id: u32, join_handle: JoinHandle<()>) -> Self {
        Self {
            thread_id,
            join_handle: Some(join_handle),
        }
    }
}

impl Drop for EventHookHandle {
    fn drop(&mut self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
        if let Some(thread) = self.join_handle.take()
            && thread.join().is_err()
        {
            tracing::error!("Event hook thread panicked");
        }
    }
}

pub(super) fn start_event_hooks(
    hub_sender: EventSender<NativeEvent>,
    shutdown: Shutdown,
) -> Result<EventHookHandle> {
    let (event_ready, event_readiness) = mpsc::sync_channel(0);
    let event_thread = std::thread::Builder::new()
        .name("dome-event-hooks".to_owned())
        .spawn(move || run_event_hooks(hub_sender, shutdown, event_ready))?;
    let event_result = event_readiness
        .recv()
        .map_err(|_| anyhow::anyhow!("event hook thread died"))
        .and_then(|result| result);
    match event_result {
        Ok(thread_id) => Ok(EventHookHandle::new(thread_id, event_thread)),
        Err(error) => {
            let _ = event_thread.join();
            Err(error)
        }
    }
}

pub(super) fn run_event_hooks(
    sender: EventSender<NativeEvent>,
    shutdown: Shutdown,
    ready: mpsc::SyncSender<Result<u32>>,
) {
    let _shutdown = StopApplicationOnExit(shutdown);
    let thread_id = unsafe { GetCurrentThreadId() };
    // Publish readiness only after quit messages can be queued and every hook is installed.
    let _ = unsafe { PeekMessageW(&mut MSG::default(), None, 0, 0, PM_NOREMOVE) };
    let _hooks = match install_event_hooks(sender) {
        Ok(hooks) => hooks,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(thread_id)).is_err() {
        return;
    }
    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
        if result <= 0 {
            if result < 0 {
                tracing::error!(error = %windows::core::Error::from_thread(), "Event hook message pump failed");
            }
            break;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

struct StopApplicationOnExit(Shutdown);

impl Drop for StopApplicationOnExit {
    fn drop(&mut self) {
        self.0.terminate();
    }
}

thread_local! {
    static SENDER: OnceCell<EventSender<NativeEvent>> = const { OnceCell::new() };
}

struct EventHooks(Vec<HWINEVENTHOOK>);

impl Drop for EventHooks {
    fn drop(&mut self) {
        for hook in &self.0 {
            if !unsafe { UnhookWinEvent(*hook) }.as_bool() {
                tracing::warn!("Failed to unhook event hook");
            }
        }
    }
}

fn install_event_hooks(sender: EventSender<NativeEvent>) -> Result<EventHooks> {
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
    let mut hooks = EventHooks(Vec::with_capacity(ranges.len()));

    for (min, max) in ranges {
        let hook = unsafe { SetWinEventHook(min, max, None, Some(event_hook_proc), 0, 0, flags) };
        if hook.is_invalid() {
            let err = unsafe { GetLastError() };
            return Err(anyhow!(
                "Failed to install event hook for range {min}-{max}: {err:?}"
            ));
        }
        hooks.0.push(hook);
    }

    Ok(hooks)
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
        let hwnd_id = HwndId::from(hwnd);
        match event {
            EVENT_OBJECT_CREATE => {
                let _ = sender.send(NativeEvent::WindowCreated(hwnd_id));
            }
            EVENT_OBJECT_SHOW => {
                let _ = sender.send(NativeEvent::WindowShown(hwnd_id));
            }
            EVENT_OBJECT_UNCLOAKED => {
                let _ = sender.send(NativeEvent::WindowUncloaked(hwnd_id));
            }
            EVENT_SYSTEM_MINIMIZEEND => {
                let _ = sender.send(NativeEvent::WindowRestored {
                    hwnd_id,
                    observed_at: Instant::now(),
                });
            }
            EVENT_OBJECT_NAMECHANGE => {
                let _ = sender.send(NativeEvent::WindowTitleChanged(hwnd_id));
            }
            EVENT_OBJECT_DESTROY => {
                let _ = sender.send(NativeEvent::WindowDestroyed(hwnd_id));
            }
            EVENT_OBJECT_HIDE => {
                let _ = sender.send(NativeEvent::WindowHidden(hwnd_id));
            }
            EVENT_OBJECT_CLOAKED => {
                let _ = sender.send(NativeEvent::WindowCloaked(hwnd_id));
            }
            EVENT_SYSTEM_MINIMIZESTART => {
                let _ = sender.send(NativeEvent::WindowMinimized(hwnd_id));
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
                let _ = sender.send(NativeEvent::WindowFocused(hwnd_id));
            }
            // MOVESIZESTART fires when user begins a drag/resize.
            // MOVESIZEEND fires after user drag/resize completes.
            // LOCATIONCHANGE catches programmatic resizes (e.g. maximize, restore)
            // which don't trigger the move/size cycle.
            EVENT_SYSTEM_MOVESIZESTART => {
                let _ = sender.send(NativeEvent::MoveSizeStart(hwnd_id));
            }
            EVENT_SYSTEM_MOVESIZEEND => {
                let _ = sender.send(NativeEvent::MoveSizeEnd {
                    hwnd_id,
                    observed_at: Instant::now(),
                });
            }
            EVENT_OBJECT_LOCATIONCHANGE => {
                let _ = sender.send(NativeEvent::LocationChanged {
                    hwnd_id,
                    observed_at: Instant::now(),
                });
            }
            _ => {
                tracing::trace!(event, ?hwnd_id, "unexpected WinEvent");
            }
        }
    });
}
