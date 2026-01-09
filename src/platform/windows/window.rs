use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GA_ROOT, GWL_EXSTYLE, GWL_STYLE, GetAncestor, GetWindowLongW,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, WS_CHILD,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PWSTR};

pub(super) use super::windows_wrapper::{hide_window, set_window_pos, show_window};

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

pub(super) fn is_manageable_window(hwnd: HWND) -> bool {
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
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

    if ex_style & WS_EX_TOOLWINDOW.0 != 0 || ex_style & WS_EX_NOACTIVATE.0 != 0 {
        return false;
    }

    let title_len = unsafe { GetWindowTextLengthW(hwnd) };
    if title_len == 0 {
        return false;
    }

    true
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
