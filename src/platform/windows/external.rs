use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{HWND_NOTOPMOST, HWND_TOPMOST};

use crate::core::LimitObservation;

/// Opaque window identity.
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

pub(crate) trait ManageOverlay: Send + Sync {
    fn set_z_order(&self, z: ZOrder);
    fn focus(&self);
}

/// Blocking reads on an external window (`SendMessageTimeout`-style calls
/// that may stall on hung apps). Constructed fresh inside dispatcher closures
/// from a bare `HwndId` because `ExternalHwnd` is a thin HWND wrapper with no
/// per-window state. The macOS `ExternalWindow` trait collapses reads and
/// writes because `AXWindow` caches an `AXUIElement` and cannot be cheaply
/// re-fabricated.
pub(crate) trait InspectExternalWindow: Send + Sync {
    fn process_id(&self) -> u32;
    fn is_maximized(&self) -> bool;
    fn check_unmanageable(&self) -> bool;
    /// Live `IsIconic` query against the OS.
    fn is_minimized(&self) -> bool;
    fn get_window_title(&self) -> Option<String>;
    fn get_process_name(&self) -> anyhow::Result<String>;
    /// Sibling of `get_process_name`. Returns the full path from
    /// `QueryFullProcessImageNameW` without splitting the basename off.
    fn get_process_path(&self) -> anyhow::Result<String>;
    /// None means the native read failed, not that the app cleared its limits.
    fn get_size_constraints(&self) -> Option<LimitObservation>;
    fn get_app_display_name(&self) -> Option<String>;
    /// Native OS monitor ownership. Non-blocking; safe on external HWNDs.
    fn get_monitor(&self) -> isize;
    fn get_class_name(&self) -> Option<String>;
    fn get_aumid(&self) -> Option<String>;
}
