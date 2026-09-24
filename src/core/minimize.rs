use crate::core::{
    Hub, WindowId,
    node::{Child, DisplayMode, MinimizedWindowEntry, WindowHost},
};

impl Hub {
    /// Detach a window from its current layout and mark it minimized.
    /// The window's `mode` field (including the float dim payload) is
    /// preserved through the round trip. The window is removed from its
    /// workspace and tracked in `minimized_windows` until restored.
    #[tracing::instrument(skip(self))]
    pub(crate) fn minimize_window(&mut self, window_id: WindowId) {
        let window = self.access.windows.get(window_id);
        let prior_mode = window.mode;
        let sticky = match window.host {
            WindowHost::Minimized { .. } => return,
            WindowHost::Monitor(_) => {
                self.detach_sticky_from_monitor(window_id);
                true
            }
            WindowHost::Workspace(prior_workspace) => {
                match prior_mode {
                    DisplayMode::Tiling => {
                        let strategy = self.strategies.for_workspace_mut(prior_workspace);
                        strategy.detach_window(&mut self.access, window_id);
                        if strategy.tiling_window_count(&self.access, prior_workspace) == 0 {
                            let ws = self.access.workspaces.get_mut(prior_workspace);
                            if ws.fullscreen_windows.is_empty() {
                                ws.is_float_focused = !ws.float_windows.is_empty();
                            }
                        }
                    }
                    DisplayMode::Float { .. } => {
                        self.detach_float_from_workspace(window_id);
                    }
                    DisplayMode::Fullscreen { .. } => {
                        self.detach_fullscreen_from_workspace(window_id);
                    }
                }
                false
            }
        };

        self.access
            .windows
            .get_mut(window_id)
            .set_host(WindowHost::Minimized { sticky });
        self.minimized_windows.push(window_id);

        tracing::info!(?prior_mode, sticky, "Window minimized");
    }

    /// Restore a minimized window to the current workspace using its preserved
    /// mode. No-op if the window is not in `minimized_windows` (guards against
    /// stale entries where a window was deleted while minimized).
    #[tracing::instrument(skip(self))]
    pub(crate) fn unminimize_window(&mut self, window_id: WindowId) {
        if !self.minimized_windows.contains(&window_id) {
            return;
        }
        self.minimized_windows.retain(|&w| w != window_id);

        let target_workspace = self.current_workspace();
        let window = self.access.windows.get(window_id);
        let prior_mode = window.mode;
        let sticky = match window.host {
            WindowHost::Minimized { sticky } => sticky,
            WindowHost::Workspace(_) | WindowHost::Monitor(_) => {
                panic!("{window_id} in minimized_windows has a non-minimized host")
            }
        };

        match prior_mode {
            DisplayMode::Tiling => {
                self.strategies
                    .for_workspace_mut(target_workspace)
                    .reattach_child(&mut self.access, Child::Window(window_id), target_workspace);
                self.set_workspace_focus(window_id);
            }
            DisplayMode::Float { border_box, .. } if sticky => {
                let monitor_id = self.access.focused_monitor;
                let border_box = self.border_box_on_monitor(border_box, monitor_id);
                self.attach_sticky_to_monitor(monitor_id, window_id, border_box);
            }
            DisplayMode::Float { border_box, .. } => {
                let border_box = self.border_box_on_monitor(
                    border_box,
                    self.access.workspaces.get(target_workspace).monitor,
                );
                self.attach_float_to_workspace(target_workspace, window_id, border_box, None);
            }
            DisplayMode::Fullscreen { prior_mode } => {
                self.attach_fullscreen_to_workspace(target_workspace, window_id, None, prior_mode);
            }
        }
        tracing::info!(?prior_mode, "Window unminimized");
    }

    /// Returns entries for all minimized windows, in insertion order.
    pub(crate) fn minimized_window_entries(&self) -> Vec<MinimizedWindowEntry> {
        self.minimized_windows
            .iter()
            .map(|&id| {
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
