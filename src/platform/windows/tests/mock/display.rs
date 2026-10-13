use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::platform::windows::dome::{MonitorInfo, QueryDisplay};

pub(crate) struct MockDisplay {
    pub(crate) failed: Arc<AtomicBool>,
    pub(crate) monitors: Arc<Mutex<Vec<MonitorInfo>>>,
}

impl QueryDisplay for MockDisplay {
    fn get_all_monitors(&self) -> anyhow::Result<Vec<MonitorInfo>> {
        anyhow::ensure!(
            !self.failed.load(Ordering::Relaxed),
            "scripted display failure"
        );
        Ok(self.monitors.lock().unwrap().clone())
    }
}
