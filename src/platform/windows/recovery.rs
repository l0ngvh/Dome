use std::cell::RefCell;
use std::collections::HashMap;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows::Win32::UI::WindowsAndMessaging::{SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos};
use windows::core::BOOL;

use crate::core::Dimension;

use super::OFFSCREEN_POS;
use super::window::get_window_dimension;

const DEFAULT_WIDTH: f32 = 800.0;
const DEFAULT_HEIGHT: f32 = 600.0;

thread_local! {
    static RECOVERY_STATE: RefCell<HashMap<isize, Dimension>> = RefCell::new(HashMap::new());
}

pub(super) fn track(hwnd: HWND) {
    let dim = get_window_dimension(hwnd);
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
    RECOVERY_STATE.with(|state| {
        state.borrow_mut().insert(hwnd.0 as isize, original_dim);
    });
}

pub(super) fn untrack(hwnd: HWND) {
    RECOVERY_STATE.with(|state| {
        state.borrow_mut().remove(&(hwnd.0 as isize));
    });
}

pub(super) fn restore_all() {
    RECOVERY_STATE.with(|state| {
        for (&hwnd_val, dim) in state.borrow().iter() {
            let hwnd = HWND(hwnd_val as *mut _);
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    dim.x as i32,
                    dim.y as i32,
                    dim.width as i32,
                    dim.height as i32,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
    });
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
