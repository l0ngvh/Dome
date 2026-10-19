use std::collections::HashMap;

use crate::platform::windows::external::ShowCmd;
use dome_core::WindowId;

use super::window::WindowState;

pub(super) struct ManagedWindow {
    pub(super) state: WindowState,
    pub(super) is_minimized: bool,
    /// Monitor the window last settled on. A move to a different one triggers a
    /// size-constraint re-read, because WM_GETMINMAXINFO answers in its units.
    pub(super) monitor: isize,
}

pub(super) struct WindowRegistry {
    by_id: HashMap<WindowId, ManagedWindow>,
}

impl WindowRegistry {
    pub(super) fn new() -> Self {
        Self {
            by_id: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, window_id: WindowId, entry: ManagedWindow) {
        self.by_id.insert(window_id, entry);
    }

    pub(super) fn remove(&mut self, id: WindowId) -> Option<ManagedWindow> {
        self.by_id.remove(&id)
    }

    pub(super) fn get(&self, id: WindowId) -> Option<&ManagedWindow> {
        self.by_id.get(&id)
    }

    pub(super) fn close_window(
        &self,
        window_id: WindowId,
        server: &dyn super::super::window_server::WindowServer,
    ) -> anyhow::Result<()> {
        if !self.by_id.contains_key(&window_id) {
            return Ok(());
        }
        server.close(window_id)
    }

    #[tracing::instrument(level = "trace", skip(self, server))]
    pub(super) fn unminimize_window(
        &self,
        window_id: WindowId,
        server: &dyn super::super::window_server::WindowServer,
    ) -> anyhow::Result<()> {
        let Some(entry) = self.get(window_id) else {
            return Ok(());
        };
        if !entry.is_minimized {
            return Ok(());
        }
        server.show_cmd(window_id, ShowCmd::Restore)
    }

    pub(super) fn get_mut(&mut self, id: WindowId) -> Option<&mut ManagedWindow> {
        self.by_id.get_mut(&id)
    }

    pub(super) fn contains(&self, id: WindowId) -> bool {
        self.by_id.contains_key(&id)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.by_id.keys().copied()
    }
}
