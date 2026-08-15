use std::collections::HashMap;

use crate::config::{LayoutWorkspaceConfig, WindowMatcher};
use crate::core::WindowMetadata;
use crate::core::allocator::{Allocator, Node, NodeId};
use crate::core::hub::HubAccess;
use crate::core::master::{MasterStrategy, WindowState};
use crate::core::node::{WindowId, WorkspaceId};
use crate::core::strategy::TilingStrategy;

impl MasterStrategy {
    pub(super) fn sync_preferred_layout(
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
            .map(|id| self.slots.get(*id).matcher.clone())
            .collect();
        let current_secondary: Vec<WindowMatcher> = state
            .secondary_matchers
            .iter()
            .map(|id| self.slots.get(*id).matcher.clone())
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
            let previous_history = state.focus_history.clone();

            let state = self.workspaces.get_mut(&ws_id).unwrap();
            for &id in &state.master_matchers {
                self.slots.delete(id);
            }
            for &id in &state.secondary_matchers {
                self.slots.delete(id);
            }
            state.master_matchers = incoming_master
                .iter()
                .map(|m| {
                    self.slots.allocate(Slot {
                        matcher: m.clone(),
                        windows: Vec::new(),
                    })
                })
                .collect();
            state.secondary_matchers = incoming_secondary
                .iter()
                .map(|m| {
                    self.slots.allocate(Slot {
                        matcher: m.clone(),
                        windows: Vec::new(),
                    })
                })
                .collect();
            state.master.clear();
            state.secondary.clear();
            // Every attach runs scroll_into_view, which resolves the focused
            // window against the panes, so a full history over empty panes panics.
            state.clear_focus_history();
            state.master_count = new_count_opt;
            state.master_ratio = new_ratio_opt;

            for &wid in &tiling_windows {
                self.attach_window(hub, wid, ws_id);
            }
            // Re-attaching enrolls in pane order, losing recency. Same set, so
            // the pre-reload order still holds.
            self.workspaces.get_mut(&ws_id).unwrap().focus_history = previous_history;
            if let Some(f) = focused {
                self.set_focus(hub, f);
            }
        } else {
            if count_changed {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.master_count = new_count_opt;
                self.reconcile_master_count(hub, ws_id);
            }
            if ratio_changed {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.master_ratio = new_ratio_opt;
            }
            self.compute_placement(hub, ws_id);
        }
    }

    pub(super) fn sort_window_into_pane(
        &mut self,
        ws_id: WorkspaceId,
        window_id: WindowId,
        metadata: &dyn WindowMetadata,
    ) -> Option<SlotId> {
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let effective_count = state.master_count.unwrap_or(self.master_count);

        for &sid in &state.master_matchers {
            if metadata.matches_window_matcher(&self.slots.get(sid).matcher) {
                if state.master.len() >= effective_count {
                    // Master is full. Evict an unmatched window if one exists, otherwise
                    // let this window fall through to the secondary stack.
                    if let Some(evict_pos) = state.master.iter().rposition(|&w| {
                        self.window_states
                            .get(&w)
                            .is_some_and(|e| e.occupy.is_none())
                    }) {
                        let evicted_window = state.master.remove(evict_pos);
                        state.secondary.insert(0, evicted_window);
                        join_slot_and_place(
                            &mut self.slots,
                            &self.window_states,
                            &mut state.master,
                            &state.master_matchers,
                            window_id,
                            sid,
                        );
                        return Some(sid);
                    }
                    break;
                }
                join_slot_and_place(
                    &mut self.slots,
                    &self.window_states,
                    &mut state.master,
                    &state.master_matchers,
                    window_id,
                    sid,
                );
                return Some(sid);
            }
        }

        for &sid in &state.secondary_matchers {
            if metadata.matches_window_matcher(&self.slots.get(sid).matcher) {
                join_slot_and_place(
                    &mut self.slots,
                    &self.window_states,
                    &mut state.secondary,
                    &state.secondary_matchers,
                    window_id,
                    sid,
                );
                return Some(sid);
            }
        }

        // This window doesn't match any slot
        if state.master.len() < effective_count {
            state.master.push(window_id);
        } else {
            state.secondary.push(window_id);
        }
        None
    }

    pub(super) fn remap_slot_on_pane_change(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        window_id: WindowId,
        dest_slots: &[SlotId],
    ) {
        if let Some(src) = self.window_states.get(&window_id).and_then(|e| e.occupy) {
            self.slots.get_mut(src).windows.retain(|w| w != &window_id);
        }
        let metadata = hub.windows.get(window_id).metadata.as_ref();
        let matched = dest_slots
            .iter()
            .copied()
            .find(|&sid| metadata.matches_window_matcher(&self.slots.get(sid).matcher));
        if let Some(entry) = self.window_states.get_mut(&window_id) {
            entry.occupy = matched;
        }

        if let Some(sid) = matched {
            let state = self.workspaces.get_mut(&ws_id).unwrap();
            // The pane Vec and dest_slots must come from the same pane: dest_slots is the
            // matched pane's matcher list, so pick the pane Vec by the same matcher list.
            // Splitting the two apart would place the window using another pane's order.
            let pane = if state.master_matchers.contains(&sid) {
                &mut state.master
            } else {
                &mut state.secondary
            };
            pane.retain(|&w| w != window_id);
            join_slot_and_place(
                &mut self.slots,
                &self.window_states,
                pane,
                dest_slots,
                window_id,
                sid,
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct SlotId(usize);

impl NodeId for SlotId {
    fn new(id: usize) -> Self {
        Self(id)
    }
    fn get(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub(super) struct Slot {
    pub(super) matcher: WindowMatcher,
    pub(super) windows: Vec<WindowId>,
}

impl Node for Slot {
    type Id = SlotId;
}

fn join_slot_and_place(
    slots: &mut Allocator<Slot>,
    window_states: &HashMap<WindowId, WindowState>,
    pane: &mut Vec<WindowId>,
    pane_matchers: &[SlotId],
    window_id: WindowId,
    slot_id: SlotId,
) {
    slots.get_mut(slot_id).windows.push(window_id);
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
