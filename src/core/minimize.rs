use crate::core::{
    Hub, WindowId,
    node::{DisplayMode, WindowRestrictions},
};

impl Hub {
    /// Detach a window from its current layout and mark it minimized.
    /// Follows the `delete_window` pattern: read workspace, match on mode,
    /// call the appropriate detach, then prune. Instead of deleting the window,
    /// sets mode to Minimized and tracks it in `minimized_windows`.
    #[tracing::instrument(skip(self))]
    pub(crate) fn minimize_window(&mut self, window_id: WindowId) {
        let window = self.access.windows.get(window_id);
        let ws = window.workspace;
        match window.mode {
            DisplayMode::Tiling => {
                self.strategy.detach_window(&mut self.access, window_id);
            }
            DisplayMode::Float => {
                let _dim = self.detach_float_from_workspace(window_id);
            }
            DisplayMode::Fullscreen => {
                self.detach_fullscreen_from_workspace(window_id);
            }
            DisplayMode::Minimized => return,
        }
        let w = self.access.windows.get_mut(window_id);
        w.mode = DisplayMode::Minimized;
        w.restrictions = WindowRestrictions::None;
        self.minimized_windows.push(window_id);
        self.prune_workspace(ws);
        tracing::info!(%window_id, "Window minimized");
    }

    /// Restore a minimized window to the current workspace as tiling.
    /// No-op if the window is not in `minimized_windows` (guards against
    /// stale picker entries where a window was deleted while minimized).
    #[tracing::instrument(skip(self))]
    pub(crate) fn unminimize_window(&mut self, window_id: WindowId) {
        if !self.minimized_windows.contains(&window_id) {
            return;
        }
        self.minimized_windows.retain(|&w| w != window_id);
        let current_ws = self.current_workspace();
        self.access.windows.get_mut(window_id).mode = DisplayMode::Tiling;
        self.strategy
            .attach_window(&mut self.access, window_id, current_ws);
        tracing::info!(%window_id, "Window unminimized");
    }

    /// Returns (id, title) pairs for all minimized windows, in insertion order.
    pub(crate) fn minimized_window_entries(&self) -> Vec<(WindowId, String)> {
        self.minimized_windows
            .iter()
            .map(|&id| {
                let title = self.access.windows.get(id).title.clone();
                (id, title)
            })
            .collect()
    }
}
