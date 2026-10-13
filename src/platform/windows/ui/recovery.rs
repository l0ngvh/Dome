use crate::platform::windows::external::HwndId;
use crate::platform::windows::window_server::RecoveryEntry;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Recovery {
    entries: HashMap<HwndId, bool>,
}
impl Recovery {
    pub(super) fn track(&mut self, key: HwndId, is_maximized: bool) {
        self.entries.entry(key).or_insert(is_maximized);
    }
    pub(super) fn remove(&mut self, key: HwndId) {
        self.entries.remove(&key);
    }
    pub(super) fn take(&mut self) -> Vec<RecoveryEntry> {
        self.entries
            .drain()
            .map(|(window, is_maximized)| RecoveryEntry {
                window,
                is_maximized,
            })
            .collect()
    }
}
