use std::cell::RefCell;
use std::sync::Once;

use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::Graphics::DirectComposition::{
    IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, CreateWindowExW, DestroyWindow, GWLP_USERDATA, HCURSOR, HICON,
    HWND_BOTTOM, HWND_TOPMOST, IDC_ARROW, LoadCursorW, MSGFLT_ALLOW, RegisterClassW,
    RegisterWindowMessageW, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

use super::wnd_proc::aux_wnd_proc;
use super::{CLASS_NAME, TRAY_UID, WM_APP_TRAY};
use crate::{AuxiliaryWindowHandler, PhysicalPosition, PhysicalSize, WindowAttributes};

pub(super) struct TrayState {
    pub(super) data: NOTIFYICONDATAW,
    /// `RegisterWindowMessageW("TaskbarCreated")`, or 0 when registration failed. The
    /// shell broadcasts it when the taskbar restarts, and the icon must be re-added then.
    pub(super) taskbar_created: u32,
}

impl Drop for TrayState {
    /// Removes the icon while the owner window is still alive. `Window`'s Drop drops this
    /// before it destroys the window, so the `(hWnd, uID)` key is still valid here.
    fn drop(&mut self) {
        if !unsafe { Shell_NotifyIconW(NIM_DELETE, &self.data) }.as_bool() {
            tracing::warn!("Shell_NotifyIconW(NIM_DELETE) failed");
        }
    }
}

/// Per-window state stored behind `GWLP_USERDATA`.
pub(super) struct WindowState {
    pub(super) handler: Box<dyn AuxiliaryWindowHandler>,
    pub(super) tray: Option<TrayState>,
    /// The DirectComposition target that roots the consumer's visual on this window. It
    /// holds the visual alive until the window drops, so composited content survives
    /// even after the renderer that created the visual releases its own reference.
    pub(super) content_target: Option<IDCompositionTarget>,
}

pub trait AuxiliaryWindowExtWindows {
    fn hwnd(&self) -> HWND;

    /// Attaches a system-tray icon to the window. The crate re-adds it when the taskbar
    /// restarts and removes it when the window drops. A context-menu request drives
    /// `tray_menu` then `on_tray_menu_selected`. The caller keeps ownership of `icon`.
    fn install_tray_icon(&self, icon: HICON, tooltip: &str) -> anyhow::Result<()>;

    /// A no-op when no icon is installed.
    fn set_tray_tooltip(&self, tooltip: &str);

    /// Roots `visual` on this window through a DirectComposition target the window then
    /// owns. The consumer builds `device` and `visual` without an HWND, so this is the
    /// window-bound half of surface creation, the analog of macOS `set_content_layer`.
    /// `target` must be created from `device` for `SetRoot` to accept `visual`.
    fn set_content_visual(
        &self,
        device: &IDCompositionDevice,
        visual: &IDCompositionVisual,
    ) -> anyhow::Result<()>;
}

impl AuxiliaryWindowExtWindows for crate::AuxiliaryWindow {
    fn hwnd(&self) -> HWND {
        self.inner.hwnd()
    }

    fn install_tray_icon(&self, icon: HICON, tooltip: &str) -> anyhow::Result<()> {
        self.inner.install_tray_icon(icon, tooltip)
    }

    fn set_tray_tooltip(&self, tooltip: &str) {
        self.inner.set_tray_tooltip(tooltip);
    }

    fn set_content_visual(
        &self,
        device: &IDCompositionDevice,
        visual: &IDCompositionVisual,
    ) -> anyhow::Result<()> {
        self.inner.set_content_visual(device, visual)
    }
}

fn ensure_class_registered() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| {
        let instance: HINSTANCE = match unsafe { GetModuleHandleW(None) } {
            Ok(module) => module.into(),
            Err(e) => {
                tracing::error!(?e, "GetModuleHandleW failed registering window class");
                return;
            }
        };
        let cursor: HCURSOR = unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(aux_wnd_proc),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            hCursor: cursor,
            ..Default::default()
        };
        unsafe { RegisterClassW(&class) };
    });
}

fn ex_style_for(attributes: &WindowAttributes) -> WINDOW_EX_STYLE {
    let mut ex_style = WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP;
    if attributes.click_through {
        ex_style |= WS_EX_LAYERED | WS_EX_TRANSPARENT;
    }
    if !attributes.focusable {
        ex_style |= WS_EX_NOACTIVATE;
    }
    ex_style
}

struct OwnedHwnd {
    hwnd: HWND,
}

impl OwnedHwnd {
    fn new(ex_style: WINDOW_EX_STYLE, attributes: &WindowAttributes) -> anyhow::Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                CLASS_NAME,
                w!(""),
                WS_POPUP,
                attributes.position.x,
                attributes.position.y,
                attributes.size.width as i32,
                attributes.size.height as i32,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        Ok(Self { hwnd })
    }

    fn hwnd(&self) -> HWND {
        self.hwnd
    }
}

