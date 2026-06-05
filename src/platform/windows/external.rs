use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{HWND_NOTOPMOST, HWND_TOPMOST};

use crate::core::{Dimension, Physical};

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
    pub(crate) const fn test(id: isize) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ZOrder {
    Topmost,
    /// Clears WS_EX_TOPMOST, dropping the window out of the topmost band.
    /// Placing self below a non-topmost reference does not, by itself, clear
    /// the flag; only HWND_NOTOPMOST and HWND_BOTTOM are documented to do so.
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

/// Non-blocking writes on an external window (HWND messages that complete
/// synchronously without round-tripping through another process). Stored as
/// `Arc<dyn ManageExternalWindow>` inside `ManagedWindow`; called from the dome
/// thread.
pub(crate) trait ManageExternalWindow: Send + Sync {
    fn id(&self) -> HwndId;
    fn pid(&self) -> u32;
    fn should_float(&self) -> bool;
    fn set_position(&self, z: ZOrder, dim: Dimension<Physical>);
    fn move_offscreen(&self);
    fn show_cmd(&self, cmd: ShowCmd);
    fn set_foreground_window(&self);
    fn is_maximized(&self) -> bool;
    fn recover(&self, was_maximized: bool);
}

/// Blocking reads on an external window (`SendMessageTimeout`-style calls
/// that may stall on hung apps). Constructed fresh inside dispatcher closures
/// from a bare `HwndId` because `ExternalHwnd` is a thin HWND wrapper with no
/// per-window state. The macOS `ExternalWindow` trait collapses reads and
/// writes because `AXWindow` caches an `AXUIElement` and cannot be cheaply
/// re-fabricated.
pub(crate) trait InspectExternalWindow: Send + Sync {
    fn is_manageable(&self) -> bool;
    /// Live `IsIconic` query against the OS.
    fn is_minimized(&self) -> bool;
    fn get_window_title(&self) -> Option<String>;
    fn get_process_name(&self) -> anyhow::Result<String>;
    fn get_size_constraints(&self) -> (f32, f32, f32, f32);
    /// Returns the visible frame bounds excluding invisible window borders,
    /// in physical pixels. Same coordinate space as `set_position`.
    fn get_visible_rect(&self) -> Dimension<Physical>;
    fn get_app_display_name(&self) -> Option<String>;
    /// Native OS monitor ownership. Non-blocking; safe on external HWNDs.
    fn get_monitor(&self) -> isize;
}
