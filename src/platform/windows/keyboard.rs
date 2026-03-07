use std::sync::mpsc;
use std::sync::{OnceLock, RwLock};
use std::thread::{self, JoinHandle};

use calloop::channel::Sender;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LMENU, VK_LWIN, VK_MENU, VK_RMENU, VK_RWIN,
    VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, MSG, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_QUIT,
    WM_SYSKEYDOWN,
};

use super::dome::HubEvent;
use crate::config::{Config, Keymap, Modifiers};

pub(super) struct KeyboardHookHandle {
    thread_id: u32,
    join_handle: Option<JoinHandle<()>>,
}

struct KeyboardState {
    sender: Sender<HubEvent>,
    config: RwLock<Config>,
}

static STATE: OnceLock<KeyboardState> = OnceLock::new();

pub(super) fn install_keyboard_hook(
    sender: Sender<HubEvent>,
    config: Config,
) -> anyhow::Result<KeyboardHookHandle> {
    STATE
        .set(KeyboardState {
            sender,
            config: RwLock::new(config),
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

pub(super) fn update_config(config: Config) {
    if let Some(state) = STATE.get() {
        *state.config.write().unwrap() = config;
    }
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
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let kb_struct = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let vk = VIRTUAL_KEY(kb_struct.vkCode as u16);

            if let Some(actions) = get_actions(vk) {
                if let Some(state) = STATE.get() {
                    state.sender.send(HubEvent::Action(actions)).ok();
                }
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn get_actions(vk: VIRTUAL_KEY) -> Option<crate::action::Actions> {
    if matches!(
        vk,
        VK_SHIFT | VK_CONTROL | VK_MENU | VK_LWIN | VK_RWIN | VK_LMENU | VK_RMENU
    ) {
        return None;
    }

    let mut modifiers = Modifiers::empty();
    if is_key_pressed(VK_LWIN) || is_key_pressed(VK_RWIN) {
        modifiers |= Modifiers::CMD;
    }
    if is_key_pressed(VK_SHIFT) {
        modifiers |= Modifiers::SHIFT;
    }
    if is_key_pressed(VK_MENU) {
        modifiers |= Modifiers::ALT;
    }
    if is_key_pressed(VK_CONTROL) {
        modifiers |= Modifiers::CTRL;
    }

    let key = vk_to_string(vk)?;
    let keymap = Keymap { key, modifiers };

    let state = STATE.get()?;
    let config = state.config.read().ok()?;
    let actions = config.keymaps.get(&keymap).cloned().unwrap_or_default();
    if actions.is_empty() {
        None
    } else {
        Some(actions)
    }
}

fn is_key_pressed(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) < 0 }
}

fn vk_to_string(vk: VIRTUAL_KEY) -> Option<String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

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
