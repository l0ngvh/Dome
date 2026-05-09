use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
use windows::core::PWSTR;

/// Resolves an HWND to the full image path of its owning process.
///
/// Returns a nul-terminated `Vec<u16>` suitable for passing directly to Win32
/// string APIs via `PCWSTR(path.as_ptr())`. Returns `None` when the PID cannot
/// be resolved (zombie window) or the process handle cannot be opened (elevated
/// target, system process).
pub(in crate::platform::windows) fn get_exe_path(hwnd: HWND) -> Option<Vec<u16>> {
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
    path.push(0); // nul-terminate for Win32 string APIs (PCWSTR consumers)
    Some(path)
}
