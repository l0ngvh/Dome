use std::collections::HashMap;
use std::sync::Arc;

use crate::core::WindowId;
use crate::platform::windows::external::{HwndId, ManageExternalWindow};

use super::window::WindowState;

pub(super) struct ManagedWindow {
    pub(super) ext: Arc<dyn ManageExternalWindow>,
    pub(super) state: WindowState,
    pub(super) is_minimized: bool,
}

pub(super) struct WindowRegistry {
    by_hwnd: HashMap<HwndId, WindowId>,
    by_id: HashMap<WindowId, ManagedWindow>,
}

impl WindowRegistry {
    pub(super) fn new() -> Self {
        Self {
            by_hwnd: HashMap::new(),
            by_id: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, id: HwndId, window_id: WindowId, entry: ManagedWindow) {
        self.by_hwnd.insert(id, window_id);
        self.by_id.insert(window_id, entry);
    }

    pub(super) fn remove_by_hwnd(&mut self, id: HwndId) -> Option<WindowId> {
        let window_id = self.by_hwnd.remove(&id)?;
        self.by_id.remove(&window_id);
        Some(window_id)
    }

    pub(super) fn get(&self, id: WindowId) -> Option<&ManagedWindow> {
        self.by_id.get(&id)
    }

    pub(super) fn get_mut(&mut self, id: WindowId) -> Option<&mut ManagedWindow> {
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
