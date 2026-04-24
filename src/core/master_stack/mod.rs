use std::collections::HashMap;

use crate::core::hub::{HubAccess, TilingWindowPlacement};
use crate::core::node::{Dimension, Direction, WindowId, WorkspaceId};
use crate::core::strategy::{TilingAction, TilingPlacements, TilingStrategy, clip, translate};

/// XMonad-style tiling: a master area on the left and a stack on the right.
/// No containers, no tabs, no scroll. The first `master_count` windows in each
/// workspace's list occupy the master area; the rest go in the stack.
#[derive(Debug)]
pub(crate) struct MasterStackStrategy {
    workspaces: HashMap<WorkspaceId, MasterStackState>,
    window_dimensions: HashMap<WindowId, Dimension>,
    master_ratio: f32,
    master_count: usize,
}

/// Per-workspace state for master-stack layout. Windows are ordered: the first
/// `master_count` entries are in the master area, the rest in the stack.
#[derive(Debug)]
struct MasterStackState {
    windows: Vec<WindowId>,
    focused_index: Option<usize>,
}

impl MasterStackStrategy {
    pub(crate) fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            window_dimensions: HashMap::new(),
            master_ratio: 0.5,
            master_count: 1,
        }
    }

    /// Compute layout dimensions for all windows in a workspace and store them
    /// in `self.window_dimensions`. Master windows split the left portion
    /// (`master_ratio * width`), stack windows split the right portion.
    /// When all windows fit in the master area (`n <= master_count`), they
    /// share the full screen width. The last window in each area absorbs
    /// rounding remainder to avoid pixel gaps.
    fn do_layout(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let n = state.windows.len();
        if n == 0 {
            return;
        }

        let screen = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension;
        let w = screen.width;
        let h = screen.height;

        if n <= self.master_count {
            let each_h = (h / n as f32).floor();
            for (i, &wid) in state.windows.iter().enumerate() {
                let y = each_h * i as f32;
                let this_h = if i == n - 1 { h - y } else { each_h };
                self.window_dimensions.insert(
                    wid,
                    Dimension {
                        x: 0.0,
                        y,
                        width: w,
                        height: this_h,
                    },
                );
            }
        } else {
            let master_w = (w * self.master_ratio).floor();
            let stack_w = w - master_w;
            let mc = self.master_count;
            let sc = n - mc;

            let master_each_h = (h / mc as f32).floor();
            for i in 0..mc {
                let y = master_each_h * i as f32;
                let this_h = if i == mc - 1 { h - y } else { master_each_h };
                self.window_dimensions.insert(
                    state.windows[i],
                    Dimension {
                        x: 0.0,
                        y,
                        width: master_w,
                        height: this_h,
                    },
                );
            }

            let stack_each_h = (h / sc as f32).floor();
            for i in 0..sc {
                let y = stack_each_h * i as f32;
                let this_h = if i == sc - 1 { h - y } else { stack_each_h };
                self.window_dimensions.insert(
                    state.windows[mc + i],
                    Dimension {
                        x: master_w,
                        y,
                        width: stack_w,
                        height: this_h,
                    },
                );
            }
        }
    }

    /// Adjust `focused_index` after removing the window at `removed_idx`.
    fn adjust_focus_after_removal(state: &mut MasterStackState, removed_idx: usize) {
        let Some(focused) = state.focused_index else {
            return;
        };
        if state.windows.is_empty() {
            state.focused_index = None;
        } else if removed_idx < focused {
            state.focused_index = Some(focused - 1);
        } else if removed_idx == focused {
            // Move to next, or previous if removed was last, or None if empty
            if removed_idx < state.windows.len() {
                state.focused_index = Some(removed_idx);
            } else {
                state.focused_index = Some(state.windows.len() - 1);
            }
        }
        // If removed_idx > focused, no adjustment needed
    }

    /// Returns true if `idx` is in the master area (index < master_count).
    fn is_master(&self, idx: usize) -> bool {
        idx < self.master_count
    }

    /// Returns the (start, end) range for the area containing `idx`.
    fn area_range(&self, idx: usize, n: usize) -> (usize, usize) {
        if self.is_master(idx) {
            (0, self.master_count.min(n))
        } else {
            (self.master_count, n)
        }
    }
}

impl TilingStrategy for MasterStackStrategy {
    fn attach_window(&mut self, hub: &mut HubAccess, id: WindowId, ws_id: WorkspaceId) {
        hub.windows.get_mut(id).workspace = ws_id;
        let state = self
            .workspaces
            .entry(ws_id)
            .or_insert_with(|| MasterStackState {
                windows: Vec::new(),
                focused_index: None,
            });
        state.windows.push(id);
        state.focused_index = Some(state.windows.len() - 1);
        self.window_dimensions.insert(id, Dimension::default());
        hub.workspaces.get_mut(ws_id).is_float_focused = false;
        self.layout_workspace(hub, ws_id);
    }

