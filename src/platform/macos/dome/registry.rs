use std::collections::HashMap;
use std::sync::Arc;

use objc2_core_graphics::CGWindowID;

use crate::core::WindowId;

use super::super::accessibility::AXWindowApi;
use super::window::WindowState;

#[derive(Clone)]
pub(in crate::platform::macos) struct WindowEntry {
    pub(in crate::platform::macos) ax: Arc<dyn AXWindowApi>,
    pub(super) cg_id: CGWindowID,
    pub(super) window_id: WindowId,
    pub(super) app_name: Option<String>,
    pub(super) bundle_id: Option<String>,
    pub(super) title: Option<String>,
    pub(super) state: WindowState,
    pub(super) is_moving: bool,
}

impl std::fmt::Display for WindowEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}|{}:{}] {}",
            self.window_id,
            self.ax.pid(),
            self.ax.cg_id(),
            self.app_name.as_deref().unwrap_or("Unknown")
        )?;
        if let Some(bundle_id) = &self.bundle_id {
            write!(f, " ({bundle_id})")?;
        }
        if let Some(title) = &self.title {
            write!(f, " - {title}")?;
        }
        Ok(())
    }
}

/// Allow querying by CGWindowID for interaction between Dome and external events/UI, and by
/// WindowId for Dome internal handling after confirming the window exist
pub(super) struct Registry {
    windows: HashMap<CGWindowID, WindowEntry>,
    id_to_cg: HashMap<WindowId, CGWindowID>,
    pid_to_cg: HashMap<i32, Vec<CGWindowID>>,
}

impl Registry {
    pub(super) fn new() -> Self {
        Self {
            windows: HashMap::new(),
            id_to_cg: HashMap::new(),
            pid_to_cg: HashMap::new(),
        }
    }

    pub(super) fn get(&self, cg_id: CGWindowID) -> Option<&WindowEntry> {
        self.windows.get(&cg_id)
    }

    pub(super) fn get_mut(&mut self, cg_id: CGWindowID) -> Option<&mut WindowEntry> {
        self.windows.get_mut(&cg_id)
    }

    pub(super) fn contains(&self, cg_id: CGWindowID) -> bool {
        self.windows.contains_key(&cg_id)
    }

    pub(super) fn remove(&mut self, cg_id: CGWindowID) -> Option<WindowId> {
        let entry = self.windows.remove(&cg_id)?;
        let pid = entry.ax.pid();
        self.id_to_cg.remove(&entry.window_id);
        if let Some(ids) = self.pid_to_cg.get_mut(&pid) {
            ids.retain(|&id| id != cg_id);
            if ids.is_empty() {
                self.pid_to_cg.remove(&pid);
            }
        }
        tracing::info!(%entry, window_id = %entry.window_id, "Window removed");
        Some(entry.window_id)
    }

    pub(super) fn by_id(&self, window_id: WindowId) -> Option<&WindowEntry> {
        self.id_to_cg
            .get(&window_id)
            .and_then(|&cg_id| self.windows.get(&cg_id))
    }

    pub(super) fn by_id_mut(&mut self, window_id: WindowId) -> Option<&mut WindowEntry> {
        self.id_to_cg
            .get(&window_id)
            .copied()
            .and_then(|cg_id| self.windows.get_mut(&cg_id))
    }

    pub(super) fn for_pid(&self, pid: i32) -> impl Iterator<Item = (CGWindowID, &WindowEntry)> {
        self.pid_to_cg
            .get(&pid)
            .into_iter()
            .flatten()
            .filter_map(|&cg_id| Some((cg_id, self.windows.get(&cg_id)?)))
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (CGWindowID, &WindowEntry)> {
        self.windows.iter().map(|(&cg_id, w)| (cg_id, w))
    }

    pub(super) fn insert(
        &mut self,
        ax: Arc<dyn AXWindowApi>,
        window_id: WindowId,
        state: WindowState,
        app_name: Option<String>,
        bundle_id: Option<String>,
        title: Option<String>,
    ) {
        let cg_id = ax.cg_id();
        let pid = ax.pid();
        if pid as u32 == std::process::id() {
            return;
        }
        self.id_to_cg.insert(window_id, cg_id);
        self.pid_to_cg.entry(pid).or_default().push(cg_id);
        self.windows.insert(
            cg_id,
            WindowEntry {
                ax,
                cg_id,
                window_id,
                app_name,
                bundle_id,
                title,
                state,
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
}
