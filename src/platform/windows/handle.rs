use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Foundation::{LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, EnumWindows, GA_ROOT, GA_ROOTOWNER, GWL_EXSTYLE, GWL_STYLE, GetAncestor,
    GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, HWND_BOTTOM,
    IsIconic, IsWindowVisible, IsZoomed, MINMAXINFO, SMTO_ABORTIFHUNG, SW_MAXIMIZE, SW_MINIMIZE,
    SW_RESTORE, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SendMessageTimeoutW,
    SetForegroundWindow, SetWindowPos, ShowWindow, ShowWindowAsync, WM_GETMINMAXINFO, WM_GETTEXT,
    WM_GETTEXTLENGTH, WS_CHILD, WS_EX_DLGMODALFRAME, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_THICKFRAME,
};
use windows::core::{BOOL, PCWSTR, PWSTR, w};

use crate::core::Dimension;
use crate::platform::windows::external::{
    HwndId, InspectExternalHwnd, ManageExternalHwnd, ShowCmd, ZOrder,
};

// Unlike macOS, we are allowed to move windows completely offscreen on Windows
pub(crate) const OFFSCREEN_POS: f32 = -32000.0;

/// Returns the window rect as a `Dimension` in PHYSICAL pixels (PMv2).
pub(crate) fn get_dimension(hwnd: HWND) -> Dimension {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect).ok() };
    Dimension {
        x: rect.left as f32,
        y: rect.top as f32,
        width: (rect.right - rect.left) as f32,
        height: (rect.bottom - rect.top) as f32,
    }
}

/// Returns the invisible border widths (left, top, right, bottom) in PHYSICAL pixels (PMv2).
pub(crate) fn get_invisible_border(hwnd: HWND) -> (i32, i32, i32, i32) {
    let mut window_rect = RECT::default();
    let mut frame_rect = RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return (0, 0, 0, 0);
        }
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut frame_rect as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_err()
        {
            return (0, 0, 0, 0);
        }
    }
    (
        frame_rect.left - window_rect.left,
        frame_rect.top - window_rect.top,
        window_rect.right - frame_rect.right,
        window_rect.bottom - frame_rect.bottom,
    )
}

const MSG_TIMEOUT_MS: u32 = 100;

pub(crate) fn is_manageable(hwnd: HWND) -> bool {
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

pub(crate) fn should_float(hwnd: HWND) -> bool {
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;

    if style & WS_POPUP.0 != 0 {
        tracing::debug!(?hwnd, "Window identified as float due to WS_POPUP style.");
        return true;
    }
    if style & WS_THICKFRAME.0 == 0 {
        tracing::debug!(?hwnd, "Window identified as float due to no WS_THICKFRAME.");
        return true;
    }
    if ex_style & WS_EX_TOPMOST.0 != 0 {
        tracing::debug!(?hwnd, "Window identified as float due to WS_EX_TOPMOST.");
        return true;
    }
    if ex_style & WS_EX_DLGMODALFRAME.0 != 0 {
        tracing::debug!(
            ?hwnd,
            "Window identified as float due to WS_EX_DLGMODALFRAME."
        );
        return true;
    }
    // WS_EX_LAYERED is not checked because apps like Steam use it for custom UI rendering.
    // WS_EX_TRANSPARENT catches actual overlay windows that should float.
    if ex_style & WS_EX_TRANSPARENT.0 != 0 {
        tracing::debug!(
            ?hwnd,
            "Window identified as float due to WS_EX_TRANSPARENT."
        );
        return true;
    }
    false
}

/// Returns physical-pixel constraints as f32. Raw WM_GETMINMAXINFO values are
/// physical on PMv2; this function subtracts invisible borders before casting.
pub(crate) fn get_size_constraints(hwnd: HWND) -> (f32, f32, f32, f32) {
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
    let border = get_invisible_border(hwnd);
    crate::platform::windows::dpi::constraints_to_physical(
        (info.ptMinTrackSize.x, info.ptMinTrackSize.y),
        (info.ptMaxTrackSize.x, info.ptMaxTrackSize.y),
        border,
    )
}

pub(crate) fn enum_windows<F>(mut callback: F) -> windows::core::Result<()>
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

pub(crate) fn get_window_title(hwnd: HWND) -> Option<String> {
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

fn get_exe_path(hwnd: HWND) -> Option<Vec<u16>> {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = [0u16; 260];
    let mut len = buf.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    }
    .ok()?;
    let mut path = buf[..len as usize].to_vec();
    path.push(0); // null-terminate for Win32 string APIs
    Some(path)
}

pub(crate) fn get_process_name(hwnd: HWND) -> anyhow::Result<String> {
    let path_wide =
        get_exe_path(hwnd).ok_or_else(|| anyhow::anyhow!("could not query process image name"))?;
    // Strip the trailing null before converting to a Rust string
    let path = String::from_utf16_lossy(&path_wide[..path_wide.len().saturating_sub(1)]);
    path.rsplit('\\')
        .next()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("no filename in path"))
}

