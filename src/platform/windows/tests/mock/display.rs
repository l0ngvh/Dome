use std::sync::{Arc, Mutex};

use crate::platform::windows::dome::{MonitorInfo, QueryDisplay};
use crate::platform::windows::external::HwndId;

pub(crate) struct MockDisplay {
    pub(crate) monitors: Arc<Mutex<Vec<MonitorInfo>>>,
    pub(crate) exclusive_fullscreen_hwnd: Arc<Mutex<Option<HwndId>>>,
}

impl QueryDisplay for MockDisplay {
    fn get_all_monitors(&self) -> anyhow::Result<Vec<MonitorInfo>> {
        Ok(self.monitors.lock().unwrap().clone())
    }

    fn get_exclusive_fullscreen_hwnd(&self) -> Option<HwndId> {
        *self.exclusive_fullscreen_hwnd.lock().unwrap()
    }
}
