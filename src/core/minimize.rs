use crate::core::{Hub, WindowId, node::DisplayMode};

impl Hub {
    /// Detach a window from its current layout and mark it minimized.
    /// The window's `mode` field (including the float dim payload) is
    /// preserved through the round trip. The window is removed from its
    /// workspace and tracked in `minimized_windows` until restored.
    #[tracing::instrument(skip(self))]
    pub(crate) fn minimize_window(&mut self, window_id: WindowId) {
        let window = self.access.windows.get(window_id);
        if window.is_minimized() {
            return;
        }
        let prior_workspace = window
            .workspace()
            .expect("non-minimized window has a workspace");
        let prior_mode = window.mode;

        match prior_mode {
            DisplayMode::Tiling => {
                self.strategy.detach_window(&mut self.access, window_id);
            }
            // Float dim rides along on the variant; nothing to stash.
            DisplayMode::Float { .. } => {
                let _dim = self.detach_float_from_workspace(window_id);
            }
            DisplayMode::Fullscreen => {
                self.detach_fullscreen_from_workspace(window_id);
            }
        }

        let w = self.access.windows.get_mut(window_id);
        w.set_minimized(true);
        w.set_workspace(None);
        self.minimized_windows.push(window_id);

        self.prune_workspace(prior_workspace);
        tracing::info!(?prior_mode, "Window minimized");
    }

    /// Restore a minimized window to the current workspace using its preserved
    /// mode. No-op if the window is not in `minimized_windows` (guards against
    /// stale picker entries where a window was deleted while minimized).
    #[tracing::instrument(skip(self))]
    pub(crate) fn unminimize_window(&mut self, window_id: WindowId) {
        if !self.minimized_windows.contains(&window_id) {
            return;
        }
        self.minimized_windows.retain(|&w| w != window_id);

        let target_workspace = self.current_workspace();
        let prior_mode = self.access.windows.get(window_id).mode;

        self.access.windows.get_mut(window_id).set_minimized(false);

        match prior_mode {
            DisplayMode::Tiling => {
                self.strategy
                    .attach_window(&mut self.access, window_id, target_workspace);
            }
            DisplayMode::Float { dim } => {
                self.attach_float_to_workspace(target_workspace, window_id, dim);
            }
            DisplayMode::Fullscreen => {
                self.attach_fullscreen_to_workspace(target_workspace, window_id);
            }
        }
        tracing::info!(?prior_mode, "Window unminimized");
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
