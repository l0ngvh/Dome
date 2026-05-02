use crate::platform::windows::dpi::icon_px_for_scale;
use egui::ColorImage;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits, HDC, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DI_NORMAL, DrawIconEx, GCLP_HICON, GetClassLongPtrW, HICON, ICON_BIG, SMTO_ABORTIFHUNG,
    SendMessageTimeoutW, WM_GETICON,
};

const MSG_TIMEOUT_MS: u32 = 100;

/// Loads the application icon for a window via `WM_GETICON` with a 100ms timeout,
/// falling back to `GetClassLongPtrW`. Returns an RGBA `ColorImage` sized by
/// [`icon_px_for_scale`] ([`crate::platform::windows::dpi::ICON_PX_LOGICAL`] x
/// monitor scale, clamped to a 16-physical-pixel floor).
///
/// Runs on a background thread because `SendMessageTimeoutW` can block up to
/// `MSG_TIMEOUT_MS` per window.
///
/// Does NOT call `DestroyIcon` on the returned HICON -- these are shared handles
/// owned by the window class.
pub(in crate::platform::windows) fn load_app_icon(hwnd: HWND, scale: f32) -> Option<ColorImage> {
    let icon_px = icon_px_for_scale(scale);
    let hicon = get_icon_handle(hwnd)?;

    let screen_dc = unsafe { CreateCompatibleDC(None) };
    if screen_dc.is_invalid() {
        return None;
    }
    let _dc_guard = DcGuard(screen_dc);

    let hbm = unsafe { CreateCompatibleBitmap(screen_dc, icon_px, icon_px) };
    if hbm.is_invalid() {
        return None;
    }
    let _bm_guard = BitmapGuard(hbm);

    let mem_dc = unsafe { CreateCompatibleDC(Some(screen_dc)) };
    if mem_dc.is_invalid() {
        return None;
    }
    let old_bm = unsafe { SelectObject(mem_dc, hbm.into()) };
    let _mem_dc_guard = MemDcGuard {
        dc: mem_dc,
        old: old_bm,
    };

    // Draw the icon into the memory DC at icon_px x icon_px.
    if unsafe { DrawIconEx(mem_dc, 0, 0, hicon, icon_px, icon_px, 0, None, DI_NORMAL) }.is_err() {
        return None;
    }

    // Read back pixels as 32-bit BGRA.
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: icon_px,
            biHeight: -icon_px, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default() // remaining fields unused for BI_RGB readback
        },
        ..Default::default() // no color table for 32-bit
    };
    let pixel_count = (icon_px * icon_px) as usize;
    let mut buf = vec![0u8; pixel_count * 4];
    let rows = unsafe {
        GetDIBits(
            mem_dc,
            hbm,
            0,
            icon_px as u32,
            Some(buf.as_mut_ptr().cast()),
            &mut bmi,
            DIB_RGB_COLORS,
        )
    };
    if rows == 0 {
        return None;
    }

    // Convert BGRA to RGBA.
    for pixel in buf.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    Some(ColorImage::from_rgba_unmultiplied(
        [icon_px as usize, icon_px as usize],
        &buf,
    ))
}

fn get_icon_handle(hwnd: HWND) -> Option<HICON> {
    // Try WM_GETICON(ICON_BIG) first.
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_GETICON,
            WPARAM(ICON_BIG as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            MSG_TIMEOUT_MS,
            Some(&mut result),
        )
    };
    if result != 0 {
        return Some(HICON(result as *mut _));
    }

    // Fallback: class icon.
    let class_icon = unsafe { GetClassLongPtrW(hwnd, GCLP_HICON) };
    if class_icon != 0 {
        return Some(HICON(class_icon as *mut _));
    }

    None
}

/// Cleans up a created DC on drop.
struct DcGuard(HDC);
impl Drop for DcGuard {
    fn drop(&mut self) {
        unsafe { DeleteDC(self.0).ok().ok() };
    }
}

/// Cleans up a created HBITMAP on drop.
struct BitmapGuard(windows::Win32::Graphics::Gdi::HBITMAP);
impl Drop for BitmapGuard {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.0.into()).ok().ok() };
    }
}

/// Restores the old object into a memory DC and deletes the DC on drop.
struct MemDcGuard {
    dc: HDC,
    old: windows::Win32::Graphics::Gdi::HGDIOBJ,
}
impl Drop for MemDcGuard {
    fn drop(&mut self) {
        unsafe { SelectObject(self.dc, self.old) };
        unsafe { DeleteDC(self.dc).ok().ok() };
    }
}
