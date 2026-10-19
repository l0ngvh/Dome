use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GWLP_USERDATA, GetClientRect, GetWindowLongPtrW, MA_NOACTIVATE, WM_CLOSE,
    WM_DPICHANGED, WM_ERASEBKGND, WM_GETDPISCALEDSIZE, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN,
    WM_RBUTTONUP,
};

use super::window::native::WindowState;
use crate::platform::windows::ui::window::MouseButton;
use crate::platform::windows::ui::window::Point;
use dome_core::Physical;

/// Universal messages handled before the per-window state lookup, because they can arrive
/// during creation while `GWLP_USERDATA` is still null. Returns `Some` when handled.
/// Visible window classes call this first. The message-only wake window skips it.
pub(in crate::platform::windows) fn wnd_proc_prologue(
    hwnd: HWND,
    msg: u32,
    lparam: LPARAM,
) -> Option<LRESULT> {
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
pub(in crate::platform::windows) fn wnd_proc_no_state(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        LRESULT(MA_NOACTIVATE as isize)
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

pub(in crate::platform::windows) unsafe extern "system" fn aux_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(reply) = wnd_proc_prologue(hwnd, msg, lparam) {
        return reply;
    }

    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
    if state_ptr.is_null() {
        return wnd_proc_no_state(hwnd, msg, wparam, lparam);
    }

    let state = unsafe { &*state_ptr };
    dispatch_input(state, hwnd, msg, wparam, lparam)
}

fn dispatch_input(
    state: &WindowState,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    state.panic.catch(|| match msg {
        WM_DPICHANGED => {
            if let Some(events) = &state.dpi_changes {
                events
                    .send(crate::platform::windows::event_loop::NativeEvent::WorkAreaChanged)
                    .ok();
            }
            LRESULT(0)
        }
        WM_PAINT => {
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut ps);
                EndPaint(hwnd, &ps).ok().ok();
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Some(input) = state.input.borrow_mut().as_mut() {
                input.mouse_moved(client_point(lparam));
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            if let Some(input) = state.input.borrow_mut().as_mut() {
                input.mouse_down(client_point(lparam), MouseButton::Primary);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if let Some(input) = state.input.borrow_mut().as_mut() {
                input.mouse_up(client_point(lparam), MouseButton::Primary);
            }
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            if let Some(input) = state.input.borrow_mut().as_mut() {
                input.mouse_down(client_point(lparam), MouseButton::Secondary);
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            if let Some(input) = state.input.borrow_mut().as_mut() {
                input.mouse_up(client_point(lparam), MouseButton::Secondary);
            }
            LRESULT(0)
        }
        WM_MBUTTONDOWN => {
            if let Some(input) = state.input.borrow_mut().as_mut() {
                input.mouse_down(client_point(lparam), MouseButton::Middle);
            }
            LRESULT(0)
        }
        WM_MBUTTONUP => {
            if let Some(input) = state.input.borrow_mut().as_mut() {
                input.mouse_up(client_point(lparam), MouseButton::Middle);
            }
            LRESULT(0)
        }
        WM_CLOSE => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    })
}

fn client_point(lparam: LPARAM) -> Point<Physical> {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    Point::new(x, y)
}
