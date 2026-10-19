use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Once;
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, LoadImageW};
use windows::core::PCWSTR;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, HICON,
    MSGFLT_ALLOW, RegisterWindowMessageW, SPI_SETWORKAREA, SetWindowLongPtrW, WM_DISPLAYCHANGE,
    WM_SETTINGCHANGE,
};
use windows::core::w;

mod menu;

use super::ui::wnd_proc::{wnd_proc_no_state, wnd_proc_prologue};
use super::{TRAY_CLASS_NAME, TRAY_UID, WM_APP_TRAY};
use crate::action::{Action, WorkspaceInfo};
use crate::platform::windows::event_loop::EventSender;
use crate::platform::windows::event_loop::NativeEvent;
use crate::platform::windows::event_loop::native::CallbackPanic;
use crate::platform::windows::ui::window::Point;
use crate::platform::windows::ui::window::Size;
use crate::platform::windows::ui::window::WindowAttributes;
use crate::platform::windows::ui::window::native::{OwnedHwnd, ex_style_for, register_class};
use menu::{MenuEntry, build_menu, focused_tooltip, is_tray_context_menu, show_context_menu};

pub(super) trait Tray {
    fn update_workspaces(&self, workspaces: &[WorkspaceInfo]);
}

/// Owns the tray icon's hidden top-level window, which receives shell callbacks and
/// taskbar-restart broadcasts that a message-only window would not receive.
pub(super) struct NativeTray {
    // Remove the icon before destroying its callback window.
    state: Rc<TrayState>,
    window: OwnedHwnd,
    on_action: Rc<dyn Fn(Action)>,
}

pub(super) fn load_icon(id: u16) -> anyhow::Result<HICON> {
    let hmodule = unsafe { GetModuleHandleW(None) }?;
    let instance = HINSTANCE(hmodule.0);
    let handle = unsafe {
        LoadImageW(
            Some(instance),
            PCWSTR(id as usize as *const u16),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        )
    }?;
    Ok(HICON(handle.0))
}

impl NativeTray {
    pub(super) fn new(
        icon: HICON,
        on_action: Rc<dyn Fn(Action)>,
        events: EventSender<NativeEvent>,
        panic: Rc<CallbackPanic>,
    ) -> anyhow::Result<Self> {
        ensure_tray_class_registered();
        let attributes = WindowAttributes {
            position: Point::new(0, 0),
            size: Size::new(1, 1),
            click_through: false,
            focusable: false,
            dpi_changes: None,
        };
        let window = OwnedHwnd::new(TRAY_CLASS_NAME, ex_style_for(&attributes), &attributes)?;

        let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        if taskbar_created == 0 {
            tracing::warn!(
                "RegisterWindowMessageW(TaskbarCreated) returned 0, tray will not survive an explorer restart"
            );
        } else if let Err(e) = unsafe {
            ChangeWindowMessageFilterEx(window.hwnd(), taskbar_created, MSGFLT_ALLOW, None)
        } {
            tracing::warn!(?e, "ChangeWindowMessageFilterEx(TaskbarCreated) failed");
        }

        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window.hwnd(),
            uID: TRAY_UID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_APP_TRAY,
            hIcon: icon,
            ..Default::default()
        };
        write_tooltip(&mut data.szTip, "");
        if !unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool() {
            anyhow::bail!("Shell_NotifyIconW(NIM_ADD) failed");
        }

        let state = Rc::new(TrayState {
            data: RefCell::new(data),
            taskbar_created,
            events,
            menu: RefCell::new(Vec::new()),
            panic,
        });
        unsafe { SetWindowLongPtrW(window.hwnd(), GWLP_USERDATA, Rc::as_ptr(&state) as isize) };
        Ok(Self {
            state,
            window,
            on_action,
        })
    }

    fn set_tooltip(&self, tooltip: &str) {
        let data = {
            let mut data = self.state.data.borrow_mut();
            write_tooltip(&mut data.szTip, tooltip);
            *data
        };
        if !unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) }.as_bool() {
            tracing::warn!("Shell_NotifyIconW(NIM_MODIFY) failed");
        }
    }
}

struct TrayState {
    data: RefCell<NOTIFYICONDATAW>,
    taskbar_created: u32,
    events: EventSender<NativeEvent>,
    menu: RefCell<Vec<MenuEntry>>,
    panic: Rc<CallbackPanic>,
}

impl Drop for TrayState {
    fn drop(&mut self) {
        if !unsafe { Shell_NotifyIconW(NIM_DELETE, self.data.get_mut()) }.as_bool() {
            tracing::warn!("Shell_NotifyIconW(NIM_DELETE) failed");
        }
    }
}

impl Tray for NativeTray {
    fn update_workspaces(&self, workspaces: &[WorkspaceInfo]) {
        self.set_tooltip(&focused_tooltip(workspaces));
        *self.state.menu.borrow_mut() = build_menu(workspaces, self.on_action.clone());
    }
}

impl Drop for NativeTray {
    fn drop(&mut self) {
        unsafe { SetWindowLongPtrW(self.window.hwnd(), GWLP_USERDATA, 0) };
    }
}

fn ensure_tray_class_registered() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| register_class(TRAY_CLASS_NAME, Some(tray_wnd_proc)));
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(reply) = wnd_proc_prologue(hwnd, msg, lparam) {
        return reply;
    }

    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut TrayState;
    if state_ptr.is_null() {
        return wnd_proc_no_state(hwnd, msg, wparam, lparam);
    }

    let state = unsafe { &*state_ptr };
    let panic = &state.panic;
    panic.catch(|| match msg {
        WM_DISPLAYCHANGE => {
            let _ = state.events.send(NativeEvent::DisplayChanged);
            LRESULT(0)
        }
        WM_SETTINGCHANGE if wparam.0 == SPI_SETWORKAREA.0 as usize => {
            let _ = state.events.send(NativeEvent::WorkAreaChanged);
            LRESULT(0)
        }
        WM_APP_TRAY => {
            if is_tray_context_menu(lparam) {
                let entries = state.menu.borrow().clone();
                if let Some(item) = show_context_menu(hwnd, &entries)
                    && !panic.is_pending()
                {
                    (item.action)();
                }
            }
            LRESULT(0)
        }
        _ => {
            if state.taskbar_created != 0 && msg == state.taskbar_created {
                let data = *state.data.borrow();
                if !unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool() {
                    tracing::warn!("Shell_NotifyIconW(NIM_ADD) failed re-adding tray icon");
                }
                LRESULT(0)
            } else {
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
        }
    })
}

fn write_tooltip(dst: &mut [u16], tooltip: &str) {
    let wide: Vec<u16> = tooltip.encode_utf16().collect();
    let n = wide.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&wide[..n]);
    for slot in dst.iter_mut().skip(n) {
        *slot = 0;
    }
}
