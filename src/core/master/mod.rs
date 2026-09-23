mod export;
mod options;
mod placement;
mod preferred_layout;
mod scroll;
#[cfg(test)]
mod validate;

use rustc_hash::FxHashMap;
use serde::Serialize;

pub(crate) use options::{MasterConfig, read_master_count_override, read_master_ratio_override};
pub(crate) use preferred_layout::PaneConfig;

use crate::core::TilingConfig;
use crate::core::allocator::Allocator;
use crate::core::hub::HubAccess;
use crate::core::master::options::{MIN_MASTER_COUNT, clamp_master_ratio};
use crate::core::master::preferred_layout::{Slot, SlotId};
use crate::core::node::{
    Child, Container, ContainerId, Dimension, Direction, Length, Logical, PixelRect, Pixels,
    WindowId, WindowMetadata, WorkspaceId,
};
use crate::core::strategy::{
    TilingPlacements, TilingStrategy, distribute_space, translate, window_constraints,
};
use crate::core::{PreferredMaster, SizeConstraints, WindowMatcher};

/// XMonad-style tiling: a master area on the left and a stack on the right.
/// Each pane scrolls vertically and independently when per-window min heights push the
/// pane's total content past the screen height. Horizontal scroll does not exist in master,
/// so per-window min width is not honored. The split follows master_ratio and each pane
/// fills its share.
#[derive(Debug)]
pub(crate) struct MasterStrategy {
    workspaces: FxHashMap<WorkspaceId, WorkspaceState>,
    window_states: FxHashMap<WindowId, WindowState>,
    slots: Allocator<Slot>,
    master_count: usize,
    master_ratio: f32,
    size_constraints: SizeConstraints,
    tab_bar_height: Pixels<Logical>,
}

