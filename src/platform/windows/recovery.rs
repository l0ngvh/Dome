use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows::Win32::UI::WindowsAndMessaging::{
    IsZoomed, SW_MAXIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, ShowWindow,
};
use windows::core::BOOL;

use crate::core::Dimension;

use super::OFFSCREEN_POS;
use super::window::WindowHandle;

const DEFAULT_WIDTH: f32 = 800.0;
const DEFAULT_HEIGHT: f32 = 600.0;

struct WindowState {
    dimension: Dimension,
    is_maximized: bool,
}

static RECOVERY_STATE: LazyLock<Mutex<HashMap<isize, WindowState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn track(handle: &WindowHandle) {
    let dim = handle.dimension();
    let hwnd = handle.hwnd();
    // These windows belongs to previous crashed Dome instances
    let original_dim = if dim.x <= OFFSCREEN_POS || dim.y <= OFFSCREEN_POS {
        Dimension {
            x: 100.0,
            y: 100.0,
            width: if dim.width > 0.0 {
                dim.width
            } else {
                DEFAULT_WIDTH
            },
            height: if dim.height > 0.0 {
                dim.height
            } else {
                DEFAULT_HEIGHT
            },
        }
    } else {
        dim
    };
    let is_maximized = unsafe { IsZoomed(hwnd) }.as_bool();

    if let Ok(mut state) = RECOVERY_STATE.lock() {
        state.insert(
            hwnd.0 as isize,
            WindowState {
                dimension: original_dim,
                is_maximized,
            },
        );
    }
}

pub(super) fn untrack(handle: &WindowHandle) {
    if let Ok(mut state) = RECOVERY_STATE.lock() {
        state.remove(&(handle.hwnd().0 as isize));
    }
}

pub(super) fn restore_all() {
    if let Ok(state) = RECOVERY_STATE.lock() {
        for (&hwnd_val, window_state) in state.iter() {
            let hwnd = HWND(hwnd_val as *mut _);
            let dim = window_state.dimension;
            unsafe {
                // Restore the window before setting its position if it was maximized
                if window_state.is_maximized {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    dim.x as i32,
                    dim.y as i32,
                    dim.width as i32,
                    dim.height as i32,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                // Maximize the window again if it was originally maximized
                if window_state.is_maximized {
                    let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                }
            }
        }
    }
}

pub(super) fn install_handlers() {
    unsafe {
        let _ = SetConsoleCtrlHandler(Some(console_handler), true);
    }
}

unsafe extern "system" fn console_handler(ctrl_type: u32) -> BOOL {
    if ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_BREAK_EVENT || ctrl_type == CTRL_CLOSE_EVENT {
        restore_all();
    }
    BOOL(0)
}
