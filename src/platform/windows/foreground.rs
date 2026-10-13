use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use super::handle;

pub(super) fn force_set_foreground(window: HWND) {
    handle::native_query();
    if unsafe { GetForegroundWindow() } != window {
        handle::submit_foreground(window.into());
    }
}
