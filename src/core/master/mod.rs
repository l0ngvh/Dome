use std::collections::HashMap;

use crate::core::hub::{HubAccess, TilingWindowPlacement};
use crate::core::node::{Dimension, Direction, Length, WindowId, WorkspaceId};
use crate::core::strategy::{
    TilingAction, TilingPlacements, TilingStrategy, clip, distribute_space, translate,
};

/// XMonad-style tiling: a master area on the left and a stack on the right.
/// No containers, no tabs. Each pane scrolls vertically and independently when
/// per-window min heights push the pane's total content past the screen height.
/// Horizontal scroll does not exist in master.
#[derive(Debug)]
pub(crate) struct MasterStrategy {
    workspaces: HashMap<WorkspaceId, MasterState>,
    window_dimensions: HashMap<WindowId, Dimension>,
}

/// Per-workspace state for master-stack layout. Windows are ordered: the first
/// `master_count` entries are in the master area, the rest in the stack.
#[derive(Debug)]
struct MasterState {
    windows: Vec<WindowId>,
    focused_index: Option<usize>,
    master_y_offset: Length,
    stack_y_offset: Length,
    master_count: usize,
    master_ratio: f32,
}

impl MasterStrategy {
    pub(crate) fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            window_dimensions: HashMap::new(),
        }
    }

    /// Compute layout dimensions for all windows in a workspace and store them
    /// in `self.window_dimensions`. Respects per-window min/max size constraints
    /// and uses `distribute_space` for vertical allocation within each pane.
    /// Master windows occupy the left pane, stack windows the right pane.
    /// When all windows fit in the master area (`n <= master_count`), they
    /// share the full screen width.
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
        let h = screen.height;
        let mc = state.master_count;

        let windows = state.windows.clone();

        if n <= mc {
            // All windows in master pane, which fills the full screen width.
            let master_pane_min_w = windows
                .iter()
                .map(|&id| effective_constraints(hub, id).0)
                .fold(Length::ZERO, Length::max);
            let pane_w = master_pane_min_w.max(screen.width);

            let constraints: Vec<(Length, Length)> = windows
                .iter()
                .map(|&id| {
                    let (_, min_h, _, max_h) = effective_constraints(hub, id);
                    (min_h, max_h)
                })
                .collect();
            let heights = distribute_space(&constraints, h);
            let sum_h: Length = heights.iter().copied().sum();
            let mut y = if sum_h < h {
                (h - sum_h) / 2.0
            } else {
                Length::ZERO
            };
            for (i, &id) in windows.iter().enumerate() {
                let (_, _, max_w, max_h) = effective_constraints(hub, id);
                let (w, x_off) = apply_max_constraint(max_w, pane_w);
                let (slot_h, y_off) = apply_max_constraint(max_h, heights[i]);
                self.window_dimensions
                    .insert(id, Dimension::new(x_off, y + y_off, w, slot_h));
                y += heights[i];
            }
        } else {
            // Two-pane layout: master on left, stack on right.
            let master_pane_min_w = windows[..mc]
                .iter()
                .map(|&id| effective_constraints(hub, id).0)
                .fold(Length::ZERO, Length::max);
            let stack_pane_min_w = windows[mc..]
                .iter()
                .map(|&id| effective_constraints(hub, id).0)
                .fold(Length::ZERO, Length::max);

            let desired_master_w = Length::new((screen.width.value() * state.master_ratio).floor());
            let total_min = master_pane_min_w + stack_pane_min_w;

            let (master_w, stack_w) = if total_min >= screen.width {
                (master_pane_min_w, stack_pane_min_w)
            } else if desired_master_w < master_pane_min_w {
                (master_pane_min_w, screen.width - master_pane_min_w)
            } else if screen.width - desired_master_w < stack_pane_min_w {
                (screen.width - stack_pane_min_w, stack_pane_min_w)
            } else {
                (desired_master_w, screen.width - desired_master_w)
            };

            // Master pane vertical layout
            let master_constraints: Vec<(Length, Length)> = windows[..mc]
                .iter()
                .map(|&id| {
                    let (_, min_h, _, max_h) = effective_constraints(hub, id);
                    (min_h, max_h)
                })
                .collect();
            let master_heights = distribute_space(&master_constraints, h);
            let master_sum_h: Length = master_heights.iter().copied().sum();
            let mut y = if master_sum_h < h {
                (h - master_sum_h) / 2.0
            } else {
                Length::ZERO
            };
            for (i, &id) in windows[..mc].iter().enumerate() {
                let (_, _, max_w, max_h) = effective_constraints(hub, id);
                let (w, x_off) = apply_max_constraint(max_w, master_w);
                let (slot_h, y_off) = apply_max_constraint(max_h, master_heights[i]);
                self.window_dimensions
                    .insert(id, Dimension::new(x_off, y + y_off, w, slot_h));
                y += master_heights[i];
            }

            // Stack pane vertical layout
            let stack_constraints: Vec<(Length, Length)> = windows[mc..]
                .iter()
                .map(|&id| {
                    let (_, min_h, _, max_h) = effective_constraints(hub, id);
                    (min_h, max_h)
                })
                .collect();
            let stack_heights = distribute_space(&stack_constraints, h);
            let stack_sum_h: Length = stack_heights.iter().copied().sum();
            let mut y = if stack_sum_h < h {
                (h - stack_sum_h) / 2.0
            } else {
                Length::ZERO
            };
            for (i, &id) in windows[mc..].iter().enumerate() {
                let (_, _, max_w, max_h) = effective_constraints(hub, id);
                let (w, x_off) = apply_max_constraint(max_w, stack_w);
                let (slot_h, y_off) = apply_max_constraint(max_h, stack_heights[i]);
                self.window_dimensions
                    .insert(id, Dimension::new(master_w + x_off, y + y_off, w, slot_h));
                y += stack_heights[i];
            }
        }

        self.clamp_scroll(hub, ws_id);
        self.scroll_into_view(hub, ws_id);
    }

    /// Adjust `focused_index` after removing the window at `removed_idx`.
    fn adjust_focus_after_removal(state: &mut MasterState, removed_idx: usize) {
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

    fn is_master(idx: usize, master_count: usize) -> bool {
        idx < master_count
    }

    /// Returns the (start, end) range for the area containing `idx`.
    fn area_range(idx: usize, master_count: usize, n: usize) -> (usize, usize) {
        if Self::is_master(idx, master_count) {
            (0, master_count.min(n))
        } else {
            (master_count, n)
        }
    }

    fn clamp_scroll(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let pane_height = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension
            .height;
        let mc = state.master_count;
        let n = state.windows.len();

        let master_content_h = self.pane_content_h(hub, &state.windows[..mc.min(n)], pane_height);
        let master_max = (master_content_h - pane_height).max(Length::ZERO);

        let stack_content_h = if n > mc {
            self.pane_content_h(hub, &state.windows[mc..], pane_height)
        } else {
            Length::ZERO
        };
        let stack_max = (stack_content_h - pane_height).max(Length::ZERO);

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.master_y_offset = state.master_y_offset.clamp(Length::ZERO, master_max);
        state.stack_y_offset = state.stack_y_offset.clamp(Length::ZERO, stack_max);
        tracing::trace!(
            ?ws_id,
            master_offset = %state.master_y_offset,
            stack_offset = %state.stack_y_offset,
            "clamped scroll offsets"
        );
    }

    fn scroll_into_view(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let Some(focused) = state.focused_index else {
            return;
        };
        let pane_height = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension
            .height;
        let mc = state.master_count;
        let in_master = focused < mc;

        let (pane_windows, offset) = if in_master {
            (
                state.windows[..mc.min(state.windows.len())].to_vec(),
                state.master_y_offset,
            )
        } else {
            (state.windows[mc..].to_vec(), state.stack_y_offset)
        };

        let slot_heights = self.pane_slot_heights(hub, &pane_windows, pane_height);
        let content_h: Length = slot_heights.iter().copied().sum();
        let max_offset = (content_h - pane_height).max(Length::ZERO);

        let focused_in_pane = if in_master { focused } else { focused - mc };

        // Compute the slot y position by summing preceding slot heights,
        // accounting for vertical centering of the group when content fits.
        let content_start = if content_h < pane_height {
            (pane_height - content_h) / 2.0
        } else {
            Length::ZERO
        };
        let slot_y: Length = content_start
            + slot_heights[..focused_in_pane]
                .iter()
                .copied()
                .sum::<Length>();
        let slot_height = slot_heights[focused_in_pane];

        let mut new_offset = offset;
        if slot_y + slot_height - new_offset > pane_height {
            new_offset = slot_y + slot_height - pane_height;
        }
        if slot_y - new_offset < Length::ZERO {
            new_offset = slot_y;
        }
        new_offset = new_offset.clamp(Length::ZERO, max_offset);

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        if in_master {
            state.master_y_offset = new_offset;
        } else {
            state.stack_y_offset = new_offset;
        }
        tracing::debug!(
            ?ws_id,
            in_master,
            focused_idx = focused,
            offset = %new_offset,
            "scroll_into_view"
        );
    }

    /// Total content height for a pane. Equals the sum of slot heights returned
    /// by distribute_space. Recomputes from constraints rather than relying on
    /// stored window dimensions (which include per-window centering offsets).
    fn pane_content_h(
        &self,
        hub: &HubAccess,
        pane_windows: &[WindowId],
        pane_height: Length,
    ) -> Length {
        let heights = self.pane_slot_heights(hub, pane_windows, pane_height);
        heights.iter().copied().sum()
    }

    /// Slot heights for a pane as returned by distribute_space.
    fn pane_slot_heights(
        &self,
        hub: &HubAccess,
        pane_windows: &[WindowId],
        pane_height: Length,
    ) -> Vec<Length> {
        if pane_windows.is_empty() {
            return Vec::new();
        }
        let constraints: Vec<(Length, Length)> = pane_windows
            .iter()
            .map(|&id| {
                let (_, min_h, _, max_h) = effective_constraints(hub, id);
                (min_h, max_h)
            })
            .collect();
        distribute_space(&constraints, pane_height)
    }
}

