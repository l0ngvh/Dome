use crate::core::Dimension;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_MENU,
};
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GA_ROOT, GWL_EXSTYLE, GWL_STYLE, GetAncestor, GetForegroundWindow, GetWindowLongW,
    GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    MINMAXINFO, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW,
    SetForegroundWindow, SetWindowPos, WM_GETMINMAXINFO, WS_CHILD, WS_EX_DLGMODALFRAME,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    WS_THICKFRAME,
};
use windows::core::{BOOL, PWSTR};

// HWND is safe to send across threads, but doesn't implement Send
// https://users.rust-lang.org/t/moving-window-hwnd-or-handle-from-one-thread-to-a-new-one/126341/2
#[derive(Clone)]
pub(super) struct WindowHandle {
    hwnd: HWND,
    title: Option<String>,
    process: String,
}

unsafe impl Send for WindowHandle {}

impl WindowHandle {
    pub(super) fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            title: get_window_title(hwnd),
            process: get_process_name(hwnd).unwrap_or_default(),
        }
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(super) fn process(&self) -> &str {
        &self.process
    }

    pub(super) fn is_manageable(&self) -> bool {
        if !unsafe { IsWindowVisible(self.hwnd) }.as_bool() {
            tracing::trace!(hwnd = ?self.hwnd, title = ?self.title, "not manageable: window is not visible");
            return false;
        }

        if is_cloaked(self.hwnd) {
            tracing::trace!(hwnd = ?self.hwnd, title = ?self.title, "not manageable: window is cloaked");
            return false;
        }

        if unsafe { GetAncestor(self.hwnd, GA_ROOT) } != self.hwnd {
            tracing::trace!(hwnd = ?self.hwnd, title = ?self.title, "not manageable: window is not root");
            return false;
        }

        let style = unsafe { GetWindowLongW(self.hwnd, GWL_STYLE) } as u32;
        let ex_style = unsafe { GetWindowLongW(self.hwnd, GWL_EXSTYLE) } as u32;

        if style & WS_CHILD.0 != 0 {
            tracing::trace!(hwnd = ?self.hwnd, title = ?self.title, "not manageable: window is a child window");
            return false;
        }

        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            tracing::trace!(hwnd = ?self.hwnd, title = ?self.title, "not manageable: window is a tool window");
            return false;
        }

        if ex_style & WS_EX_NOACTIVATE.0 != 0 {
            tracing::trace!(hwnd = ?self.hwnd, title = ?self.title, "not manageable: window has WS_EX_NOACTIVATE");
            return false;
        }

        true
    }

    pub(super) fn should_tile(&self) -> bool {
        let style = unsafe { GetWindowLongW(self.hwnd, GWL_STYLE) } as u32;
        let ex_style = unsafe { GetWindowLongW(self.hwnd, GWL_EXSTYLE) } as u32;

        if style & WS_POPUP.0 != 0 {
            return false;
        }
        if style & WS_THICKFRAME.0 == 0 {
            return false;
        }
        if ex_style & WS_EX_TOPMOST.0 != 0 {
            return false;
        }
        if ex_style & WS_EX_DLGMODALFRAME.0 != 0 {
            return false;
        }
        if ex_style & WS_EX_LAYERED.0 != 0 {
            return false;
        }
        if ex_style & WS_EX_TRANSPARENT.0 != 0 {
            return false;
        }
        true
    }

    pub(super) fn dimension(&self) -> Dimension {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut rect).ok() };
        Dimension {
            x: rect.left as f32,
            y: rect.top as f32,
            width: (rect.right - rect.left) as f32,
            height: (rect.bottom - rect.top) as f32,
        }
    }

    pub(super) fn set_position(&self, dim: &Dimension) {
        let (left, top, right, bottom) = get_invisible_border(self.hwnd);
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                dim.x as i32 - left,
                dim.y as i32 - top,
                dim.width as i32 + left + right,
                dim.height as i32 + top + bottom,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .ok()
        };
    }

    pub(super) fn hide(&self) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                super::OFFSCREEN_POS as i32,
                super::OFFSCREEN_POS as i32,
                0,
                0,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE,
            )
            .ok()
        };
    }

    pub(super) fn set_topmost(&self) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .ok()
        };
    }

    pub(super) fn focus(&self) {
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
        write!(f, "'{title}' from '{}'", self.process)
    }
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

pub(super) fn get_size_constraints(hwnd: HWND) -> (f32, f32, f32, f32) {
    let mut info = MINMAXINFO::default();
    unsafe {
        SendMessageW(
            hwnd,
            WM_GETMINMAXINFO,
            Some(WPARAM(0)),
            Some(LPARAM(&mut info as *mut _ as isize)),
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

pub(super) fn get_window_title(hwnd: HWND) -> Option<String> {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if copied == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..copied as usize]))
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

pub(super) struct Taskbar(ITaskbarList);

impl Taskbar {
    pub(super) fn new() -> windows::core::Result<Self> {
        unsafe {
            let list: ITaskbarList = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;
            list.HrInit()?;
            Ok(Self(list))
        }
    }

    pub(super) fn add_tab(&self, hwnd: HWND) -> windows::core::Result<()> {
        unsafe { self.0.AddTab(hwnd) }
    }

    pub(super) fn delete_tab(&self, hwnd: HWND) -> windows::core::Result<()> {
        unsafe { self.0.DeleteTab(hwnd) }
    }
}
