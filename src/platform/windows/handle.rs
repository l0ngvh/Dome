use std::mem::size_of;

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows::Win32::Foundation::{LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect, MonitorFromWindow,
};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_ELEVATION,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenElevation, TokenIntegrityLevel,
};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Threading::{
    OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, DPI_AWARENESS_CONTEXT,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, DPI_AWARENESS_CONTEXT_UNAWARE, GetDpiForMonitor,
    GetDpiForWindow, GetWindowDpiAwarenessContext, MDT_EFFECTIVE_DPI, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, EnumWindows, GA_ROOT, GA_ROOTOWNER, GW_HWNDPREV, GW_OWNER, GWL_EXSTYLE,
    GWL_STYLE, GetAncestor, GetClassNameW, GetWindow, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, HWND_BOTTOM, IsIconic, IsWindowVisible, IsZoomed, MINMAXINFO,
    PostMessageW, SMTO_ABORTIFHUNG, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SWP_ASYNCWINDOWPOS,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageTimeoutW, SetWindowPos,
    ShowWindow, ShowWindowAsync, WM_CLOSE, WM_GETMINMAXINFO, WM_GETTEXT, WM_GETTEXTLENGTH,
    WS_CHILD, WS_EX_APPWINDOW, WS_EX_DLGMODALFRAME, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_THICKFRAME,
};
use windows::core::{BOOL, PCWSTR, w};

use crate::core::{Dimension, Length, LimitObservation, LimitUpdate, PixelRect, Pixels};
use crate::platform::windows::external::{
    HwndId, InspectExternalWindow, ManageExternalWindow, ShowCmd, ZOrder,
};
use crate::platform::windows::foreground::force_set_foreground;

// Unlike macOS, we are allowed to move windows completely offscreen on Windows
pub(crate) const OFFSCREEN_POS: Pixels = Pixels::new(-32000);

const MSG_TIMEOUT_MS: u32 = 100;

pub(in crate::platform::windows) trait ManageZOrder {
    fn window_above(&self, hwnd: HwndId) -> Option<HwndId>;
    /// `overlay` belongs to the window thread, so this call only completes while that
    /// thread pumps messages.
    fn demote_below(&self, overlay: HwndId, managed: HwndId);
}

pub(in crate::platform::windows) struct Win32ZOrder;

impl ManageZOrder for Win32ZOrder {
    fn window_above(&self, hwnd: HwndId) -> Option<HwndId> {
        let prev = unsafe { GetWindow(hwnd.into(), GW_HWNDPREV) }.ok();
        prev.map(HwndId::from)
    }

    fn demote_below(&self, overlay: HwndId, managed: HwndId) {
        let target: HWND = managed.into();
        unsafe {
            SetWindowPos(
                overlay.into(),
                Some(target),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .ok();
        }
    }
}

/// A buggy WndProc can return garbage from WM_GETTEXTLENGTH.
const MAX_WINDOW_TITLE_U16: usize = 32 * 1024;

const MAX_VERSION_INFO_BYTES: usize = 1 << 20;

const MAX_FILE_DESCRIPTION_U16: usize = 1024;

pub(crate) struct ExternalHwnd(HWND);

unsafe impl Send for ExternalHwnd {}

unsafe impl Sync for ExternalHwnd {}

impl ExternalHwnd {
    pub(crate) fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }
}

/// Because Dome is Per-Monitor v2 DPI-aware (see `resources/windows/dome.manifest`),
/// GetWindowRect returns physical pixels regardless of the target HWND's own DPI
/// awareness. Windows virtualizes the return based on the CALLER's awareness, not the
/// target's, and this holds for windows owned by other processes, which is the case
/// Dome depends on.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-physicaltologicalpointforpermonitordpi
/// https://stackoverflow.com/a/37829235
///
/// WM_GETMINMAXINFO is NOT virtualized the same way. See `target_scale_to_physical`.
pub(crate) fn get_pixel_rect(hwnd: HWND) -> PixelRect {
    let mut rect = RECT::default();
    if let Err(e) = unsafe { GetWindowRect(hwnd, &mut rect) } {
        tracing::trace!(?hwnd, "GetWindowRect failed: {e}");
        // Callers tolerate a zero rect (e.g. check_unmanageable rejects zero-extent windows).
        return rect_to_pixel_rect(rect);
    }
    rect_to_pixel_rect(rect)
}

