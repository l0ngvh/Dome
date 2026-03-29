use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};

pub(crate) struct Taskbar(ITaskbarList);

impl Taskbar {
    pub(crate) fn new() -> windows::core::Result<Self> {
        unsafe {
            let list: ITaskbarList = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;
            list.HrInit()?;
            Ok(Self(list))
        }
    }

    pub(crate) fn add_tab(&self, hwnd: HWND) -> windows::core::Result<()> {
        unsafe { self.0.AddTab(hwnd) }
    }

    pub(crate) fn delete_tab(&self, hwnd: HWND) -> windows::core::Result<()> {
        unsafe { self.0.DeleteTab(hwnd) }
    }
}
