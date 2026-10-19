use crate::platform::keymap::KeymapView;
use std::cell::RefCell;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_LCONTROL, VK_LEFT, VK_LMENU,
    VK_LSHIFT, VK_LWIN, VK_MENU, VK_OEM_4, VK_OEM_6, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, MSG,
    PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::config::{Keystroke, Modifiers};
use crate::platform::windows::dome::HubEvent;

use crate::platform::windows::event_loop::EventSender;

pub(super) struct KeyboardHookHandle {
    thread_id: u32,
    join_handle: Option<JoinHandle<()>>,
}

impl KeyboardHookHandle {
    pub(super) fn new(thread_id: u32, join_handle: JoinHandle<()>) -> Self {
        Self {
            thread_id,
            join_handle: Some(join_handle),
        }
    }
}

pub(super) fn run_keyboard_hook(
    keymap_rx: mpsc::Receiver<KeymapView>,
    hub_sender: EventSender<HubEvent>,
    ready: mpsc::SyncSender<Result<u32, windows::core::Error>>,
) {
    STATE.set(KeyboardState { hub_sender }).ok();
    KEYMAP_RX.with(|cell| *cell.borrow_mut() = Some(keymap_rx));
    let thread_id = unsafe { GetCurrentThreadId() };
    match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0) } {
        Ok(hook) => {
            ready.send(Ok(thread_id)).ok();
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
            ready.send(Err(e)).ok();
        }
    }
}

struct KeyboardState {
    hub_sender: EventSender<HubEvent>,
}

static STATE: OnceLock<KeyboardState> = OnceLock::new();

/// Tracks physical sides because releasing one must not clear the other.
/// In-hook polling is unreliable because the OS updates async key state after
/// the hook. Only the hook thread touches this, so Relaxed ordering is enough.
/// https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc
static MODIFIERS: AtomicU8 = AtomicU8::new(0);

