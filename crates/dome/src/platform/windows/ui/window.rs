use crate::platform::windows::event_loop::native::CallbackPanic;
use crate::platform::windows::event_loop::{EventSender, NativeEvent};
use dome_core::Physical;
pub use dome_core::{Point, Size};
use std::rc::Rc;
pub(in crate::platform::windows) mod native;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Primary,
    Secondary,
    Middle,
}

/// Where a window sits in the platform's stacking order. macOS enforces the band for the
/// window's lifetime. On Windows it is a one-time z-order the consumer maintains.
#[derive(Clone, Copy, Debug)]
pub enum WindowLevel {
    /// Below normal application windows.
    Bottom,
}

#[derive(Clone)]
pub struct WindowAttributes {
    pub position: Point<Physical>,
    pub size: Size<Physical>,
    pub click_through: bool,
    /// The window may hold keyboard focus. This only sets eligibility. The crate never
    /// forces focus itself.
    pub focusable: bool,
    pub dpi_changes: Option<EventSender<NativeEvent>>,
}

mod input;
pub(in crate::platform::windows) use input::WindowInput;

pub(crate) enum Window {
    Native(native::NativeWindow),
}

impl Window {
    pub(crate) fn set_frame(&self, position: Point<Physical>, size: Size<Physical>) {
        match self {
            Self::Native(window) => window.set_frame(position, size),
        }
    }
    pub(crate) fn set_visible(&self, visible: bool) {
        match self {
            Self::Native(window) => window.set_visible(visible),
        }
    }
    pub(crate) fn set_level(&self, level: WindowLevel) {
        match self {
            Self::Native(window) => window.set_level(level),
        }
    }

    pub(crate) fn hwnd(&self) -> windows::Win32::Foundation::HWND {
        match self {
            Self::Native(window) => window.hwnd(),
        }
    }
    pub(crate) fn set_frame_without_redraw(&self, position: Point<Physical>, size: Size<Physical>) {
        match self {
            Self::Native(window) => window.set_frame_without_redraw(position, size),
        }
    }
    pub(crate) fn set_content_visual(
        &self,
        device: &windows::Win32::Graphics::DirectComposition::IDCompositionDevice,
        visual: &windows::Win32::Graphics::DirectComposition::IDCompositionVisual,
    ) -> anyhow::Result<()> {
        match self {
            Self::Native(window) => window.set_content_visual(device, visual),
        }
    }
}

impl Window {
    pub(in crate::platform::windows) fn new(
        callback_panic: Rc<CallbackPanic>,
        attributes: &WindowAttributes,
        input: Option<WindowInput>,
    ) -> anyhow::Result<Self> {
        Ok(Self::Native(native::NativeWindow::new(
            callback_panic,
            attributes,
            input,
        )?))
    }
}
