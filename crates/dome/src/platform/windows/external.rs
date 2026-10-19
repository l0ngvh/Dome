use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{HWND_NOTOPMOST, HWND_TOPMOST};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum NativeWindowOrder {
    Topmost,
    /// Clears WS_EX_TOPMOST, dropping the window out of the topmost band.
    /// Placing self below a non-topmost reference does not, by itself, clear
    /// the flag; only HWND_NOTOPMOST and HWND_BOTTOM are documented to do so.
    NotTopmost,
    After(HwndId),
    Unchanged,
}

impl From<NativeWindowOrder> for Option<HWND> {
    fn from(z: NativeWindowOrder) -> Self {
        match z {
            NativeWindowOrder::Topmost => Some(HWND_TOPMOST),
            NativeWindowOrder::NotTopmost => Some(HWND_NOTOPMOST),
            NativeWindowOrder::After(id) => Some(HWND(id.0 as *mut _)),
            NativeWindowOrder::Unchanged => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ShowCmd {
    Restore,
    Minimize,
}