/// Converts a Win32 `RECT` (left, top, right, bottom edges) into (x, y, width, height).
pub(crate) fn rect_to_pixel_rect(rect: RECT) -> PixelRect {
    PixelRect::new(
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    )
}

pub(crate) fn rect_to_dimension(rect: RECT) -> Dimension {
    rect_to_pixel_rect(rect).to_dimension()
}

/// Deliberately omits `SWP_NOZORDER` so the z-drop to HWND_BOTTOM takes effect. This ensures
/// offscreen windows cannot occlude visible windows and the reposition does not
/// steal foreground activation.
pub(crate) fn move_window_offscreen(hwnd: HWND) {
    if let Err(e) = unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_BOTTOM),
            OFFSCREEN_POS.value(),
            OFFSCREEN_POS.value(),
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
        )
    } {
        tracing::trace!(?hwnd, "move_window_offscreen SetWindowPos failed: {e}");
    }
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

impl ManageExternalWindow for ExternalHwnd {
    fn id(&self) -> HwndId {
        HwndId::from(self.0)
    }

    fn pid(&self) -> u32 {
        let mut pid = 0u32;
        // Non-blocking thread/process-map lookup, safe on external HWNDs.
        // Returns 0 on a zombie HWND (window already destroyed). 0 is never a
        // valid Windows pid, so callers can use it as an unambiguous sentinel.
        unsafe { GetWindowThreadProcessId(self.0, Some(&mut pid)) };
        if pid == 0 {
            tracing::warn!(id = %HwndId::from(self.0), "GetWindowThreadProcessId returned 0");
        }
        pid
    }

    /// Compensates for invisible borders (the gap between `GetWindowRect` and
    /// `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)`) and moves any thread-owned
    /// child windows by the same delta.
    fn set_position(&self, z: ZOrder, rect: PixelRect) {
        let hwnd = self.0;
        let old = get_pixel_rect(hwnd);
        let (bl, bt, br, bb) = get_invisible_border(hwnd);
        let x = rect.x().value() - bl;
        let y = rect.y().value() - bt;
        let cx = rect.width().value() + bl + br;
        let cy = rect.height().value() + bt + bb;

        let insert_after: Option<HWND> = z.into();
        let mut flags = SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
        if insert_after.is_none() {
            flags |= SWP_NOZORDER;
        }

        let ((x, y, cx, cy), restore_ctx) = enter_placement_context(hwnd, x, y, cx, cy);
        if let Err(e) = unsafe { SetWindowPos(hwnd, insert_after, x, y, cx, cy, flags) } {
            tracing::trace!(?hwnd, rect = ?(x, y, cx, cy), "SetWindowPos failed: {e}");
        }
        if let Some(previous) = restore_ctx {
            // Restore Dome's PMv2 context before the child propagation below,
            // whose GetWindowRect reads assume physical pixels.
            unsafe { SetThreadDpiAwarenessContext(previous) };
        }

        // Propagate the position delta to owned child windows so they stay anchored
        // relative to the parent.
        let dx = x - old.x().value();
        let dy = y - old.y().value();
        if dx != 0 || dy != 0 {
            for_each_owned(hwnd, |child| {
                let mut child_rect = RECT::default();
                if unsafe { GetWindowRect(child, &mut child_rect).is_ok() }
                    && let Err(e) = unsafe {
                        SetWindowPos(
                            child,
                            None,
                            child_rect.left + dx,
                            child_rect.top + dy,
                            0,
                            0,
                            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
                        )
                    }
                {
                    tracing::trace!(?child, dx, dy, "SetWindowPos (child propagate) failed: {e}");
                }
            });
        }
    }

    fn move_offscreen(&self) {
        move_window_offscreen(self.0);
    }

    fn show_cmd(&self, cmd: ShowCmd) {
        let sw = match cmd {
            ShowCmd::Restore => SW_RESTORE,
            ShowCmd::Minimize => SW_MINIMIZE,
        };
        unsafe { ShowWindowAsync(self.0, sw).ok().ok() };
    }

    fn close(&self) {
        if let Err(e) = unsafe { PostMessageW(Some(self.0), WM_CLOSE, WPARAM(0), LPARAM(0)) } {
            tracing::trace!(hwnd = ?self.0, "PostMessage WM_CLOSE failed: {e}");
        }
    }

