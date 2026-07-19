use std::collections::HashMap;

use egui::ahash::HashSet;

use crate::config::{LayoutWorkspaceConfig, WindowMatcher};
use crate::core::WindowMetadata;
use crate::core::allocator::{Node, NodeId};
use crate::core::hub::HubAccess;
use crate::core::master::{MasterStrategy, WindowState};
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
            .master_matchers
            .iter()
            .map(|id| self.matchers.get(*id).clone())
            .collect();
        let current_secondary: Vec<WindowMatcher> = state
            .secondary_matchers
            .iter()
            .map(|id| self.matchers.get(*id).clone())
            .collect();
        let matchers_changed = current_master.as_slice() != incoming_master.as_slice()
            || current_secondary.as_slice() != incoming_secondary.as_slice();
        let new_effective_count = new_count_opt.unwrap_or(self.master_count);
        let cur_effective_count = state.master_count.unwrap_or(self.master_count);
        let count_changed = new_count_opt.is_some() && new_effective_count != cur_effective_count;
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
                .chain(state.secondary.iter())
                .copied()
                .collect();

            let focused = self.focused_tiling_window(ws_id);

            let state = self.workspaces.get_mut(&ws_id).unwrap();
            for &id in &state.master_matchers {
                self.matchers.delete(id);
            }
            for &id in &state.secondary_matchers {
                self.matchers.delete(id);
            }
            state.master_matchers = incoming_master
                .iter()
                .map(|m| self.matchers.allocate(m.clone()))
                .collect();
            state.secondary_matchers = incoming_secondary
                .iter()
                .map(|m| self.matchers.allocate(m.clone()))
                .collect();
            state.master.clear();
            state.secondary.clear();
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

    pub(super) fn insert_window_against_preferred_layout(
        &mut self,
        ws_id: WorkspaceId,
        window_id: WindowId,
        metadata: &dyn WindowMetadata,
    ) -> Option<MatcherId> {
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.focus = Some(window_id);
        let effective_count = state.master_count.unwrap_or(self.master_count);
        let occupied_master: HashSet<MatcherId> = state
            .master
            .iter()
            .filter_map(|&wid| self.window_states.get(&wid).and_then(|e| e.occupy))
            .filter(|mid| state.master_matchers.contains(mid))
            .collect();
        let occupied_secondary: HashSet<MatcherId> = state
            .secondary
            .iter()
            .filter_map(|&wid| self.window_states.get(&wid).and_then(|e| e.occupy))
            .filter(|mid| state.secondary_matchers.contains(mid))
            .collect();

        for &mid in &state.master_matchers {
            if occupied_master.contains(&mid) {
                continue;
            }
            if metadata.matches_window_matcher(self.matchers.get(mid)) {
                if state.master.len() >= effective_count {
                    // Check whether we can evict any unmatched window
                    if let Some(evict_pos) = state.master.iter().rposition(|&w| {
                        self.window_states
                            .get(&w)
                            .is_some_and(|e| e.occupy.is_none())
                    }) {
                        let evicted_window = state.master.remove(evict_pos);
                        state.secondary.insert(0, evicted_window);
                        insert_matched_window(
                            &self.window_states,
                            &mut state.master,
                            &state.master_matchers,
                            window_id,
                            mid,
                        );
                        return Some(mid);
                    }
                    // Proceed with matching in the secondary stack
                } else {
                    // There are still space so just push into master
                    insert_matched_window(
                        &self.window_states,
                        &mut state.master,
                        &state.master_matchers,
                        window_id,
                        mid,
                    );
                    return Some(mid);
                }
            }
        }

        for &mid in &state.secondary_matchers {
            if occupied_secondary.contains(&mid) {
                continue;
            }
            if metadata.matches_window_matcher(self.matchers.get(mid)) {
                insert_matched_window(
                    &self.window_states,
                    &mut state.secondary,
                    &state.secondary_matchers,
                    window_id,
                    mid,
                );
                return Some(mid);
            }
        }
        None
    }
}

fn insert_matched_window(
    window_states: &HashMap<WindowId, WindowState>,
    pane: &mut Vec<WindowId>,
    pane_matchers: &[MatcherId],
    window_id: WindowId,
    slot_id: MatcherId,
) {
    let slot_position = pane_matchers.iter().position(|&x| x == slot_id).unwrap();
    // Get the insert position for a matcher slot, in the order specified in the preferred
    // layout.
    // Note that since matched windows can be moved, we can no longer ensure that all
    // matched windows follow the specified order. Placing the window right before
    // the first found subsequent slot is acceptable here
    let insert_position = pane
        .iter()
        .position(|&w| {
            let Some(mid) = window_states.get(&w).unwrap().occupy else {
                return false;
            };
            pane_matchers
                .iter()
                .position(|&m| m == mid)
                .is_some_and(|s| s > slot_position)
        })
        .unwrap_or(pane.len());
    pane.insert(insert_position, window_id);
}