impl TilingStrategy for MasterStrategy {
    fn attach_window(&mut self, hub: &mut HubAccess, id: WindowId, ws_id: WorkspaceId) {
        hub.windows.get_mut(id).set_workspace(Some(ws_id));
        let ws_name = hub.workspaces.get(ws_id).name.clone();
        let override_block = hub
            .config
            .master
            .workspace
            .iter()
            .find(|w| w.name == ws_name);
        let initial_master_count = override_block
            .and_then(|w| w.master_count)
            .unwrap_or(hub.config.master.master_count);
        let initial_master_ratio = override_block
            .and_then(|w| w.master_ratio)
            .unwrap_or(hub.config.master.master_ratio);
        let state = self.workspaces.entry(ws_id).or_insert_with(|| MasterState {
            windows: Vec::new(),
            focused_index: None,
            master_y_offset: Length::ZERO,
            stack_y_offset: Length::ZERO,
            master_count: initial_master_count,
            master_ratio: initial_master_ratio,
        });
        state.windows.push(id);
        state.focused_index = Some(state.windows.len() - 1);
        // Zero placeholder -- the layout_workspace call below computes the real rect
        // before any reader observes this entry.
        self.window_dimensions.insert(id, Dimension::default());
        hub.workspaces.get_mut(ws_id).is_float_focused = false;
        self.layout_workspace(hub, ws_id);
    }