impl Drop for KeyboardHookHandle {
    fn drop(&mut self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
        if let Some(thread) = self.join_handle.take() {
            thread.join().ok();
        }
    }
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let keyboard = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if let Some((keymap, keystroke)) =
            resolve_keyboard_event(wparam.0 as u32, keyboard, &MODIFIERS)
            && let Some(state) = STATE.get()
        {
            tracing::trace!(%keymap, %keystroke, "Keystroke matched binding");
            let _ = state
                .hub_sender
                .send(HubEvent::RunBinding { keymap, keystroke });
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn resolve_keyboard_event(
    message: u32,
    keyboard: &KBDLLHOOKSTRUCT,
    physical_modifiers: &AtomicU8,
) -> Option<(String, Keystroke)> {
    let is_down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
    if let Some(bit) = physical_modifier_bit(keyboard) {
        if is_down {
            physical_modifiers.fetch_or(bit, Ordering::Relaxed);
        } else if message == WM_KEYUP || message == WM_SYSKEYUP {
            physical_modifiers.fetch_and(!bit, Ordering::Relaxed);
        }
        return None;
    }
    if !is_down {
        return None;
    }
    let physical = physical_modifiers.load(Ordering::Relaxed);
    let mut modifiers = Modifiers::empty();
    for (sides, modifier) in [
        (0b0000_0011, Modifiers::CTRL),
        (0b0000_1100, Modifiers::SHIFT),
        (0b0011_0000, Modifiers::ALT),
        (0b1100_0000, Modifiers::META),
    ] {
        if physical & sides != 0 {
            modifiers |= modifier;
        }
    }
    resolve_key(VIRTUAL_KEY(keyboard.vkCode as u16), modifiers)
}

fn physical_modifier_bit(keyboard: &KBDLLHOOKSTRUCT) -> Option<u8> {
    let side = match VIRTUAL_KEY(keyboard.vkCode as u16) {
        VK_LCONTROL => 0,
        VK_RCONTROL => 1,
        VK_LSHIFT => 2,
        VK_RSHIFT => 3,
        VK_LMENU => 4,
        VK_RMENU => 5,
        VK_LWIN => 6,
        VK_RWIN => 7,
        VK_CONTROL => u8::from(keyboard.flags.contains(LLKHF_EXTENDED)),
        VK_MENU => 4 + u8::from(keyboard.flags.contains(LLKHF_EXTENDED)),
        // Missing or unrecognized scan codes retain generic-key support by
        // falling back to the left side.
        VK_SHIFT => 2 + u8::from(keyboard.scanCode == 0x36),
        _ => return None,
    };
    Some(1 << side)
}

// A `Receiver` is not `Sync`, so it cannot live in the global `STATE`. The hook
// thread that runs the proc owns it instead.
thread_local! {
    static KEYMAP: RefCell<KeymapView> = RefCell::new(KeymapView::new());
    static KEYMAP_RX: RefCell<Option<mpsc::Receiver<KeymapView>>> = const { RefCell::new(None) };
}

fn resolve_key(vk: VIRTUAL_KEY, modifiers: Modifiers) -> Option<(String, Keystroke)> {
    let key = vk_to_string(vk)?;
    let keystroke = Keystroke { key, modifiers };

    KEYMAP_RX.with(|rx| {
        if let Some(rx) = rx.borrow().as_ref() {
            KEYMAP.with(|local| {
                while let Ok(next) = rx.try_recv() {
                    *local.borrow_mut() = next;
                }
            });
        }
    });
    let keymap = KEYMAP.with(|local| local.borrow().resolve(&keystroke).map(str::to_string))?;
    Some((keymap, keystroke))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{KeymapEffects, ModalKeymaps};
    use crate::platform::keymap::KeymapPublisher;

    fn key_event(key: VIRTUAL_KEY, scan_code: u32, extended: bool) -> KBDLLHOOKSTRUCT {
        KBDLLHOOKSTRUCT {
            vkCode: u32::from(key.0),
            scanCode: scan_code,
            flags: if extended {
                LLKHF_EXTENDED
            } else {
                Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn release_modifier_sides_keeps_binding_until_both_are_released() {
        let specific = |key| key_event(key, 0, false);
        let mut cases = Vec::new();
        for (left, right, modifiers) in [
            (VK_LCONTROL, VK_RCONTROL, Modifiers::CTRL),
            (VK_LSHIFT, VK_RSHIFT, Modifiers::SHIFT),
            (VK_LMENU, VK_RMENU, Modifiers::ALT),
            (VK_LWIN, VK_RWIN, Modifiers::META),
        ] {
            let sides = [specific(left), specific(right)];
            cases.push((sides, sides, modifiers));
        }
        for (generic, left, right, left_scan, right_scan, modifiers) in [
            (
                VK_CONTROL,
                VK_LCONTROL,
                VK_RCONTROL,
                0x1d,
                0x1d,
                Modifiers::CTRL,
            ),
            (VK_SHIFT, VK_LSHIFT, VK_RSHIFT, 0x2a, 0x36, Modifiers::SHIFT),
            (VK_MENU, VK_LMENU, VK_RMENU, 0x38, 0x38, Modifiers::ALT),
        ] {
            let generic_sides = [
                key_event(generic, left_scan, false),
                key_event(generic, right_scan, generic != VK_SHIFT),
            ];
            let specific_sides = [specific(left), specific(right)];
            cases.push((generic_sides, specific_sides, modifiers));
            cases.push((specific_sides, generic_sides, modifiers));
        }
        cases.push((
            [specific(VK_SHIFT), specific(VK_RSHIFT)],
            [specific(VK_LSHIFT), specific(VK_RSHIFT)],
            Modifiers::SHIFT,
        ));

        let lua = mlua::Lua::new();
        let callback = lua.create_function(|_, ()| Ok(())).unwrap();
        for (pressed, released, modifiers) in cases {
            let stroke = |modifiers| Keystroke {
                key: "h".into(),
                modifiers,
            };
            let keymaps = ModalKeymaps {
                modes: std::collections::HashMap::from([(
                    "resize".into(),
                    std::collections::HashMap::from([
                        (stroke(modifiers), callback.clone()),
                        (stroke(Modifiers::empty()), callback.clone()),
                    ]),
                )]),
            };
            let (sender, receiver) = mpsc::channel();
            KEYMAP_RX.with(|cell| *cell.borrow_mut() = Some(receiver));
            let mut publisher = KeymapPublisher::new(KeymapView::new(), sender);
            publisher.update_keymaps(&keymaps);
            publisher.switch_mode("resize");
            let physical = AtomicU8::new(0);
            let (down, up) = if modifiers == Modifiers::ALT {
                (WM_SYSKEYDOWN, WM_SYSKEYUP)
            } else {
                (WM_KEYDOWN, WM_KEYUP)
            };
            for side in pressed {
                assert!(resolve_keyboard_event(down, &side, &physical).is_none());
            }
            let selected = || {
                resolve_keyboard_event(WM_KEYDOWN, &specific(VIRTUAL_KEY(b'H'.into())), &physical)
            };
            assert_eq!(selected(), Some(("resize".into(), stroke(modifiers))));
            assert!(resolve_keyboard_event(up, &released[0], &physical).is_none());
            assert_eq!(selected(), Some(("resize".into(), stroke(modifiers))));
            assert!(resolve_keyboard_event(up, &released[1], &physical).is_none());
            assert_eq!(
                selected(),
                Some(("resize".into(), stroke(Modifiers::empty())))
            );
        }
    }
}
