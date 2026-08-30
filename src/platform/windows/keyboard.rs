use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, OnceLock, RwLock};
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_LCONTROL, VK_LEFT, VK_LMENU,
    VK_LSHIFT, VK_LWIN, VK_MENU, VK_OEM_4, VK_OEM_6, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, MSG, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
    WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use super::HubSender;
use super::dome::HubEvent;
use crate::config::{Keymap, Modifiers};
use crate::keymap::{KeymapState, Resolved};
use crate::lua_runtime::RuntimeMsg;

pub(super) struct KeyboardHookHandle {
    thread_id: u32,
    join_handle: Option<JoinHandle<()>>,
}

struct KeyboardState {
    sender: HubSender,
    keymap_state: Arc<RwLock<KeymapState>>,
    runtime_sender: mpsc::Sender<RuntimeMsg>,
}

static STATE: OnceLock<KeyboardState> = OnceLock::new();

/// Modifier set built from the keydown/keyup transitions the hook observes.
/// No poll reads the current keystroke reliably inside a low-level keyboard
/// hook: GetAsyncKeyState updates only after Raw Input, which runs after the
/// hook, and GetKeyState/GetKeyboardState advance only as the thread pumps its
/// message queue, which the hook has not done. So a modifier still held can
/// read as released at hotkey time and the binding misses its modifier. Only
/// the hook thread touches this, so Relaxed ordering is enough.
///
/// Refs: https://github.com/input-leap/input-leap/discussions/1458;
/// https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc
/// (advises monitoring raw input over in-hook polling).
static MODIFIERS: AtomicU8 = AtomicU8::new(0);

pub(super) fn install_keyboard_hook(
    sender: HubSender,
    keymap_state: Arc<RwLock<KeymapState>>,
    runtime_sender: mpsc::Sender<RuntimeMsg>,
) -> anyhow::Result<KeyboardHookHandle> {
    STATE
        .set(KeyboardState {
            sender,
            keymap_state,
            runtime_sender,
        })
        .ok();

    let (tx, rx) = mpsc::sync_channel::<Result<u32, windows::core::Error>>(0);

    let join_handle = thread::spawn(move || {
        let thread_id = unsafe { GetCurrentThreadId() };
        match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0) } {
            Ok(hook) => {
                tx.send(Ok(thread_id)).ok();
                let mut msg = MSG::default();
                unsafe {
                    while GetMessageW(&mut msg, None, 0, 0).into() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                if let Err(e) = unsafe { UnhookWindowsHookEx(hook) } {
                    tracing::warn!("UnhookWindowsHookEx failed: {e}");
                }
            }
            Err(e) => {
                tx.send(Err(e)).ok();
            }
        }
    });

    let thread_id = rx
        .recv()
        .map_err(|_| anyhow::anyhow!("keyboard hook thread died"))??;

    Ok(KeyboardHookHandle {
        thread_id,
        join_handle: Some(join_handle),
    })
}

pub(super) fn uninstall_keyboard_hook(mut handle: KeyboardHookHandle) {
    unsafe { PostThreadMessageW(handle.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
    if let Some(jh) = handle.join_handle.take() {
        jh.join().ok();
    }
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        let kb_struct = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = VIRTUAL_KEY(kb_struct.vkCode as u16);
        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;

        if let Some(modifier) = modifier_of(vk) {
            if is_down {
                MODIFIERS.fetch_or(modifier.bits(), Ordering::Relaxed);
            } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                MODIFIERS.fetch_and(!modifier.bits(), Ordering::Relaxed);
            }
        } else if is_down {
            let modifiers = Modifiers::from_bits_truncate(MODIFIERS.load(Ordering::Relaxed));
            if let Some(state) = STATE.get()
                && let Some(resolved) = resolve_key(vk, modifiers, &state.keymap_state)
            {
                match resolved {
                    Resolved::Actions(actions) => {
                        tracing::trace!(%actions, "Keymap matched");
                        state.sender.send(HubEvent::Action(actions));
                    }
                    Resolved::Callback(id) => {
                        tracing::trace!(?id, "Keymap matched callback");
                        if state
                            .runtime_sender
                            .send(RuntimeMsg::RunCallback(id))
                            .is_err()
                        {
                            tracing::warn!("dome-lua thread unavailable, callback dropped");
                        }
                    }
                }
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// A low-level hook reports the side-specific virtual key (VK_LSHIFT, not
/// VK_SHIFT). Map both the generic and the left/right codes so the tracked set
/// stays correct whichever the driver sends.
fn modifier_of(vk: VIRTUAL_KEY) -> Option<Modifiers> {
    match vk {
        VK_LWIN | VK_RWIN => Some(Modifiers::META),
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT => Some(Modifiers::SHIFT),
        VK_MENU | VK_LMENU | VK_RMENU => Some(Modifiers::ALT),
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL => Some(Modifiers::CTRL),
        _ => None,
    }
}

fn resolve_key(
    vk: VIRTUAL_KEY,
    modifiers: Modifiers,
    keymap_state: &Arc<RwLock<KeymapState>>,
) -> Option<Resolved> {
    let key = vk_to_string(vk)?;
    let keymap = Keymap { key, modifiers };

    let mut ks = keymap_state.write().ok()?;
    ks.resolve(&keymap)
}

fn vk_to_string(vk: VIRTUAL_KEY) -> Option<String> {
    let s = match vk {
        VK_RETURN => "return",
        VK_BACK => "backspace",
        VK_ESCAPE => "escape",
        VK_TAB => "tab",
        VK_SPACE => "space",
        VK_UP => "up",
        VK_DOWN => "down",
        VK_LEFT => "left",
        VK_RIGHT => "right",
        VK_OEM_4 => "[",
        VK_OEM_6 => "]",
        _ => {
            let code = vk.0 as u8;
            if matches!(code, b'0'..=b'9' | b'A'..=b'Z') {
                return Some((code.to_ascii_lowercase() as char).to_string());
            }
            return None;
        }
    };
    Some(s.to_string())
}
