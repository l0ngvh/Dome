use crate::core::{
    Hub, WindowId,
    hub::{FloatWindowPlacement, HubAccess},
    matcher::{WindowMatcher, WindowMode},
    node::{MonitorId, PixelRect, WorkspaceId},
    slot::Slot,
};

/// The float windows of one workspace, each with its border box.
#[derive(Debug, Default)]
pub(super) struct FloatWindows {
    /// Bottom to top.
    windows: Vec<(WindowId, PixelRect)>,
    /// When true, the topmost float takes keyboard focus over the tiling windows.
    pub(super) is_float_focused: bool,
}

impl FloatWindows {
    /// Allocates one free float slot per matcher in the hub's arena, in matcher order.
    pub(super) fn allocate_slots(
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        matchers: &[WindowMatcher],
    ) {
        for matcher in matchers {
            hub.slots
                .allocate(Slot::new(matcher.clone(), ws_id, WindowMode::Float));
        }
    }

    /// Pushes the window on top without selecting float focus.
    pub(super) fn attach(&mut self, window_id: WindowId, border_box: PixelRect) {
        self.windows.push((window_id, border_box));
    }

    /// Returns the border box of the removed window, or `None` when the window is not a
    /// member. Clears the float selection when the last float leaves.
    pub(super) fn detach(&mut self, window_id: WindowId) -> Option<PixelRect> {
        let pos = self.position(window_id)?;
        let (_, border_box) = self.windows.remove(pos);
        if self.windows.is_empty() {
            self.is_float_focused = false;
        }
        Some(border_box)
    }

    /// Raises a member and selects float focus. Returns whether the window is a member, which
    /// says nothing about whether it gets keyboard focus.
    pub(super) fn focus(&mut self, window_id: WindowId) -> bool {
        let Some(pos) = self.position(window_id) else {
            return false;
        };
        let entry = self.windows.remove(pos);
        self.windows.push(entry);
        self.is_float_focused = true;
        true
    }

    pub(super) fn topmost(&self) -> Option<WindowId> {
        self.windows.last().map(|&(id, _)| id)
    }

    /// Stores the border box of a member. Returns `false` and changes nothing for any other
    /// window.
    pub(super) fn update_float_rect(&mut self, window_id: WindowId, border_box: PixelRect) -> bool {
        let Some(pos) = self.position(window_id) else {
            return false;
        };
        self.windows[pos].1 = border_box;
        true
    }

    /// `highlighted` marks the topmost float as the focused window. A float gets no placement when
    /// it lies wholly outside the monitor's work area, or when its border leaves no room for
    /// content.
    pub(super) fn collect_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
    ) -> Vec<FloatWindowPlacement> {
        let monitor = hub.workspaces.get(ws_id).monitor;
        let screen = hub.monitors.get(monitor).work_area;
        let border = hub.border(monitor);
        let topmost = self.topmost();
        self.windows
            .iter()
            .filter_map(|&(id, border_box)| {
                let visible_border_box = border_box.clip(screen)?;
                let content_box = border_box.inset_by(border);
                if content_box.is_empty() {
                    return None;
                }
                Some(FloatWindowPlacement {
                    id,
                    border_box,
                    visible_border_box,
                    content_box,
                    is_highlighted: highlighted && topmost == Some(id),
                })
            })
            .collect()
    }

    /// One matcher per float from its live metadata, bottom to top. A general matcher in the
    /// layout file therefore comes back as one exact matcher per window it placed.
    pub(super) fn export(&self, hub: &HubAccess) -> Vec<WindowMatcher> {
        self.windows
            .iter()
            .map(|&(id, _)| hub.windows.get(id).metadata.to_window_matcher())
            .collect()
    }

    /// Returns the floats bottom to top, which is the order to attach them again, and clears
    /// the float selection.
    pub(super) fn clear(&mut self) -> Vec<(WindowId, PixelRect)> {
        self.is_float_focused = false;
        std::mem::take(&mut self.windows)
    }

    #[cfg(test)]
    pub(super) fn windows(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.windows.iter().map(|&(id, _)| id)
    }

    fn position(&self, window_id: WindowId) -> Option<usize> {
        self.windows.iter().position(|&(id, _)| id == window_id)
    }
}

impl Hub {
    /// Stores the observed screen-absolute content box of a float as its border box. A float
    /// reported on another monitor moves to that monitor's active workspace. The caller must
    /// pass the monitor that holds the content box, or the window joins a workspace it does not
    /// sit on, and switching workspaces on that monitor hides and shows it. A report for a window
    /// that is minimized or no longer floats arrived too late, so it changes nothing.
    #[tracing::instrument(skip(self))]
    pub(crate) fn update_float_rect(
        &mut self,
        window_id: WindowId,
        content_box: PixelRect,
        monitor_id: MonitorId,
    ) {
        let border = self.access.border(monitor_id);
        let border_box = content_box.outset_by(border);
        let Some(old_ws) = self.access.windows.get(window_id).workspace() else {
            tracing::debug!("Ignoring a float rect update for a minimized window");
            return;
        };
        if !self.strategies.for_workspace_mut(old_ws).update_float_rect(
            &mut self.access,
            window_id,
            border_box,
        ) {
            tracing::debug!("Ignoring a float rect update for a window that no longer floats");
            return;
        }

        let old_monitor = self.access.workspaces.get(old_ws).monitor;
        tracing::debug!(%old_monitor, %monitor_id, ?border_box, "Float rect updated");
        if monitor_id != old_monitor {
            let target_ws = self.access.monitors.get(monitor_id).active_workspace;
            if target_ws != old_ws {
                let mode = self
                    .strategies
                    .for_workspace_mut(old_ws)
                    .detach_window(&mut self.access, window_id);
                self.strategies.for_workspace_mut(target_ws).attach_window(
                    &mut self.access,
                    window_id,
                    target_ws,
                    mode,
                    None,
                );
                self.set_workspace_focus(window_id);
            }
        }
    }
}
