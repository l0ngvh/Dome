use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Foundation::{LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
    GetWindowDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, EnumWindows, GA_ROOT, GA_ROOTOWNER, GW_OWNER, GWL_EXSTYLE, GWL_STYLE,
    GetAncestor, GetClassNameW, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, HWND_BOTTOM, IsIconic, IsWindowVisible, IsZoomed, MINMAXINFO,
    SMTO_ABORTIFHUNG, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE,
    SWP_NOSIZE, SWP_NOZORDER, SendMessageTimeoutW, SetForegroundWindow, SetWindowPos, ShowWindow,
    ShowWindowAsync, WM_GETMINMAXINFO, WM_GETTEXT, WM_GETTEXTLENGTH, WS_CHILD, WS_EX_APPWINDOW,
    WS_EX_DLGMODALFRAME, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_POPUP, WS_THICKFRAME,
};
use windows::core::{BOOL, PCWSTR, w};

use crate::core::{Dimension, Length, Physical};
use crate::platform::windows::dome::rejection_log_filter::RejectionReason;
use crate::platform::windows::external::{
    HwndId, InspectExternalWindow, ManageExternalWindow, ShowCmd, ZOrder,
};

// Unlike macOS, we are allowed to move windows completely offscreen on Windows
pub(crate) const OFFSCREEN_POS: Length<Physical> = Length::new(-32000.0);

/// Returns the window's physical-pixel frame.
///
/// # Cross-process DPI behaviour
///
/// Because Dome is Per-Monitor v2 DPI-aware (see `resources/windows/dome.manifest`),
/// GetWindowRect returns physical pixels regardless of the target HWND's own
/// DPI awareness. Windows virtualizes the return based on the CALLER's
/// awareness, not the target's. This is documented in the Microsoft Learn
/// "PhysicalToLogicalPointForPerMonitorDPI" page:
///
/// > Consider two applications, one has a PROCESS_DPI_AWARENESS value of
/// > PROCESS_DPI_UNAWARE and the other has a value of PROCESS_PER_MONITOR_AWARE.
/// > The PROCESS_PER_MONITOR_AWARE app creates a window on a single monitor
/// > where the scale factor is 200% (192 DPI). If both apps call GetWindowRect
/// > on this window, they will receive different values. The PROCESS_DPI_UNAWARE
/// > app will receive a rect based on 96 DPI coordinates, while the
/// > PROCESS_PER_MONITOR_AWARE app will receive coordinates matching the actual
/// > DPI of the monitor.
///
/// Corroborating sources:
/// - MS Learn, GetWindowRect: "GetWindowRect is virtualized for DPI."
///   https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowrect
/// - MS Learn, PhysicalToLogicalPointForPerMonitorDPI: "The system returns
///   all points to an application in its own coordinate space." Also: "since
///   a PROCESS_PER_MONITOR_AWARE uses the actual DPI of the monitor, logical
///   and physical coordinates are identical."
///   https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-physicaltologicalpointforpermonitordpi
/// - Stack Overflow (Cody Gray, 2016): "if you call GetWindowRect or GetClientRect
///   from a high-DPI aware application, you will get the actual values in
///   screen coordinates. This will be true not only for windows belonging to
///   your application's process, but also for windows belonging to other
///   processes, regardless of that other process's DPI awareness setting."
///   https://stackoverflow.com/a/37829235
///
/// Upshot: typing this as `Dimension<Physical>` is honest unconditionally
/// for PMv2 callers. Separate from this, WM_GETMINMAXINFO is NOT virtualized
/// in the same way -- see `target_scale_to_physical`.
pub(crate) fn get_dimension(hwnd: HWND) -> Dimension {
    let mut rect = RECT::default();
    if let Err(e) = unsafe { GetWindowRect(hwnd, &mut rect) } {
        tracing::trace!(?hwnd, "GetWindowRect failed: {e}");
        // Callers tolerate a zero Dimension (e.g. check_unmanageable rejects zero-dim windows).
        return rect_to_dimension(rect);
    }
    rect_to_dimension(rect)
}

/// Converts a Win32 `RECT` (left, top, right, bottom edges) into a `Dimension<Physical>`
/// with (x, y, width, height). This is the single site for the `RECT -> Dimension` crossing;
/// callers in `display.rs` and within this module use it instead of ad-hoc arithmetic.
pub(crate) fn rect_to_dimension(rect: RECT) -> Dimension {
    Dimension::new(
        Length::new(rect.left as f32),
        Length::new(rect.top as f32),
        Length::new((rect.right - rect.left) as f32),
        Length::new((rect.bottom - rect.top) as f32),
    )
}

/// Positions `hwnd` at `OFFSCREEN_POS` with z-order HWND_BOTTOM.
///
/// Uses `SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS` and deliberately
/// omits `SWP_NOZORDER` so the z-drop to HWND_BOTTOM takes effect. This ensures
/// offscreen windows cannot occlude visible windows and the reposition does not
/// steal foreground activation.
pub(crate) fn move_window_offscreen(hwnd: HWND) {
    if let Err(e) = unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_BOTTOM),
            OFFSCREEN_POS.value() as i32,
            OFFSCREEN_POS.value() as i32,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
        )
    } {
        tracing::trace!(?hwnd, "move_window_offscreen SetWindowPos failed: {e}");
    }
}

