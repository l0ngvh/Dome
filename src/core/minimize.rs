use crate::core::{Hub, WindowId, node::MinimizedWindowEntry};

impl Hub {
    /// Detaches a window from its workspace and keeps it, with the display mode it had and a
    /// float's border box, in `minimized_windows` until it is restored.
    #[tracing::instrument(skip(self))]
    pub(crate) fn minimize_window(&mut self, window_id: WindowId) {
        let window = self.access.windows.get(window_id);
        if window.is_minimized() {
            return;
        }
        let prior_workspace = window
            .workspace()
            .expect("non-minimized window has a workspace");

        let prior_mode = self
            .strategies
            .for_workspace_mut(prior_workspace)
            .detach_window(&mut self.access, window_id);

        let w = self.access.windows.get_mut(window_id);
        w.set_minimized(true);
        w.set_workspace(None);
        self.minimized_windows.push((window_id, prior_mode));

        tracing::info!(?prior_mode, "Window minimized");
    }

    /// Restores a minimized window to the current workspace in the display mode it had, then
    /// requests focus for it. Does nothing for a window that is not minimized, such as one
    /// deleted while minimized.
    #[tracing::instrument(skip(self))]
    pub(crate) fn unminimize_window(&mut self, window_id: WindowId) {
        let Some(pos) = self
            .minimized_windows
            .iter()
            .position(|&(id, _)| id == window_id)
        else {
            return;
        };
        let (_, prior_mode) = self.minimized_windows.remove(pos);

        let target_workspace = self.current_workspace();

        self.access.windows.get_mut(window_id).set_minimized(false);
        self.strategies
            .for_workspace_mut(target_workspace)
            .attach_window(
                &mut self.access,
                window_id,
                target_workspace,
                prior_mode,
                None,
            );
        self.set_workspace_focus(window_id);
        tracing::info!(?prior_mode, "Window unminimized");
    }

    /// Returns entries for all minimized windows, in insertion order.
    pub(crate) fn minimized_window_entries(&self) -> Vec<MinimizedWindowEntry> {
        self.minimized_windows
            .iter()
            .map(|&(id, _)| {
                let w = self.access.windows.get(id);
                MinimizedWindowEntry {
                    id,
                    title: w.metadata.title().map(str::to_owned).unwrap_or_default(),
                    app_name: w.metadata.app_name(),
                    bundle_id: w.metadata.bundle_id(),
                    executable_path: w.metadata.executable_path(),
                }
            })
            .collect()
    }
}
