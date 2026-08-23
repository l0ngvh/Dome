mod export;
mod placement;
mod preferred_layout;
mod scroll;
#[cfg(test)]
mod validate;

use rustc_hash::FxHashMap;

use crate::config::{LayoutWorkspaceConfig, SizeConstraints};
use crate::core::GlobalLayoutConfig;
use crate::core::allocator::Allocator;
use crate::core::hub::HubAccess;
use crate::core::master::preferred_layout::{Slot, SlotId};
use crate::core::node::{
    Child, Dimension, Direction, Length, PixelRect, WindowId, WindowMetadata, WorkspaceId,
};
use crate::core::strategy::{
    TilingAction, TilingPlacements, TilingStrategy, WorkspaceExport, distribute_space, translate,
    window_constraints,
};

/// XMonad-style tiling: a master area on the left and a stack on the right.
/// No containers, no tabs. Each pane scrolls vertically and independently when
/// per-window min heights push the pane's total content past the screen height.
/// Horizontal scroll does not exist in master.
#[derive(Debug)]
pub(crate) struct MasterStrategy {
    workspaces: FxHashMap<WorkspaceId, WorkspaceState>,
    window_states: FxHashMap<WindowId, WindowState>,
    slots: Allocator<Slot>,
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
                    focus_history: Vec::new(),
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

        let master_ids: Vec<SlotId> = master
            .iter()
            .map(|m| {
                self.slots.allocate(Slot {
                    matcher: m.clone(),
                    windows: Vec::new(),
                })
            })
            .collect();
        let secondary_ids: Vec<SlotId> = secondary
            .iter()
            .map(|m| {
                self.slots.allocate(Slot {
                    matcher: m.clone(),
                    windows: Vec::new(),
                })
            })
            .collect();

        self.workspaces.insert(
            ws_id,
            WorkspaceState {
                master: Vec::new(),
                secondary: Vec::new(),
                master_matchers: master_ids,
                secondary_matchers: secondary_ids,
                focus_history: Vec::new(),
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
        self.compute_placement(hub, ws_id);
    }

    fn detach_window(&mut self, hub: &mut HubAccess, id: WindowId) -> PixelRect {
        let ws_id = hub
            .windows
            .get(id)
            .workspace()
            .expect("detaching tiling window has a workspace");
        let work_area = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .work_area;

        let state = self.workspaces.get_mut(&ws_id).unwrap_or_else(|| {
            panic!("master: detach_window called for {id:?} but workspace {ws_id} has no state")
        });

        let y_offset = state.remove_window(id);

        let removed = self.window_states.remove(&id).unwrap_or_else(|| {
            panic!("master: detach_window called for {id:?} but window_states has no entry")
        });
        if let Some(sid) = removed.occupy {
            self.slots.get_mut(sid).windows.retain(|w| w != &id);
        }
        let dim = removed.dimension;
        let result = translate(dim, Length::ZERO, y_offset, work_area.x(), work_area.y());

        self.reconcile_master_count(hub, ws_id);
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
        state.record_focus(window_id);
        self.scroll_into_view(hub, ws_id);
    }

    fn focused_tiling_window(&self, ws_id: WorkspaceId) -> Option<WindowId> {
        self.workspaces
            .get(&ws_id)
            .and_then(WorkspaceState::focused_window)
    }

    fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        focused: bool,
    ) -> TilingPlacements {
        self.collect_tiling_placements(hub, ws_id, focused)
    }

