mod export;
mod placement;
mod preferred_layout;
mod scroll;
#[cfg(test)]
mod validate;

use std::collections::HashMap;

use crate::config::{LayoutWorkspaceConfig, SizeConstraints, WindowMatcher};
use crate::core::GlobalLayoutConfig;
use crate::core::allocator::Allocator;
use crate::core::hub::HubAccess;
use crate::core::master::preferred_layout::MatcherId;
use crate::core::node::{Child, Constraints, Dimension, Direction, Length, WindowId, WorkspaceId};
use crate::core::strategy::{
    TilingAction, TilingPlacements, TilingStrategy, WorkspaceExport, distribute_space,
};

/// XMonad-style tiling: a master area on the left and a stack on the right.
/// No containers, no tabs. Each pane scrolls vertically and independently when
/// per-window min heights push the pane's total content past the screen height.
/// Horizontal scroll does not exist in master.
#[derive(Debug)]
pub(crate) struct MasterStrategy {
    workspaces: HashMap<WorkspaceId, WorkspaceState>,
    window_states: HashMap<WindowId, WindowState>,
    matchers: Allocator<WindowMatcher>,
    master_count: usize,
    master_ratio: f32,
    size_constraints: SizeConstraints,
}

impl TilingStrategy for MasterStrategy {
    fn prepare_workspace(
        &mut self,
        ws_id: WorkspaceId,
        preferred_layout: Option<&LayoutWorkspaceConfig>,
    ) {
        let Some(preferred_layout) = preferred_layout else {
            self.workspaces.insert(
                ws_id,
                WorkspaceState {
                    master: Vec::new(),
                    secondary: Vec::new(),
                    master_matchers: Vec::new(),
                    secondary_matchers: Vec::new(),
                    focus: None,
                    master_y_offset: Length::ZERO,
                    stack_y_offset: Length::ZERO,
                    master_count: None,
                    master_ratio: None,
                },
            );
            return;
        };
        let LayoutWorkspaceConfig::Master {
            master_count,
            master_ratio,
            master,
            secondary,
            ..
        } = preferred_layout
        else {
            panic!("Preparing partition tree workspace in master strategy");
        };

        let master_ids: Vec<MatcherId> = master
            .iter()
            .map(|m| self.matchers.allocate(m.clone()))
            .collect();
        let secondary_ids: Vec<MatcherId> = secondary
            .iter()
            .map(|m| self.matchers.allocate(m.clone()))
            .collect();

        self.workspaces.insert(
            ws_id,
            WorkspaceState {
                master: Vec::new(),
                secondary: Vec::new(),
                master_matchers: master_ids,
                secondary_matchers: secondary_ids,
                focus: None,
                master_y_offset: Length::ZERO,
                stack_y_offset: Length::ZERO,
                master_count: *master_count,
                master_ratio: *master_ratio,
            },
        );
    }

    fn attach_window(&mut self, hub: &mut HubAccess, id: WindowId, ws_id: WorkspaceId) {
        hub.windows.get_mut(id).set_workspace(Some(ws_id));
        self.place(hub, ws_id, id);
        hub.workspaces.get_mut(ws_id).is_float_focused = false;
        self.compute_placement(hub, ws_id);
    }

    fn detach_window(&mut self, hub: &HubAccess, id: WindowId) -> Dimension {
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

        let y_offset = state.remove_window(id);

        let dim = self
            .window_states
            .remove(&id)
            .unwrap_or_else(|| {
                panic!("master: detach_window called for {id:?} but window_states has no entry")
            })
            .dimension;
        let result = Dimension::new(
            dim.x + screen.x,
            dim.y - y_offset + screen.y,
            dim.width,
            dim.height,
        );

        self.compute_placement(hub, ws_id);
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
        let exists = state.master.contains(&window_id) || state.secondary.contains(&window_id);
        if !exists {
            return;
        }
        state.focus = Some(window_id);
        hub.workspaces.get_mut(ws_id).is_float_focused = false;
        self.scroll_into_view(hub, ws_id);
    }

    fn focused_tiling_window(&self, ws_id: WorkspaceId) -> Option<WindowId> {
        self.workspaces.get(&ws_id).and_then(|s| s.focus)
    }

    fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        focused: bool,
    ) -> TilingPlacements {
        self.collect_placements(hub, ws_id, focused)
    }

    fn handle_action(&mut self, hub: &mut HubAccess, action: TilingAction) {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;

        let (pane, idx, master_len, stack_len) = {
            let Some(state) = self.workspaces.get(&ws_id) else {
                return;
            };
            let Some(focus_id) = state.focus else {
                return;
            };
            let (pane, idx) = state
                .find_window(focus_id)
                .unwrap_or_else(|| panic!("focus {focus_id:?} not found in workspace {ws_id}"));
            (pane, idx, state.master.len(), state.secondary.len())
        };

        match action {
            TilingAction::FocusDirection { direction, forward } => {
                if master_len + stack_len <= 1 {
                    return;
                }
                match (direction, forward) {
                    (Direction::Horizontal, false) => {
                        if pane == Pane::Secondary && master_len > 0 {
                            let state = self.workspaces.get_mut(&ws_id).unwrap();
                            state.focus = state.master.first().copied();
                        }
                    }
                    (Direction::Horizontal, true) => {
                        if pane == Pane::Master && stack_len > 0 {
                            let state = self.workspaces.get_mut(&ws_id).unwrap();
                            state.focus = state.secondary.first().copied();
                        }
                    }
                    (Direction::Vertical, false) => {
                        let len = match pane {
                            Pane::Master => master_len,
                            Pane::Secondary => stack_len,
                        };
                        if len <= 1 {
                            return;
                        }
                        let new_idx = if idx == 0 { len - 1 } else { idx - 1 };
                        let state = self.workspaces.get_mut(&ws_id).unwrap();
                        state.focus = match pane {
                            Pane::Master => state.master.get(new_idx).copied(),
                            Pane::Secondary => state.secondary.get(new_idx).copied(),
                        };
                    }
                    (Direction::Vertical, true) => {
                        let len = match pane {
                            Pane::Master => master_len,
                            Pane::Secondary => stack_len,
                        };
                        if len <= 1 {
                            return;
                        }
                        let new_idx = if idx == len - 1 { 0 } else { idx + 1 };
                        let state = self.workspaces.get_mut(&ws_id).unwrap();
                        state.focus = match pane {
                            Pane::Master => state.master.get(new_idx).copied(),
                            Pane::Secondary => state.secondary.get(new_idx).copied(),
                        };
                    }
                }
                let state = self.workspaces.get(&ws_id).unwrap();
                if state.focus.is_some() {
                    self.scroll_into_view(hub, ws_id);
                }
            }
            TilingAction::MoveDirection { direction, forward } => {
                if master_len + stack_len <= 1 {
                    return;
                }
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                match (direction, forward) {
                    (Direction::Horizontal, false) => {
                        if pane == Pane::Secondary {
                            let moved = state.secondary.remove(idx);
                            let count = self.master_count;
                            let effective = state.master_count.unwrap_or(count);
                            if state.master.len() >= effective && master_len > 0 {
                                let swapped = state.master.pop().unwrap();
                                state.master.push(moved);
                                state.secondary.push(swapped);
                                state.focus = Some(moved);
                            } else if state.master.len() < effective {
                                state.master.push(moved);
                                state.focus = Some(moved);
                            }
                        }
                    }
                    (Direction::Horizontal, true) => {
                        if pane == Pane::Master && stack_len > 0 {
                            let moved = state.master.remove(idx);
                            let swapped = state.secondary.remove(0);
                            state.master.push(swapped);
                            state.secondary.push(moved);
                            state.focus = Some(moved);
                        }
                    }
                    (Direction::Vertical, false) => {
                        let len = match pane {
                            Pane::Master => state.master.len(),
                            Pane::Secondary => state.secondary.len(),
                        };
                        if len <= 1 {
                            return;
                        }
                        let target = if idx == 0 { len - 1 } else { idx - 1 };
                        let vec = match pane {
                            Pane::Master => &mut state.master,
                            Pane::Secondary => &mut state.secondary,
                        };
                        vec.swap(idx, target);
                        state.focus = Some(vec[target]);
                    }
                    (Direction::Vertical, true) => {
                        let len = match pane {
                            Pane::Master => state.master.len(),
                            Pane::Secondary => state.secondary.len(),
                        };
                        if len <= 1 {
                            return;
                        }
                        let target = if idx == len - 1 { 0 } else { idx + 1 };
                        let vec = match pane {
                            Pane::Master => &mut state.master,
                            Pane::Secondary => &mut state.secondary,
                        };
                        vec.swap(idx, target);
                        state.focus = Some(vec[target]);
                    }
                }
                self.compute_placement(hub, ws_id);
            }
            TilingAction::GrowMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                let global_ratio = self.master_ratio;
                let current = state.master_ratio.unwrap_or(global_ratio);
                state.master_ratio = Some((current + 0.05).clamp(0.1, 0.9));
                self.compute_placement(hub, ws_id);
            }
            TilingAction::ShrinkMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                let global_ratio = self.master_ratio;
                let current = state.master_ratio.unwrap_or(global_ratio);
                state.master_ratio = Some((current - 0.05).clamp(0.1, 0.9));
                self.compute_placement(hub, ws_id);
            }
            TilingAction::MoreMaster => {
                let global_count = self.master_count;
                {
                    let state = self.workspaces.get_mut(&ws_id).unwrap();
                    let current = state.master_count.unwrap_or(global_count);
                    state.master_count = Some(current + 1);
                }
                self.reconcile_master_count(ws_id);
                self.compute_placement(hub, ws_id);
            }
            TilingAction::FewerMaster => {
                let global_count = self.master_count;
                let current = self
                    .workspaces
                    .get(&ws_id)
                    .and_then(|s| s.master_count)
                    .unwrap_or(global_count);
                if current <= 1 {
                    return;
                }
                {
                    let state = self.workspaces.get_mut(&ws_id).unwrap();
                    state.master_count = Some(current - 1);
                }
                self.reconcile_master_count(ws_id);
                self.compute_placement(hub, ws_id);
            }
            _ => {}
        }
    }

    fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        self.compute_placement_against_constraint(hub, ws_id);
    }

    fn tiling_window_count(&self, ws_id: WorkspaceId) -> usize {
        self.workspaces
            .get(&ws_id)
            .map_or(0, |ws| ws.master.len() + ws.secondary.len())
    }

    fn detach_focused_child(&mut self, hub: &HubAccess, ws_id: WorkspaceId) -> Option<Child> {
        let state = self.workspaces.get_mut(&ws_id)?;
        let focus_id = state.focus?;

        state.remove_window(focus_id);

        self.window_states.remove(&focus_id);
        self.compute_placement(hub, ws_id);

        Some(Child::Window(focus_id))
    }

    fn reattach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId) {
        let Child::Window(id) = child else {
            panic!("MasterStrategy does not support Container children");
        };
        self.attach_window(hub, id, ws_id);
        self.set_focus(hub, id);
    }

    fn migrate(&mut self, ws_id: WorkspaceId) -> (Vec<WindowId>, Option<WindowId>) {
        let focused = self.focused_tiling_window(ws_id);
        let mut tiling = Vec::new();
        if let Some(state) = self.workspaces.remove(&ws_id) {
            tiling.extend(state.master.iter().copied());
            tiling.extend(state.secondary.iter().copied());
            for &wid in &state.master {
                self.window_states.remove(&wid);
            }
            for &wid in &state.secondary {
                self.window_states.remove(&wid);
            }
            for &id in &state.master_matchers {
                self.matchers.delete(id);
            }
            for &id in &state.secondary_matchers {
                self.matchers.delete(id);
            }
        }
        (tiling, focused)
    }

    fn sync_preferred_layout(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        incoming: Option<&LayoutWorkspaceConfig>,
    ) {
        self.do_sync_preferred_layout(hub, ws_id, incoming)
    }

    fn apply_config(&mut self, hub: &mut HubAccess, layout: GlobalLayoutConfig) {
        let old_master_count = self.master_count;
        self.master_ratio = layout.master.master_ratio;
        self.master_count = layout.master.master_count;
        self.size_constraints = layout.size_constraints;
        for ws_id in self.workspaces.keys().copied().collect::<Vec<_>>() {
            let needs_reconcile = self
                .workspaces
                .get(&ws_id)
                .map(|s| s.master_count.is_none() && old_master_count != self.master_count)
                .unwrap_or(false);
            if needs_reconcile {
                self.reconcile_master_count(ws_id);
            }
            self.compute_placement(hub, ws_id);
        }
    }

    fn export_workspace(&mut self, hub: &HubAccess, ws_id: WorkspaceId) -> Option<WorkspaceExport> {
        self.do_export_workspace(hub, ws_id)
    }
}