    fn detach_window(&mut self, hub: &mut HubAccess, id: WindowId) -> Dimension {
        let ws_id = hub
            .windows
            .get(id)
            .workspace()
            .expect("detaching tiling window has a workspace");
        let screen = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension;

        let state = self.workspaces.get_mut(&ws_id).unwrap_or_else(|| {
            panic!("master: detach_window called for {id:?} but workspace {ws_id} has no state")
        });

        let idx = state
            .windows
            .iter()
            .position(|&w| w == id)
            .unwrap_or_else(|| {
                panic!("master: detach_window called for {id:?} but window is not in workspace {ws_id} state.windows")
            });

        // Capture the pane y offset BEFORE removal. The post-removal layout pass
        // can clamp the offset, so we need the pre-removal value for the returned
        // screen-absolute position.
        let y_offset = if idx < state.master_count {
            state.master_y_offset
        } else {
            state.stack_y_offset
        };

        state.windows.remove(idx);
        Self::adjust_focus_after_removal(state, idx);

        if state.windows.is_empty() {
            let ws = hub.workspaces.get_mut(ws_id);
            ws.is_float_focused = !ws.float_windows.is_empty();
        }

        let dim = self.window_dimensions.remove(&id).unwrap_or_else(|| {
            panic!("master: detach_window called for {id:?} but window_dimensions has no entry")
        });
        let result = Dimension::new(
            dim.x + screen.x,
            dim.y - y_offset + screen.y,
            dim.width,
            dim.height,
        );

        self.layout_workspace(hub, ws_id);
        result
    }

    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        let ws_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("setting focus on tiling window requires a workspace");
        let Some(state) = self.workspaces.get_mut(&ws_id) else {
            return;
        };
        let Some(idx) = state.windows.iter().position(|&w| w == window_id) else {
            return;
        };
        state.focused_index = Some(idx);
        hub.workspaces.get_mut(ws_id).is_float_focused = false;
        self.scroll_into_view(hub, ws_id);
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

