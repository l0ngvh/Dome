use std::collections::HashMap;

use dome_core::{FractionalLength, Logical, WindowId};

use super::super::window_server::WindowServer;
use super::window::WindowState;

#[derive(Clone)]
pub(in crate::platform::macos) struct ManagedWindow {
    pub(in crate::platform::macos) pid: i32,
    pub(in crate::platform::macos) window_id: WindowId,
    pub(super) state: WindowState,
    pub(super) is_minimized: bool,
    pub(super) is_moving: bool,
    /// Predicted from the close button, because macOS has no public API for the real radius.
    pub(in crate::platform::macos) corner_radius: FractionalLength<Logical>,
}

impl ManagedWindow {
    pub(in crate::platform::macos) fn is_minimized(&self) -> bool {
        self.is_minimized || matches!(self.state, WindowState::BorderlessMinimized { .. })
    }

    pub(in crate::platform::macos) fn is_native_fullscreen(&self) -> bool {
        matches!(self.state, WindowState::NativeFullscreen)
    }
}

impl std::fmt::Display for ManagedWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[id={} pid={}]", self.window_id, self.pid)
    }
}

pub(in crate::platform::macos) struct WindowRegistry {
    windows: HashMap<WindowId, ManagedWindow>,
    pid_to_windows: HashMap<i32, Vec<WindowId>>,
}

impl WindowRegistry {
    pub(super) fn new() -> Self {
        Self {
            windows: HashMap::new(),
            pid_to_windows: HashMap::new(),
        }
    }

    pub(super) fn get(&self, window_id: WindowId) -> Option<&ManagedWindow> {
        self.windows.get(&window_id)
    }

    pub(super) fn get_mut(&mut self, window_id: WindowId) -> Option<&mut ManagedWindow> {
        self.windows.get_mut(&window_id)
    }

    pub(super) fn contains(&self, window_id: WindowId) -> bool {
        self.windows.contains_key(&window_id)
    }

    pub(super) fn remove(&mut self, window_id: WindowId) -> Option<WindowId> {
        let entry = self.windows.remove(&window_id)?;
        let pid = entry.pid;
        if let Some(ids) = self.pid_to_windows.get_mut(&pid) {
            ids.retain(|&id| id != window_id);
            if ids.is_empty() {
                self.pid_to_windows.remove(&pid);
            }
        }
        tracing::info!(window_id = %entry.window_id, "Window removed");
        Some(entry.window_id)
    }

    pub(in crate::platform::macos) fn close_window(
        &self,
        window_id: WindowId,
        server: &dyn WindowServer,
    ) -> anyhow::Result<()> {
        let Some(window) = self.get(window_id) else {
            return Ok(());
        };
        server.close(window.window_id)
    }

    #[tracing::instrument(skip(self, server), fields(window_id = %window_id))]
    pub(in crate::platform::macos) fn unminimize_window(
        &self,
        window_id: WindowId,
        server: &dyn WindowServer,
    ) -> anyhow::Result<()> {
        let Some(window) = self.get(window_id) else {
            return Ok(());
        };
        if !window.is_minimized {
            return Ok(());
        }
        server.unminimize(window.window_id)
    }

    pub(super) fn for_pid(&self, pid: i32) -> impl Iterator<Item = (WindowId, &ManagedWindow)> {
        self.pid_to_windows
            .get(&pid)
            .into_iter()
            .flatten()
            .filter_map(|&window_id| Some((window_id, self.windows.get(&window_id)?)))
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (WindowId, &ManagedWindow)> {
        self.windows.iter().map(|(&window_id, w)| (window_id, w))
    }

    pub(super) fn insert(
        &mut self,
        window_id: WindowId,
        pid: i32,
        state: WindowState,
        corner_radius: FractionalLength<Logical>,
    ) {
        if pid as u32 == std::process::id() {
            return;
        }
        self.pid_to_windows.entry(pid).or_default().push(window_id);
        self.windows.insert(
            window_id,
            ManagedWindow {
                pid,
                window_id,
                state,
                is_minimized: false,
                is_moving: false,
                corner_radius,
            },
        );
    }

    pub(super) fn set_pid_moving(&mut self, pid: i32, moving: bool) {
        if let Some(window_ids) = self.pid_to_windows.get(&pid) {
            for &window_id in window_ids {
                if let Some(entry) = self.windows.get_mut(&window_id) {
                    entry.is_moving = moving;
                }
            }
        }
    }
}