impl Drop for OwnedHwnd {
    /// Win32 refuses to destroy a window owned by another thread and only reports it
    /// through the return value, so swallowing the error hides both a cross-thread
    /// destroy and whatever teardown the window still owed the OS.
    fn drop(&mut self) {
        if let Err(e) = unsafe { DestroyWindow(self.hwnd) } {
            tracing::error!(?e, "failed to destroy window");
        }
    }
}

pub(crate) struct Window {
    window: OwnedHwnd,
    state: *mut RefCell<WindowState>,
}

impl Window {
    pub(crate) fn new(
        attributes: &WindowAttributes,
        handler: Box<dyn AuxiliaryWindowHandler>,
    ) -> anyhow::Result<Self> {
        ensure_class_registered();
        let window = OwnedHwnd::new(ex_style_for(attributes), attributes)?;
        // GWLP_USERDATA holds one machine word, so the state is boxed to a thin pointer
        // the wnd-proc reads back.
        let state: *mut RefCell<WindowState> = Box::into_raw(Box::new(RefCell::new(WindowState {
            handler,
            tray: None,
            content_target: None,
        })));
        unsafe { SetWindowLongPtrW(window.hwnd(), GWLP_USERDATA, state as isize) };
        Ok(Self { window, state })
    }

    pub(crate) fn set_frame(&self, position: PhysicalPosition, size: PhysicalSize) {
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                None,
                position.x,
                position.y,
                size.width as i32,
                size.height as i32,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .ok();
        }
    }

    pub(crate) fn set_visible(&self, visible: bool) {
        let cmd = if visible { SW_SHOWNA } else { SW_HIDE };
        unsafe { ShowWindow(self.window.hwnd(), cmd).ok().ok() };
    }

    pub(crate) fn set_level(&self, level: crate::WindowLevel) {
        let insert_after = match level {
            crate::WindowLevel::Floating => HWND_TOPMOST,
            crate::WindowLevel::Bottom => HWND_BOTTOM,
        };
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                Some(insert_after),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .ok();
        }
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.window.hwnd()
    }

    pub(crate) fn install_tray_icon(&self, icon: HICON, tooltip: &str) -> anyhow::Result<()> {
        let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        if taskbar_created == 0 {
            tracing::warn!(
                "RegisterWindowMessageW(TaskbarCreated) returned 0, tray will not survive an explorer restart"
            );
        } else if let Err(e) =
            unsafe { ChangeWindowMessageFilterEx(self.hwnd(), taskbar_created, MSGFLT_ALLOW, None) }
        {
            tracing::warn!(?e, "ChangeWindowMessageFilterEx(TaskbarCreated) failed");
        }

        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd(),
            uID: TRAY_UID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_APP_TRAY,
            hIcon: icon,
            ..Default::default()
        };
        write_tooltip(&mut data.szTip, tooltip);
        if !unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool() {
            anyhow::bail!("Shell_NotifyIconW(NIM_ADD) failed");
        }

        unsafe { (*self.state).borrow_mut() }.tray = Some(TrayState {
            data,
            taskbar_created,
        });
        Ok(())
    }

    pub(crate) fn set_tray_tooltip(&self, tooltip: &str) {
        let mut state = unsafe { (*self.state).borrow_mut() };
        if let Some(tray) = state.tray.as_mut() {
            write_tooltip(&mut tray.data.szTip, tooltip);
            if !unsafe { Shell_NotifyIconW(NIM_MODIFY, &tray.data) }.as_bool() {
                tracing::warn!("Shell_NotifyIconW(NIM_MODIFY) failed");
            }
        }
    }

    pub(crate) fn set_content_visual(
        &self,
        device: &IDCompositionDevice,
        visual: &IDCompositionVisual,
    ) -> anyhow::Result<()> {
        let target = unsafe { device.CreateTargetForHwnd(self.window.hwnd(), true)? };
        unsafe {
            target.SetRoot(visual)?;
            device.Commit()?;
        }
        unsafe { (*self.state).borrow_mut() }.content_target = Some(target);
        Ok(())
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe { SetWindowLongPtrW(self.window.hwnd(), GWLP_USERDATA, 0) };
        drop(unsafe { Box::from_raw(self.state) });
    }
}

fn write_tooltip(dst: &mut [u16], tooltip: &str) {
    let wide: Vec<u16> = tooltip.encode_utf16().collect();
    let n = wide.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&wide[..n]);
    for slot in dst.iter_mut().skip(n) {
        *slot = 0;
    }
}