    fn set_foreground_window(&self) {
        force_set_foreground(self.0);
    }

    fn is_maximized(&self) -> bool {
        unsafe { IsZoomed(self.0) }.as_bool()
    }

    fn recover(&self, was_maximized: bool) {
        let hwnd = self.0;
        unsafe {
            if was_maximized {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            if let Err(e) = SetWindowPos(
                hwnd,
                None,
                100,
                100,
                0,
                0,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE,
            ) {
                tracing::trace!(
                    ?hwnd,
                    op = "recover_set_position",
                    "SetWindowPos failed: {e}"
                );
            }
            if was_maximized {
                let _ = ShowWindow(hwnd, SW_MAXIMIZE);
            }
        }
    }
}

impl InspectExternalWindow for ExternalHwnd {
    fn check_unmanageable(&self) -> bool {
        // We don't check for empty title here, as most some text editor apps open windows with
        // empty title for untitled documents
        let hwnd = self.0;
        let pid = self.pid();
        let title = self.get_window_title();
        if is_silent_unmanageable_title(&title) {
            return true;
        }
        let process_name = self.get_process_name().ok();
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            // Those windows that shouldn't be managed have quite preditable title.
            // e.g. OLEChannelWnd or Default IME
            // Previous attempt to key by hwnd proved to be futile, those windows are spawned
            // multiple times with different hwnd. windows with same title and process should be rare
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: not visible"
            );
            return true;
        }
        if unsafe { IsIconic(hwnd) }.as_bool() {
            // Already-minimized windows are skipped at registration time. Their
            // visible rect is the iconic-cache value (-32000,-32000), the monitor
            // is unreliable, and we have no way to know the user's intended
            // tiling-vs-float state. Picked back up by the standard create path
            // when the user restores the window via WM_RESTORE / unminimize.
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: iconic"
            );
            return true;
        }
        if is_cloaked(hwnd) {
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: cloaked"
            );
            return true;
        }
        if unsafe { GetAncestor(hwnd, GA_ROOT) } != hwnd {
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: not top-level ancestor"
            );
            return true;
        }
        let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
        let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
        if style & WS_CHILD.0 != 0 {
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: WS_CHILD"
            );
            return true;
        }
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: WS_EX_TOOLWINDOW"
            );
            return true;
        }
        if ex_style & WS_EX_NOACTIVATE.0 != 0 {
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: WS_EX_NOACTIVATE"
            );
            return true;
        }
        if ex_style & WS_EX_TRANSPARENT.0 != 0 {
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: WS_EX_TRANSPARENT"
            );
            return true;
        }
        if ex_style & WS_EX_DLGMODALFRAME.0 != 0 {
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: WS_EX_DLGMODALFRAME"
            );
            return true;
        }
        if style & WS_POPUP.0 != 0
            && style & (WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0) == 0
        {
            let hmonitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
            // Fullscreen games usually use these styles, so we need to filter out for them
            let fullscreen = if hmonitor.0.is_null() {
                false
            } else {
                let mut info = MONITORINFO {
                    cbSize: size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                if unsafe { GetMonitorInfoW(hmonitor, &mut info) }.as_bool() {
                    let rect = get_pixel_rect(hwnd);
                    let left = Pixels::new(info.rcWork.left);
                    let top = Pixels::new(info.rcWork.top);
                    let right = Pixels::new(info.rcWork.right);
                    let bottom = Pixels::new(info.rcWork.bottom);
                    rect.x() <= left
                        && rect.y() <= top
                        && rect.x() + rect.width() >= right
                        && rect.y() + rect.height() >= bottom
                } else {
                    false
                }
            };
            if !fullscreen {
                crate::trace_once!(
                    key: (title.clone(), process_name.clone()),
                    ?title, ?pid, ?process_name, "not manageable: WS_POPUP without frame"
                );
                return true;
            }
        }
        // Mirror the Windows Shell's taskbar/Alt-Tab rule: a top-level app window
        // is either ownerless or sets WS_EX_APPWINDOW. Owned windows without that
        // flag are transients (dialogs, tool palettes, custom popups). Steam's main
        // window passes because it is ownerless despite using WS_POPUP. GW_OWNER is
        // used (not GA_ROOTOWNER) because it returns the direct owner, matching the
        // Shell's documented rule
        // (https://learn.microsoft.com/en-us/windows/win32/shell/taskbar#managing-taskbar-buttons).
        // Treat both Err and Ok(invalid) as ownerless:
        // upstream gates (IsWindowVisible, is_cloaked, GetAncestor(GA_ROOT) == hwnd)
        // already established a valid top-level HWND.
        let has_owner = matches!(
            unsafe { GetWindow(hwnd, GW_OWNER) },
            Ok(h) if !h.is_invalid(),
        );
        if has_owner && ex_style & WS_EX_APPWINDOW.0 == 0 {
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: owned window without WS_EX_APPWINDOW"
            );
            return true;
        }
        if is_process_elevated(pid) {
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: elevated"
            );
            return true;
        }
        let rect = get_pixel_rect(hwnd);
        if rect.width() == Pixels::ZERO || rect.height() == Pixels::ZERO {
            crate::trace_once!(
                key: (title.clone(), process_name.clone()),
                ?title, ?pid, ?process_name, "not manageable: zero dimension"
            );
            return true;
        }
        false
    }

    fn is_minimized(&self) -> bool {
        unsafe { IsIconic(self.0) }.as_bool()
    }

    fn get_window_title(&self) -> Option<String> {
        let hwnd = self.0;
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
        if ret == LRESULT(0) {
            return None;
        }
        if len == 0 || len > MAX_WINDOW_TITLE_U16 {
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
        if ret == LRESULT(0) {
            return None;
        }
        // Clamp a buggy WndProc that reports copied > buf.len().
        let end = copied.min(buf.len());
        if end == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..end]))
    }

    fn get_process_name(&self) -> anyhow::Result<String> {
        let hwnd = self.0;
        let path_wide = crate::platform::windows::process::get_exe_path(hwnd)
            .ok_or_else(|| anyhow::anyhow!("could not query process image name"))?;
        // Strip the trailing null before converting to a Rust string
        let path = String::from_utf16_lossy(&path_wide[..path_wide.len().saturating_sub(1)]);
        path.rsplit('\\')
            .next()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("no filename in path"))
    }

    fn get_process_path(&self) -> anyhow::Result<String> {
        let hwnd = self.0;
        let path_wide = crate::platform::windows::process::get_exe_path(hwnd)
            .ok_or_else(|| anyhow::anyhow!("could not query process image name"))?;
        let end = path_wide.len().saturating_sub(1);
        Ok(String::from_utf16_lossy(&path_wide[..end]))
    }

    /// A failed or timed-out read reports `Unchanged`, leaving core's stored limits alone.
    fn get_size_constraints(&self) -> LimitObservation {
        let hwnd = self.0;
        // Zero-initialisation is the documented initial state for MINMAXINFO, which the
        // target wndproc fills in before returning.
        let mut info = MINMAXINFO::default();
        let sent = unsafe {
            SendMessageTimeoutW(
                hwnd,
                WM_GETMINMAXINFO,
                WPARAM(0),
                LPARAM(&mut info as *mut _ as isize),
                SMTO_ABORTIFHUNG,
                MSG_TIMEOUT_MS,
                None,
            )
        };
        if sent.0 == 0 {
            tracing::trace!(?hwnd, "WM_GETMINMAXINFO failed or timed out");
            return LimitObservation::default();
        }
        let scale = target_scale_to_physical(hwnd);
        let (left, top, right, bottom) = get_invisible_border(hwnd);
        let horizontal = left + right;
        let vertical = top + bottom;
        LimitObservation {
            min_width: scaled_track_limit(info.ptMinTrackSize.x, horizontal, scale),
            min_height: scaled_track_limit(info.ptMinTrackSize.y, vertical, scale),
            max_width: scaled_track_limit(info.ptMaxTrackSize.x, horizontal, scale),
            max_height: scaled_track_limit(info.ptMaxTrackSize.y, vertical, scale),
        }
    }

    /// Returns the DWM extended frame bounds in physical pixels. Falls back to
    /// `GetWindowRect` if the DWM attribute is unavailable.
    fn get_visible_rect(&self) -> PixelRect {
        let hwnd = self.0;
        let mut frame_rect = RECT::default();
        let result = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut frame_rect as *mut _ as *mut _,
                std::mem::size_of::<RECT>() as u32,
            )
        };
        if result.is_ok() {
            rect_to_pixel_rect(frame_rect)
        } else {
            get_pixel_rect(hwnd)
        }
    }

    // Returns None for UWP shells, elevated processes we can't open, apps with no
    // version info, or empty FileDescription. Callers fall back to the executable name.
    fn get_app_display_name(&self) -> Option<String> {
        let hwnd = self.0;
        let path = crate::platform::windows::process::get_exe_path(hwnd)?;
        let path_ptr = PCWSTR(path.as_ptr());

        let size = unsafe { GetFileVersionInfoSizeW(path_ptr, None) };
        if size == 0 || size as usize > MAX_VERSION_INFO_BYTES {
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
        let key_wide: Vec<u16> =
            format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription")
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
        let slice_len = clamp_desc_len(
            desc_ptr as usize,
            buf.as_ptr() as usize,
            buf.len(),
            desc_len,
        );
        if slice_len == 0 {
            return None;
        }
        let desc_slice = unsafe { std::slice::from_raw_parts(desc_ptr as *const u16, slice_len) };
        let result = String::from_utf16_lossy(desc_slice).trim().to_string();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    // `MonitorFromWindow` is non-blocking and safe to call on external HWNDs.
    fn get_monitor(&self) -> isize {
        unsafe { MonitorFromWindow(self.0, MONITOR_DEFAULTTONEAREST) }.0 as isize
    }

    fn get_class_name(&self) -> Option<String> {
        let mut buf = [0u16; 256];
        let len = unsafe { GetClassNameW(self.0, &mut buf) };
        if len <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }

    fn get_aumid(&self) -> Option<String> {
        use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
        use windows::Win32::System::Com::CoTaskMemFree;
        use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
        use windows::Win32::UI::Shell::PropertiesSystem::{
            IPropertyStore, SHGetPropertyStoreForWindow,
        };
        unsafe {
            let store: IPropertyStore = SHGetPropertyStoreForWindow(self.0).ok()?;
            let pv = store.GetValue(&PKEY_AppUserModel_ID).ok()?;
            let pwstr = PropVariantToStringAlloc(&pv).ok()?;
            let result = pwstr.to_string().ok();
            CoTaskMemFree(Some(pwstr.as_ptr() as *const _));
            result
        }
    }
}

/// Returns the invisible border widths (left, top, right, bottom) as raw i32 in physical pixels.
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

// SAFETY: GetWindowDpiAwarenessContext and AreDpiAwarenessContextsEqual both
// accept HWNDs from other processes.
fn target_has_context(hwnd: HWND, ctx: DPI_AWARENESS_CONTEXT) -> bool {
    let target = unsafe { GetWindowDpiAwarenessContext(hwnd) };
    unsafe { AreDpiAwarenessContextsEqual(target, ctx) }.as_bool()
}

/// Scale from the units WM_GETMINMAXINFO answers in to Dome's physical pixels.
///
/// MINMAXINFO fields are filled by the target HWND's wndproc under the
/// awareness context the HWND was created with. A PMv2 target matches Dome's
/// caller context and reports physical pixels already. System-aware targets
/// report in system-DPI units, scaled via GetDpiForWindow. Unaware targets
/// report in their virtualized space, scaled via the monitor they occupy.
fn target_scale_to_physical(hwnd: HWND) -> f32 {
    if target_has_context(hwnd, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
        return 1.0;
    }
    if !target_has_context(hwnd, DPI_AWARENESS_CONTEXT_UNAWARE) {
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        return if dpi == 0 {
            // this means an invalid hwnd was passed in
            1.0
        } else {
            dpi as f32 / 96.0
        };
    }
    // An unaware wndproc fills MINMAXINFO in its virtualized space, which
    // Windows re-materializes at the landing monitor's scale. GetDpiForWindow
    // reports a constant 96 there regardless of location, so scale by the
    // monitor the window currently sits on.
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return 1.0;
    }
    monitor_effective_dpi(&rect).map_or(1.0, |dpi| dpi as f32 / 96.0)
}

