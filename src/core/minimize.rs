//! Minimize boundary.
//!
//! Mutators on `Hub` that take a `WindowId` require the window to be
//! non-minimized at call time. Callers observe minimized state through
//! their own registry (e.g. `ManagedWindow::is_minimized` in the macOS
//! and Windows shells) and must call `unminimize_window` first if they
//! intend to mutate a minimized window. Enforcement is implicit: each
//! in-scope mutator runs `workspace().expect("non-minimized window has
//! a workspace")`, which panics on a minimized `WindowId` because a
//! minimized window has no workspace. No explicit `is_minimized` assert
//! is added on each mutator.
//!
//! Exemptions: `minimize_window` and `unminimize_window` (the boundary
//! primitives defined in this module); `delete_window` (lifecycle,
//! owned by the OS); `set_window_title` and `set_window_constraint`
//! (bookkeeping that does not affect layout).

use crate::core::{
    Hub, WindowId,
    node::{DisplayMode, MinimizedWindowEntry},
};

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
                let strategy = self.strategies.for_workspace_mut(prior_workspace);
                strategy.detach_window(&self.access, window_id);
                if strategy.tiling_window_count(prior_workspace) == 0 {
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

        let w = self.access.windows.get_mut(window_id);
        w.set_minimized(true);
        w.set_workspace(None);
        self.minimized_windows.push(window_id);

        tracing::info!(?prior_mode, "Window minimized");
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
        let prior_mode = self.access.windows.get(window_id).mode;

        self.access.windows.get_mut(window_id).set_minimized(false);

        match prior_mode {
            DisplayMode::Tiling => {
                self.strategies
                    .for_workspace_mut(target_workspace)
                    .attach_window(&mut self.access, window_id, target_workspace);
                self.set_workspace_focus(window_id);
            }
            DisplayMode::Float { border_box, .. } => {
                // unminimize restores to current_workspace(), and minimize
                // clears the origin, so the restore target may differ from the
                // origin workspace. Drop occupy unconditionally to avoid
                // leaking the origin's matcher into a different export section.
                self.attach_float_to_workspace(target_workspace, window_id, border_box, None);
            }
            DisplayMode::Fullscreen { .. } => {
                self.attach_fullscreen_to_workspace(target_workspace, window_id, None);
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
