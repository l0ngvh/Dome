use std::collections::HashMap;

use windows::Win32::Foundation::HWND;

use crate::core::WindowId;
use crate::platform::windows::handle::{ManagedHwnd, WindowMode};

pub(super) struct WindowEntry {
    pub(super) hwnd: HWND,
    pub(super) mode: WindowMode,
    pub(super) title: Option<String>,
    pub(super) process: String,
}

pub(super) struct WindowRegistry {
    by_hwnd: HashMap<ManagedHwnd, WindowId>,
    by_id: HashMap<WindowId, WindowEntry>,
}

impl WindowRegistry {
    pub(super) fn new() -> Self {
        Self {
            by_hwnd: HashMap::new(),
            by_id: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, hwnd: ManagedHwnd, id: WindowId, entry: WindowEntry) {
        self.by_hwnd.insert(hwnd, id);
        self.by_id.insert(id, entry);
    }

    pub(super) fn remove_by_hwnd(&mut self, hwnd: ManagedHwnd) -> Option<WindowId> {
        let id = self.by_hwnd.remove(&hwnd)?;
        self.by_id.remove(&id);
        Some(id)
    }

    pub(super) fn get(&self, id: WindowId) -> Option<&WindowEntry> {
        self.by_id.get(&id)
    }

    pub(super) fn get_mut(&mut self, id: WindowId) -> Option<&mut WindowEntry> {
        self.by_id.get_mut(&id)
    }

    pub(super) fn get_id(&self, hwnd: ManagedHwnd) -> Option<WindowId> {
        self.by_hwnd.get(&hwnd).copied()
    }

    pub(super) fn contains_hwnd(&self, hwnd: ManagedHwnd) -> bool {
        self.by_hwnd.contains_key(&hwnd)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (ManagedHwnd, WindowId)> + '_ {
        self.by_hwnd.iter().map(|(&h, &id)| (h, id))
    }
}
