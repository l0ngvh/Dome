use crate::platform::windows::event_loop::native::CallbackPanic;
use crate::platform::windows::event_loop::{EventSender, NativeEvent};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Once;

use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::Graphics::DirectComposition::{
    IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GWLP_USERDATA, HCURSOR, HWND_BOTTOM, HWND_MESSAGE, IDC_ARROW,
    LoadCursorW, RegisterClassW, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOREDRAW,
    SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE,
    WNDCLASSW, WNDPROC, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use super::super::wnd_proc::aux_wnd_proc;
use crate::platform::windows::CLASS_NAME;
use crate::platform::windows::ui::window::Point;
use crate::platform::windows::ui::window::Size;
use crate::platform::windows::ui::window::WindowAttributes;
use crate::platform::windows::ui::window::WindowInput;
use dome_core::Physical;

/// Per-window state stored behind `GWLP_USERDATA`.
pub(in crate::platform::windows) struct WindowState {
    pub(in crate::platform::windows) input: RefCell<Option<WindowInput>>,
    pub(in crate::platform::windows) dpi_changes: Option<EventSender<NativeEvent>>,
    pub(in crate::platform::windows) panic: Rc<CallbackPanic>,
    /// The DirectComposition target that roots the consumer's visual on this window. It
    /// holds the visual alive until the window drops, so composited content survives
    /// even after the renderer that created the visual releases its own reference.
    pub(in crate::platform::windows) content_target: RefCell<Option<IDCompositionTarget>>,
}

pub(crate) struct NativeWindow {
    window: OwnedHwnd,
    state: *mut WindowState,
}

impl NativeWindow {
    pub(in crate::platform::windows) fn new(
        callback_panic: Rc<CallbackPanic>,
        attributes: &WindowAttributes,
        input: Option<WindowInput>,
    ) -> anyhow::Result<Self> {
        ensure_class_registered();
        let window = OwnedHwnd::new(CLASS_NAME, ex_style_for(attributes), attributes)?;
        // GWLP_USERDATA holds one machine word, so the state is boxed to a thin pointer
        // the wnd-proc reads back.
        let state: *mut WindowState = Box::into_raw(Box::new(WindowState {
            input: RefCell::new(input),
            dpi_changes: attributes.dpi_changes.clone(),
            panic: callback_panic,
            content_target: RefCell::new(None),
        }));
        unsafe { SetWindowLongPtrW(window.hwnd(), GWLP_USERDATA, state as isize) };
        Ok(Self { window, state })
    }

    pub(crate) fn set_frame(&self, position: Point<Physical>, size: Size<Physical>) {
        assert!(
            size.width().value() > 0 && size.height().value() > 0,
            "window size must be positive"
        );
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                None,
                position.x().value(),
                position.y().value(),
                i32::try_from(size.width().value()).expect("window width out of range"),
                i32::try_from(size.height().value()).expect("window height out of range"),
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .ok();
        }
    }

    pub(crate) fn set_frame_without_redraw(&self, position: Point<Physical>, size: Size<Physical>) {
        assert!(
            size.width().value() > 0 && size.height().value() > 0,
            "window size must be positive"
        );
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                None,
                position.x().value(),
                position.y().value(),
                i32::try_from(size.width().value()).expect("window width out of range"),
                i32::try_from(size.height().value()).expect("window height out of range"),
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW,
            )
            .ok();
        }
    }

    pub(crate) fn set_visible(&self, visible: bool) {
        let cmd = if visible { SW_SHOWNA } else { SW_HIDE };
        unsafe { ShowWindow(self.window.hwnd(), cmd).ok().ok() };
    }

    pub(crate) fn set_level(&self, level: crate::platform::windows::ui::window::WindowLevel) {
        let insert_after = match level {
            crate::platform::windows::ui::window::WindowLevel::Bottom => HWND_BOTTOM,
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
        *unsafe { &*self.state }.content_target.borrow_mut() = Some(target);
        Ok(())
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe { SetWindowLongPtrW(self.window.hwnd(), GWLP_USERDATA, 0) };
        // The composition target must release before OwnedHwnd destroys the window.
        drop(unsafe { Box::from_raw(self.state) });
    }
}

pub(in crate::platform::windows) struct OwnedHwnd {
    hwnd: HWND,
}

pub(in crate::platform::windows) fn ex_style_for(attributes: &WindowAttributes) -> WINDOW_EX_STYLE {
    let mut ex_style = WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP;
    if attributes.click_through {
        ex_style |= WS_EX_LAYERED | WS_EX_TRANSPARENT;
    }
    if !attributes.focusable {
        ex_style |= WS_EX_NOACTIVATE;
    }
    ex_style
}

/// Registers a window class binding `name` to `wndproc`. Each class name maps to one
/// wnd-proc, so the tray window and the overlay windows register distinct classes.
pub(in crate::platform::windows) fn register_class(name: PCWSTR, wndproc: WNDPROC) {
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

impl OwnedHwnd {
    pub(in crate::platform::windows) fn message_only(class: PCWSTR) -> anyhow::Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                Default::default(),
                class,
                w!(""),
                Default::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        Ok(Self { hwnd })
    }

    pub(in crate::platform::windows) fn new(
        class: PCWSTR,
        ex_style: WINDOW_EX_STYLE,
        attributes: &WindowAttributes,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            attributes.size.width().value() > 0 && attributes.size.height().value() > 0,
            "window size must be positive"
        );
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                class,
                w!(""),
                WS_POPUP,
                attributes.position.x().value(),
                attributes.position.y().value(),
                i32::try_from(attributes.size.width().value())?,
                i32::try_from(attributes.size.height().value())?,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        Ok(Self { hwnd })
    }

    pub(in crate::platform::windows) fn hwnd(&self) -> HWND {
        self.hwnd
    }
}

fn ensure_class_registered() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| register_class(CLASS_NAME, Some(aux_wnd_proc)));
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