impl MasterStrategy {
    pub(crate) fn new(
        master_count: usize,
        master_ratio: f32,
        size_constraints: SizeConstraints,
    ) -> Self {
        Self {
            master_count,
            master_ratio,
            size_constraints,
            workspaces: HashMap::new(),
            window_states: HashMap::new(),
            matchers: Allocator::new(),
        }
    }

    fn place(&mut self, hub: &HubAccess, ws_id: WorkspaceId, id: WindowId) {
        let metadata = hub.windows.get(id).metadata.as_ref();

        let matcher = if let Some(matcher_id) =
            self.insert_window_against_preferred_layout(ws_id, id, metadata)
        {
            Some(matcher_id)
        } else {
            let state = self.workspaces.get_mut(&ws_id).unwrap();
            let effective_count = state.master_count.unwrap_or(self.master_count);
            // This window doesn't match any slot
            if state.master.len() < effective_count {
                state.master.push(id);
            } else {
                state.secondary.push(id);
            };
            state.focus = Some(id);
            None
        };

        self.window_states.insert(
            id,
            WindowState {
                occupy: matcher,
                // Only a place holder, will be populated later
                dimension: Dimension::default(),
            },
        );
    }

    fn reconcile_master_count(&mut self, ws_id: WorkspaceId) {
        let Some(state) = self.workspaces.get_mut(&ws_id) else {
            return;
        };
        let effective_count = state.master_count.unwrap_or(self.master_count);

        while state.master.len() < effective_count {
            let pos = state.secondary.iter().position(|&w| {
                self.window_states
                    .get(&w)
                    .is_some_and(|e| e.occupy.is_none())
            });
            if let Some(pos) = pos {
                let wid = state.secondary.remove(pos);
                state.master.push(wid);
            } else {
                break;
            }
        }

        while state.master.len() > effective_count {
            if let Some(wid) = state.master.pop() {
                state.secondary.insert(0, wid);
            }
        }
    }

