use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GWLP_USERDATA, GetClientRect, GetWindowLongPtrW, MA_NOACTIVATE,
    SPI_SETWORKAREA, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ERASEBKGND, WM_GETDPISCALEDSIZE,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE,
    WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_SIZE,
};

use super::BASE_DPI;
use super::window::WindowState;
use crate::{MouseButton, NativeUnit, Point, Size};

/// Universal messages handled before the per-window state lookup, because they can arrive
/// during creation while `GWLP_USERDATA` is still null. Returns `Some` when handled.
/// Every window class's wnd-proc calls this first.
pub(super) fn wnd_proc_prologue(hwnd: HWND, msg: u32, lparam: LPARAM) -> Option<LRESULT> {
    match msg {
        WM_ERASEBKGND => Some(LRESULT(1)),
        WM_GETDPISCALEDSIZE => {
            // The crate's windows are borderless WS_POPUP with no non-client area, so
            // GetClientRect == window size. Reporting the current size as the desired size
            // makes Windows 11's automatic DPI resize a no-op. A future window class with a
            // title bar or border must NOT copy this without adding the non-client delta.
            let mut rect = RECT::default();
            unsafe { GetClientRect(hwnd, &mut rect).ok() };
            let size = windows::Win32::Foundation::SIZE {
                cx: rect.right - rect.left,
                cy: rect.bottom - rect.top,
            };
            let out = lparam.0 as *mut windows::Win32::Foundation::SIZE;
            unsafe { *out = size };
            Some(LRESULT(1))
        }
        _ => None,
    }
}

/// The reply for a message that arrives before the window's state is stored.
/// `WM_MOUSEACTIVATE` can arrive during creation, and the crate never raises a window on
/// click, so decline activation. Everything else defers to the default handler.
pub(super) fn wnd_proc_no_state(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        LRESULT(MA_NOACTIVATE as isize)
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

pub(super) unsafe extern "system" fn aux_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(reply) = wnd_proc_prologue(hwnd, msg, lparam) {
        return reply;
    }

    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut RefCell<WindowState>;
    if state_ptr.is_null() {
        return wnd_proc_no_state(hwnd, msg, wparam, lparam);
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
                .on_resized(Size::new(width, height));
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
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn client_point(lparam: LPARAM) -> Point<NativeUnit> {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    Point::new(x, y)
}
