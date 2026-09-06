use windows::Win32::UI::WindowsAndMessaging::HICON;

use super::Window;
use crate::{
    AppShellHandler, AuxiliaryWindowHandler, MenuEntry, PhysicalPosition, PhysicalSize,
    WindowAttributes,
};

/// Bridges the hidden window's `AuxiliaryWindowHandler` callbacks to the consumer's
/// `AppShellHandler`. The window's wnd-proc drives these, and it drops each `WindowState`
/// borrow before the modal tray menu, so forwarding here holds no borrow across the menu.
struct AppShellWindowHandler {
    handler: Box<dyn AppShellHandler>,
}

impl AuxiliaryWindowHandler for AppShellWindowHandler {
    fn on_display_changed(&mut self) {
        self.handler.on_display_changed();
    }

    fn on_work_area_changed(&mut self) {
        self.handler.on_work_area_changed();
    }

    fn tray_menu(&mut self) -> Vec<MenuEntry> {
        self.handler.menu()
    }

    fn on_tray_menu_selected(&mut self, id: u32) {
        self.handler.on_menu_selected(id);
    }
}

pub(crate) struct AppShell {
    // The hidden top-level window that carries the tray icon and receives display and
    // work-area messages. Its Drop removes the icon, then destroys the window.
    window: Window,
}

impl AppShell {
    pub(crate) fn new(icon: HICON, handler: Box<dyn AppShellHandler>) -> anyhow::Result<Self> {
        let attributes = WindowAttributes {
            position: PhysicalPosition { x: 0, y: 0 },
            size: PhysicalSize {
                width: 0,
                height: 0,
            },
            click_through: false,
            focusable: false,
        };
        let window = Window::new(&attributes, Box::new(AppShellWindowHandler { handler }))?;
        window.install_tray_icon(icon, "")?;
        Ok(Self { window })
    }

    pub(crate) fn set_tooltip(&self, tooltip: &str) {
        self.window.set_tray_tooltip(tooltip);
    }
}