        let mc = state.master_count;
        let mut windows = Vec::with_capacity(state.windows.len());
        for (i, &wid) in state.windows.iter().enumerate() {
            let dim = *self.window_dimensions.get(&wid).expect(
                "master: window present in state.windows but missing from window_dimensions",
            );
            let y_offset = if i < mc {
                state.master_y_offset
            } else {
                state.stack_y_offset
            };
            let frame = translate(dim, Length::ZERO, y_offset, screen);
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
                let mc = state.master_count;
                match (direction, forward) {
                    // Left: from stack -> focus first master
                    (Direction::Horizontal, false) => {
                        if !Self::is_master(focused, mc) {
                            self.workspaces.get_mut(&ws_id).unwrap().focused_index = Some(0);
                        }
                    }
                    // Right: from master -> focus first stack window
                    (Direction::Horizontal, true) => {
                        if Self::is_master(focused, mc) && mc < n {
                            self.workspaces.get_mut(&ws_id).unwrap().focused_index = Some(mc);
                        }
                    }
                    // Up: prev within area, wrapping
                    (Direction::Vertical, false) => {
                        let (start, end) = Self::area_range(focused, mc, n);
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
                        let (start, end) = Self::area_range(focused, mc, n);
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
                self.scroll_into_view(hub, ws_id);
            }
            TilingAction::MoveDirection { direction, forward } => {
                if n <= 1 {
                    return;
                }
                let mc = state.master_count;
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
            TilingAction::GrowMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.master_ratio = (state.master_ratio + 0.05).clamp(0.1, 0.9);
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::ShrinkMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.master_ratio = (state.master_ratio - 0.05).clamp(0.1, 0.9);
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::MoreMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.master_count += 1;
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::FewerMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                if state.master_count <= 1 {
                    return;
                }
                state.master_count -= 1;
                self.layout_workspace(hub, ws_id);
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

    fn prune_workspace(&mut self, ws_id: WorkspaceId) {
        if let Some(state) = self.workspaces.remove(&ws_id) {
            for wid in &state.windows {
                self.window_dimensions.remove(wid);
            }
        }
    }

    fn apply_config(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        self.layout_workspace(hub, ws_id);
    }

    fn detach_focused(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Vec<WindowId> {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return Vec::new();
        };
        let Some(focused) = state.focused_index else {
            return Vec::new();
        };
        let id = state.windows[focused];

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.windows.remove(focused);
        Self::adjust_focus_after_removal(state, focused);

        if state.windows.is_empty() {
            let ws = hub.workspaces.get_mut(ws_id);
            ws.is_float_focused = !ws.float_windows.is_empty();
        }

        self.window_dimensions.remove(&id);
        self.layout_workspace(hub, ws_id);

        vec![id]
    }

    fn attach_detached(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, windows: &[WindowId]) {
        for &wid in windows {
            self.attach_window(hub, wid, ws_id);
        }
        if let Some(&last) = windows.last() {
            self.set_focus(hub, last);
        }
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

            if state.windows.is_empty() {
                continue;
            }

            let pane_height = hub
                .monitors
                .get(hub.workspaces.get(ws_id).monitor)
                .dimension
                .height;
            let mc = state.master_count;
            let n = state.windows.len();

            for &wid in &state.windows {
                let dim = self.window_dimensions.get(&wid).unwrap_or_else(|| {
                    panic!("master-stack workspace {ws_id}: window {wid:?} missing from window_dimensions")
                });
                assert!(
                    dim.width > Length::ZERO,
                    "master-stack workspace {ws_id}: window {wid:?} has non-positive width {}",
                    dim.width
                );
                assert!(
                    dim.height > Length::ZERO,
                    "master-stack workspace {ws_id}: window {wid:?} has non-positive height {}",
                    dim.height
                );

                let (min_w, min_h, _, _) = effective_constraints(hub, wid);
                assert!(
                    dim.width >= min_w,
                    "master-stack workspace {ws_id}: window {wid:?} width {} < effective min_w {}",
                    dim.width,
                    min_w
                );
                assert!(
                    dim.height >= min_h,
                    "master-stack workspace {ws_id}: window {wid:?} height {} < effective min_h {}",
                    dim.height,
                    min_h
                );
            }

            let master_content_h =
                self.pane_content_h(hub, &state.windows[..mc.min(n)], pane_height);
            let master_max_offset = (master_content_h - pane_height).max(Length::ZERO);
            assert!(
                state.master_y_offset >= Length::ZERO && state.master_y_offset <= master_max_offset,
                "master-stack workspace {ws_id}: master_y_offset {} out of bounds [0, {}]",
                state.master_y_offset,
                master_max_offset
            );

            if n > mc {
                let stack_content_h = self.pane_content_h(hub, &state.windows[mc..], pane_height);
                let stack_max_offset = (stack_content_h - pane_height).max(Length::ZERO);
                assert!(
                    state.stack_y_offset >= Length::ZERO
                        && state.stack_y_offset <= stack_max_offset,
                    "master-stack workspace {ws_id}: stack_y_offset {} out of bounds [0, {}]",
                    state.stack_y_offset,
                    stack_max_offset
                );
            } else {
                assert!(
                    state.stack_y_offset == Length::ZERO,
                    "master-stack workspace {ws_id}: stack_y_offset {} should be zero (no stack windows)",
                    state.stack_y_offset
                );
            }
        }
    }
}

fn effective_constraints(hub: &HubAccess, wid: WindowId) -> (Length, Length, Length, Length) {
    let ws_id = hub
        .windows
        .get(wid)
        .workspace()
        .expect("tiling window has a workspace");
    let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
    let scale = monitor.scale;
    let screen = monitor.dimension;

    let global_min_w = hub.config.min_width.resolve(screen.width, scale);
    let global_min_h = hub.config.min_height.resolve(screen.height, scale);
    let global_max_w = hub.config.max_width.resolve(screen.width, scale);
    let global_max_h = hub.config.max_height.resolve(screen.height, scale);

    let window = hub.windows.get(wid);
    let (raw_min_w, raw_min_h) = window.min_size();
    let (raw_max_w, raw_max_h) = window.max_size();
    let win_min_w = Length::new(raw_min_w);
    let win_min_h = Length::new(raw_min_h);
    let win_max_w = Length::new(raw_max_w);
    let win_max_h = Length::new(raw_max_h);

    let max_w = if win_max_w > Length::ZERO {
        win_max_w
    } else {
        global_max_w
    };
    let max_h = if win_max_h > Length::ZERO {
        win_max_h
    } else {
        global_max_h
    };

    let min_w = if max_w > Length::ZERO {
        win_min_w.max(global_min_w).min(max_w)
    } else {
        win_min_w.max(global_min_w)
    };
    let min_h = if max_h > Length::ZERO {
        win_min_h.max(global_min_h).min(max_h)
    } else {
        win_min_h.max(global_min_h)
    };

    (min_w, min_h, max_w, max_h)
}

/// Returns (size, offset) for centering a max-constrained child inside its slot.
/// When max is zero or >= slot_extent, the child fills the slot with no offset.
fn apply_max_constraint(max: Length, slot_extent: Length) -> (Length, Length) {
    let size = if max > Length::ZERO && max < slot_extent {
        max
    } else {
        slot_extent
    };
    let offset = (slot_extent - size) / 2.0;
    (size, offset.max(Length::ZERO))
}
