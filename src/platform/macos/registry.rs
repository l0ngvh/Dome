use std::collections::HashMap;

use objc2_core_graphics::CGWindowID;

use crate::core::{Window, WindowId};

use super::accessibility::AXWindow;
use super::dome::MessageSender;
use super::monitor::MonitorInfo;
use super::window::MacWindow;

pub(super) struct Registry {
    windows: HashMap<CGWindowID, MacWindow>,
    id_to_cg: HashMap<WindowId, CGWindowID>,
    pid_to_cg: HashMap<i32, Vec<CGWindowID>>,
    monitors: Vec<MonitorInfo>,
    sender: MessageSender,
}

impl Registry {
    pub(super) fn new(monitors: Vec<MonitorInfo>, sender: MessageSender) -> Self {
        Self {
            windows: HashMap::new(),
            id_to_cg: HashMap::new(),
            pid_to_cg: HashMap::new(),
            monitors,
            sender,
        }
    }

    pub(super) fn get(&self, cg_id: CGWindowID) -> Option<&MacWindow> {
        self.windows.get(&cg_id)
    }

    pub(super) fn get_mut(&mut self, cg_id: CGWindowID) -> Option<&mut MacWindow> {
        self.windows.get_mut(&cg_id)
    }

    pub(super) fn contains(&self, cg_id: CGWindowID) -> bool {
        self.windows.contains_key(&cg_id)
    }

    pub(super) fn remove(&mut self, cg_id: CGWindowID) -> Option<WindowId> {
        let window = self.windows.remove(&cg_id)?;
        let pid = window.pid();
        self.id_to_cg.remove(&window.window_id());
        if let Some(ids) = self.pid_to_cg.get_mut(&pid) {
            ids.retain(|&id| id != cg_id);
            if ids.is_empty() {
                self.pid_to_cg.remove(&pid);
            }
        }
        tracing::info!(%window, window_id = %window.window_id(), "Window removed");
        Some(window.window_id())
    }

    pub(super) fn by_id(&self, window_id: WindowId) -> Option<&MacWindow> {
        self.id_to_cg
            .get(&window_id)
            .and_then(|&cg_id| self.windows.get(&cg_id))
    }

    pub(super) fn by_id_mut(&mut self, window_id: WindowId) -> Option<&mut MacWindow> {
        self.id_to_cg
            .get(&window_id)
            .copied()
            .and_then(|cg_id| self.windows.get_mut(&cg_id))
    }

    pub(super) fn for_pid(&self, pid: i32) -> impl Iterator<Item = (CGWindowID, &MacWindow)> {
        self.pid_to_cg
            .get(&pid)
            .into_iter()
            .flatten()
            .filter_map(|&cg_id| Some((cg_id, self.windows.get(&cg_id)?)))
    }

    pub(super) fn remove_by_pid(&mut self, pid: i32) -> Vec<(CGWindowID, WindowId)> {
        let Some(cg_ids) = self.pid_to_cg.remove(&pid) else {
            return Vec::new();
        };
        let mut removed = Vec::new();
        for cg_id in cg_ids {
            if let Some(window) = self.windows.remove(&cg_id) {
                self.id_to_cg.remove(&window.window_id());
                tracing::info!(%window, window_id = %window.window_id(), "Window removed");
                removed.push((cg_id, window.window_id()));
            }
        }
        removed
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (CGWindowID, &MacWindow)> {
        self.windows.iter().map(|(&cg_id, w)| (cg_id, w))
    }

    pub(super) fn insert(&mut self, ax: AXWindow, window_id: WindowId, hub_window: &Window) {
        let cg_id = ax.cg_id();
        let pid = ax.pid();
        if pid as u32 == std::process::id() {
            return;
        }
        self.id_to_cg.insert(window_id, cg_id);
        self.pid_to_cg.entry(pid).or_default().push(cg_id);
        self.windows.insert(
            cg_id,
            MacWindow::new(ax, window_id, hub_window, self.sender.clone(), self.monitors.clone()),
        );
    }

    pub(super) fn set_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        self.monitors = monitors.clone();
        for window in self.windows.values_mut() {
            window.on_monitor_change(monitors.clone());
        }
    }
}