/// Converts one MINMAXINFO track size into core's limit vocabulary.
/// Win32 reports a zero track size when the app set no limit on that axis. A positive
/// track size that does not exceed the invisible frame has no content-box equivalent, so
/// it is reported as no limit rather than as a zero-sized one.
fn scaled_track_limit(track: i32, border: i32, scale: f32) -> LimitUpdate {
    let track = (track as f32 * scale) as i32;
    if track <= 0 || track <= border {
        return LimitUpdate::Cleared;
    }
    LimitUpdate::Set(Length::new((track - border) as f32))
}

fn is_silent_unmanageable_title(title: &Option<String>) -> bool {
    let Some(t) = title.as_deref() else {
        return false;
    };
    matches!(
        t,
        "OleMainThreadWndName" | "OLEChannelWnd" | "Default IME" | "MSCTFIME UI"
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

fn is_process_elevated(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return false;
    };
    let mut token = windows::Win32::Foundation::HANDLE::default();
    let token_ok = unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.is_ok();
    let elevated = if token_ok {
        is_token_elevated_or_high_integrity(token)
    } else {
        false
    };
    unsafe {
        let _ = CloseHandle(process);
        if token_ok {
            let _ = CloseHandle(token);
        }
    }
    elevated
}

fn is_token_elevated_or_high_integrity(token: windows::Win32::Foundation::HANDLE) -> bool {
    let mut elevation = TOKEN_ELEVATION::default();
    let mut ret_len = 0u32;
    if unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        )
    }
    .is_ok()
        && elevation.TokenIsElevated != 0
    {
        return true;
    }
    let mut needed = 0u32;
    let _ = unsafe { GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut needed) };
    if needed == 0 {
        return false;
    }
    let mut buf = vec![0u8; needed as usize];
    if unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buf.as_mut_ptr() as *mut _),
            needed,
            &mut needed,
        )
    }
    .is_err()
    {
        return false;
    }
    let label = unsafe { &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL) };
    let sid = label.Label.Sid;
    if sid.is_invalid() {
        return false;
    }
    let count = unsafe { GetSidSubAuthorityCount(sid) };
    if count.is_null() {
        return false;
    }
    let sub_count = unsafe { *count };
    if sub_count == 0 {
        return false;
    }
    let rid_ptr = unsafe { GetSidSubAuthority(sid, u32::from(sub_count - 1)) };
    if rid_ptr.is_null() {
        return false;
    }
    let rid = unsafe { *rid_ptr };
    rid > 0x2000
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

