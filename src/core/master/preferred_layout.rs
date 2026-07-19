use std::collections::HashSet;

use crate::config::{LayoutWorkspaceConfig, WindowMatcher};
use crate::core::WindowMetadata;
use crate::core::allocator::{Allocator, Node, NodeId};
use crate::core::hub::HubAccess;
use crate::core::master::{MasterStrategy, Pane, PlacementTag, WorkspaceState};
use crate::core::node::{WindowId, WorkspaceId};
use crate::core::strategy::TilingStrategy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MatcherId(usize);

impl NodeId for MatcherId {
    fn new(id: usize) -> Self {
        Self(id)
    }
    fn get(self) -> usize {
        self.0
    }
}

impl Node for WindowMatcher {
    type Id = MatcherId;
}

impl MasterStrategy {
    pub(super) fn do_sync_preferred_layout(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        incoming: Option<&LayoutWorkspaceConfig>,
    ) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };

        let (new_count_opt, new_ratio_opt, incoming_master, incoming_secondary) = match incoming {
            Some(LayoutWorkspaceConfig::Master {
                master_count: incoming_count,
                master_ratio: incoming_ratio,
                master,
                secondary,
                ..
            }) => (
                *incoming_count,
                *incoming_ratio,
                master.clone(),
                secondary.clone(),
            ),
            _ => (None, None, Vec::new(), Vec::new()),
        };

        let current_master: Vec<WindowMatcher> = state
            .master_matcher_ids
            .iter()
            .map(|id| self.master_matchers.get(*id).clone())
            .collect();
        let current_secondary: Vec<WindowMatcher> = state
            .secondary_matcher_ids
            .iter()
            .map(|id| self.secondary_matchers.get(*id).clone())
            .collect();
        let matchers_changed = current_master.as_slice() != incoming_master.as_slice()
            || current_secondary.as_slice() != incoming_secondary.as_slice();
        let new_effective_count = new_count_opt.unwrap_or(self.master_count);
        let cur_effective_count = state.master_count.unwrap_or(self.master_count);
        let count_changed =
            new_count_opt.is_some() && new_effective_count != cur_effective_count;
        let new_effective_ratio = new_ratio_opt.unwrap_or(self.master_ratio);
        let cur_effective_ratio = state.master_ratio.unwrap_or(self.master_ratio);
        let ratio_changed = new_ratio_opt.is_some()
            && (new_effective_ratio - cur_effective_ratio).abs() > f32::EPSILON;

        if !matchers_changed && !count_changed && !ratio_changed {
            return;
        }

        tracing::debug!(%ws_id, "Master preferred layout changed, reloading");

        if matchers_changed {
            let tiling_windows: Vec<WindowId> = state
                .master
                .iter()
                .chain(state.stack.iter())
                .map(|(id, _)| *id)
                .collect();

            let focused = self.focused_tiling_window(ws_id);

            let state = self.workspaces.get_mut(&ws_id).unwrap();
            for &id in &state.master_matcher_ids {
                self.master_matchers.delete(id);
            }
            for &id in &state.secondary_matcher_ids {
                self.secondary_matchers.delete(id);
            }
            state.master_matcher_ids = incoming_master
                .iter()
                .map(|m| self.master_matchers.allocate(m.clone()))
                .collect();
            state.secondary_matcher_ids = incoming_secondary
                .iter()
                .map(|m| self.secondary_matchers.allocate(m.clone()))
                .collect();
            state.master.clear();
            state.stack.clear();
            state.focus = None;
            state.master_count = new_count_opt;
            state.master_ratio = new_ratio_opt;

            for &wid in &tiling_windows {
                self.attach_window(hub, wid, ws_id);
            }
            if let Some(f) = focused {
                self.set_focus(hub, f);
            }
        } else {
            if count_changed {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.master_count = new_count_opt;
                self.reconcile_master_count(ws_id);
            }
            if ratio_changed {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.master_ratio = new_ratio_opt;
            }
            self.layout_workspace(hub, ws_id);
        }
    }

    /// Decide where a new window should go. Pure function, no mutation.
    /// Returns `(target_pane, insertion_index, tag)`.
    pub(super) fn place(
        state: &WorkspaceState,
        metadata: &dyn WindowMetadata,
        master_matchers: &Allocator<WindowMatcher>,
        secondary_matchers: &Allocator<WindowMatcher>,
        global_master_count: usize,
    ) -> (Pane, usize, PlacementTag) {
        let occupied_master: HashSet<usize> = state.occupied_master().collect();
        let occupied_secondary: HashSet<usize> = state.occupied_secondary().collect();

        for (slot, id) in state.master_matcher_ids.iter().enumerate() {
            if occupied_master.contains(&slot) {
                continue;
            }
            if metadata.matches_window_matcher(master_matchers.get(*id)) {
                return Self::insert_for_match(state, Pane::Master, slot, global_master_count);
            }
        }
        for (slot, id) in state.secondary_matcher_ids.iter().enumerate() {
            if occupied_secondary.contains(&slot) {
                continue;
            }
            if metadata.matches_window_matcher(secondary_matchers.get(*id)) {
                return Self::insert_for_match(state, Pane::Secondary, slot, global_master_count);
            }
        }

        let effective_count = state.master_count.unwrap_or(global_master_count);
        if state.master.len() < effective_count {
            (Pane::Master, state.master.len(), PlacementTag::Unmatched)
        } else {
            (Pane::Secondary, state.stack.len(), PlacementTag::Unmatched)
        }
    }

    /// Insert a matched window. When master-targeted and master is full,
    /// cascades to stack only if master has no Unmatched windows to evict.
    fn insert_for_match(
        state: &WorkspaceState,
        pane: Pane,
        slot: usize,
        global_master_count: usize,
    ) -> (Pane, usize, PlacementTag) {
        let tag = PlacementTag::Matched { pane, slot };

        let effective_count = state.master_count.unwrap_or(global_master_count);
        if pane == Pane::Master && state.master.len() >= effective_count {
            let has_unmatched = state
                .master
                .iter()
                .any(|(_, t)| matches!(t, PlacementTag::Unmatched));
            if !has_unmatched {
                let ins = state
                    .stack
                    .iter()
                    .position(|(_, t)| Self::slot_of(t).is_some_and(|k| k > slot))
                    .unwrap_or(state.stack.len());
                return (Pane::Secondary, ins, tag);
            }
        }

        let vec = match pane {
            Pane::Master => &state.master,
            Pane::Secondary => &state.stack,
        };
        let ins = vec
            .iter()
            .position(|(_, t)| Self::slot_of(t).is_some_and(|k| k > slot))
            .unwrap_or(vec.len());
        (pane, ins, tag)
    }
}
