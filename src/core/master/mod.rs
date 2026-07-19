mod preferred_layout;

use std::collections::HashMap;

use crate::config::{LayoutWorkspaceConfig, SizeConstraints, WindowMatcher};
use crate::core::GlobalLayoutConfig;
use crate::core::allocator::Allocator;
use crate::core::hub::{HubAccess, TilingWindowPlacement};
use crate::core::master::preferred_layout::MatcherId;
use crate::core::node::{Child, Constraints, Dimension, Direction, Length, WindowId, WorkspaceId};
use crate::core::strategy::{
    TilingAction, TilingPlacements, TilingStrategy, WorkspaceExport, clip, distribute_space,
    translate,
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

        let (pane, idx) = find_window(state, id).unwrap_or_else(|| {
            panic!("master: detach_window called for {id:?} but window is not in workspace {ws_id}")
        });

        let y_offset = match pane {
            Pane::Master => state.master_y_offset,
            Pane::Secondary => state.stack_y_offset,
        };

        match pane {
            Pane::Master => state.master.remove(idx),
            Pane::Secondary => state.secondary.remove(idx),
        };

        Self::adjust_focus_after_removal(state, id, pane, idx);

        if state.master.is_empty() && state.secondary.is_empty() {
            let ws = hub.workspaces.get_mut(ws_id);
            ws.is_float_focused = !ws.float_windows.is_empty();
        }

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

        let mut windows = Vec::with_capacity(state.master.len() + state.secondary.len());

        let focused_id = if highlighted && !ws.is_float_focused {
            state.focus
        } else {
            None
        };

        let mut push_pane = |_pane: Pane, vec: &[WindowId], y_offset: Length| {
            for &wid in vec.iter() {
                let dim = self.window_states[&wid].dimension;
                let frame = translate(dim, Length::ZERO, y_offset, screen);
                if let Some(visible_frame) = clip(frame, screen) {
                    let is_highlighted = focused_id == Some(wid);
                    windows.push(TilingWindowPlacement {
                        id: wid,
                        frame,
                        visible_frame,
                        is_highlighted,
                        spawn_indicator: None,
                    });
                }
            }
        };

        push_pane(Pane::Master, &state.master, state.master_y_offset);
        push_pane(Pane::Secondary, &state.secondary, state.stack_y_offset);

        TilingPlacements {
            windows,
            containers: Vec::new(),
        }
    }

    fn handle_action(&mut self, hub: &mut HubAccess, action: TilingAction) {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;

        let (_focus_id, pane, idx, master_len, stack_len) = {
            let Some(state) = self.workspaces.get(&ws_id) else {
                return;
            };
            let Some(focus_id) = state.focus else {
                return;
            };
            let (pane, idx) = find_window(state, focus_id)
                .unwrap_or_else(|| panic!("focus {focus_id:?} not found in workspace {ws_id}"));
            (
                focus_id,
                pane,
                idx,
                state.master.len(),
                state.secondary.len(),
            )
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
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::GrowMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                let global_ratio = self.master_ratio;
                let current = state.master_ratio.unwrap_or(global_ratio);
                state.master_ratio = Some((current + 0.05).clamp(0.1, 0.9));
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::ShrinkMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                let global_ratio = self.master_ratio;
                let current = state.master_ratio.unwrap_or(global_ratio);
                state.master_ratio = Some((current - 0.05).clamp(0.1, 0.9));
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::MoreMaster => {
                let global_count = self.master_count;
                {
                    let state = self.workspaces.get_mut(&ws_id).unwrap();
                    let current = state.master_count.unwrap_or(global_count);
                    state.master_count = Some(current + 1);
                }
                self.reconcile_master_count(ws_id);
                self.layout_workspace(hub, ws_id);
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

    fn tiling_window_count(&self, ws_id: WorkspaceId) -> usize {
        self.workspaces
            .get(&ws_id)
            .map_or(0, |ws| ws.master.len() + ws.secondary.len())
    }

    fn detach_focused_child(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Option<Child> {
        let state = self.workspaces.get(&ws_id)?;
        let focus_id = state.focus?;

        let (pane, idx) = find_window(state, focus_id)?;

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        match pane {
            Pane::Master => state.master.remove(idx),
            Pane::Secondary => state.secondary.remove(idx),
        };
        Self::adjust_focus_after_removal(state, focus_id, pane, idx);

        if state.master.is_empty() && state.secondary.is_empty() {
            let ws = hub.workspaces.get_mut(ws_id);
            ws.is_float_focused = !ws.float_windows.is_empty();
        }

        self.window_states.remove(&focus_id);
        self.layout_workspace(hub, ws_id);

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
            self.layout_workspace(hub, ws_id);
        }
    }

    #[cfg(test)]
    fn validate_tree(&self, hub: &HubAccess) {
        for (&ws_id, state) in &self.workspaces {
            let mut seen = std::collections::HashSet::new();
            for &wid in state.master.iter().chain(state.secondary.iter()) {
                hub.windows.get(wid);
                assert!(
                    seen.insert(wid),
                    "master-stack workspace {ws_id}: duplicate window {wid:?}"
                );
            }
            let effective_count = state.master_count.unwrap_or(self.master_count);
            assert!(
                state.master.len() <= effective_count,
                "master-stack workspace {ws_id}: master.len() {} > master_count {effective_count}",
                state.master.len()
            );

            match state.focus {
                Some(fid) => {
                    let exists = state.master.contains(&fid) || state.secondary.contains(&fid);
                    assert!(
                        exists,
                        "master-stack workspace {ws_id}: focused window {fid:?} not found"
                    );
                }
                None => {
                    assert!(
                        state.master.is_empty() && state.secondary.is_empty(),
                        "master-stack workspace {ws_id}: focus is None but windows exist"
                    );
                }
            }

            for &wid in state.master.iter().chain(state.secondary.iter()) {
                assert!(
                    self.window_states.contains_key(&wid),
                    "master-stack workspace {ws_id}: window {wid:?} missing from window_states"
                );
            }

            if state.master.is_empty() && state.secondary.is_empty() {
                continue;
            }

            let pane_height = hub
                .monitors
                .get(hub.workspaces.get(ws_id).monitor)
                .dimension
                .height;

            for &wid in &state.master {
                let dim = self.window_states[&wid].dimension;
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
                let c = effective_constraints(hub, &self.size_constraints, wid);
                assert!(
                    dim.width >= c.min_width,
                    "master-stack workspace {ws_id}: window {wid:?} width {} < effective min_width {}",
                    dim.width,
                    c.min_width
                );
                assert!(
                    dim.height >= c.min_height,
                    "master-stack workspace {ws_id}: window {wid:?} height {} < effective min_height {}",
                    dim.height,
                    c.min_height
                );
            }

            for &wid in &state.secondary {
                let dim = self.window_states[&wid].dimension;
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
                let c = effective_constraints(hub, &self.size_constraints, wid);
                assert!(
                    dim.width >= c.min_width,
                    "master-stack workspace {ws_id}: window {wid:?} width {} < effective min_width {}",
                    dim.width,
                    c.min_width
                );
                assert!(
                    dim.height >= c.min_height,
                    "master-stack workspace {ws_id}: window {wid:?} height {} < effective min_height {}",
                    dim.height,
                    c.min_height
                );
            }

            let master_ids: Vec<WindowId> = state.master.clone();
            if !master_ids.is_empty() {
                let master_content_h = self.pane_content_h(hub, &master_ids, pane_height);
                let master_max_offset = (master_content_h - pane_height).max(Length::ZERO);
                assert!(
                    state.master_y_offset >= Length::ZERO
                        && state.master_y_offset <= master_max_offset,
                    "master-stack workspace {ws_id}: master_y_offset {} out of bounds [0, {}]",
                    state.master_y_offset,
                    master_max_offset
                );
            } else {
                assert!(
                    state.master_y_offset == Length::ZERO,
                    "master-stack workspace {ws_id}: master_y_offset should be zero (no master windows)"
                );
            }

            let stack_ids: Vec<WindowId> = state.secondary.clone();
            if !stack_ids.is_empty() {
                let stack_content_h = self.pane_content_h(hub, &stack_ids, pane_height);
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
                    "master-stack workspace {ws_id}: stack_y_offset should be zero (no stack windows)"
                );
            }
        }
    }

    fn export_workspace(&mut self, hub: &HubAccess, ws_id: WorkspaceId) -> Option<WorkspaceExport> {
        let state = self.workspaces.get(&ws_id)?;

        let master: Vec<WindowMatcher> = state
            .master
            .iter()
            .map(
                |&wid| match self.window_states.get(&wid).and_then(|e| e.occupy) {
                    Some(mid) => self.matchers.get(mid).clone(),
                    None => hub.windows.get(wid).metadata.to_window_matcher(),
                },
            )
            .collect();
        let secondary: Vec<WindowMatcher> = state
            .secondary
            .iter()
            .map(
                |&wid| match self.window_states.get(&wid).and_then(|e| e.occupy) {
                    Some(mid) => self.matchers.get(mid).clone(),
                    None => hub.windows.get(wid).metadata.to_window_matcher(),
                },
            )
            .collect();

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let old_master_ids = state.master_matchers.clone();
        let old_secondary_ids = state.secondary_matchers.clone();

        for &id in &state.master_matchers {
            self.matchers.delete(id);
        }
        for &id in &state.secondary_matchers {
            self.matchers.delete(id);
        }

        state.master_matchers = master
            .iter()
            .map(|m| self.matchers.allocate(m.clone()))
            .collect();
        state.secondary_matchers = secondary
            .iter()
            .map(|m| self.matchers.allocate(m.clone()))
            .collect();

        for &wid in &state.master {
            let new_occupy = self
                .window_states
                .get(&wid)
                .and_then(|e| e.occupy)
                .and_then(|old_id| {
                    old_master_ids
                        .iter()
                        .position(|&x| x == old_id)
                        .and_then(|slot| state.master_matchers.get(slot).copied())
                        .or_else(|| {
                            old_secondary_ids
                                .iter()
                                .position(|&x| x == old_id)
                                .and_then(|slot| state.secondary_matchers.get(slot).copied())
                        })
                });
            if let Some(entry) = self.window_states.get_mut(&wid) {
                entry.occupy = new_occupy;
            }
        }
        for &wid in &state.secondary {
            let new_occupy = self
                .window_states
                .get(&wid)
                .and_then(|e| e.occupy)
                .and_then(|old_id| {
                    old_master_ids
                        .iter()
                        .position(|&x| x == old_id)
                        .and_then(|slot| state.master_matchers.get(slot).copied())
                        .or_else(|| {
                            old_secondary_ids
                                .iter()
                                .position(|&x| x == old_id)
                                .and_then(|slot| state.secondary_matchers.get(slot).copied())
                        })
                });
            if let Some(entry) = self.window_states.get_mut(&wid) {
                entry.occupy = new_occupy;
            }
        }

        Some(WorkspaceExport {
            strategy: "master".into(),
            master_ratio: state.master_ratio,
            master_count: state.master_count,
            master,
            secondary,
            ..Default::default()
        })
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

    fn do_layout(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let master_n = state.master.len();
        let stack_n = state.secondary.len();
        if master_n == 0 && stack_n == 0 {
            return;
        }

        let screen = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension;
        let h = screen.height;

        let master_ids: Vec<WindowId> = state.master.clone();
        let stack_ids: Vec<WindowId> = state.secondary.clone();

        match (master_n, stack_n) {
            (_, 0) => {
                self.do_pane_layout(hub, &master_ids, screen.width, Length::ZERO, h);
            }
            (0, _) => {
                self.do_pane_layout(hub, &stack_ids, screen.width, Length::ZERO, h);
            }
            (_, _) => {
                let master_min_w = master_ids
                    .iter()
                    .map(|&id| effective_constraints(hub, &self.size_constraints, id).min_width)
                    .fold(Length::ZERO, Length::max);
                let stack_min_w = stack_ids
                    .iter()
                    .map(|&id| effective_constraints(hub, &self.size_constraints, id).min_width)
                    .fold(Length::ZERO, Length::max);

                let desired_master_w = Length::new(
                    (screen.width.value() * state.master_ratio.unwrap_or(self.master_ratio))
                        .floor(),
                );
                let total_min = master_min_w + stack_min_w;

                let (master_w, stack_w) = if total_min >= screen.width {
                    (master_min_w, stack_min_w)
                } else if desired_master_w < master_min_w {
                    (master_min_w, screen.width - master_min_w)
                } else if screen.width - desired_master_w < stack_min_w {
                    (screen.width - stack_min_w, stack_min_w)
                } else {
                    (desired_master_w, screen.width - desired_master_w)
                };

                self.do_pane_layout(hub, &master_ids, master_w, Length::ZERO, h);
                self.do_pane_layout(hub, &stack_ids, stack_w, master_w, h);
            }
        }

        self.clamp_scroll(hub, ws_id);
        self.scroll_into_view(hub, ws_id);
    }

    fn do_pane_layout(
        &mut self,
        hub: &HubAccess,
        ids: &[WindowId],
        pane_width: Length,
        x_start: Length,
        h: Length,
    ) {
        if ids.is_empty() {
            return;
        }
        let pane_min_w = ids
            .iter()
            .map(|&id| effective_constraints(hub, &self.size_constraints, id).min_width)
            .fold(Length::ZERO, Length::max);
        let adjusted_w = pane_min_w.max(pane_width);

        let constraints: Vec<(Length, Length)> = ids
            .iter()
            .map(|&id| {
                let c = effective_constraints(hub, &self.size_constraints, id);
                (c.min_height, c.max_height)
            })
            .collect();
        let heights = distribute_space(&constraints, h);
        let sum_h: Length = heights.iter().copied().sum();
        let mut y = if sum_h < h {
            (h - sum_h) / 2.0
        } else {
            Length::ZERO
        };
        for (i, &id) in ids.iter().enumerate() {
            let c = effective_constraints(hub, &self.size_constraints, id);
            let (w, x_off) = apply_max_constraint(c.max_width, adjusted_w);
            let (slot_h, y_off) = apply_max_constraint(c.max_height, heights[i]);
            let dim = Dimension::new(x_start + x_off, y + y_off, w, slot_h);
            self.window_states
                .entry(id)
                .and_modify(|s| s.dimension = dim)
                .or_insert(WindowState {
                    occupy: None,
                    dimension: dim,
                });
            y += heights[i];
        }
    }

    fn adjust_focus_after_removal(
        state: &mut WorkspaceState,
        removed_id: WindowId,
        removed_pane: Pane,
        removed_idx: usize,
    ) {
        if state.focus != Some(removed_id) {
            return;
        }
        let vec = match removed_pane {
            Pane::Master => &state.master,
            Pane::Secondary => &state.secondary,
        };
        let successor = vec
            .get(removed_idx)
            .copied()
            .or_else(|| {
                if removed_idx > 0 {
                    vec.get(removed_idx - 1).copied()
                } else {
                    None
                }
            })
            .or_else(|| {
                let other = match removed_pane {
                    Pane::Master => &state.secondary,
                    Pane::Secondary => &state.master,
                };
                other.first().copied()
            });
        state.focus = successor;
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

        let master_ids: Vec<WindowId> = state.master.clone();
        let master_max = if !master_ids.is_empty() {
            let content_h = self.pane_content_h(hub, &master_ids, pane_height);
            (content_h - pane_height).max(Length::ZERO)
        } else {
            Length::ZERO
        };

        let stack_ids: Vec<WindowId> = state.secondary.clone();
        let stack_max = if !stack_ids.is_empty() {
            let content_h = self.pane_content_h(hub, &stack_ids, pane_height);
            (content_h - pane_height).max(Length::ZERO)
        } else {
            Length::ZERO
        };

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.master_y_offset = state.master_y_offset.clamp(Length::ZERO, master_max);
        state.stack_y_offset = state.stack_y_offset.clamp(Length::ZERO, stack_max);
    }

    fn scroll_into_view(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let Some(focus_id) = state.focus else {
            return;
        };
        let (pane, idx) = match find_window(state, focus_id) {
            Some(v) => v,
            None => return,
        };
        let pane_height = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension
            .height;

        let (pane_windows, offset): (Vec<WindowId>, Length) = match pane {
            Pane::Master => (state.master.clone(), state.master_y_offset),
            Pane::Secondary => (state.secondary.clone(), state.stack_y_offset),
        };

        let slot_heights = self.pane_slot_heights(hub, &pane_windows, pane_height);
        let content_h: Length = slot_heights.iter().copied().sum();
        let max_offset = (content_h - pane_height).max(Length::ZERO);

        let content_start = if content_h < pane_height {
            (pane_height - content_h) / 2.0
        } else {
            Length::ZERO
        };
        let slot_y: Length = content_start + slot_heights[..idx].iter().copied().sum::<Length>();
        let slot_height = slot_heights[idx];

        let mut new_offset = offset;
        if slot_y + slot_height - new_offset > pane_height {
            new_offset = slot_y + slot_height - pane_height;
        }
        if slot_y - new_offset < Length::ZERO {
            new_offset = slot_y;
        }
        new_offset = new_offset.clamp(Length::ZERO, max_offset);

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        match pane {
            Pane::Master => state.master_y_offset = new_offset,
            Pane::Secondary => state.stack_y_offset = new_offset,
        }
    }

    fn pane_content_h(
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

fn find_window(state: &WorkspaceState, id: WindowId) -> Option<(Pane, usize)> {
    state
        .master
        .iter()
        .position(|&w| w == id)
        .map(|i| (Pane::Master, i))
        .or_else(|| {
            state
                .secondary
                .iter()
                .position(|&w| w == id)
                .map(|i| (Pane::Secondary, i))
        })
}

fn apply_max_constraint(max: Length, slot_extent: Length) -> (Length, Length) {
    let size = if max > Length::ZERO && max < slot_extent {
        max
    } else {
        slot_extent
    };
    let offset = (slot_extent - size) / 2.0;
    (size, offset.max(Length::ZERO))
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