// Returns None for UWP shells, elevated processes we can't open, apps with no
// version info, or empty FileDescription. Callers fall back to the executable name.
pub(crate) fn get_app_display_name(hwnd: HWND) -> Option<String> {
    let path = get_exe_path(hwnd)?;
    let path_ptr = PCWSTR(path.as_ptr());

    let size = unsafe { GetFileVersionInfoSizeW(path_ptr, None) };
    if size == 0 {
        return None;
    }

    let mut buf = vec![0u8; size as usize];
    unsafe { GetFileVersionInfoW(path_ptr, None, size, buf.as_mut_ptr().cast()) }.ok()?;

    let buf_ptr = buf.as_ptr().cast();
    let mut ptr = std::ptr::null_mut();
    let mut len = 0u32;

    let ok = unsafe {
        VerQueryValueW(
            buf_ptr,
            w!("\\VarFileInfo\\Translation"),
            &mut ptr,
            &mut len,
        )
    };
    if !ok.as_bool() || len == 0 || ptr.is_null() {
        return None;
    }
    let lang = unsafe { *(ptr as *const u16) };
    let codepage = unsafe { *((ptr as *const u16).add(1)) };

    // key_wide must live until after VerQueryValueW returns.
    let key_wide: Vec<u16> = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut desc_ptr = std::ptr::null_mut();
    let mut desc_len = 0u32;
    let ok = unsafe {
        VerQueryValueW(
            buf_ptr,
            PCWSTR(key_wide.as_ptr()),
            &mut desc_ptr,
            &mut desc_len,
        )
    };
    if !ok.as_bool() || desc_len == 0 || desc_ptr.is_null() {
        return None;
    }
    // desc_len includes the trailing null
    let slice_len = (desc_len as usize).saturating_sub(1);
    let desc_slice = unsafe { std::slice::from_raw_parts(desc_ptr as *const u16, slice_len) };
    let result = String::from_utf16_lossy(desc_slice).trim().to_string();
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
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

pub(crate) fn for_each_owned<F: FnMut(HWND)>(hwnd: HWND, callback: F) {
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, None) };
    if thread_id == 0 {
        return;
    }

    unsafe extern "system" fn enum_proc<F: FnMut(HWND)>(child: HWND, lparam: LPARAM) -> BOOL {
        let (owner, callback) = unsafe { &mut *(lparam.0 as *mut (HWND, F)) };
        let root_owner = unsafe { GetAncestor(child, GA_ROOTOWNER) };
        if root_owner == *owner && child != *owner {
            callback(child);
        }
        BOOL(1)
    }

    let mut data = (hwnd, callback);
    // BOOL is FALSE when the callback returns FALSE or no windows are found,
    // neither of which is an error condition.
    unsafe {
        EnumThreadWindows(
            thread_id,
            Some(enum_proc::<F>),
            LPARAM(&mut data as *mut _ as isize),
        )
        .ok()
        .ok();
    }
}

pub(crate) struct ExternalHwnd(HWND);

unsafe impl Send for ExternalHwnd {}
unsafe impl Sync for ExternalHwnd {}

impl ExternalHwnd {
    pub(crate) fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }
}

impl ManageExternalHwnd for ExternalHwnd {
    fn id(&self) -> HwndId {
        HwndId::from(self.0)
    }

    fn should_float(&self) -> bool {
        should_float(self.0)
    }

    fn is_iconic(&self) -> bool {
        unsafe { IsIconic(self.0) }.as_bool()
    }

