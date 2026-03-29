use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{HWND_NOTOPMOST, HWND_TOPMOST};

use crate::core::Dimension;
use crate::platform::windows::handle::WindowMode;
use crate::platform::windows::taskbar::Taskbar;

/// Opaque window identity. Replaces `ManagedHwnd` throughout the codebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HwndId(isize);

impl From<HWND> for HwndId {
    fn from(hwnd: HWND) -> Self {
        Self(hwnd.0 as isize)
    }
}

#[cfg(test)]
impl HwndId {
    pub(crate) fn test(id: isize) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ZOrder {
    Topmost,
    NotTopmost,
    After(HwndId),
    Unchanged,
}

impl From<ZOrder> for Option<HWND> {
    fn from(z: ZOrder) -> Self {
        match z {
            ZOrder::Topmost => Some(HWND_TOPMOST),
            ZOrder::NotTopmost => Some(HWND_NOTOPMOST),
            ZOrder::After(id) => Some(HWND(id.0 as *mut _)),
            ZOrder::Unchanged => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ShowCmd {
    Restore,
    Minimize,
}

/// Abstracts all Win32 operations on an external window.
///
/// Inspection methods are called by Dome (hub thread) when processing events.
/// Positioning methods are called by Wm (UI thread) when applying layout.
/// Both hold `Arc<dyn ManageExternalHwnd>` — hence `Send + Sync`.
pub(crate) trait ManageExternalHwnd: Send + Sync {
    fn id(&self) -> HwndId;

    // --- Inspection (Dome) ---
    fn is_manageable(&self) -> bool;
    fn get_window_title(&self) -> Option<String>;
    fn get_process_name(&self) -> anyhow::Result<String>;
    fn initial_window_mode(&self, monitor: Option<&Dimension>) -> WindowMode;
    fn get_dimension(&self) -> Dimension;
    fn get_size_constraints(&self) -> (f32, f32, f32, f32);
    fn get_monitor_handle(&self) -> Option<isize>;

    // --- Positioning (Wm) ---
    fn is_iconic(&self) -> bool;
    fn set_position(&self, z: ZOrder, x: i32, y: i32, cx: i32, cy: i32);
    fn move_offscreen(&self);
    fn show_cmd(&self, cmd: ShowCmd);
    fn set_foreground_window(&self);

    // --- Taskbar ---
    fn add_to_taskbar(&self, taskbar: &Taskbar);
    fn remove_from_taskbar(&self, taskbar: &Taskbar);

    // --- Recovery ---
    fn is_maximized(&self) -> bool;
    fn recover(&self, dim: Dimension, was_maximized: bool);
}
