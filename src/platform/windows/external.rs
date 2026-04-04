use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{HWND_NOTOPMOST, HWND_TOPMOST};

use crate::core::Dimension;
/// Opaque window identity. Replaces `ManagedHwnd` throughout the codebase.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HwndId(isize);

impl std::fmt::Debug for HwndId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}

impl std::fmt::Display for HwndId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}

impl From<HWND> for HwndId {
    fn from(hwnd: HWND) -> Self {
        Self(hwnd.0 as isize)
    }
}

impl From<HwndId> for HWND {
    fn from(id: HwndId) -> Self {
        HWND(id.0 as *mut _)
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

/// Positioning + lifecycle operations on an external window.
/// Dome stores `Arc<dyn ManageExternalHwnd>` — no blocking reads.
pub(crate) trait ManageExternalHwnd: Send + Sync {
    fn id(&self) -> HwndId;
    fn should_float(&self) -> bool;
    fn get_dimension(&self) -> Dimension;
    fn get_monitor_handle(&self) -> Option<isize>;
    fn is_iconic(&self) -> bool;
    fn set_position(&self, z: ZOrder, x: i32, y: i32, cx: i32, cy: i32);
    fn move_offscreen(&self);
    fn show_cmd(&self, cmd: ShowCmd);
    fn set_foreground_window(&self);
    fn is_maximized(&self) -> bool;
    fn recover(&self, dim: Dimension, was_maximized: bool);
}

/// Blocking reads on an external window (SendMessageTimeoutW).
/// Used only by the read dispatcher — never called on the dome thread.
pub(crate) trait InspectExternalHwnd: Send + Sync {
    fn is_manageable(&self) -> bool;
    fn get_window_title(&self) -> Option<String>;
    fn get_process_name(&self) -> anyhow::Result<String>;
    fn get_size_constraints(&self) -> (f32, f32, f32, f32);
}
