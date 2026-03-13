use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GA_ROOT, GWL_EXSTYLE, GWL_STYLE, GetAncestor, GetWindowLongW,
    GetWindowThreadProcessId, IsWindowVisible, MINMAXINFO, SMTO_ABORTIFHUNG, SendMessageTimeoutW,
    WM_GETMINMAXINFO, WM_GETTEXT, WM_GETTEXTLENGTH, WS_CHILD, WS_EX_DLGMODALFRAME,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_THICKFRAME,
};
use windows::core::{BOOL, PWSTR};

use super::super::handle::{WindowMode, get_dimension, get_invisible_border};
use crate::core::Dimension;

pub(super) fn is_manageable(hwnd: HWND) -> bool {
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return false;
    }
    if is_cloaked(hwnd) {
        return false;
    }
    if unsafe { GetAncestor(hwnd, GA_ROOT) } != hwnd {
        return false;
    }
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    if style & WS_CHILD.0 != 0 {
        return false;
    }
    if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
        return false;
    }
    if ex_style & WS_EX_NOACTIVATE.0 != 0 {
        return false;
    }
    let dim = get_dimension(hwnd);
    if dim.width == 0.0 || dim.height == 0.0 {
        return false;
    }
    true
}

pub(super) fn is_fullscreen(dim: &Dimension, monitor: &Dimension) -> bool {
    dim.x <= monitor.x
        && dim.y <= monitor.y
        && dim.x + dim.width >= monitor.x + monitor.width
        && dim.y + dim.height >= monitor.y + monitor.height
}

pub(super) fn initial_window_mode(hwnd: HWND, monitor: Option<&Dimension>) -> WindowMode {
    if monitor.is_some_and(|m| is_fullscreen(&get_dimension(hwnd), m)) {
        return WindowMode::FullscreenBorderless;
    }
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;

    if style & WS_POPUP.0 != 0 {
        tracing::debug!(?hwnd, "Window identified as float due to WS_POPUP style.");
        return WindowMode::Float;
    }
    if style & WS_THICKFRAME.0 == 0 {
        tracing::debug!(?hwnd, "Window identified as float due to no WS_THICKFRAME.");
        return WindowMode::Float;
    }
    if ex_style & WS_EX_TOPMOST.0 != 0 {
        tracing::debug!(?hwnd, "Window identified as float due to WS_EX_TOPMOST.");
        return WindowMode::Float;
    }
    if ex_style & WS_EX_DLGMODALFRAME.0 != 0 {
        tracing::debug!(
            ?hwnd,
            "Window identified as float due to WS_EX_DLGMODALFRAME."
        );
        return WindowMode::Float;
    }
    // WS_EX_LAYERED is not checked because apps like Steam use it for custom UI rendering.
    // WS_EX_TRANSPARENT catches actual overlay windows that should float.
    if ex_style & WS_EX_TRANSPARENT.0 != 0 {
        tracing::debug!(
            ?hwnd,
            "Window identified as float due to WS_EX_TRANSPARENT."
        );
        return WindowMode::Float;
    }
    tracing::debug!(?hwnd, "Window determined to be Tiling.");
    WindowMode::Tiling
}

/// Returns (min_width, min_height, max_width, max_height) constraints
/// with invisible borders subtracted.
///
/// This can be slow, due to the fact that external windows may take time to respond or even
/// hang
pub(super) fn get_size_constraints(hwnd: HWND) -> (f32, f32, f32, f32) {
    let mut info = MINMAXINFO::default();
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_GETMINMAXINFO,
            WPARAM(0),
            LPARAM(&mut info as *mut _ as isize),
            SMTO_ABORTIFHUNG,
            MSG_TIMEOUT_MS,
            Some(&mut result),
        )
    };
    let (left, top, right, bottom) = get_invisible_border(hwnd);
    (
        (info.ptMinTrackSize.x - left - right).max(0) as f32,
        (info.ptMinTrackSize.y - top - bottom).max(0) as f32,
        (info.ptMaxTrackSize.x - left - right).max(0) as f32,
        (info.ptMaxTrackSize.y - top - bottom).max(0) as f32,
    )
}

pub(super) fn enum_windows<F>(mut callback: F) -> windows::core::Result<()>
where
    F: FnMut(HWND),
{
    unsafe extern "system" fn enum_proc<F: FnMut(HWND)>(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let callback = unsafe { &mut *(lparam.0 as *mut F) };
        callback(hwnd);
        BOOL(1)
    }

    unsafe {
        EnumWindows(
            Some(enum_proc::<F>),
            LPARAM(&mut callback as *mut _ as isize),
        )
    }
}

const MSG_TIMEOUT_MS: u32 = 100;

pub(super) fn get_window_title(hwnd: HWND) -> Option<String> {
    let mut len = 0usize;
    let ret = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_GETTEXTLENGTH,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            MSG_TIMEOUT_MS,
            Some(&mut len),
        )
    };
    if ret == LRESULT(0) || len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len + 1];
    let mut copied = 0usize;
    let ret = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_GETTEXT,
            WPARAM(buf.len()),
            LPARAM(buf.as_mut_ptr() as isize),
            SMTO_ABORTIFHUNG,
            MSG_TIMEOUT_MS,
            Some(&mut copied),
        )
    };
    if ret == LRESULT(0) || copied == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..copied]))
}

pub(super) fn get_process_name(hwnd: HWND) -> anyhow::Result<String> {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    anyhow::ensure!(pid != 0, "GetWindowThreadProcessId failed");

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)? };
    let mut buf = [0u16; 260];
    let mut len = buf.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )?
    };

    let path = String::from_utf16_lossy(&buf[..len as usize]);
    path.rsplit('\\')
        .next()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("no filename in path"))
}

fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            std::ptr::from_mut(&mut cloaked).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    result.is_ok() && cloaked != 0
}