    fn set_position(&self, z: ZOrder, x: i32, y: i32, cx: i32, cy: i32) {
        let old = get_dimension(self.0);
        let (left, top, right, bottom) = get_invisible_border(self.0);
        let insert_after: Option<HWND> = z.into();
        let mut flags = SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
        if insert_after.is_none() {
            flags |= SWP_NOZORDER;
        }
        unsafe {
            SetWindowPos(
                self.0,
                insert_after,
                x - left,
                y - top,
                cx + left + right,
                cy + top + bottom,
                flags,
            )
            .ok()
        };
        let dx = (x - left) - old.x as i32;
        let dy = (y - top) - old.y as i32;
        if dx != 0 || dy != 0 {
            for_each_owned(self.0, |child| {
                let mut rect = RECT::default();
                if unsafe { GetWindowRect(child, &mut rect).is_ok() } {
                    unsafe {
                        SetWindowPos(
                            child,
                            None,
                            rect.left + dx,
                            rect.top + dy,
                            0,
                            0,
                            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
                        )
                        .ok()
                    };
                }
            });
        }
    }

    fn move_offscreen(&self) {
        unsafe {
            SetWindowPos(
                self.0,
                Some(HWND_BOTTOM),
                OFFSCREEN_POS as i32,
                OFFSCREEN_POS as i32,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
            )
            .ok()
        };
    }

    fn show_cmd(&self, cmd: ShowCmd) {
        let sw = match cmd {
            ShowCmd::Restore => SW_RESTORE,
            ShowCmd::Minimize => SW_MINIMIZE,
        };
        unsafe { ShowWindowAsync(self.0, sw).ok().ok() };
    }

    fn set_foreground_window(&self) {
        if unsafe { GetForegroundWindow() } == self.0 {
            return;
        }
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        ..Default::default()
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    },
                },
            },
        ];
        unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if !unsafe { SetForegroundWindow(self.0) }.as_bool() {
            tracing::warn!("SetForegroundWindow failed, another app may have focus lock");
        }
    }

    fn is_maximized(&self) -> bool {
        unsafe { IsZoomed(self.0) }.as_bool()
    }

    fn recover(&self, was_maximized: bool) {
        unsafe {
            if was_maximized {
                let _ = ShowWindow(self.0, SW_RESTORE);
            }
            let _ = SetWindowPos(
                self.0,
                None,
                100,
                100,
                0,
                0,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE,
            );
            if was_maximized {
                let _ = ShowWindow(self.0, SW_MAXIMIZE);
            }
        }
    }
}

impl InspectExternalHwnd for ExternalHwnd {
    fn is_manageable(&self) -> bool {
        is_manageable(self.0)
    }

    fn get_window_title(&self) -> Option<String> {
        get_window_title(self.0)
    }

    fn get_process_name(&self) -> anyhow::Result<String> {
        get_process_name(self.0)
    }

    fn get_size_constraints(&self) -> (f32, f32, f32, f32) {
        get_size_constraints(self.0)
    }

    /// Returns (x, y, width, height) in PHYSICAL pixels (PMv2). Consumed by
    /// drift detection which also operates in physical pixels.
    fn get_visible_rect(&self) -> (i32, i32, i32, i32) {
        let mut frame_rect = RECT::default();
        if unsafe {
            DwmGetWindowAttribute(
                self.0,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut frame_rect as *mut _ as *mut _,
                std::mem::size_of::<RECT>() as u32,
            )
        }
        .is_ok()
        {
            (
                frame_rect.left,
                frame_rect.top,
                frame_rect.right - frame_rect.left,
                frame_rect.bottom - frame_rect.top,
            )
        } else {
            let dim = get_dimension(self.0);
            (
                dim.x as i32,
                dim.y as i32,
                dim.width as i32,
                dim.height as i32,
            )
        }
    }

    fn is_fullscreen(&self) -> bool {
        let dim = get_dimension(self.0);
        let monitor = unsafe { MonitorFromWindow(self.0, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            return false;
        }
        let rc = info.rcWork;
        dim.x <= rc.left as f32
            && dim.y <= rc.top as f32
            && dim.x + dim.width >= rc.right as f32
            && dim.y + dim.height >= rc.bottom as f32
    }

    fn get_app_display_name(&self) -> Option<String> {
        get_app_display_name(self.0)
    }
}
