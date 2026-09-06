use anyhow::Result;
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    HICON, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, LoadImageW,
};
use windows::core::PCWSTR;

const TRAY_ICON_RESOURCE_ID: u16 = 1;

/// `LR_DEFAULTSIZE` picks the system-tray size for the current DPI. `LR_SHARED` lets
/// Windows cache the handle, so no `DestroyIcon` is owed, which suits an app-lifetime
/// resource.
pub(super) fn load_tray_icon() -> Result<HICON> {
    let hmodule = unsafe { GetModuleHandleW(None) }?;
    let instance = HINSTANCE(hmodule.0);
    let icon_handle = unsafe {
        LoadImageW(
            Some(instance),
            PCWSTR(TRAY_ICON_RESOURCE_ID as usize as *const u16),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        )
    }?;
    Ok(HICON(icon_handle.0))
}