/// Clamps `desc_len` so the returned slice cannot read past `buf`. `desc_len`
/// from VerQueryValueW counts the trailing null u16, hence the `-1`.
fn clamp_desc_len(desc_ptr: usize, buf_ptr: usize, buf_len: usize, desc_len: u32) -> usize {
    let offset_bytes = desc_ptr.saturating_sub(buf_ptr);
    let remaining_u16 = buf_len.saturating_sub(offset_bytes) / 2;
    (desc_len as usize)
        .saturating_sub(1)
        .min(remaining_u16)
        .min(MAX_FILE_DESCRIPTION_U16)
}

/// Effective DPI of the nearest monitor to `rect`, or None when either query
/// fails.
fn monitor_effective_dpi(rect: &RECT) -> Option<u32> {
    let hmonitor = unsafe { MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST) };
    if hmonitor.0.is_null() {
        return None;
    }
    let mut dpi = 0u32;
    if unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi) }.is_err() {
        return None;
    }
    (dpi > 0).then_some(dpi)
}

/// Scales a physical rect into logical units of a display at `dpi`, rounding
/// each edge half away from zero so width and height derive from rounded
/// opposite edges.
fn to_logical_rect(dpi: u32, x: i32, y: i32, cx: i32, cy: i32) -> (i32, i32, i32, i32) {
    let scale = f64::from(dpi) / 96.0;
    let edge = |v: i32| (f64::from(v) / scale).round() as i32;
    let left = edge(x);
    let top = edge(y);
    let right = edge(x + cx);
    let bottom = edge(y + cy);
    (left, top, right - left, bottom - top)
}

