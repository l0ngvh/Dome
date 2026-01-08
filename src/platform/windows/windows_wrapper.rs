use anyhow::anyhow;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOP, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOZORDER, SetForegroundWindow,
    SetWindowPos, ShowWindow,
};

use crate::core::Dimension;

pub(super) fn set_window_pos(hwnd: HWND, dim: &Dimension) -> windows::core::Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            dim.x as i32,
            dim.y as i32,
            dim.width as i32,
            dim.height as i32,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
    }
}

pub(super) fn show_window(hwnd: HWND) {
    // Return value is previous visibility state, not success/failure
    let _ = unsafe { ShowWindow(hwnd, SW_SHOWNA) };
}

pub(super) fn hide_window(hwnd: HWND) {
    let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
}

pub(super) fn set_foreground_window(hwnd: HWND) -> anyhow::Result<()> {
    if unsafe { SetForegroundWindow(hwnd) }.as_bool() {
        Ok(())
    } else {
        Err(anyhow!("SetForegroundWindow failed, another app may have focus lock"))
    }
}
