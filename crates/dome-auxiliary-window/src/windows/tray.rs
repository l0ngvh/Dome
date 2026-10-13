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
use super::run_loop::{NativeCallbacks, guard_callback};
use super::window::{OwnedHwnd, ex_style_for, register_class};
use super::wnd_proc::{wnd_proc_no_state, wnd_proc_prologue};
use super::{TRAY_CLASS_NAME, TRAY_UID, WM_APP_TRAY};
use crate::{Point, Size, WindowAttributes};

pub(crate) type SharedHandler = Rc<dyn NativeCallbacks>;

/// The system-tray icon and its hidden owner window.
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
    /// The owner window must remain alive until icon removal finishes.
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

        let state: *mut RefCell<TrayState> = Box::into_raw(Box::new(RefCell::new(TrayState {
            data,
            taskbar_created,
            handler,
        })));
        unsafe { SetWindowLongPtrW(window.hwnd(), GWLP_USERDATA, state as isize) };
        Ok(Self { window, state })
    }

    #[cfg(test)]
    pub(super) fn window_for_test(&self) -> HWND {
        self.window.hwnd()
    }

    pub(crate) fn tooltip(&self) -> TrayTooltip {
        TrayTooltip { state: self.state }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe { SetWindowLongPtrW(self.window.hwnd(), GWLP_USERDATA, 0) };
        // Icon removal requires a live owner window.
        drop(unsafe { Box::from_raw(self.state) });
    }
}

/// A lifetime-free handle to the tray icon's tooltip. Production handles remain valid
/// while the owning tray lives.
pub(crate) struct TrayTooltip {
    state: *mut RefCell<TrayState>,
}

impl TrayTooltip {
    #[cfg(test)]
    pub(super) fn for_test() -> Self {
        Self {
            state: std::ptr::null_mut(),
        }
    }

    pub(crate) fn set(&self, tooltip: &str) {
        assert!(!self.state.is_null());
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
    guard_callback(|| tray_dispatch(hwnd, msg, wparam, lparam))
}

fn tray_dispatch(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(reply) = wnd_proc_prologue(hwnd, msg, lparam) {
        return reply;
    }

    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut RefCell<TrayState>;
    if state_ptr.is_null() {
        return wnd_proc_no_state(hwnd, msg, wparam, lparam);
    }

    let state = unsafe { &*state_ptr };
    if !state.borrow().handler.dispatch_enabled() {
        return wnd_proc_no_state(hwnd, msg, wparam, lparam);
    }
    match msg {
        WM_DISPLAYCHANGE => {
            let handler = state.borrow().handler.clone();
            handler.display_changed();
            LRESULT(0)
        }
        WM_SETTINGCHANGE if wparam.0 == SPI_SETWORKAREA.0 as usize => {
            let handler = state.borrow().handler.clone();
            handler.work_area_changed();
            LRESULT(0)
        }
        WM_APP_TRAY => {
            if is_tray_context_menu(lparam) {
                let handler = state.borrow().handler.clone();
                dispatch_menu(handler, move |entries| show_context_menu(hwnd, entries));
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

fn dispatch_menu(
    handler: SharedHandler,
    track: impl FnOnce(&[crate::MenuEntry]) -> Option<u32> + 'static,
) {
    let weak = Rc::downgrade(&handler);
    handler.menu(Box::new(move |entries| {
        let selection = track(&entries);
        if let Some(handler) = weak.upgrade()
            && !handler.should_stop()
            && let Some(id) = selection
        {
            handler.selected(id);
        }
    }));
}

fn write_tooltip(dst: &mut [u16], tooltip: &str) {
    let wide: Vec<u16> = tooltip.encode_utf16().collect();
    let n = wide.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&wide[..n]);
    for slot in dst.iter_mut().skip(n) {
        *slot = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::super::run_loop::tests::{Event, Fixture};
    use super::*;
    use crate::delivery::Callbacks;

    #[test]
    fn dispatch_menu_releases_handler_borrow() {
        let fixture = Fixture::new();
        let state = Rc::downgrade(&fixture.state);
        let waker = fixture.waker.clone();
        dispatch_menu(fixture.state.clone(), move |entries| {
            assert!(matches!(&entries[0], crate::MenuEntry::Item(item) if item.id == 7));
            waker.wake(1);
            guard_callback(|| state.upgrade().unwrap().wake());
            Some(7)
        });
        assert_eq!(
            *fixture.events.borrow(),
            [Event::Menu, Event::Wake(1), Event::Selected(7)]
        );
    }

    #[test]
    fn select_menu_after_nested_stop_suppresses_selection() {
        for stop in [0, 1, 2] {
            let fixture = Fixture::new();
            let state = Rc::downgrade(&fixture.state);
            dispatch_menu(fixture.state.clone(), move |_| {
                match stop {
                    1 => state.upgrade().unwrap().terminate(),
                    2 => guard_callback(|| panic!("nested menu callback")),
                    _ => (),
                }
                Some(7)
            });
            if stop == 0 {
                assert_eq!(*fixture.events.borrow(), [Event::Menu, Event::Selected(7)]);
            } else {
                assert!(fixture.state.should_stop());
                assert_eq!(*fixture.events.borrow(), [Event::Menu]);
            }
        }
    }
}