/// Enters the target's own DPI awareness context for a placement and returns
/// the coordinates to hand SetWindowPos plus the previous context the caller
/// must restore once the call and any physical-pixel reads are done. Returns
/// identity coordinates and no restore context for an aware target, or when
/// the context swap fails.
///
/// A DPI-unaware target receiving a cross-process SetWindowPos gets its rect
/// translated per edge, each edge using the scale of the edge's own monitor,
/// and the result re-materialized at the anchor monitor's scale. An outer
/// rect crossing onto a differently-scaled monitor is thus distorted in both
/// directions (a requested right edge of 2563 beside a 100% monitor lands at
/// 3204. A requested left edge 4px onto a 125% monitor from a 100% anchor
/// pulls half a kilopixel sideways). Issuing pre-converted coordinates from
/// inside the target's own context skips the translation entirely, so the
/// swap is taken for every unaware target and the conversion is the only
/// scale-dependent part.
fn enter_placement_context(
    hwnd: HWND,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
) -> ((i32, i32, i32, i32), Option<DPI_AWARENESS_CONTEXT>) {
    if !target_has_context(hwnd, DPI_AWARENESS_CONTEXT_UNAWARE) {
        return ((x, y, cx, cy), None);
    }
    // Resolve the landing monitor before the swap, so MonitorFromRect reads the
    // rect in Dome's physical-pixel context.
    let rect = RECT {
        left: x,
        top: y,
        right: x + cx,
        bottom: y + cy,
    };
    let coords = match monitor_effective_dpi(&rect) {
        Some(dpi) if dpi != 96 => to_logical_rect(dpi, x, y, cx, cy),
        _ => (x, y, cx, cy),
    };
    let previous = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_UNAWARE) };
    if previous.0.is_null() {
        // The swap failed, so the OS would translate the rect per edge. Issue
        // identity coords rather than the pre-converted ones.
        return ((x, y, cx, cy), None);
    }
    (coords, Some(previous))
}