impl TilingStrategy for MasterStrategy {
    fn attach_window(&mut self, hub: &mut HubAccess, id: WindowId, ws_id: WorkspaceId) {
        self.track_window(hub, ws_id, id);
        self.sort_window_into_pane(hub, ws_id, id);
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

        let y_offset = self.remove_window(hub, ws_id, id);

        self.release_slot(id);
        let removed = self.window_states.remove(&id).unwrap_or_else(|| {
            panic!("master: detach_window called for {id:?} but window_states has no entry")
        });
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
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .record_focus(window_id);
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

    fn focus_direction(&mut self, hub: &mut HubAccess, direction: Direction, forward: bool) {
        let Some(FocusedPanes {
            ws_id,
            kind,
            idx,
            master_container,
            secondary_container,
            master_len,
            stack_len,
        }) = self.focused_panes(hub)
        else {
            return;
        };
        if master_len + stack_len <= 1 {
            return;
        }
        match (direction, forward) {
            (Direction::Horizontal, false) => {
                if kind == PaneKind::Secondary && master_len > 0 {
                    let target = self.last_focused_in(hub, ws_id, PaneKind::Master);
                    self.workspaces
                        .get_mut(&ws_id)
                        .unwrap()
                        .record_focus(target);
                }
            }
            (Direction::Horizontal, true) => {
                if kind == PaneKind::Master && stack_len > 0 {
                    let target = self.last_focused_in(hub, ws_id, PaneKind::Secondary);
                    self.workspaces
                        .get_mut(&ws_id)
                        .unwrap()
                        .record_focus(target);
                }
            }
            (Direction::Vertical, _) => {
                let cid = if kind == PaneKind::Master {
                    master_container
                } else {
                    secondary_container
                };
                let members = Self::pane_windows(hub, cid);
                let len = members.len();
                if len <= 1 {
                    return;
                }
                let target = members[wrap_index(idx, len, forward)];
                self.workspaces
                    .get_mut(&ws_id)
                    .unwrap()
                    .record_focus(target);
            }
        }
        self.scroll_into_view(hub, ws_id);
    }

    fn move_direction(&mut self, hub: &mut HubAccess, direction: Direction, forward: bool) {
        let Some(FocusedPanes {
            ws_id,
            kind,
            idx,
            master_container,
            secondary_container,
            master_len,
            stack_len,
        }) = self.focused_panes(hub)
        else {
            return;
        };
        if master_len + stack_len <= 1 {
            return;
        }
        let effective = {
            let state = self.workspaces.get(&ws_id).unwrap();
            state.master_count.unwrap_or(self.master_count)
        };
        match (direction, forward) {
            (Direction::Horizontal, false) => {
                if kind == PaneKind::Secondary {
                    let moved = Self::remove_from_pane(hub, secondary_container, idx);
                    if Self::pane_len(hub, master_container) >= effective && master_len > 0 {
                        let swapped = Self::pop_from_pane(hub, master_container).unwrap();
                        Self::push_to_pane(hub, master_container, moved);
                        Self::push_to_pane(hub, secondary_container, swapped);
                    } else if Self::pane_len(hub, master_container) < effective {
                        Self::push_to_pane(hub, master_container, moved);
                    }
                }
            }
            (Direction::Horizontal, true) => {
                if kind == PaneKind::Master && stack_len > 0 {
                    let moved = Self::remove_from_pane(hub, master_container, idx);
                    let swapped = Self::remove_from_pane(hub, secondary_container, 0);
                    Self::push_to_pane(hub, master_container, swapped);
                    Self::push_to_pane(hub, secondary_container, moved);
                }
            }
            (Direction::Vertical, _) => {
                let cid = if kind == PaneKind::Master {
                    master_container
                } else {
                    secondary_container
                };
                let len = Self::pane_len(hub, cid);
                if len <= 1 {
                    return;
                }
                let target = wrap_index(idx, len, forward);
                hub.containers.get_mut(cid).children.swap(idx, target);
            }
        }
        self.compute_placement(hub, ws_id);
    }

    fn toggle_container_layout(&mut self, hub: &mut HubAccess) {
        let Some(FocusedPanes { ws_id, kind, .. }) = self.focused_panes(hub) else {
            return;
        };
        let pane = self.workspaces.get_mut(&ws_id).unwrap().pane_mut(kind);
        pane.display = match pane.display {
            PaneDisplay::Tiled => PaneDisplay::Tabbed,
            PaneDisplay::Tabbed => PaneDisplay::Tiled,
        };
        self.compute_placement(hub, ws_id);
    }

    fn focus_tab(&mut self, hub: &mut HubAccess, forward: bool) {
        let Some(FocusedPanes {
            ws_id,
            kind,
            idx,
            master_container,
            secondary_container,
            ..
        }) = self.focused_panes(hub)
        else {
            return;
        };
        let cid = if kind == PaneKind::Master {
            master_container
        } else {
            secondary_container
        };
        let is_tabbed =
            self.workspaces.get(&ws_id).unwrap().pane(kind).display == PaneDisplay::Tabbed;
        let members = Self::pane_windows(hub, cid);
        if !is_tabbed || members.len() < 2 {
            return;
        }
        let target = members[wrap_index(idx, members.len(), forward)];
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .record_focus(target);
        self.compute_placement(hub, ws_id);
    }

    fn tab_clicked(&mut self, hub: &mut HubAccess, container_id: ContainerId, index: usize) {
        let Some(FocusedPanes {
            ws_id,
            master_container,
            secondary_container,
            ..
        }) = self.focused_panes(hub)
        else {
            return;
        };
        let clicked_kind = if container_id == master_container {
            PaneKind::Master
        } else if container_id == secondary_container {
            PaneKind::Secondary
        } else {
            return;
        };
        let is_tabbed = self
            .workspaces
            .get(&ws_id)
            .unwrap()
            .pane(clicked_kind)
            .display
            == PaneDisplay::Tabbed;
        let members = Self::pane_windows(hub, container_id);
        if !is_tabbed || members.len() < 2 {
            return;
        }
        let Some(&target) = members.get(index) else {
            return;
        };
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .record_focus(target);
        self.compute_placement(hub, ws_id);
    }

    fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        self.compute_placement(hub, ws_id);
    }

    fn tiling_window_count(&self, hub: &HubAccess, ws_id: WorkspaceId) -> usize {
        self.workspaces.get(&ws_id).map_or(0, |ws| {
            Self::pane_windows(hub, ws.master.container).len()
                + Self::pane_windows(hub, ws.secondary.container).len()
        })
    }