/// Returns the invisible border widths (left, top, right, bottom) as raw i32 in physical pixels.
/// Used internally by `set_position` for border compensation and by `get_size_constraints`
/// for track-size adjustment.
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

const MSG_TIMEOUT_MS: u32 = 100;

/// Target-dependent scale factor for values returned by WM_GETMINMAXINFO.
///
/// MINMAXINFO fields are filled by the target HWND's wndproc, which runs
/// under the DPI-awareness context the HWND was created with (per Windows'
/// Mixed-Mode DPI rules). For a PMv2 target matching Dome's PMv2 caller
/// context, the values are already physical pixels. For legacy DPI-unaware
/// or System-DPI-aware targets, the values are in the target's own context
/// (96-DPI logical or system-DPI-logical); Dome must scale them to match
/// its own physical-pixel coordinate system.
///
/// This wrapper detects the target's awareness via GetWindowDpiAwarenessContext
/// and, when it differs from PMv2, uses GetDpiForWindow to derive the scale
/// factor target_dpi / 96.0. This fixes a pre-existing bug where legacy
/// target apps reported size constraints in the wrong unit.
fn target_scale_to_physical(hwnd: HWND) -> f32 {
    // SAFETY: GetWindowDpiAwarenessContext works across processes (Win10 1607+).
    let ctx = unsafe { GetWindowDpiAwarenessContext(hwnd) };
    let is_pmv2 =
        unsafe { AreDpiAwarenessContextsEqual(ctx, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
            .as_bool();
    if is_pmv2 {
        1.0
    } else {
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        if dpi == 0 {
            // this means an invalid hwnd was passed in
            1.0
        } else {
            dpi as f32 / 96.0
        }
    }
}

/// Subtracts invisible border widths from raw min/max track-size pairs, returning
/// the content-area constraints as f32. Negative results are clamped to zero.
fn constraints_subtract_border(
    min_track: (i32, i32),
    max_track: (i32, i32),
    border: (i32, i32, i32, i32),
) -> (f32, f32, f32, f32) {
    let h_border = border.0 + border.2;
    let v_border = border.1 + border.3;
    (
        (min_track.0 - h_border).max(0) as f32,
        (min_track.1 - v_border).max(0) as f32,
        (max_track.0 - h_border).max(0) as f32,
        (max_track.1 - v_border).max(0) as f32,
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

pub(crate) struct ExternalHwnd(HWND);

unsafe impl Send for ExternalHwnd {}
unsafe impl Sync for ExternalHwnd {}

impl ExternalHwnd {
    pub(crate) fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }
}

/// Activate `hwnd` as the foreground window. The leading Alt key-down/up via
/// SendInput clears the foreground lock so SetForegroundWindow succeeds even
/// when the calling thread does not own the foreground window. No-op when
/// `hwnd` is already in the foreground.
pub(super) fn force_set_foreground(hwnd: HWND) {
    if unsafe { GetForegroundWindow() } == hwnd {
        return;
    }
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_MENU,
                    // wScan, time, dwExtraInfo zeroed: documented no-op values
                    // for a synthetic VK_MENU keypress. dwFlags 0 = keydown.
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
                    // wScan, time, dwExtraInfo zeroed: same no-op defaults.
                    ..Default::default()
                },
            },
        },
    ];
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
        tracing::warn!("SetForegroundWindow failed, another app may have focus lock");
    }
}

impl ManageExternalWindow for ExternalHwnd {
    fn id(&self) -> HwndId {
        HwndId::from(self.0)
    }

