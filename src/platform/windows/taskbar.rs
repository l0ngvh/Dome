use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};

use crate::platform::windows::external::HwndId;

pub(super) trait ManageTaskbar {
    fn add_tab(&self, hwnd: HwndId);
    fn delete_tab(&self, hwnd: HwndId);
}

pub(crate) struct Taskbar(ITaskbarList);

impl Taskbar {
    pub(crate) fn new() -> windows::core::Result<Self> {
        unsafe {
            let list: ITaskbarList = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;
            list.HrInit()?;
            Ok(Self(list))
        }
    }
}

impl ManageTaskbar for Taskbar {
    fn add_tab(&self, hwnd: HwndId) {
        unsafe { self.0.AddTab(HWND::from(hwnd)) }.ok();
    }

    fn delete_tab(&self, hwnd: HwndId) {
        unsafe { self.0.DeleteTab(HWND::from(hwnd)) }.ok();
    }
}