    fn detach_window(&mut self, hub: &mut HubAccess, id: WindowId) -> Dimension {
        let ws_id = hub.windows.get(id).workspace;
        let screen = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension;

        let Some(state) = self.workspaces.get_mut(&ws_id) else {
            return Dimension::default();
        };

        let Some(idx) = state.windows.iter().position(|&w| w == id) else {
            return Dimension::default();
        };
        state.windows.remove(idx);
        Self::adjust_focus_after_removal(state, idx);

        if state.windows.is_empty() {
            let ws = hub.workspaces.get_mut(ws_id);
            ws.is_float_focused = !ws.float_windows.is_empty();
        }

        let dim = self.window_dimensions.remove(&id).unwrap_or_default();
        // Translate layout-space coords to screen-absolute by adding monitor origin
        let result = Dimension {
            x: dim.x + screen.x,
            y: dim.y + screen.y,
            ..dim
        };

        self.layout_workspace(hub, ws_id);
        result
    }

    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        let ws_id = hub.windows.get(window_id).workspace;
        let Some(state) = self.workspaces.get_mut(&ws_id) else {
            return;
        };
        let Some(idx) = state.windows.iter().position(|&w| w == window_id) else {
            return;
        };
        state.focused_index = Some(idx);
        hub.workspaces.get_mut(ws_id).is_float_focused = false;
    }

    fn focused_tiling_window(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> Option<WindowId> {
        let state = self.workspaces.get(&ws_id)?;
        let idx = state.focused_index?;
        Some(state.windows[idx])
    }

    fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
    ) -> TilingPlacements {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return TilingPlacements {
                windows: Vec::new(),
                containers: Vec::new(),
            };
        };

        let ws = hub.workspaces.get(ws_id);
        let screen = hub.monitors.get(ws.monitor).dimension;
        let focused_idx = if highlighted && !ws.is_float_focused {
            state.focused_index
        } else {
            None
        };

        let mut windows = Vec::with_capacity(state.windows.len());
        for (i, &wid) in state.windows.iter().enumerate() {
            let dim = self
                .window_dimensions
                .get(&wid)
                .copied()
                .unwrap_or_default();
            let frame = translate(dim, 0.0, 0.0, screen);
            if let Some(visible_frame) = clip(frame, screen) {
                windows.push(TilingWindowPlacement {
                    id: wid,
                    frame,
                    visible_frame,
                    is_highlighted: focused_idx == Some(i),
                    spawn_indicator: None,
                });
            }
        }

        TilingPlacements {
            windows,
            containers: Vec::new(),
        }
    }

    fn handle_action(&mut self, hub: &mut HubAccess, action: TilingAction) {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;

        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let Some(focused) = state.focused_index else {
            return;
        };
        let n = state.windows.len();

        match action {
            TilingAction::FocusDirection { direction, forward } => {
                // Direction mapping: Left=(Horizontal,false), Right=(Horizontal,true),
                // Up=(Vertical,false), Down=(Vertical,true)
                if n <= 1 {
                    return;
                }
                match (direction, forward) {
                    // Left: from stack -> focus first master
                    (Direction::Horizontal, false) => {
                        if !self.is_master(focused) {
                            self.workspaces.get_mut(&ws_id).unwrap().focused_index = Some(0);
                        }
                    }
                    // Right: from master -> focus first stack window
                    (Direction::Horizontal, true) => {
                        if self.is_master(focused) && self.master_count < n {
                            self.workspaces.get_mut(&ws_id).unwrap().focused_index =
                                Some(self.master_count);
                        }
                    }
                    // Up: prev within area, wrapping
                    (Direction::Vertical, false) => {
                        let (start, end) = self.area_range(focused, n);
                        if end - start <= 1 {
                            return;
                        }
                        let new = if focused == start {
                            end - 1
                        } else {
                            focused - 1
                        };
                        self.workspaces.get_mut(&ws_id).unwrap().focused_index = Some(new);
                    }
                    // Down: next within area, wrapping
                    (Direction::Vertical, true) => {
                        let (start, end) = self.area_range(focused, n);
                        if end - start <= 1 {
                            return;
                        }
                        let new = if focused == end - 1 {
                            start
                        } else {
                            focused + 1
                        };
                        self.workspaces.get_mut(&ws_id).unwrap().focused_index = Some(new);
                    }
                }
            }
            TilingAction::MoveDirection { direction, forward } => {
                if n <= 1 {
                    return;
                }
                let mc = self.master_count;
                let is_master = focused < mc;
                let (area_start, area_end) = if is_master { (0, mc.min(n)) } else { (mc, n) };
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                match (direction, forward) {
                    // Left from stack: swap with last master
                    (Direction::Horizontal, false) => {
                        if !is_master {
                            let target = mc - 1;
                            state.windows.swap(focused, target);
                            state.focused_index = Some(target);
                        }
                    }
                    // Right from master: swap with first stack window
                    (Direction::Horizontal, true) => {
                        if is_master && mc < n {
                            state.windows.swap(focused, mc);
                            state.focused_index = Some(mc);
                        }
                    }
                    // Up: swap with prev within area, wrapping
                    (Direction::Vertical, false) => {
                        if area_end - area_start <= 1 {
                            return;
                        }
                        let target = if focused == area_start {
                            area_end - 1
                        } else {
                            focused - 1
                        };
                        state.windows.swap(focused, target);
                        state.focused_index = Some(target);
                    }
                    // Down: swap with next within area, wrapping
                    (Direction::Vertical, true) => {
                        if area_end - area_start <= 1 {
                            return;
                        }
                        let target = if focused == area_end - 1 {
                            area_start
                        } else {
                            focused + 1
                        };
                        state.windows.swap(focused, target);
                        state.focused_index = Some(target);
                    }
                }
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::IncreaseMasterRatio => {
                self.master_ratio = (self.master_ratio + 0.05).clamp(0.1, 0.9);
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::DecreaseMasterRatio => {
                self.master_ratio = (self.master_ratio - 0.05).clamp(0.1, 0.9);
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::IncrementMasterCount => {
                self.master_count += 1;
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::DecrementMasterCount => {
                if self.master_count > 1 {
                    self.master_count -= 1;
                    self.layout_workspace(hub, ws_id);
                }
            }
            TilingAction::ToggleSpawnMode
            | TilingAction::ToggleDirection
            | TilingAction::ToggleContainerLayout
            | TilingAction::FocusParent
            | TilingAction::FocusTab { .. }
            | TilingAction::TabClicked { .. } => {}
        }
    }

    fn layout_workspace(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        self.do_layout(hub, ws_id);
    }

    fn has_tiling_windows(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> bool {
        self.workspaces
            .get(&ws_id)
            .is_some_and(|s| !s.windows.is_empty())
    }

    fn tiling_window_count(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> usize {
        self.workspaces.get(&ws_id).map_or(0, |ws| ws.windows.len())
    }

    fn prune_workspace(&mut self, ws_id: WorkspaceId) {
        if let Some(state) = self.workspaces.remove(&ws_id) {
            for wid in &state.windows {
                self.window_dimensions.remove(wid);
            }
        }
    }

    fn move_focused_to_workspace(
        &mut self,
        hub: &mut HubAccess,
        from_ws: WorkspaceId,
        to_ws: WorkspaceId,
    ) {
        let Some(state) = self.workspaces.get(&from_ws) else {
            return;
        };
        let Some(focused) = state.focused_index else {
            return;
        };
        let id = state.windows[focused];

        // Inline removal from source (don't call detach_window because hub may
        // have already updated window.workspace)
        let state = self.workspaces.get_mut(&from_ws).unwrap();
        state.windows.remove(focused);
        Self::adjust_focus_after_removal(state, focused);

        if state.windows.is_empty() {
            let ws = hub.workspaces.get_mut(from_ws);
            ws.is_float_focused = !ws.float_windows.is_empty();
        }

        self.window_dimensions.remove(&id);
        self.layout_workspace(hub, from_ws);

        self.attach_window(hub, id, to_ws);
    }

    #[cfg(test)]
    fn validate_tree(&self, hub: &HubAccess) {
        use std::collections::HashSet;

        for (&ws_id, state) in &self.workspaces {
            let mut seen = HashSet::new();
            for &wid in &state.windows {
                // Panics if window doesn't exist in hub (core is infallible)
                hub.windows.get(wid);
                assert!(
                    seen.insert(wid),
                    "master-stack workspace {ws_id}: duplicate window {wid:?}"
                );
            }

            match state.focused_index {
                Some(idx) => {
                    assert!(
                        idx < state.windows.len(),
                        "master-stack workspace {ws_id}: focused_index {idx} out of bounds (len={})",
                        state.windows.len()
                    );
                }
                None => {
                    assert!(
                        state.windows.is_empty(),
                        "master-stack workspace {ws_id}: focused_index is None but windows is non-empty"
                    );
                }
            }
        }
    }
}
