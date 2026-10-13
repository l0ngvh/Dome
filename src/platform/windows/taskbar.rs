use std::collections::HashSet;
use std::rc::Rc;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};

use crate::platform::windows::external::HwndId;

pub(super) trait ManageTaskbar {
    fn add_tab(&self, hwnd: HwndId);
    fn delete_tab(&self, hwnd: HwndId);
}

pub(super) struct TaskbarRestoreGuard {
    taskbar: Rc<dyn ManageTaskbar>,
    live: HashSet<HwndId>,
}

impl TaskbarRestoreGuard {
    pub(super) fn new(taskbar: Rc<dyn ManageTaskbar>) -> Self {
        Self {
            taskbar,
            live: HashSet::new(),
        }
    }
    pub(super) fn track(&mut self, key: HwndId) {
        self.live.insert(key);
    }
    pub(super) fn remove(&mut self, key: HwndId) {
        self.live.remove(&key);
        self.taskbar.delete_tab(key);
    }
}
impl ManageTaskbar for TaskbarRestoreGuard {
    fn add_tab(&self, key: HwndId) {
        self.taskbar.add_tab(key);
    }
    fn delete_tab(&self, key: HwndId) {
        self.taskbar.delete_tab(key);
    }
}
impl Drop for TaskbarRestoreGuard {
    fn drop(&mut self) {
        for &key in &self.live {
            self.taskbar.add_tab(key);
        }
    }
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
        if let Err(e) = unsafe { self.0.AddTab(HWND::from(hwnd)) } {
            tracing::trace!(?hwnd, "ITaskbarList::AddTab failed: {e}");
        }
    }

    fn delete_tab(&self, hwnd: HwndId) {
        if let Err(e) = unsafe { self.0.DeleteTab(HWND::from(hwnd)) } {
            tracing::trace!(?hwnd, "ITaskbarList::DeleteTab failed: {e}");
        }
    }
}
