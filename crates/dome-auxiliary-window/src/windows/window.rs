use std::cell::RefCell;
use std::sync::Once;

use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::Graphics::DirectComposition::{
    IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GWLP_USERDATA, HCURSOR, HWND_BOTTOM, HWND_TOPMOST, IDC_ARROW,
    LoadCursorW, RegisterClassW, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WNDCLASSW, WNDPROC,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use super::CLASS_NAME;
use super::wnd_proc::aux_wnd_proc;
use crate::{AuxiliaryWindowHandler, PhysicalPosition, PhysicalSize, WindowAttributes};

/// Per-window state stored behind `GWLP_USERDATA`.
pub(super) struct WindowState {
    pub(super) handler: Box<dyn AuxiliaryWindowHandler>,
    /// The DirectComposition target that roots the consumer's visual on this window. It
    /// holds the visual alive until the window drops, so composited content survives
    /// even after the renderer that created the visual releases its own reference.
    pub(super) content_target: Option<IDCompositionTarget>,
}

pub trait AuxiliaryWindowExtWindows {
    fn hwnd(&self) -> HWND;

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

    fn set_content_visual(
        &self,
        device: &IDCompositionDevice,
        visual: &IDCompositionVisual,
    ) -> anyhow::Result<()> {
        self.inner.set_content_visual(device, visual)
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
        let window = OwnedHwnd::new(CLASS_NAME, ex_style_for(attributes), attributes)?;
        // GWLP_USERDATA holds one machine word, so the state is boxed to a thin pointer
        // the wnd-proc reads back.
        let state: *mut RefCell<WindowState> = Box::into_raw(Box::new(RefCell::new(WindowState {
            handler,
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

    pub(crate) fn deliver(&self, message: Box<dyn std::any::Any>) {
        // Borrow lives and ends here. Calls no window API, so it never re-enters the
        // wnd-proc while the WindowState borrow is held.
        unsafe { &*self.state }
            .borrow_mut()
            .handler
            .on_message(message);
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
        // Frees WindowState (and its Renderer / DirectComposition target) before the
        // OwnedHwnd field's Drop calls DestroyWindow.
        drop(unsafe { Box::from_raw(self.state) });
    }
}

fn ensure_class_registered() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| register_class(CLASS_NAME, Some(aux_wnd_proc)));
}

/// Registers a window class binding `name` to `wndproc`. Each class name maps to one
/// wnd-proc, so the tray window and the overlay windows register distinct classes.
pub(super) fn register_class(name: PCWSTR, wndproc: WNDPROC) {
    let instance: HINSTANCE = match unsafe { GetModuleHandleW(None) } {
        Ok(module) => module.into(),
        Err(e) => {
            tracing::error!(?e, "GetModuleHandleW failed registering window class");
            return;
        }
    };
    let cursor: HCURSOR = unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default();
    let class = WNDCLASSW {
        lpfnWndProc: wndproc,
        hInstance: instance,
        lpszClassName: name,
        hCursor: cursor,
        ..Default::default()
    };
    unsafe { RegisterClassW(&class) };
}

pub(super) fn ex_style_for(attributes: &WindowAttributes) -> WINDOW_EX_STYLE {
    let mut ex_style = WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP;
    if attributes.click_through {
        ex_style |= WS_EX_LAYERED | WS_EX_TRANSPARENT;
    }
    if !attributes.focusable {
        ex_style |= WS_EX_NOACTIVATE;
    }
    ex_style
}

pub(super) struct OwnedHwnd {
    hwnd: HWND,
}

impl OwnedHwnd {
    pub(super) fn new(
        class: PCWSTR,
        ex_style: WINDOW_EX_STYLE,
        attributes: &WindowAttributes,
    ) -> anyhow::Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                class,
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

    pub(super) fn hwnd(&self) -> HWND {
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
