use std::collections::HashMap;
use std::sync::Arc;

use crate::core::WindowId;
use crate::platform::windows::external::{HwndId, ManageExternalHwnd};
use crate::platform::windows::handle::WindowMode;

pub(super) struct WindowEntry {
    pub(super) ext: Arc<dyn ManageExternalHwnd>,
    pub(super) mode: WindowMode,
    pub(super) title: Option<String>,
    pub(super) process: String,
}

pub(super) struct WindowRegistry {
    by_hwnd: HashMap<HwndId, WindowId>,
    by_id: HashMap<WindowId, WindowEntry>,
}

impl WindowRegistry {
    pub(super) fn new() -> Self {
        Self {
            by_hwnd: HashMap::new(),
            by_id: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, id: HwndId, window_id: WindowId, entry: WindowEntry) {
        self.by_hwnd.insert(id, window_id);
        self.by_id.insert(window_id, entry);
    }

    pub(super) fn remove_by_hwnd(&mut self, id: HwndId) -> Option<WindowId> {
        let window_id = self.by_hwnd.remove(&id)?;
        self.by_id.remove(&window_id);
        Some(window_id)
    }

    pub(super) fn get(&self, id: WindowId) -> Option<&WindowEntry> {
        self.by_id.get(&id)
    }

    pub(super) fn get_mut(&mut self, id: WindowId) -> Option<&mut WindowEntry> {
        self.by_id.get_mut(&id)
    }

    pub(super) fn get_id(&self, id: HwndId) -> Option<WindowId> {
        self.by_hwnd.get(&id).copied()
    }

    pub(super) fn contains_hwnd(&self, id: HwndId) -> bool {
        self.by_hwnd.contains_key(&id)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (HwndId, WindowId)> + '_ {
        self.by_hwnd.iter().map(|(&h, &id)| (h, id))
    }
}
