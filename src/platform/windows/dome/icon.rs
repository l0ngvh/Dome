//! Windows icon capture for the minimized-window picker.
//!
//! `load_app_icon(hwnd, scale)` resolves an HICON through a 6-stage fallback
//! chain and captures it into an RGBA `ColorImage`:
//!
//! 1. `WM_GETICON(ICON_BIG)` - highest-quality per-window icon
//! 2. `WM_GETICON(ICON_SMALL2)` - system-managed upscaled variant
//! 3. `WM_GETICON(ICON_SMALL)` - per-window small icon
//! 4. `GetClassLongPtrW(GCLP_HICON)` - shared class icon
//! 5. `GetClassLongPtrW(GCLP_HICONSM)` - shared class small icon
//! 6. `SHGetFileInfoW` - shell exe icon (fallback for apps with no window icon)
//!
//! Steps 1-5 return shared handles (owned by the window/class, not destroyed).
//! Step 6 returns an owned handle that must be destroyed via `DestroyIcon`.
//! `IconHandle` encapsulates this ownership distinction.
//!
//! Capture uses a 32-bit top-down BGRA `CreateDIBSection` with direct pixel
//! access (no `GetDIBits` round-trip), giving well-defined alpha regardless of
//! the memory DC's initial bitmap format.
//!
//! All `SendMessageTimeoutW` calls use `SMTO_ABORTIFHUNG` with `MSG_TIMEOUT_MS`
//! (100 ms) to avoid hanging on unresponsive windows. Runs on the rayon thread
//! pool via `dispatch_picker_icons` in `runner.rs`.

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;

use egui::ColorImage;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GdiFlush, HDC, SelectObject,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::UI::Shell::{
    SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_SMALLICON, SHGetFileInfoW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DI_NORMAL, DestroyIcon, DrawIconEx, GCLP_HICON, GCLP_HICONSM, GET_CLASS_LONG_INDEX,
    GetClassLongPtrW, HICON, ICON_BIG, ICON_SMALL, ICON_SMALL2, SMTO_ABORTIFHUNG,
    SendMessageTimeoutW, WM_GETICON,
};
use windows::core::PCWSTR;

const MSG_TIMEOUT_MS: u32 = 100;

/// Logical icon edge length matching `ICON_SIZE` in `src/picker.rs`.
/// Used by `icon_px_for_scale` to derive the physical capture size.
const ICON_PX_LOGICAL: i32 = 24;

/// Physical icon capture size for a given monitor scale.
/// Scales `ICON_PX_LOGICAL` by the monitor scale factor, with a 16-physical-pixel
/// floor because `DrawIconEx` rasterising a shared HICON below ~16px loses
/// recognisable shape.
pub(in crate::platform::windows) fn icon_px_for_scale(scale: f32) -> i32 {
    ((ICON_PX_LOGICAL as f32 * scale).round() as i32).max(16)
}

/// Loads the application icon for a window, returning an RGBA `ColorImage`
/// sized by [`icon_px_for_scale`].
///
/// Runs on a background thread because `SendMessageTimeoutW` can block up to
/// `MSG_TIMEOUT_MS` per probe (worst case: 3 probes x 100ms = 300ms).
pub(in crate::platform::windows) fn load_app_icon(hwnd: HWND, scale: f32) -> Option<ColorImage> {
    let icon_px = icon_px_for_scale(scale);
    let handle = resolve_icon_handle(hwnd, icon_px)?;
    capture_hicon(handle.hicon, icon_px)
    // handle drops here; DestroyIcon runs only when handle.owned
}

/// RAII guard for an HICON that tracks whether we own it.
/// Shared handles (from WM_GETICON, GetClassLongPtrW) must NOT be destroyed.
/// Owned handles (from SHGetFileInfoW) must be destroyed by the caller.
struct IconHandle {
    hicon: HICON,
    owned: bool,
}