    fn matches_tiling(&self, ws_id: WorkspaceId, metadata: &dyn WindowMetadata) -> bool {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return false;
        };
        self.find_free_slot(&state.master.slots, metadata).is_some()
            || self
                .find_free_slot(&state.secondary.slots, metadata)
                .is_some()
    }

    fn detach_focused_child(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Option<Child> {
        let focus_id = self.workspaces.get(&ws_id)?.focused_window()?;

        self.remove_window(hub, ws_id, focus_id);

        self.release_slot(focus_id);
        self.window_states.remove(&focus_id);
        self.reconcile_master_count(hub, ws_id);
        self.compute_placement(hub, ws_id);

        Some(Child::Window(focus_id))
    }

    fn reattach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId) {
        let arrivals = hub.take_windows(child);
        for &id in &arrivals {
            self.track_window(hub, ws_id, id);
            self.push_unmatched_window(hub, ws_id, id);
        }
        self.compute_placement(hub, ws_id);
        if let Some(&focus) = arrivals.first() {
            self.set_focus(hub, focus);
        }
    }

    fn migrate(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Vec<WindowId> {
        let Some(state) = self.workspaces.remove(&ws_id) else {
            return Vec::new();
        };
        let mut tiling = Vec::new();
        for cid in [state.master.container, state.secondary.container] {
            tiling.extend(Self::pane_windows(hub, cid));
            hub.free_container(cid);
        }
        for &wid in &tiling {
            self.window_states.remove(&wid);
        }
        for &id in state.master.slots.iter().chain(&state.secondary.slots) {
            self.slots.delete(id);
        }
        tiling
    }

    fn apply_config(&mut self, hub: &mut HubAccess, tiling: TilingConfig) {
        let old_master_count = self.master_count;
        self.master_ratio = tiling.master.master_ratio;
        self.master_count = tiling.master.master_count;
        self.size_constraints = tiling.size_constraints;
        self.tab_bar_height = tiling.partition_tree.tab_bar_height;
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
}

impl MasterStrategy {
    pub(crate) fn new(
        master_count: usize,
        master_ratio: f32,
        size_constraints: SizeConstraints,
        tab_bar_height: Pixels<Logical>,
    ) -> Self {
        Self {
            master_count,
            master_ratio,
            size_constraints,
            tab_bar_height,
            workspaces: FxHashMap::default(),
            window_states: FxHashMap::default(),
            slots: Allocator::new(),
        }
    }

    fn allocate_slots(&mut self, matchers: &[WindowMatcher]) -> Vec<SlotId> {
        matchers
            .iter()
            .map(|m| {
                self.slots.allocate(Slot {
                    matcher: m.clone(),
                    window: None,
                })
            })
            .collect()
    }

    pub(super) fn prepare_workspace(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        layout: &PreferredMaster,
    ) {
        let master_container = hub.allocate_container(Container {
            children: Vec::new(),
        });
        let secondary_container = hub.allocate_container(Container {
            children: Vec::new(),
        });
        let master_slots = self.allocate_slots(&layout.master.children);
        let secondary_slots = self.allocate_slots(&layout.secondary.children);
        self.workspaces.insert(
            ws_id,
            WorkspaceState {
                master: Pane::new(master_container, master_slots, layout.master.display),
                secondary: Pane::new(
                    secondary_container,
                    secondary_slots,
                    layout.secondary.display,
                ),
                focus_history: Vec::new(),
                master_count: layout.master_count,
                master_ratio: layout.master_ratio,
            },
        );
    }

    pub(super) fn grow(&mut self, hub: &mut HubAccess) {
        let Some(FocusedPanes { ws_id, .. }) = self.focused_panes(hub) else {
            return;
        };
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let global_ratio = self.master_ratio;
        let current = state.master_ratio.unwrap_or(global_ratio);
        state.master_ratio = Some(clamp_master_ratio(current + 0.05));
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn shrink(&mut self, hub: &mut HubAccess) {
        let Some(FocusedPanes { ws_id, .. }) = self.focused_panes(hub) else {
            return;
        };
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let global_ratio = self.master_ratio;
        let current = state.master_ratio.unwrap_or(global_ratio);
        state.master_ratio = Some(clamp_master_ratio(current - 0.05));
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn more(&mut self, hub: &mut HubAccess) {
        let Some(FocusedPanes { ws_id, .. }) = self.focused_panes(hub) else {
            return;
        };
        let global_count = self.master_count;
        {
            let state = self.workspaces.get_mut(&ws_id).unwrap();
            let current = state.master_count.unwrap_or(global_count);
            state.master_count = Some(current + 1);
        }
        self.reconcile_master_count(hub, ws_id);
        self.compute_placement(hub, ws_id);
    }

    pub(super) fn fewer(&mut self, hub: &mut HubAccess) {
        let Some(FocusedPanes { ws_id, .. }) = self.focused_panes(hub) else {
            return;
        };
        let global_count = self.master_count;
        let current = self
            .workspaces
            .get(&ws_id)
            .and_then(|s| s.master_count)
            .unwrap_or(global_count);
        if current <= MIN_MASTER_COUNT {
            return;
        }
        {
            let state = self.workspaces.get_mut(&ws_id).unwrap();
            state.master_count = Some(current - 1);
        }
        self.reconcile_master_count(hub, ws_id);
        self.compute_placement(hub, ws_id);
    }

    fn tab_bar_length(&self, scale: f32) -> Length {
        Length::from_pixels(self.tab_bar_height).to_unit(scale)
    }
    fn pane_windows(hub: &HubAccess, container: ContainerId) -> Vec<WindowId> {
        hub.containers
            .get(container)
            .children()
            .iter()
            .filter_map(|c| match c {
                Child::Window(w) => Some(*w),
                Child::Container(_) => None,
            })
            .collect()
    }

    fn pane_len(hub: &HubAccess, container: ContainerId) -> usize {
        hub.containers.get(container).children().len()
    }

    fn position_in_pane(hub: &HubAccess, container: ContainerId, id: WindowId) -> Option<usize> {
        hub.containers
            .get(container)
            .children()
            .iter()
            .position(|c| matches!(c, Child::Window(w) if *w == id))
    }

    fn push_to_pane(hub: &mut HubAccess, container: ContainerId, id: WindowId) {
        hub.containers
            .get_mut(container)
            .children
            .push(Child::Window(id));
    }

    fn insert_into_pane(hub: &mut HubAccess, container: ContainerId, idx: usize, id: WindowId) {
        hub.containers
            .get_mut(container)
            .children
            .insert(idx, Child::Window(id));
    }

    fn remove_from_pane(hub: &mut HubAccess, container: ContainerId, idx: usize) -> WindowId {
        match hub.containers.get_mut(container).children.remove(idx) {
            Child::Window(w) => w,
            Child::Container(_) => unreachable!("master pane holds only windows"),
        }
    }

    fn pop_from_pane(hub: &mut HubAccess, container: ContainerId) -> Option<WindowId> {
        hub.containers
            .get_mut(container)
            .children
            .pop()
            .map(|c| match c {
                Child::Window(w) => w,
                Child::Container(_) => unreachable!("master pane holds only windows"),
            })
    }

    fn locate(
        hub: &HubAccess,
        master: ContainerId,
        secondary: ContainerId,
        id: WindowId,
    ) -> (PaneKind, usize) {
        if let Some(i) = Self::position_in_pane(hub, master, id) {
            return (PaneKind::Master, i);
        }
        let i = Self::position_in_pane(hub, secondary, id)
            .unwrap_or_else(|| panic!("window {id:?} is in neither master nor secondary pane"));
        (PaneKind::Secondary, i)
    }

    fn focused_panes(&self, hub: &HubAccess) -> Option<FocusedPanes> {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;
        let (kind, idx) = self.focused_position(hub, ws_id)?;
        let (master_container, secondary_container) = {
            let state = self.workspaces.get(&ws_id).unwrap();
            (state.master.container, state.secondary.container)
        };
        Some(FocusedPanes {
            ws_id,
            kind,
            idx,
            master_container,
            secondary_container,
            master_len: Self::pane_len(hub, master_container),
            stack_len: Self::pane_len(hub, secondary_container),
        })
    }

    /// `None` only for an empty workspace.
    fn focused_position(&self, hub: &HubAccess, ws_id: WorkspaceId) -> Option<(PaneKind, usize)> {
        let state = self.workspaces.get(&ws_id)?;
        let focus = state.focused_window()?;
        Some(Self::locate(
            hub,
            state.master.container,
            state.secondary.container,
            focus,
        ))
    }

    /// Returns the most recently focused window that the `kind` pane's container holds now,
    /// or the first window of that pane. Panics when the pane is empty.
    fn last_focused_in(&self, hub: &HubAccess, ws_id: WorkspaceId, kind: PaneKind) -> WindowId {
        let state = self.workspaces.get(&ws_id).unwrap();
        let members = Self::pane_windows(hub, state.pane(kind).container);
        state
            .focus_history
            .iter()
            .find(|w| members.contains(w))
            .copied()
            .or_else(|| members.first().copied())
            .unwrap_or_else(|| panic!("last_focused_in called on empty {kind:?} pane"))
    }

    /// Focus repair needs no ladder here. Dropping `window_id` from the history leaves the head
    /// on the surviving window focused before it, whichever pane that window lives in.
    fn remove_window(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        window_id: WindowId,
    ) -> Length {
        let (container, y_offset, idx) = {
            let state = self.workspaces.get(&ws_id).unwrap();
            let (kind, idx) = Self::locate(
                hub,
                state.master.container,
                state.secondary.container,
                window_id,
            );
            (state.pane(kind).container, state.pane(kind).y_offset, idx)
        };
        Self::remove_from_pane(hub, container, idx);
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .drop_from_history(window_id);
        y_offset
    }

    fn track_window(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, id: WindowId) {
        hub.windows.get_mut(id).set_workspace(Some(ws_id));
        self.window_states.insert(
            id,
            WindowState {
                held_slot: None,
                dimension: Dimension::default(),
            },
        );
        self.workspaces.get_mut(&ws_id).unwrap().add_to_history(id);
    }

    fn push_unmatched_window(&self, hub: &mut HubAccess, ws_id: WorkspaceId, id: WindowId) {
        let state = self.workspaces.get(&ws_id).unwrap();
        let effective_count = state.master_count.unwrap_or(self.master_count);
        if Self::pane_len(hub, state.master.container) < effective_count {
            Self::push_to_pane(hub, state.master.container, id);
        } else {
            Self::push_to_pane(hub, state.secondary.container, id);
        }
    }

    fn reconcile_master_count(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        let (effective_count, master, secondary) = {
            let Some(state) = self.workspaces.get(&ws_id) else {
                return;
            };
            (
                state.master_count.unwrap_or(self.master_count),
                state.master.container,
                state.secondary.container,
            )
        };

        while Self::pane_len(hub, master) < effective_count {
            let secondary_slots = &self.workspaces.get(&ws_id).unwrap().secondary.slots;
            let pos = hub
                .containers
                .get(secondary)
                .children()
                .iter()
                .position(|c| {
                    matches!(c, Child::Window(w)
                    if self.window_states.get(w).is_some_and(|e| {
                        // A window that overflowed from master to secondary still holds its
                        // master slot. This filter accepts that window too.
                        e.held_slot.is_none_or(|s| !secondary_slots.contains(&s))
                    }))
                });
            let Some(pos) = pos else {
                break;
            };
            let wid = Self::remove_from_pane(hub, secondary, pos);
            Self::push_to_pane(hub, master, wid);
        }

        while Self::pane_len(hub, master) > effective_count {
            let Some(wid) = Self::pop_from_pane(hub, master) else {
                break;
            };
            Self::insert_into_pane(hub, secondary, 0, wid);
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
    master: Pane,
    secondary: Pane,
    /// Windows of this workspace from most to least recently focused. Always set-equal to
    /// the master pane plus the secondary pane.
    focus_history: Vec<WindowId>,
    master_count: Option<usize>,
    master_ratio: Option<f32>,
}

impl WorkspaceState {
    fn pane(&self, kind: PaneKind) -> &Pane {
        match kind {
            PaneKind::Master => &self.master,
            PaneKind::Secondary => &self.secondary,
        }
    }

    fn pane_mut(&mut self, kind: PaneKind) -> &mut Pane {
        match kind {
            PaneKind::Master => &mut self.master,
            PaneKind::Secondary => &mut self.secondary,
        }
    }

    fn focused_window(&self) -> Option<WindowId> {
        self.focus_history.first().copied()
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
}

/// One side of the master-stack split. Windows live in `container`, a flat `Container`
/// of `Child::Window` that never nests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PaneDisplay {
    #[default]
    Tiled,
    Tabbed,
}

crate::config::lua::deserializer::string_enum!(
    PaneDisplay,
    "\"tiled\" or \"tabbed\"",
    "tiled" => PaneDisplay::Tiled,
    "tabbed" => PaneDisplay::Tabbed,
);

#[derive(Debug)]
struct Pane {
    container: ContainerId,
    slots: Vec<SlotId>,
    y_offset: Length,
    display: PaneDisplay,
}

impl Pane {
    fn new(container: ContainerId, slots: Vec<SlotId>, display: PaneDisplay) -> Self {
        Pane {
            container,
            slots,
            y_offset: Length::ZERO,
            display,
        }
    }
}

#[derive(Debug)]
struct WindowState {
    held_slot: Option<SlotId>,
    dimension: Dimension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneKind {
    Master,
    Secondary,
}

struct FocusedPanes {
    ws_id: WorkspaceId,
    kind: PaneKind,
    idx: usize,
    master_container: ContainerId,
    secondary_container: ContainerId,
    master_len: usize,
    stack_len: usize,
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
