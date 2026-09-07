use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Once;

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

use super::menu::{is_tray_context_menu, show_context_menu};
use super::window::{OwnedHwnd, ex_style_for, register_class};
use super::wnd_proc::{wnd_proc_no_state, wnd_proc_prologue};
use super::{TRAY_CLASS_NAME, TRAY_UID, WM_APP_TRAY};
use crate::{AppHandler, Point, Size, WindowAttributes};

/// The consumer's `AppHandler`, shared between `App`, which drives the wake path, and the
/// tray window's wnd-proc, which drives the menu and the display notifications.
pub(crate) type SharedHandler = Rc<RefCell<Box<dyn AppHandler>>>;

/// The system-tray icon and its hidden owner window. The icon's callback, its context
/// menu, and the taskbar-restart re-add all run through this one window's wnd-proc, so no
/// other window carries tray state.
pub(crate) struct Tray {
    window: OwnedHwnd,
    state: *mut RefCell<TrayState>,
}

struct TrayState {
    data: NOTIFYICONDATAW,
    /// `RegisterWindowMessageW("TaskbarCreated")`, or 0 when registration failed. The
    /// shell broadcasts it when the taskbar restarts, and the icon must be re-added then.
    taskbar_created: u32,
    handler: SharedHandler,
}

impl Drop for TrayState {
    /// Removes the icon while the owner window is still alive. `Tray`'s Drop drops this
    /// before it destroys the window, so the `(hWnd, uID)` key is still valid here.
    fn drop(&mut self) {
        if !unsafe { Shell_NotifyIconW(NIM_DELETE, &self.data) }.as_bool() {
            tracing::warn!("Shell_NotifyIconW(NIM_DELETE) failed");
        }
    }
}

impl Tray {
    pub(crate) fn new(icon: HICON, handler: SharedHandler) -> anyhow::Result<Self> {
        ensure_tray_class_registered();
        let attributes = WindowAttributes {
            position: Point::new(0, 0),
            size: Size::new(0, 0),
            click_through: false,
            focusable: false,
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

        // Boxed to a thin pointer the wnd-proc reads back from GWLP_USERDATA.
        let state: *mut RefCell<TrayState> = Box::into_raw(Box::new(RefCell::new(TrayState {
            data,
            taskbar_created,
            handler,
        })));
        unsafe { SetWindowLongPtrW(window.hwnd(), GWLP_USERDATA, state as isize) };
        Ok(Self { window, state })
    }

    pub(crate) fn tooltip(&self) -> TrayTooltip {
        TrayTooltip { state: self.state }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe { SetWindowLongPtrW(self.window.hwnd(), GWLP_USERDATA, 0) };
        // Frees TrayState (its Drop sends NIM_DELETE) while the window is still alive,
        // before the OwnedHwnd field's Drop destroys it.
        drop(unsafe { Box::from_raw(self.state) });
    }
}

/// A lifetime-free handle to the tray icon's tooltip. The raw pointer stays valid while
/// the owning `Tray` lives, which outlives every `Shell` the app lends to a callback.
pub(crate) struct TrayTooltip {
    state: *mut RefCell<TrayState>,
}

impl TrayTooltip {
    pub(crate) fn set(&self, tooltip: &str) {
        let mut state = unsafe { (*self.state).borrow_mut() };
        write_tooltip(&mut state.data.szTip, tooltip);
        if !unsafe { Shell_NotifyIconW(NIM_MODIFY, &state.data) }.as_bool() {
            tracing::warn!("Shell_NotifyIconW(NIM_MODIFY) failed");
        }
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

    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut RefCell<TrayState>;
    if state_ptr.is_null() {
        return wnd_proc_no_state(hwnd, msg, wparam, lparam);
    }

    let state = unsafe { &*state_ptr };
    match msg {
        WM_DISPLAYCHANGE => {
            state.borrow().handler.borrow_mut().on_display_changed();
            LRESULT(0)
        }
        WM_SETTINGCHANGE if wparam.0 == SPI_SETWORKAREA.0 as usize => {
            state.borrow().handler.borrow_mut().on_work_area_changed();
            LRESULT(0)
        }
        WM_APP_TRAY => {
            if is_tray_context_menu(lparam) {
                // The menu entries are pulled fresh, then every borrow ends before the
                // modal show. TrackPopupMenu pumps messages, and a dispatched message that
                // re-enters this wnd-proc would re-borrow these cells and panic.
                let entries = state.borrow().handler.borrow_mut().menu();
                if let Some(id) = show_context_menu(hwnd, &entries) {
                    state.borrow().handler.borrow_mut().on_menu_selected(id);
                }
            }
            LRESULT(0)
        }
        _ => {
            let st = state.borrow();
            if st.taskbar_created != 0 && msg == st.taskbar_created {
                if !unsafe { Shell_NotifyIconW(NIM_ADD, &st.data) }.as_bool() {
                    tracing::warn!("Shell_NotifyIconW(NIM_ADD) failed re-adding tray icon");
                }
                LRESULT(0)
            } else {
                drop(st);
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
        }
    }
}

fn write_tooltip(dst: &mut [u16], tooltip: &str) {
    let wide: Vec<u16> = tooltip.encode_utf16().collect();
    let n = wide.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&wide[..n]);
    for slot in dst.iter_mut().skip(n) {
        *slot = 0;
    }
}
