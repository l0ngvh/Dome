//! Window creation and the window event loop for Dome and its status-bar
//! subprocess. Names no Dome domain type.

use std::marker::PhantomData;

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
pub use windows::AuxiliaryWindowExtWindows;

/// Logical points. Physical pixels divided by the display scale factor.
pub enum Logical {}

/// Physical device pixels.
pub enum Physical {}

/// The platform's native window coordinate unit. macOS window APIs speak logical points,
/// Windows speaks physical pixels. Events and frames arrive in this unit, and the crate
/// applies no scale conversion of its own.
#[cfg(target_os = "macos")]
pub type NativeUnit = Logical;
#[cfg(target_os = "windows")]
pub type NativeUnit = Physical;

/// The origin may be negative across multiple monitors.
pub struct Point<U> {
    x: i32,
    y: i32,
    _marker: PhantomData<U>,
}

pub struct Size<U> {
    width: u32,
    height: u32,
    _marker: PhantomData<U>,
}

impl<U> Point<U> {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            _marker: PhantomData,
        }
    }
    pub fn x(&self) -> i32 {
        self.x
    }
    pub fn y(&self) -> i32 {
        self.y
    }
}

impl<U> Size<U> {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            _marker: PhantomData,
        }
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Point<Physical> {
    /// Rounds to the nearest whole logical point. Only a physical point exposes this, so a
    /// logical point cannot be scaled twice.
    pub fn to_logical(self, scale: f32) -> Point<Logical> {
        Point::new(
            (self.x as f32 / scale).round() as i32,
            (self.y as f32 / scale).round() as i32,
        )
    }
}

impl<U> Clone for Point<U> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<U> Copy for Point<U> {}
impl<U> std::fmt::Debug for Point<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Point")
            .field("x", &self.x)
            .field("y", &self.y)
            .finish()
    }
}

impl<U> Clone for Size<U> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<U> Copy for Size<U> {}
impl<U> std::fmt::Debug for Size<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Size")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
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
    pub position: Point<NativeUnit>,
    pub size: Size<NativeUnit>,
    pub click_through: bool,
    /// The window may hold keyboard focus. This only sets eligibility. The crate never
    /// forces focus itself.
    pub focusable: bool,
}

/// Every method defaults to a no-op, so a consumer implements only the events it needs.
pub trait AuxiliaryWindowHandler {
    fn on_mouse_down(&mut self, _at: Point<NativeUnit>, _button: MouseButton) {}
    fn on_mouse_up(&mut self, _at: Point<NativeUnit>, _button: MouseButton) {}
    fn on_mouse_moved(&mut self, _at: Point<NativeUnit>) {}

    fn on_redraw(&mut self) {}
    fn on_resized(&mut self, _size: Size<NativeUnit>) {}
    fn on_scale_changed(&mut self, _scale: f32) {}
    fn on_close_requested(&mut self) {}

    fn on_display_changed(&mut self) {}
    fn on_work_area_changed(&mut self) {}

    /// A payload handed to this handler by `AuxiliaryWindow::deliver`. The crate treats it
    /// as opaque, so the handler downcasts it to its own message type.
    fn on_message(&mut self, _message: Box<dyn std::any::Any>) {}
}

/// The app's whole main-thread lifecycle in one handler: the loop lifecycle, plus the
/// shell's menu and display notifications. The menu is pulled on each open. Every method
/// defaults to a no-op.
pub trait AppHandler {
    fn on_started(&mut self, _shell: &Shell) {}
    fn on_stopping(&mut self) {}
    fn on_wake(&mut self, _shell: &Shell) {}
    fn menu(&mut self) -> Vec<MenuEntry> {
        Vec::new()
    }
    fn on_menu_selected(&mut self, _id: u32) {}
    fn on_display_changed(&mut self) {}
    fn on_work_area_changed(&mut self) {}
}

/// A menu-bar or tray icon, built before the loop runs. macOS decodes template PNG bytes.
/// Windows loads a compiled icon resource by id.
pub struct Icon {
    inner: imp::Icon,
}

impl Icon {
    #[cfg(target_os = "macos")]
    pub fn from_png(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Self {
            inner: imp::Icon::from_png(bytes)?,
        })
    }

    #[cfg(target_os = "windows")]
    pub fn from_resource_id(id: u16) -> anyhow::Result<Self> {
        Ok(Self {
            inner: imp::Icon::from_resource_id(id)?,
        })
    }
}

/// A live handle to the app's shell presence, valid only for the callback it is passed to.
/// The consumer reads it there and does not store it.
pub struct Shell {
    inner: imp::Shell,
}

impl Shell {
    pub(crate) fn new(inner: imp::Shell) -> Self {
        Self { inner }
    }

    pub fn set_tooltip(&self, tooltip: &str) {
        self.inner.set_tooltip(tooltip);
    }
}

/// The app: the event loop that owns the main thread, holding the desktop-shell presence
/// (the menu-bar status item on macOS, the system-tray icon on Windows). `run` blocks the
/// calling thread until the loop stops.
pub struct App {
    inner: imp::App,
}

impl App {
    pub fn new(icon: Icon, handler: Box<dyn AppHandler>) -> anyhow::Result<Self> {
        Ok(Self {
            inner: imp::App::new(icon.inner, handler)?,
        })
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
    pub fn set_frame(&self, position: Point<NativeUnit>, size: Size<NativeUnit>) {
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
