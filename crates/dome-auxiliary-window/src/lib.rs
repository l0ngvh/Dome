//! Window creation and the window event loop for Dome and its status-bar
//! subprocess. Names no Dome domain type.

mod menu;
pub use menu::{MenuEntry, MenuItem};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use crate::macos as imp;
#[cfg(target_os = "macos")]
pub use macos::AuxiliaryWindowExtMacOs;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use crate::windows as imp;
#[cfg(target_os = "windows")]
use ::windows::Win32::UI::WindowsAndMessaging::HICON;
#[cfg(target_os = "windows")]
pub use windows::AuxiliaryWindowExtWindows;

/// A point in physical pixels. The origin may be negative across multiple monitors.
#[derive(Clone, Copy, Debug)]
pub struct PhysicalPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

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
    /// Above normal application windows.
    Floating,
    /// Below normal application windows.
    Bottom,
}

#[derive(Clone, Copy, Debug)]
pub struct WindowAttributes {
    pub position: PhysicalPosition,
    pub size: PhysicalSize,
    pub click_through: bool,
    /// The window may hold keyboard focus. This only sets eligibility. The crate never
    /// forces focus itself.
    pub focusable: bool,
}

/// Every method defaults to a no-op, so a consumer implements only the events it needs.
pub trait AuxiliaryWindowHandler {
    fn on_mouse_down(&mut self, _at: PhysicalPosition, _button: MouseButton) {}
    fn on_mouse_up(&mut self, _at: PhysicalPosition, _button: MouseButton) {}
    fn on_mouse_moved(&mut self, _at: PhysicalPosition) {}
    fn on_mouse_left(&mut self) {}

    fn on_redraw(&mut self) {}
    fn on_resized(&mut self, _size: PhysicalSize) {}
    fn on_scale_changed(&mut self, _scale: f32) {}
    fn on_close_requested(&mut self) {}

    fn on_display_changed(&mut self) {}
    fn on_work_area_changed(&mut self) {}

    /// A payload handed to this handler by `AuxiliaryWindow::deliver`. The crate treats it
    /// as opaque, so the handler downcasts it to its own message type.
    fn on_message(&mut self, _message: Box<dyn std::any::Any>) {}

    /// The entries to show when the tray icon's context menu opens. Called on each open,
    /// so it reflects current state.
    #[cfg(target_os = "windows")]
    fn tray_menu(&mut self) -> Vec<crate::MenuEntry> {
        Vec::new()
    }

    /// A tray context-menu row was chosen. `id` is the `MenuItem::id` of the row.
    #[cfg(target_os = "windows")]
    fn on_tray_menu_selected(&mut self, _id: u32) {}
}

/// The loop's own lifecycle, distinct from any window.
pub trait AuxiliaryLoopHandler {
    fn on_started(&mut self) {}
    fn on_stopping(&mut self) {}
    fn on_wake(&mut self) {}
}

/// The app's shell integration: the tray or menu-bar menu, and the shell's display
/// notifications. Every method defaults to a no-op.
pub trait AppShellHandler {
    /// The entries to show when the menu opens. Called on each open, so it reflects
    /// current state.
    fn menu(&mut self) -> Vec<MenuEntry> {
        Vec::new()
    }

    /// A menu row was chosen. `id` is the `MenuItem::id` of the row.
    fn on_menu_selected(&mut self, _id: u32) {}

    fn on_display_changed(&mut self) {}
    fn on_work_area_changed(&mut self) {}
}

/// The app's presence in the desktop shell, one type across platforms: the system-tray
/// icon on Windows and the menu-bar status item on macOS, each with a menu and the
/// shell's display notifications. On Windows the host window lives entirely inside this
/// type and is never exposed.
pub struct AppShell {
    inner: imp::AppShell,
}

impl AppShell {
    #[cfg(target_os = "macos")]
    pub fn new(icon_png: &[u8], handler: Box<dyn AppShellHandler>) -> anyhow::Result<Self> {
        Ok(Self {
            inner: imp::AppShell::new(icon_png, handler)?,
        })
    }

    #[cfg(target_os = "windows")]
    pub fn new(icon: HICON, handler: Box<dyn AppShellHandler>) -> anyhow::Result<Self> {
        Ok(Self {
            inner: imp::AppShell::new(icon, handler)?,
        })
    }

    pub fn set_tooltip(&self, tooltip: &str) {
        self.inner.set_tooltip(tooltip);
    }
}

/// A borderless auxiliary window, one type across platforms. Reach the native handle
/// through the platform extension trait (`AuxiliaryWindowExtWindows` on Windows).
pub struct AuxiliaryWindow {
    inner: imp::Window,
}

impl AuxiliaryWindow {
    pub fn new(
        attributes: &WindowAttributes,
        handler: Box<dyn AuxiliaryWindowHandler>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            inner: imp::Window::new(attributes, handler)?,
        })
    }

    /// Combined rather than split into position and size. Every caller places the
    /// window at a full rect, and this maps to one native call per platform.
    pub fn set_frame(&self, position: PhysicalPosition, size: PhysicalSize) {
        self.inner.set_frame(position, size);
    }

    pub fn set_visible(&self, visible: bool) {
        self.inner.set_visible(visible);
    }

    pub fn set_level(&self, level: WindowLevel) {
        self.inner.set_level(level);
    }

    /// Hands an opaque payload to this window's handler on the window thread, synchronously.
    /// The handler borrow lives and ends inside this call, so a caller cannot hold it across
    /// a window-mutating call such as `set_frame`.
    pub fn deliver(&self, message: Box<dyn std::any::Any>) {
        self.inner.deliver(message);
    }
}

/// The window event loop, one type across platforms. `run` blocks the calling thread
/// until the loop stops. Reach any platform-specific control through the platform
/// extension trait (`EventLoopExtMacOs` on macOS).
pub struct EventLoop {
    inner: imp::EventLoop,
}

impl EventLoop {
    pub fn new(handler: Box<dyn AuxiliaryLoopHandler>) -> Self {
        Self {
            inner: imp::EventLoop::new(handler),
        }
    }

    pub fn waker(&self) -> LoopWaker {
        LoopWaker {
            inner: self.inner.waker(),
        }
    }

    pub fn handle(&self) -> LoopHandle {
        LoopHandle {
            inner: self.inner.handle(),
        }
    }

    pub fn run(self) {
        self.inner.run();
    }
}

/// Controls the loop from the thread that owns it. Neither `Send` nor callable from a
/// foreign thread, a property the inner handle enforces by construction.
#[derive(Clone, Copy)]
pub struct LoopHandle {
    inner: imp::LoopHandle,
}

impl LoopHandle {
    pub fn terminate(&self) {
        self.inner.terminate();
    }
}

/// Wakes the loop from any thread. Carries no payload, so the consumer owns its own queue.
#[derive(Clone)]
pub struct LoopWaker {
    inner: imp::LoopWaker,
}

impl LoopWaker {
    pub fn wake(&self) {
        self.inner.wake();
    }
}
