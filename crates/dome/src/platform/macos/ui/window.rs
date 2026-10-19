use dome_core::Logical;
pub use dome_core::{Point, Size};
pub(in crate::platform::macos) mod native;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::platform::macos) enum MouseButton {
    Primary,
}

/// Where a window sits in the platform's stacking order. macOS enforces the band for the
/// window's lifetime. On Windows it is a one-time z-order the consumer maintains.
#[derive(Clone, Copy, Debug)]
pub(in crate::platform::macos) enum WindowLevel {
    /// Above normal application windows.
    Floating,
    /// Below normal application windows.
    Bottom,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::platform::macos) struct WindowAttributes {
    pub position: Point<Logical>,
    pub size: Size<Logical>,
    pub click_through: bool,
    /// Sets focus eligibility without requesting focus.
    pub focusable: bool,
}

mod input;
pub(in crate::platform::macos) use input::WindowInput;

pub(in crate::platform::macos) struct Window(native::NativeWindow);

impl Window {
    pub(in crate::platform::macos) fn set_frame(
        &self,
        position: Point<Logical>,
        size: Size<Logical>,
    ) {
        self.0.set_frame(position, size);
    }
    pub(in crate::platform::macos) fn set_visible(&self, visible: bool) {
        self.0.set_visible(visible);
    }
    pub(in crate::platform::macos) fn set_level(&self, level: WindowLevel) {
        self.0.set_level(level);
    }
    pub(in crate::platform::macos) fn set_content_layer(&self, layer: &objc2_quartz_core::CALayer) {
        self.0.set_content_layer(layer);
    }
    pub(in crate::platform::macos) fn focus(&self) {
        self.0.focus();
    }
}

impl Window {
    pub(in crate::platform::macos) fn new(
        attributes: &WindowAttributes,
        input: Option<WindowInput>,
    ) -> anyhow::Result<Self> {
        Ok(Self(native::NativeWindow::new(attributes, input)?))
    }
}
