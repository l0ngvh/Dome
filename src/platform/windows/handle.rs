use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Foundation::{LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromWindow};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, EnumWindows, GA_ROOT, GA_ROOTOWNER, GWL_EXSTYLE, GWL_STYLE, GetAncestor,
    GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, IsZoomed, MINMAXINFO, SMTO_ABORTIFHUNG, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE,
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SendMessageTimeoutW,
    SetForegroundWindow, SetWindowPos, ShowWindow, ShowWindowAsync, WM_GETMINMAXINFO, WM_GETTEXT,
    WM_GETTEXTLENGTH, WS_CHILD, WS_EX_DLGMODALFRAME, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_THICKFRAME,
};
use windows::core::{BOOL, PWSTR};

use crate::core::Dimension;
use crate::platform::windows::OFFSCREEN_POS;
use crate::platform::windows::external::{HwndId, ManageExternalHwnd, ShowCmd, ZOrder};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowMode {
    Tiling,
    Float,
    FullscreenBorderless,
    ManagedFullscreen,
    FullscreenExclusive,
}

impl WindowMode {
    pub(crate) fn is_fullscreen(self) -> bool {
        matches!(
            self,
            Self::FullscreenBorderless | Self::ManagedFullscreen | Self::FullscreenExclusive
        )
    }
}

impl std::fmt::Display for WindowMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tiling => write!(f, "tiling"),
            Self::Float => write!(f, "float"),
            Self::FullscreenBorderless => write!(f, "fullscreen-borderless"),
            Self::ManagedFullscreen => write!(f, "managed-fullscreen"),
            Self::FullscreenExclusive => write!(f, "fullscreen-exclusive"),
        }
    }
}

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

pub(crate) fn is_fullscreen(dim: &Dimension, monitor: &Dimension) -> bool {
    dim.x <= monitor.x
        && dim.y <= monitor.y
        && dim.x + dim.width >= monitor.x + monitor.width
        && dim.y + dim.height >= monitor.y + monitor.height
}

pub(crate) fn initial_window_mode(hwnd: HWND, monitor: Option<&Dimension>) -> WindowMode {
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
    let (left, top, right, bottom) = get_invisible_border(hwnd);
    (
        (info.ptMinTrackSize.x - left - right).max(0) as f32,
        (info.ptMinTrackSize.y - top - bottom).max(0) as f32,
        (info.ptMaxTrackSize.x - left - right).max(0) as f32,
        (info.ptMaxTrackSize.y - top - bottom).max(0) as f32,
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

pub(crate) fn get_process_name(hwnd: HWND) -> anyhow::Result<String> {
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

    fn is_manageable(&self) -> bool {
        is_manageable(self.0)
    }

    fn get_window_title(&self) -> Option<String> {
        get_window_title(self.0)
    }

    fn get_process_name(&self) -> anyhow::Result<String> {
        get_process_name(self.0)
    }

    fn initial_window_mode(&self, monitor: Option<&Dimension>) -> WindowMode {
        initial_window_mode(self.0, monitor)
    }

    fn get_dimension(&self) -> Dimension {
        get_dimension(self.0)
    }

    fn get_size_constraints(&self) -> (f32, f32, f32, f32) {
        get_size_constraints(self.0)
    }

    fn get_monitor_handle(&self) -> Option<isize> {
        let hmonitor = unsafe { MonitorFromWindow(self.0, MONITOR_DEFAULTTONULL) };
        if hmonitor.is_invalid() {
            None
        } else {
            Some(hmonitor.0 as isize)
        }
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
                None,
                OFFSCREEN_POS as i32,
                OFFSCREEN_POS as i32,
                0,
                0,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
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

    fn recover(&self, dim: Dimension, was_maximized: bool) {
        unsafe {
            if was_maximized {
                let _ = ShowWindow(self.0, SW_RESTORE);
            }
            let _ = SetWindowPos(
                self.0,
                None,
                dim.x as i32,
                dim.y as i32,
                dim.width as i32,
                dim.height as i32,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            if was_maximized {
                let _ = ShowWindow(self.0, SW_MAXIMIZE);
            }
        }
    }
}