    fn pane_content_height(
        &self,
        hub: &HubAccess,
        pane_windows: &[WindowId],
        pane_height: Length,
    ) -> Length {
        let heights = self.pane_slot_heights(hub, pane_windows, pane_height);
        heights.iter().copied().sum()
    }

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
                let c = effective_constraints(hub, &self.size_constraints, id);
                (c.min_height, c.max_height)
            })
            .collect();
        distribute_space(&constraints, pane_height)
    }
}

/// Per-workspace state for master-stack layout.
#[derive(Debug)]
struct WorkspaceState {
    master: Vec<WindowId>,
    secondary: Vec<WindowId>,
    master_matchers: Vec<MatcherId>,
    secondary_matchers: Vec<MatcherId>,
    focus: Option<WindowId>,
    master_y_offset: Length,
    stack_y_offset: Length,
    master_count: Option<usize>,
    master_ratio: Option<f32>,
}

impl WorkspaceState {
    fn remove_window(&mut self, window_id: WindowId) -> Length {
        let (pane, idx) = self.find_window(window_id).unwrap();

        let (active, other, y_offset) = match pane {
            Pane::Master => (&mut self.master, &self.secondary, self.master_y_offset),
            Pane::Secondary => (&mut self.secondary, &self.master, self.stack_y_offset),
        };

        active.remove(idx);

        if self.focus == Some(window_id) {
            self.focus = active
                .get(idx)
                .copied()
                .or_else(|| idx.checked_sub(1).and_then(|i| active.get(i).copied()))
                .or_else(|| other.first().copied());
        }
        y_offset
    }

    fn find_window(&self, id: WindowId) -> Option<(Pane, usize)> {
        self.master
            .iter()
            .position(|&w| w == id)
            .map(|i| (Pane::Master, i))
            .or_else(|| {
                self.secondary
                    .iter()
                    .position(|&w| w == id)
                    .map(|i| (Pane::Secondary, i))
            })
    }
}

/// Per-window state: matcher slot occupancy and computed dimension.
#[derive(Debug)]
struct WindowState {
    occupy: Option<MatcherId>,
    dimension: Dimension,
}

/// Which side of the master-stack split a window lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Master,
    Secondary,
}

fn effective_constraints(
    hub: &HubAccess,
    size_constraints: &SizeConstraints,
    wid: WindowId,
) -> Constraints {
    let ws_id = hub
        .windows
        .get(wid)
        .workspace()
        .expect("tiling window has a workspace");
    let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
    let scale = monitor.scale;
    let screen = monitor.dimension;

    let global_min_w = size_constraints.minimum_width.resolve(screen.width, scale);
    let global_min_h = size_constraints
        .minimum_height
        .resolve(screen.height, scale);
    let global_max_w = size_constraints.maximum_width.resolve(screen.width, scale);
    let global_max_h = size_constraints
        .maximum_height
        .resolve(screen.height, scale);

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

    Constraints {
        min_width: min_w,
        min_height: min_h,
        max_width: max_w,
        max_height: max_h,
    }
}
