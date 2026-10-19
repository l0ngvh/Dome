use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};

use crate::platform::windows::external::HwndId;

pub(super) struct Taskbar(ITaskbarList);

impl Taskbar {
    pub(super) fn new() -> windows::core::Result<Self> {
        unsafe {
            let list: ITaskbarList = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;
            list.HrInit()?;
            Ok(Self(list))
        }
    }
}

impl Taskbar {
    pub(super) fn add_tab(&self, hwnd: HwndId) {
        if let Err(e) = unsafe { self.0.AddTab(HWND::from(hwnd)) } {
            tracing::trace!(?hwnd, "ITaskbarList::AddTab failed: {e}");
        }
    }

    pub(super) fn delete_tab(&self, hwnd: HwndId) {
        if let Err(e) = unsafe { self.0.DeleteTab(HWND::from(hwnd)) } {
            tracing::trace!(?hwnd, "ITaskbarList::DeleteTab failed: {e}");
        }
    }
}
