use std::collections::HashMap;
use std::sync::Arc;

use objc2_core_graphics::CGWindowID;

use crate::core::WindowId;

use super::super::accessibility::ExternalWindow;
use super::NewWindow;
use super::window::WindowState;

#[derive(Clone)]
pub(in crate::platform::macos) struct ManagedWindow {
    pub(in crate::platform::macos) ext: Arc<dyn ExternalWindow>,
    pub(super) cg_id: CGWindowID,
    pub(in crate::platform::macos) window_id: WindowId,
    pub(super) state: WindowState,
    pub(super) is_minimized: bool,
    pub(super) is_moving: bool,
}

impl std::fmt::Display for ManagedWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[id={}] {}", self.window_id, self.ext)
    }
}

/// Allow querying by CGWindowID for interaction between Dome and external events/UI, and by
/// WindowId for Dome internal handling after confirming the window exist
pub(super) struct WindowRegistry {
    windows: HashMap<CGWindowID, ManagedWindow>,
    id_to_cg: HashMap<WindowId, CGWindowID>,
    pid_to_cg: HashMap<i32, Vec<CGWindowID>>,
}

impl WindowRegistry {
    pub(super) fn new() -> Self {
        Self {
            windows: HashMap::new(),
            id_to_cg: HashMap::new(),
            pid_to_cg: HashMap::new(),
        }
    }

    pub(super) fn get(&self, cg_id: CGWindowID) -> Option<&ManagedWindow> {
        self.windows.get(&cg_id)
    }

    pub(super) fn get_mut(&mut self, cg_id: CGWindowID) -> Option<&mut ManagedWindow> {
        self.windows.get_mut(&cg_id)
    }

    pub(super) fn contains(&self, cg_id: CGWindowID) -> bool {
        self.windows.contains_key(&cg_id)
    }

    pub(super) fn remove(&mut self, cg_id: CGWindowID) -> Option<WindowId> {
        let entry = self.windows.remove(&cg_id)?;
        let pid = entry.ext.pid();
        self.id_to_cg.remove(&entry.window_id);
        if let Some(ids) = self.pid_to_cg.get_mut(&pid) {
            ids.retain(|&id| id != cg_id);
            if ids.is_empty() {
                self.pid_to_cg.remove(&pid);
            }
        }
        tracing::info!(window_id = %entry.window_id, "Window removed");
        Some(entry.window_id)
    }

    pub(super) fn by_id(&self, window_id: WindowId) -> Option<&ManagedWindow> {
        self.id_to_cg
            .get(&window_id)
            .and_then(|&cg_id| self.windows.get(&cg_id))
    }

    pub(super) fn by_id_mut(&mut self, window_id: WindowId) -> Option<&mut ManagedWindow> {
        self.id_to_cg
            .get(&window_id)
            .copied()
            .and_then(|cg_id| self.windows.get_mut(&cg_id))
    }

    pub(super) fn for_pid(&self, pid: i32) -> impl Iterator<Item = (CGWindowID, &ManagedWindow)> {
        self.pid_to_cg
            .get(&pid)
            .into_iter()
            .flatten()
            .filter_map(|&cg_id| Some((cg_id, self.windows.get(&cg_id)?)))
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (CGWindowID, &ManagedWindow)> {
        self.windows.iter().map(|(&cg_id, w)| (cg_id, w))
    }

    pub(super) fn insert(&mut self, new: NewWindow, window_id: WindowId, state: WindowState) {
        let NewWindow { ax, metadata: _ } = new;
        let cg_id = ax.cg_id();
        let pid = ax.pid();
        if pid as u32 == std::process::id() {
            return;
        }
        self.id_to_cg.insert(window_id, cg_id);
        self.pid_to_cg.entry(pid).or_default().push(cg_id);
        self.windows.insert(
            cg_id,
            ManagedWindow {
                ext: ax,
                cg_id,
                window_id,
                state,
                is_minimized: false,
                is_moving: false,
            },
        );
    }

    pub(super) fn set_pid_moving(&mut self, pid: i32, moving: bool) {
        if let Some(cg_ids) = self.pid_to_cg.get(&pid) {
            for &cg_id in cg_ids {
                if let Some(entry) = self.windows.get_mut(&cg_id) {
                    entry.is_moving = moving;
                }
            }
        }
    }

    /// Replaces the `ext` handle of an existing tracked entry. Returns true if
    /// the entry existed.
    pub(super) fn replace_ext(&mut self, cg_id: CGWindowID, ext: Arc<dyn ExternalWindow>) -> bool {
        let Some(entry) = self.windows.get_mut(&cg_id) else {
            return false;
        };
        let old_pid = entry.ext.pid();
        let new_pid = ext.pid();
        if old_pid != new_pid {
            // This shouldn't happen, like at all. But if it happens, the worst is that we have 1
            // window leak, which is not that significant
            tracing::warn!(%cg_id, old_pid, new_pid, "Window has a different pid than tracked");
        }
        entry.ext = ext;
        true
    }
}