impl Drop for IconHandle {
    fn drop(&mut self) {
        if self.owned {
            // DestroyIcon returns Result<()> in windows-rs 0.62; a failure during
            // teardown is not actionable, so discard via .ok().
            unsafe { DestroyIcon(self.hicon).ok() };
        }
    }
}

/// Tries six sources in order and returns the first non-null icon handle.
/// Returns `None` only when all probes fail (window has no accessible icon).
fn resolve_icon_handle(hwnd: HWND, icon_px: i32) -> Option<IconHandle> {
    // Ordered by fidelity and specificity: per-window slots first (highest
    // quality), then class icons (shared across all windows of the class),
    // then the shell exe icon (generic, size-capped at 32x32).
    if let Some(h) = fetch_wm_icon(hwnd, ICON_BIG) {
        tracing::debug!(?hwnd, source = "wm_icon_big", "Resolved icon handle");
        return Some(h);
    }
    if let Some(h) = fetch_wm_icon(hwnd, ICON_SMALL2) {
        tracing::debug!(?hwnd, source = "wm_icon_small2", "Resolved icon handle");
        return Some(h);
    }
    if let Some(h) = fetch_wm_icon(hwnd, ICON_SMALL) {
        tracing::debug!(?hwnd, source = "wm_icon_small", "Resolved icon handle");
        return Some(h);
    }
    if let Some(h) = fetch_class_icon(hwnd, GCLP_HICON) {
        tracing::debug!(?hwnd, source = "class_icon", "Resolved icon handle");
        return Some(h);
    }
    if let Some(h) = fetch_class_icon(hwnd, GCLP_HICONSM) {
        tracing::debug!(?hwnd, source = "class_icon_sm", "Resolved icon handle");
        return Some(h);
    }
    if let Some(h) = fetch_shell_icon(hwnd, icon_px) {
        tracing::debug!(?hwnd, source = "shell", "Resolved icon handle");
        return Some(h);
    }
    tracing::debug!(?hwnd, source = "none", "No icon handle found");
    None
}

/// Sends `WM_GETICON` with `SMTO_ABORTIFHUNG` and the module-level timeout.
/// Returns a shared (non-owned) handle when the window responds with a nonzero icon.
fn fetch_wm_icon(hwnd: HWND, kind: u32) -> Option<IconHandle> {
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_GETICON,
            WPARAM(kind as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            MSG_TIMEOUT_MS,
            Some(&mut result),
        )
    };
    if result != 0 {
        Some(IconHandle {
            hicon: HICON(result as *mut _),
            owned: false,
        })
    } else {
        None
    }
}

/// Reads the class-level icon via `GetClassLongPtrW`.
/// Returns a shared (non-owned) handle when the class has a registered icon.
fn fetch_class_icon(hwnd: HWND, idx: GET_CLASS_LONG_INDEX) -> Option<IconHandle> {
    let value = unsafe { GetClassLongPtrW(hwnd, idx) };
    if value != 0 {
        Some(IconHandle {
            hicon: HICON(value as *mut _),
            owned: false,
        })
    } else {
        None
    }
}

/// Resolves the shell icon for the window's backing executable.
/// Returns an owned handle that must be destroyed via `DestroyIcon`.
fn fetch_shell_icon(hwnd: HWND, icon_px: i32) -> Option<IconHandle> {
    let path = crate::platform::windows::process::get_exe_path(hwnd)?;

    let size_flag = if icon_px >= 32 {
        SHGFI_LARGEICON
    } else {
        SHGFI_SMALLICON
    };

    let mut sfi = SHFILEINFOW {
        hIcon: HICON(ptr::null_mut()),
        iIcon: 0,
        dwAttributes: 0,
        szDisplayName: [0u16; 260],
        szTypeName: [0u16; 80],
    };

    let ret = unsafe {
        SHGetFileInfoW(
            PCWSTR(path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut sfi),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | size_flag,
        )
    };

    if ret != 0 && !sfi.hIcon.0.is_null() {
        Some(IconHandle {
            hicon: sfi.hIcon,
            owned: true, // SHGetFileInfoW(SHGFI_ICON) allocates; caller must DestroyIcon
        })
    } else {
        None
    }
}

