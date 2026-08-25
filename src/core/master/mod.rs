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
    Child, Container, ContainerId, Dimension, Direction, Length, Logical, PixelRect, Pixels,
    WindowId, WindowMetadata, WorkspaceId,
};
use crate::core::strategy::{
    TilingAction, TilingPlacements, TilingStrategy, WorkspaceExport, distribute_space, translate,
    window_constraints,
};

/// XMonad-style tiling: a master area on the left and a stack on the right.
/// Each pane scrolls vertically and independently when per-window min heights push the
/// pane's total content past the screen height. Horizontal scroll does not exist in master,
/// so per-window min width is not honored: the split follows master_ratio and each pane
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
    fn prepare_workspace(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        preferred_layout: Option<&LayoutWorkspaceConfig>,
    ) {
        // Reject a non-master config before allocating, so the panic path cannot
        // leak the pane containers.
        let master_cfg = match preferred_layout {
            Some(LayoutWorkspaceConfig::Master {
                master_count,
                master_ratio,
                master,
                secondary,
                ..
            }) => Some((*master_count, *master_ratio, master, secondary)),
            Some(_) => panic!("Preparing partition tree workspace in master strategy"),
            None => None,
        };

        let master_container = hub.allocate_container(Container {
            children: Vec::new(),
        });
        let secondary_container = hub.allocate_container(Container {
            children: Vec::new(),
        });

        let (master_ids, secondary_ids, master_count, master_ratio) = match master_cfg {
            Some((master_count, master_ratio, master, secondary)) => {
                let master_ids = master
                    .iter()
                    .map(|m| {
                        self.slots.allocate(Slot {
                            matcher: m.clone(),
                            windows: Vec::new(),
                        })
                    })
                    .collect();
                let secondary_ids = secondary
                    .iter()
                    .map(|m| {
                        self.slots.allocate(Slot {
                            matcher: m.clone(),
                            windows: Vec::new(),
                        })
                    })
                    .collect();
                (master_ids, secondary_ids, master_count, master_ratio)
            }
            None => (Vec::new(), Vec::new(), None, None),
        };

        self.workspaces.insert(
            ws_id,
            WorkspaceState {
                master: Pane::new(master_container, master_ids),
                secondary: Pane::new(secondary_container, secondary_ids),
                focus_history: Vec::new(),
                master_count,
                master_ratio,
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

        let y_offset = self.remove_window(hub, ws_id, id);

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

    fn handle_action(&mut self, hub: &mut HubAccess, action: TilingAction) {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;

        let Some((kind, idx)) = self.focused_position(hub, ws_id) else {
            return;
        };
        let (master_cid, secondary_cid) = {
            let state = self.workspaces.get(&ws_id).unwrap();
            (state.master.container, state.secondary.container)
        };
        let master_len = Self::pane_len(hub, master_cid);
        let stack_len = Self::pane_len(hub, secondary_cid);

        match action {
            TilingAction::FocusDirection { direction, forward } => {
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
                            master_cid
                        } else {
                            secondary_cid
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
            TilingAction::MoveDirection { direction, forward } => {
                if master_len + stack_len <= 1 {
                    return;
                }
                let (master_matchers, secondary_matchers, effective) = {
                    let state = self.workspaces.get(&ws_id).unwrap();
                    (
                        state.master.matchers.clone(),
                        state.secondary.matchers.clone(),
                        state.master_count.unwrap_or(self.master_count),
                    )
                };
                match (direction, forward) {
                    (Direction::Horizontal, false) => {
                        if kind == PaneKind::Secondary {
                            let moved = Self::remove_from_pane(hub, secondary_cid, idx);
                            if Self::pane_len(hub, master_cid) >= effective && master_len > 0 {
                                let swapped = Self::pop_from_pane(hub, master_cid).unwrap();
                                Self::push_to_pane(hub, master_cid, moved);
                                Self::push_to_pane(hub, secondary_cid, swapped);
                                self.remap_slot_on_pane_change(hub, ws_id, moved, &master_matchers);
                                self.remap_slot_on_pane_change(
                                    hub,
                                    ws_id,
                                    swapped,
                                    &secondary_matchers,
                                );
                            } else if Self::pane_len(hub, master_cid) < effective {
                                Self::push_to_pane(hub, master_cid, moved);
                                self.remap_slot_on_pane_change(hub, ws_id, moved, &master_matchers);
                            }
                        }
                    }
                    (Direction::Horizontal, true) => {
                        if kind == PaneKind::Master && stack_len > 0 {
                            let moved = Self::remove_from_pane(hub, master_cid, idx);
                            let swapped = Self::remove_from_pane(hub, secondary_cid, 0);
                            Self::push_to_pane(hub, master_cid, swapped);
                            Self::push_to_pane(hub, secondary_cid, moved);
                            self.remap_slot_on_pane_change(hub, ws_id, moved, &secondary_matchers);
                            self.remap_slot_on_pane_change(hub, ws_id, swapped, &master_matchers);
                        }
                    }
                    (Direction::Vertical, _) => {
                        let cid = if kind == PaneKind::Master {
                            master_cid
                        } else {
                            secondary_cid
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
            TilingAction::ToggleContainerLayout => {
                let pane = self.workspaces.get_mut(&ws_id).unwrap().pane_mut(kind);
                pane.display = match pane.display {
                    PaneDisplay::Tiled => PaneDisplay::Tabbed,
                    PaneDisplay::Tabbed => PaneDisplay::Tiled,
                };
                self.compute_placement(hub, ws_id);
            }
            TilingAction::FocusTab { forward } => {
                let cid = if kind == PaneKind::Master {
                    master_cid
                } else {
                    secondary_cid
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
            TilingAction::TabClicked {
                container_id,
                index,
            } => {
                let clicked_kind = if container_id == master_cid {
                    PaneKind::Master
                } else if container_id == secondary_cid {
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
            _ => {}
        }
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
        state
            .master
            .matchers
            .iter()
            .chain(state.secondary.matchers.iter())
            .any(|&sid| metadata.matches_window_matcher(&self.slots.get(sid).matcher))
    }

    fn detach_focused_child(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Option<Child> {
        let focus_id = self.workspaces.get(&ws_id)?.focused_window()?;

        self.remove_window(hub, ws_id, focus_id);

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
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
    ) -> (Vec<WindowId>, Option<WindowId>) {
        let focused = self.focused_tiling_window(ws_id);
        let mut tiling = Vec::new();
        if let Some(state) = self.workspaces.remove(&ws_id) {
            for cid in [state.master.container, state.secondary.container] {
                tiling.extend(Self::pane_windows(hub, cid));
                hub.free_container(cid);
            }
            for &wid in &tiling {
                self.window_states.remove(&wid);
            }
            for &id in &state.master.matchers {
                self.slots.delete(id);
            }
            for &id in &state.secondary.matchers {
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
        self.tab_bar_height = layout.partition_tree.tab_bar_height;
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

    /// Reads membership from the live containers, so a migrated window answers for the pane it
    /// occupies now.
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

    fn place(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, id: WindowId) {
        let occupy = self.sort_window_into_pane(hub, ws_id, id);
        self.workspaces.get_mut(&ws_id).unwrap().add_to_history(id);
        self.window_states.insert(
            id,
            WindowState {
                occupy,
                dimension: Dimension::default(),
            },
        );
    }

    fn reconcile_master_count(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        let (effective_count, secondary_slots, master, secondary) = {
            let Some(state) = self.workspaces.get(&ws_id) else {
                return;
            };
            (
                state.master_count.unwrap_or(self.master_count),
                state.secondary.matchers.clone(),
                state.master.container,
                state.secondary.container,
            )
        };

        // Pull unmatched windows up from secondary until master reaches the count.
        while Self::pane_len(hub, master) < effective_count {
            let pos = hub
                .containers
                .get(secondary)
                .children()
                .iter()
                .position(|c| {
                    matches!(c, Child::Window(w)
                    if self.window_states.get(w).is_some_and(|e| e.occupy.is_none()))
                });
            let Some(pos) = pos else {
                break;
            };
            let wid = Self::remove_from_pane(hub, secondary, pos);
            Self::push_to_pane(hub, master, wid);
        }

        // Spill master overflow onto the front of secondary, then remap the moved slots.
        let mut overflow = Vec::new();
        while Self::pane_len(hub, master) > effective_count {
            let Some(wid) = Self::pop_from_pane(hub, master) else {
                break;
            };
            Self::insert_into_pane(hub, secondary, 0, wid);
            overflow.push(wid);
        }
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

    fn clear_focus_history(&mut self) {
        self.focus_history.clear();
    }
}

/// One side of the master-stack split. Windows live in `container`, a flat `Container`
/// of `Child::Window` that never nests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneDisplay {
    Tiled,
    Tabbed,
}

#[derive(Debug)]
struct Pane {
    container: ContainerId,
    matchers: Vec<SlotId>,
    y_offset: Length,
    display: PaneDisplay,
}

impl Pane {
    fn new(container: ContainerId, matchers: Vec<SlotId>) -> Self {
        Pane {
            container,
            matchers,
            y_offset: Length::ZERO,
            display: PaneDisplay::Tiled,
        }
    }
}

/// Per-window state: matcher slot occupancy and computed dimension.
#[derive(Debug)]
struct WindowState {
    occupy: Option<SlotId>,
    dimension: Dimension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneKind {
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
