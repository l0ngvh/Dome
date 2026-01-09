use std::cell::OnceCell;
use std::sync::mpsc::Sender;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LMENU, VK_LWIN, VK_MENU, VK_RMENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, KBDLLHOOKSTRUCT, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_SYSKEYDOWN,
};

use super::hub::HubEvent;
use crate::config::{Config, Keymap, Modifiers};

struct KeyboardState {
    sender: Sender<HubEvent>,
    config: Config,
}

thread_local! {
    static STATE: OnceCell<KeyboardState> = const { OnceCell::new() };
}

pub(super) fn install_keyboard_hook(sender: Sender<HubEvent>) -> windows::core::Result<HHOOK> {
    let config = Config::default();
    STATE.with(|s| s.set(KeyboardState { sender, config }).ok());
    unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0) }
}

pub(super) fn uninstall_keyboard_hook(hook: HHOOK) {
    if let Err(e) = unsafe { UnhookWindowsHookEx(hook) } {
        tracing::warn!("UnhookWindowsHookEx failed: {e}");
    }
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let kb_struct = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let vk = VIRTUAL_KEY(kb_struct.vkCode as u16);

            if let Some(keymap) = build_keymap(vk) {
                let handled = STATE.with(|s| {
                    let state = s.get().unwrap();
                    let actions = state.config.get_actions(&keymap);
                    if actions.is_empty() {
                        return false;
                    }
                    state.sender.send(HubEvent::Action(actions)).ok();
                    true
                });
                if handled {
                    return LRESULT(1);
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn build_keymap(vk: VIRTUAL_KEY) -> Option<Keymap> {
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
    Some(Keymap { key, modifiers })
}

fn is_key_pressed(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetKeyState(vk.0 as i32) < 0 }
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
