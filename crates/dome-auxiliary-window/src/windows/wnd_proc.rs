use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::UI::Shell::{NIM_ADD, Shell_NotifyIconW};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GWLP_USERDATA, GetClientRect, GetWindowLongPtrW, MA_NOACTIVATE,
    SPI_SETWORKAREA, WM_APP, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ERASEBKGND,
    WM_GETDPISCALEDSIZE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETTINGCHANGE,
    WM_SIZE,
};

use super::menu::{is_tray_context_menu, show_context_menu};
use super::window::WindowState;
use super::{BASE_DPI, WM_APP_TRAY};
use crate::{MouseButton, PhysicalPosition, PhysicalSize};

pub(super) unsafe extern "system" fn aux_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Universal messages, handled before the per-window state lookup because they can
    // arrive during creation while GWLP_USERDATA is still null.
    match msg {
        WM_ERASEBKGND => return LRESULT(1),
        WM_GETDPISCALEDSIZE => {
            let mut rect = RECT::default();
            unsafe { GetClientRect(hwnd, &mut rect).ok() };
            let size = windows::Win32::Foundation::SIZE {
                cx: rect.right - rect.left,
                cy: rect.bottom - rect.top,
            };
            let out = lparam.0 as *mut windows::Win32::Foundation::SIZE;
            unsafe { *out = wm_getdpiscaledsize_reply(size) };
            return LRESULT(1);
        }
        _ => {}
    }

    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut RefCell<WindowState>;
    if state_ptr.is_null() {
        // WM_MOUSEACTIVATE can arrive during creation before the state is stored.
        // Decline activation rather than raise the window.
        if msg == WM_MOUSEACTIVATE {
            return LRESULT(MA_NOACTIVATE as isize);
        }
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }

    // Borrowed fresh per arm. A window-mutating call inside a handler or a registered
    // callback can synchronously re-enter this wnd-proc, so a per-arm borrow turns that
    // re-entry into a loud `RefCell` panic rather than aliasing UB.
    let state = unsafe { &*state_ptr };
    match msg {
        // Decline click-activation on every window. This covers the accessibility
        // dispatch path that the WS_EX_NOACTIVATE style bit misses, and the crate never
        // raises a window on click. The `focusable` attribute governs eligibility to
        // hold focus, not this reply.
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_DPICHANGED => {
            let dpi = (wparam.0 & 0xFFFF) as u32;
            state
                .borrow_mut()
                .handler
                .on_scale_changed(dpi as f32 / BASE_DPI);
            LRESULT(0)
        }
        WM_PAINT => {
            state.borrow_mut().handler.on_redraw();
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut ps);
                EndPaint(hwnd, &ps).ok().ok();
            }
            LRESULT(0)
        }
        WM_SIZE => {
            let width = (lparam.0 & 0xFFFF) as u32;
            let height = ((lparam.0 >> 16) & 0xFFFF) as u32;
            state
                .borrow_mut()
                .handler
                .on_resized(PhysicalSize { width, height });
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            state
                .borrow_mut()
                .handler
                .on_mouse_moved(client_point(lparam));
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            state
                .borrow_mut()
                .handler
                .on_mouse_down(client_point(lparam), MouseButton::Primary);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            state
                .borrow_mut()
                .handler
                .on_mouse_up(client_point(lparam), MouseButton::Primary);
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            state
                .borrow_mut()
                .handler
                .on_mouse_down(client_point(lparam), MouseButton::Secondary);
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            state
                .borrow_mut()
                .handler
                .on_mouse_up(client_point(lparam), MouseButton::Secondary);
            LRESULT(0)
        }
        WM_MBUTTONDOWN => {
            state
                .borrow_mut()
                .handler
                .on_mouse_down(client_point(lparam), MouseButton::Middle);
            LRESULT(0)
        }
        WM_MBUTTONUP => {
            state
                .borrow_mut()
                .handler
                .on_mouse_up(client_point(lparam), MouseButton::Middle);
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            state.borrow_mut().handler.on_display_changed();
            LRESULT(0)
        }
        WM_SETTINGCHANGE if wparam.0 == SPI_SETWORKAREA.0 as usize => {
            state.borrow_mut().handler.on_work_area_changed();
            LRESULT(0)
        }
        WM_CLOSE => {
            state.borrow_mut().handler.on_close_requested();
            LRESULT(0)
        }
        // The tray icon's callback and its TaskbarCreated re-add both land at or above
        // WM_APP. Any other high id, and every system message below it, falls through to
        // the default handler.
        _ if msg >= WM_APP => {
            let mut st = state.borrow_mut();
            match st.tray.as_ref().map(|t| t.taskbar_created) {
                Some(_) if msg == WM_APP_TRAY => {
                    if is_tray_context_menu(lparam) {
                        // TrackPopupMenu pumps messages, so release the borrow before the
                        // modal show. A frame update dispatched meanwhile re-borrows this
                        // cell, and a held borrow would panic.
                        let entries = st.handler.tray_menu();
                        drop(st);
                        if let Some(id) = show_context_menu(hwnd, &entries) {
                            state.borrow_mut().handler.on_tray_menu_selected(id);
                        }
                    }
                    LRESULT(0)
                }
                Some(taskbar_created) if msg == taskbar_created => {
                    if let Some(tray) = st.tray.as_ref()
                        && !unsafe { Shell_NotifyIconW(NIM_ADD, &tray.data) }.as_bool()
                    {
                        tracing::warn!("Shell_NotifyIconW(NIM_ADD) failed re-adding tray icon");
                    }
                    LRESULT(0)
                }
                _ => {
                    drop(st);
                    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
                }
            }
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn client_point(lparam: LPARAM) -> PhysicalPosition {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    PhysicalPosition { x, y }
}

/// Reports the current window size as the desired size for a WM_GETDPISCALEDSIZE reply,
/// which makes Windows 11's automatic DPI resize a no-op.
///
/// The crate's windows are borderless WS_POPUP with no non-client area, so
/// GetClientRect == window size. A future window class with a title bar or border must
/// NOT copy this pattern without adding the non-client delta.
fn wm_getdpiscaledsize_reply(
    current: windows::Win32::Foundation::SIZE,
) -> windows::Win32::Foundation::SIZE {
    current
}

#[cfg(test)]
mod tests {
    use super::wm_getdpiscaledsize_reply;
    use windows::Win32::Foundation::SIZE;

    #[test]
    fn wm_getdpiscaledsize_reply_returns_current_size() {
        let input = SIZE { cx: 1920, cy: 1080 };
        let output = wm_getdpiscaledsize_reply(input);
        assert_eq!(output.cx, 1920);
        assert_eq!(output.cy, 1080);

        let zero = SIZE { cx: 0, cy: 0 };
        let out_zero = wm_getdpiscaledsize_reply(zero);
        assert_eq!(out_zero.cx, 0);
        assert_eq!(out_zero.cy, 0);
    }
}