    fn handle_action(&mut self, hub: &mut HubAccess, action: TilingAction) {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;

        let (pane, idx, master_len, stack_len) = {
            let Some(state) = self.workspaces.get(&ws_id) else {
                return;
            };
            let Some((pane, idx)) = state.focused_position() else {
                return;
            };
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
                            let target = state.last_focused_in(Pane::Master);
                            state.record_focus(target);
                        }
                    }
                    (Direction::Horizontal, true) => {
                        if pane == Pane::Master && stack_len > 0 {
                            let state = self.workspaces.get_mut(&ws_id).unwrap();
                            let target = state.last_focused_in(Pane::Secondary);
                            state.record_focus(target);
                        }
                    }
                    (Direction::Vertical, _) => {
                        let state = self.workspaces.get_mut(&ws_id).unwrap();
                        let len = state.pane_vec(pane).len();
                        if len <= 1 {
                            return;
                        }
                        let target = state.pane_vec(pane)[wrap_index(idx, len, forward)];
                        state.record_focus(target);
                    }
                }
                self.scroll_into_view(hub, ws_id);
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
                                let moved_slots = state.master_matchers.clone();
                                let swapped_slots = state.secondary_matchers.clone();
                                self.remap_slot_on_pane_change(hub, ws_id, moved, &moved_slots);
                                self.remap_slot_on_pane_change(hub, ws_id, swapped, &swapped_slots);
                            } else if state.master.len() < effective {
                                state.master.push(moved);
                                let moved_slots = state.master_matchers.clone();
                                self.remap_slot_on_pane_change(hub, ws_id, moved, &moved_slots);
                            }
                        }
                    }
                    (Direction::Horizontal, true) => {
                        if pane == Pane::Master && stack_len > 0 {
                            let moved = state.master.remove(idx);
                            let swapped = state.secondary.remove(0);
                            state.master.push(swapped);
                            state.secondary.push(moved);
                            let moved_slots = state.secondary_matchers.clone();
                            let swapped_slots = state.master_matchers.clone();
                            self.remap_slot_on_pane_change(hub, ws_id, moved, &moved_slots);
                            self.remap_slot_on_pane_change(hub, ws_id, swapped, &swapped_slots);
                        }
                    }
                    (Direction::Vertical, _) => {
                        let len = state.pane_vec(pane).len();
                        if len <= 1 {
                            return;
                        }
                        let target = wrap_index(idx, len, forward);
                        let vec = state.pane_vec_mut(pane);
                        vec.swap(idx, target);
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
                self.reconcile_master_count(hub, ws_id);
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
                self.reconcile_master_count(hub, ws_id);
                self.compute_placement(hub, ws_id);
            }
            _ => {}
        }
    }

    fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        self.compute_placement(hub, ws_id);
    }

    fn tiling_window_count(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> usize {
        self.workspaces
            .get(&ws_id)
            .map_or(0, |ws| ws.master.len() + ws.secondary.len())
    }

    fn matches_tiling(&self, ws_id: WorkspaceId, metadata: &dyn WindowMetadata) -> bool {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return false;
        };
        state
            .master_matchers
            .iter()
            .chain(state.secondary_matchers.iter())
            .any(|&sid| metadata.matches_window_matcher(&self.slots.get(sid).matcher))
    }

    fn detach_focused_child(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Option<Child> {
        let state = self.workspaces.get_mut(&ws_id)?;
        let focus_id = state.focused_window()?;

        state.remove_window(focus_id);

        let removed = self.window_states.remove(&focus_id);
        if let Some(sid) = removed.and_then(|e| e.occupy) {
            self.slots.get_mut(sid).windows.retain(|w| w != &focus_id);
        }
        self.reconcile_master_count(hub, ws_id);
        self.compute_placement(hub, ws_id);

        Some(Child::Window(focus_id))
    }

    fn reattach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId) {
        let arrivals = hub.take_windows(child);
        for &id in &arrivals {
            self.attach_window(hub, id, ws_id);
        }
        if let Some(&focus) = arrivals.first() {
            self.set_focus(hub, focus);
        }
    }

    fn migrate(
        &mut self,
        _hub: &mut HubAccess,
        ws_id: WorkspaceId,
    ) -> (Vec<WindowId>, Option<WindowId>) {
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
                self.slots.delete(id);
            }
            for &id in &state.secondary_matchers {
                self.slots.delete(id);
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
        self.sync_preferred_layout(hub, ws_id, incoming)
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
                self.reconcile_master_count(hub, ws_id);
            }
            self.compute_placement(hub, ws_id);
        }
    }

    fn export_workspace(&mut self, hub: &HubAccess, ws_id: WorkspaceId) -> WorkspaceExport {
        self.export_workspace(hub, ws_id)
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
            workspaces: FxHashMap::default(),
            window_states: FxHashMap::default(),
            slots: Allocator::new(),
        }
    }

    fn place(&mut self, hub: &HubAccess, ws_id: WorkspaceId, id: WindowId) {
        let metadata = hub.windows.get(id).metadata.as_ref();
        let occupy = self.sort_window_into_pane(ws_id, id, metadata);

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.add_to_history(id);

        self.window_states.insert(
            id,
            WindowState {
                occupy,
                // Only a place holder, will be populated later
                dimension: Dimension::default(),
            },
        );
    }

    fn reconcile_master_count(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
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

        let mut overflow = Vec::new();
        while state.master.len() > effective_count {
            if let Some(wid) = state.master.pop() {
                state.secondary.insert(0, wid);
                overflow.push(wid);
            }
        }
        let secondary_slots = state.secondary_matchers.clone();
        for wid in overflow {
            self.remap_slot_on_pane_change(hub, ws_id, wid, &secondary_slots);
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
                let c = window_constraints(hub, &self.size_constraints, id);
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
    master_matchers: Vec<SlotId>,
    secondary_matchers: Vec<SlotId>,
    /// Windows of this workspace from most to least recently focused. Always set-equal to
    /// `master` plus `secondary`.
    focus_history: Vec<WindowId>,
    master_y_offset: Length,
    stack_y_offset: Length,
    master_count: Option<usize>,
    master_ratio: Option<f32>,
}

impl WorkspaceState {
    fn focused_window(&self) -> Option<WindowId> {
        self.focus_history.first().copied()
    }

    /// `None` only for an empty workspace. A focused window outside both panes panics.
    fn focused_position(&self) -> Option<(Pane, usize)> {
        let focus_id = self.focused_window()?;
        Some(self.find_window(focus_id))
    }

    fn record_focus(&mut self, window_id: WindowId) {
        self.drop_from_history(window_id);
        self.focus_history.insert(0, window_id);
    }

    /// Appends as least recently focused, keeping `focus_history` set-equal to the
    /// panes without claiming focus. Idempotent, so a rebuild preserves order.
    fn add_to_history(&mut self, window_id: WindowId) {
        if !self.focus_history.contains(&window_id) {
            self.focus_history.push(window_id);
        }
    }

    fn drop_from_history(&mut self, window_id: WindowId) {
        if let Some(pos) = self.focus_history.iter().position(|&w| w == window_id) {
            self.focus_history.remove(pos);
        }
    }

    fn clear_focus_history(&mut self) {
        self.focus_history.clear();
    }

    /// Membership is read from the pane vector on every call, so a window that migrated between
    /// panes answers for the pane it occupies now and migration sites need no fix-up.
    fn last_focused_in(&self, pane: Pane) -> WindowId {
        let members = self.pane_vec(pane);
        self.focus_history
            .iter()
            .find(|w| members.contains(w))
            .copied()
            .or_else(|| members.first().copied())
            .unwrap_or_else(|| panic!("last_focused_in called on empty {pane:?} pane"))
    }

    fn pane_vec(&self, pane: Pane) -> &[WindowId] {
        match pane {
            Pane::Master => &self.master,
            Pane::Secondary => &self.secondary,
        }
    }

    fn pane_vec_mut(&mut self, pane: Pane) -> &mut Vec<WindowId> {
        match pane {
            Pane::Master => &mut self.master,
            Pane::Secondary => &mut self.secondary,
        }
    }

    /// Focus repair needs no ladder here. Dropping `window_id` from the history leaves the head
    /// on the surviving window focused before it, whichever pane that window lives in.
    fn remove_window(&mut self, window_id: WindowId) -> Length {
        let (pane, idx) = self.find_window(window_id);

        let y_offset = match pane {
            Pane::Master => self.master_y_offset,
            Pane::Secondary => self.stack_y_offset,
        };

        self.pane_vec_mut(pane).remove(idx);
        self.drop_from_history(window_id);
        y_offset
    }

    fn find_window(&self, id: WindowId) -> (Pane, usize) {
        if let Some(i) = self.master.iter().position(|&w| w == id) {
            return (Pane::Master, i);
        }
        let i = self
            .secondary
            .iter()
            .position(|&w| w == id)
            .unwrap_or_else(|| panic!("window {id:?} is in neither master nor secondary pane"));
        (Pane::Secondary, i)
    }
}

/// Per-window state: matcher slot occupancy and computed dimension.
#[derive(Debug)]
struct WindowState {
    occupy: Option<SlotId>,
    dimension: Dimension,
}

/// Which side of the master-stack split a window lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Master,
    Secondary,
}

fn wrap_index(idx: usize, len: usize, forward: bool) -> usize {
    if forward {
        if idx + 1 == len { 0 } else { idx + 1 }
    } else if idx == 0 {
        len - 1
    } else {
        idx - 1
    }
}