    fn pid(&self) -> u32 {
        let mut pid = 0u32;
        // Non-blocking thread/process-map lookup; safe on external HWNDs.
        // Returns 0 on a zombie HWND (window already destroyed); 0 is never a
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
    ///
    /// Single `.value() as i32` site for all `SetWindowPos` placement calls on the managed-window path.
    fn set_position(&self, z: ZOrder, dim: Dimension) {
        let hwnd = self.0;
        let old = get_dimension(hwnd);
        let (bl, bt, br, bb) = get_invisible_border(hwnd);
        let x = dim.x.value() as i32 - bl;
        let y = dim.y.value() as i32 - bt;
        let cx = dim.width.value() as i32 + bl + br;
        let cy = dim.height.value() as i32 + bt + bb;

        let insert_after: Option<HWND> = z.into();
        let mut flags = SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
        if insert_after.is_none() {
            flags |= SWP_NOZORDER;
        }

        if let Err(e) = unsafe { SetWindowPos(hwnd, insert_after, x, y, cx, cy, flags) } {
            tracing::trace!(?hwnd, rect = ?(x, y, cx, cy), "SetWindowPos failed: {e}");
        }

        // Propagate the position delta to owned child windows so they stay anchored
        // relative to the parent. Short-circuits on windows with no owned children.
        let dx = x - old.x.value() as i32;
        let dy = y - old.y.value() as i32;
        if dx != 0 || dy != 0 {
            for_each_owned(hwnd, |child| {
                let mut rect = RECT::default();
                if unsafe { GetWindowRect(child, &mut rect).is_ok() }
                    && let Err(e) = unsafe {
                        SetWindowPos(
                            child,
                            None,
                            rect.left + dx,
                            rect.top + dy,
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
    fn check_unmanageable(&self) -> Option<RejectionReason> {
        // We don't check for empty title here, as most some text editor apps open windows with
        // empty title for untitled documents
        let hwnd = self.0;
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return Some(RejectionReason::NotVisible);
        }
        if unsafe { IsIconic(hwnd) }.as_bool() {
            // Already-minimized windows are skipped at registration time. Their
            // visible rect is the iconic-cache value (-32000,-32000), the monitor
            // is unreliable, and we have no way to know the user's intended
            // tiling-vs-float state. Picked back up by the standard create path
            // when the user restores the window via WM_RESTORE / unminimize.
            return Some(RejectionReason::Iconic);
        }
        if is_cloaked(hwnd) {
            return Some(RejectionReason::Cloaked);
        }
        if unsafe { GetAncestor(hwnd, GA_ROOT) } != hwnd {
            return Some(RejectionReason::Ancestor);
        }
        let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
        let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
        if style & WS_CHILD.0 != 0 {
            return Some(RejectionReason::WsChild);
        }
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return Some(RejectionReason::Toolwindow);
        }
        if ex_style & WS_EX_NOACTIVATE.0 != 0 {
            return Some(RejectionReason::Noactivate);
        }
        if ex_style & WS_EX_TRANSPARENT.0 != 0 {
            return Some(RejectionReason::Transparent);
        }
        if ex_style & WS_EX_DLGMODALFRAME.0 != 0 {
            return Some(RejectionReason::DlgModalFrame);
        }
        if style & WS_POPUP.0 != 0
            && style & (WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0) == 0
        {
            return Some(RejectionReason::PopupNoFrame);
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
            return Some(RejectionReason::OwnedNoAppWindow);
        }
        let dim = get_dimension(hwnd);
        if dim.width == Length::ZERO || dim.height == Length::ZERO {
            return Some(RejectionReason::ZeroDim);
        }
        None
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

    /// Applies `target_scale_to_physical` to handle legacy-DPI-unaware targets,
    /// then subtracts invisible borders.
    fn get_size_constraints(&self) -> (f32, f32, f32, f32) {
        let hwnd = self.0;
        // MINMAXINFO is an in/out parameter to WM_GETMINMAXINFO.
        // Zero-initialisation is the documented initial state: the target wndproc
        // fills all fields before returning. See Win32 docs for WM_GETMINMAXINFO.
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
        let scale = target_scale_to_physical(hwnd);
        let min_track = (
            (info.ptMinTrackSize.x as f32 * scale) as i32,
            (info.ptMinTrackSize.y as f32 * scale) as i32,
        );
        let max_track = (
            (info.ptMaxTrackSize.x as f32 * scale) as i32,
            (info.ptMaxTrackSize.y as f32 * scale) as i32,
        );
        let border = get_invisible_border(hwnd);
        constraints_subtract_border(min_track, max_track, border)
    }

    /// Returns the DWM extended frame bounds in physical pixels. Falls back to
    /// `GetWindowRect` if the DWM attribute is unavailable.
    fn get_visible_rect(&self) -> Dimension {
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
            rect_to_dimension(frame_rect)
        } else {
            get_dimension(hwnd)
        }
    }

    // Returns None for UWP shells, elevated processes we can't open, apps with no
    // version info, or empty FileDescription. Callers fall back to the executable name.
    fn get_app_display_name(&self) -> Option<String> {
        let hwnd = self.0;
        let path = crate::platform::windows::process::get_exe_path(hwnd)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constraints_to_physical_subtracts_border() {
        assert_eq!(
            constraints_subtract_border((200, 200), (1600, 1200), (0, 0, 0, 0)),
            (200.0, 200.0, 1600.0, 1200.0)
        );
        assert_eq!(
            constraints_subtract_border((420, 320), (2060, 1160), (10, 10, 10, 10)),
            (400.0, 300.0, 2040.0, 1140.0)
        );
    }

    #[test]
    fn target_scale_to_physical_inner_dpi_table() {
        let cases = [(96, 1.0), (144, 1.5), (192, 2.0)];
        for (dpi, expected) in cases {
            assert_eq!(target_scale_to_physical_inner(dpi), expected, "dpi={dpi}");
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn target_scale_to_physical_inner_rejects_zero_dpi_in_debug() {
        let _ = target_scale_to_physical_inner(0);
    }

    #[test]
    fn rect_to_dimension_roundtrip() {
        let rect = RECT {
            left: 100,
            top: 200,
            right: 400,
            bottom: 500,
        };
        let dim = rect_to_dimension(rect);
        assert_eq!(dim.x, Length::new(100.0));
        assert_eq!(dim.y, Length::new(200.0));
        assert_eq!(dim.width, Length::new(300.0));
        assert_eq!(dim.height, Length::new(300.0));
    }
}