/// Captures an HICON into an RGBA `ColorImage` at `icon_px x icon_px`.
///
/// Uses a 32-bit top-down BGRA `CreateDIBSection` for direct pixel access with
/// well-defined alpha. The DIB is pre-zeroed so pixels not written by DrawIconEx
/// remain fully transparent.
fn capture_hicon(hicon: HICON, icon_px: i32) -> Option<ColorImage> {
    let screen_dc = unsafe { CreateCompatibleDC(None) };
    if screen_dc.is_invalid() {
        return None;
    }
    let _dc_guard = DcGuard(screen_dc);

    // 32-bit top-down BGRA DIBSection. biHeight is negative for top-down layout.
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: icon_px,
            biHeight: -icon_px, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default() // biSizeImage, biXPelsPerMeter, etc. unused for BI_RGB
        },
        ..Default::default() // no color table for 32-bit
    };

    let mut bits: *mut c_void = ptr::null_mut();
    let hbm =
        unsafe { CreateDIBSection(Some(screen_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) }
            .ok()?;

    if bits.is_null() {
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

    // Zero the backing store so unwritten pixels have alpha=0 (transparent).
    let byte_count = (icon_px * icon_px * 4) as usize;
    unsafe { ptr::write_bytes(bits as *mut u8, 0, byte_count) };

    // Draw the icon into the memory DC at icon_px x icon_px.
    if unsafe { DrawIconEx(mem_dc, 0, 0, hicon, icon_px, icon_px, 0, None, DI_NORMAL) }.is_err() {
        return None;
    }

    // Flush GDI batch so DrawIconEx writes reach the DIB backing store before
    // we read it. MSDN recommends GdiFlush before direct DIBSection pixel access.
    if !unsafe { GdiFlush() }.as_bool() {
        tracing::debug!("GdiFlush returned FALSE; proceeding with pixel read");
    }

    // Copy pixels from the DIBSection into an owned buffer.
    let mut buf = vec![0u8; byte_count];
    unsafe { ptr::copy_nonoverlapping(bits as *const u8, buf.as_mut_ptr(), byte_count) };

    bgra_to_rgba_in_place(&mut buf);

    Some(ColorImage::from_rgba_unmultiplied(
        [icon_px as usize, icon_px as usize],
        &buf,
    ))
}

/// Swaps B and R channels in a BGRA buffer to produce RGBA.
/// The buffer length must be a multiple of 4 (one u8 per channel, 4 channels per pixel).
fn bgra_to_rgba_in_place(buf: &mut [u8]) {
    debug_assert!(
        buf.len().is_multiple_of(4),
        "bgra_to_rgba_in_place: buffer length {} is not a multiple of 4",
        buf.len()
    );
    for pixel in buf.chunks_exact_mut(4) {
        pixel.swap(0, 2); // swap B <-> R; G and A stay in place
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_to_rgba_swaps_channels() {
        // Two BGRA pixels: [B, G, R, A]
        let mut buf = vec![0x10, 0x20, 0x30, 0x40, 0xA0, 0xB0, 0xC0, 0xD0];
        bgra_to_rgba_in_place(&mut buf);
        // Expected RGBA: [R, G, B, A]
        assert_eq!(buf, vec![0x30, 0x20, 0x10, 0x40, 0xC0, 0xB0, 0xA0, 0xD0]);
    }

    #[test]
    fn bgra_to_rgba_empty_noop() {
        let mut buf: Vec<u8> = Vec::new();
        bgra_to_rgba_in_place(&mut buf);
        assert!(buf.is_empty());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "not a multiple of 4")]
    fn bgra_to_rgba_debug_asserts_on_unaligned_len() {
        let mut buf = vec![0u8; 5];
        bgra_to_rgba_in_place(&mut buf);
    }
}