#[cfg(test)]
mod tests {
    use super::{scaled_track_limit, to_logical_rect};
    use crate::core::LimitUpdate;

    #[test]
    fn identity_at_96_dpi() {
        assert_eq!(
            to_logical_rect(96, -3, 43, 1286, 1400),
            (-3, 43, 1286, 1400)
        );
    }

    // Values verified live against a tkinter window straddling a 125%/100%
    // monitor boundary: issuing this logical rect landed the visible frame
    // exactly on the tile instead of inflating the crossing edge by 1.25.
    #[test]
    fn scales_edges_independently_at_120_dpi() {
        assert_eq!(
            to_logical_rect(120, 1277, 43, 1286, 1400),
            (1022, 34, 1028, 1120)
        );
    }

    #[test]
    fn rounds_half_away_from_zero() {
        assert_eq!(to_logical_rect(192, 5, -5, 10, 20), (3, -3, 5, 11));
    }

    #[test]
    fn cleared_for_zero_negative_or_subframe_tracks() {
        assert!(matches!(
            scaled_track_limit(0, 18, 1.0),
            LimitUpdate::Cleared
        ));
        assert!(matches!(
            scaled_track_limit(-5, 18, 1.0),
            LimitUpdate::Cleared
        ));
        assert!(matches!(
            scaled_track_limit(17, 18, 1.0),
            LimitUpdate::Cleared
        ));
    }

    #[test]
    fn silent_title_filter_covers_known_hidden_windows() {
        assert!(super::is_silent_unmanageable_title(&Some(
            "OleMainThreadWndName".to_string()
        )));
        assert!(super::is_silent_unmanageable_title(&Some(
            "Default IME".to_string()
        )));
        assert!(super::is_silent_unmanageable_title(&Some(
            "MSCTFIME UI".to_string()
        )));
        assert!(super::is_silent_unmanageable_title(&Some(
            "OLEChannelWnd".to_string()
        )));
        assert!(!super::is_silent_unmanageable_title(&Some(
            ".NET-BroadcastEventWindow.bf7771.0".to_string()
        )));
        assert!(!super::is_silent_unmanageable_title(&Some(
            "GDI+ Window (ProtonDrive.exe)".to_string()
        )));
        assert!(!super::is_silent_unmanageable_title(&Some(
            "C:\\LDPlayer\\LDPlayer14\\adb.exe".to_string()
        )));
        assert!(!super::is_silent_unmanageable_title(&Some(
            "Notepad".to_string()
        )));
        assert!(!super::is_silent_unmanageable_title(&None));
    }

    #[test]
    fn scales_track_before_stripping_borders() {
        let LimitUpdate::Set(length) = scaled_track_limit(30, 18, 1.25) else {
            panic!("expected a set limit");
        };
        // 30 virtualized units scale to 37 physical pixels (truncated), then
        // the 18px frame pair comes off.
        assert!((length.value() - 19.0).abs() < 1e-4);
    }
}
