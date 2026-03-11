use crate::core::Dimension;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_MENU,
};
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, EnumWindows, GA_ROOT, GA_ROOTOWNER, GWL_EXSTYLE, GWL_STYLE, GetAncestor,
    GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, MINMAXINFO, SMTO_ABORTIFHUNG, SW_MINIMIZE, SW_RESTORE, SWP_ASYNCWINDOWPOS,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageTimeoutW, SetForegroundWindow,
    SetWindowPos, ShowWindowAsync, WM_GETMINMAXINFO, WM_GETTEXT, WM_GETTEXTLENGTH, WS_CHILD,
    WS_EX_DLGMODALFRAME, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP, WS_THICKFRAME,
};
use windows::core::{BOOL, PWSTR};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ManagedHwnd(isize);

impl ManagedHwnd {
    pub(crate) fn new(hwnd: HWND) -> Self {
        Self(hwnd.0 as isize)
    }

    pub(crate) fn hwnd(self) -> HWND {
        HWND(self.0 as *mut _)
    }
}

unsafe impl Send for ManagedHwnd {}

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

// HWND is safe to send across threads, but doesn't implement Send
// https://users.rust-lang.org/t/moving-window-hwnd-or-handle-from-one-thread-to-a-new-one/126341/2
#[derive(Clone)]
pub(super) struct WindowHandle {
    hwnd: HWND,
    title: Option<String>,
    process: String,
    mode: WindowMode,
}

unsafe impl Send for WindowHandle {}

impl WindowHandle {
    /// Construct from pre-queried data — no Win32 calls. Used by the UI thread
    /// when it receives a WindowCreate from the dome thread.
    pub(super) fn new_from_create(
        hwnd: HWND,
        title: Option<String>,
        process: String,
        mode: WindowMode,
    ) -> Self {
        Self {
            hwnd,
            title,
            process,
            mode,
        }
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(super) fn set_title(&mut self, title: Option<String>) {
        self.title = title;
    }

    pub(super) fn mode(&self) -> WindowMode {
        self.mode
    }

    pub(super) fn set_mode(&mut self, mode: WindowMode) {
        self.mode = mode;
    }

    pub(super) fn set_fullscreen(&mut self, dim: &Dimension) {
        match self.mode {
            WindowMode::FullscreenBorderless | WindowMode::FullscreenExclusive => return,
            _ => {}
        }
        self.mode = WindowMode::ManagedFullscreen;
        set_position(self.hwnd, dim, None);
    }

    pub(super) fn show(
        &mut self,
        dim: &Dimension,
        border: f32,
        is_float: bool,
        z_after: Option<HWND>,
    ) {
        if self.mode == WindowMode::FullscreenExclusive {
            return;
        }
        let content = apply_inset(*dim, border);
        set_position(self.hwnd, &content, z_after);
        if is_float && self.mode != WindowMode::Float {
            set_children_topmost(self.hwnd);
        }
        self.mode = if is_float {
            WindowMode::Float
        } else {
            WindowMode::Tiling
        };
    }

    pub(super) fn hide(&self) {
        match self.mode {
            WindowMode::FullscreenExclusive => {}
            WindowMode::FullscreenBorderless => {
                unsafe { ShowWindowAsync(self.hwnd, SW_MINIMIZE) };
            }
            _ => {
                unsafe {
                    SetWindowPos(
                        self.hwnd,
                        None,
                        super::super::OFFSCREEN_POS as i32,
                        super::super::OFFSCREEN_POS as i32,
                        0,
                        0,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
                    )
                    .ok()
                };
            }
        }
    }

    pub(super) fn focus(&self) {
        if self.mode == WindowMode::FullscreenExclusive {
            return;
        }
        if unsafe { GetForegroundWindow() } == self.hwnd {
            return;
        }
        // Simulate ALT key press to bypass SetForegroundWindow restrictions
        // https://gist.github.com/Aetopia/1581b40f00cc0cadc93a0e8ccb65dc8c
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
        if !unsafe { SetForegroundWindow(self.hwnd) }.as_bool() {
            tracing::warn!("SetForegroundWindow failed, another app may have focus lock");
        }
    }
}

impl std::fmt::Display for WindowHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let title = self.title().unwrap_or("<no title>");
        write!(
            f,
            "'{title}' from '{}' hwnd={:?} mode={}",
            self.process, self.hwnd, self.mode
        )
    }
}

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

pub(crate) fn is_d3d_exclusive_fullscreen_active() -> bool {
    unsafe { SHQueryUserNotificationState() }
        .is_ok_and(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN)
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

const MSG_TIMEOUT_MS: u32 = 100;

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

fn get_invisible_border(hwnd: HWND) -> (i32, i32, i32, i32) {
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

fn apply_inset(dim: Dimension, border: f32) -> Dimension {
    Dimension {
        x: dim.x + border,
        y: dim.y + border,
        width: (dim.width - 2.0 * border).max(0.0),
        height: (dim.height - 2.0 * border).max(0.0),
    }
}

fn for_each_owned<F: FnMut(HWND)>(hwnd: HWND, callback: F) {
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

fn set_position(hwnd: HWND, dim: &Dimension, z_after: Option<HWND>) {
    if unsafe { IsIconic(hwnd) }.as_bool() {
        let _was_visible = unsafe { ShowWindowAsync(hwnd, SW_RESTORE) };
    }
    let old = get_dimension(hwnd);
    let (left, top, right, bottom) = get_invisible_border(hwnd);
    let mut flags = SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
    if z_after.is_none() {
        flags |= SWP_NOZORDER;
    }
    unsafe {
        SetWindowPos(
            hwnd,
            z_after,
            dim.x as i32 - left,
            dim.y as i32 - top,
            dim.width as i32 + left + right,
            dim.height as i32 + top + bottom,
            flags,
        )
        .ok()
    };
    let dx = (dim.x as i32 - left) - old.x as i32;
    let dy = (dim.y as i32 - top) - old.y as i32;
    if dx != 0 || dy != 0 {
        for_each_owned(hwnd, |child| {
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

fn set_children_topmost(hwnd: HWND) {
    for_each_owned(hwnd, |child| {
        unsafe {
            SetWindowPos(
                child,
                Some(windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            )
            .ok()
        };
    });
}
