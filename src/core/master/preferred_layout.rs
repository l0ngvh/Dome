use crate::config::{LayoutWorkspaceConfig, WindowMatcher};
use crate::core::allocator::{Node, NodeId};
use crate::core::hub::HubAccess;
use crate::core::master::MasterStrategy;
use crate::core::node::{Child, ContainerId, WindowId, WorkspaceId};
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

        let (new_count_opt, new_ratio_opt, incoming_master, incoming_secondary, incoming_displays) =
            match incoming {
                Some(LayoutWorkspaceConfig::Master {
                    master_count: incoming_count,
                    master_ratio: incoming_ratio,
                    master,
                    secondary,
                    ..
                }) => (
                    *incoming_count,
                    *incoming_ratio,
                    master.children.clone(),
                    secondary.children.clone(),
                    Some((master.display, secondary.display)),
                ),
                _ => (None, None, Vec::new(), Vec::new(), None),
            };

        let current_master: Vec<WindowMatcher> = state
            .master
            .matchers
            .iter()
            .map(|id| self.slots.get(*id).matcher.clone())
            .collect();
        let current_secondary: Vec<WindowMatcher> = state
            .secondary
            .matchers
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
        let display_changed = incoming_displays
            .is_some_and(|(m, s)| state.master.display != m || state.secondary.display != s);

        if !matchers_changed && !count_changed && !ratio_changed && !display_changed {
            return;
        }

        tracing::debug!(%ws_id, "Master preferred layout changed, reloading");

        if let Some((m_disp, s_disp)) = incoming_displays {
            let state = self.workspaces.get_mut(&ws_id).unwrap();
            state.master.display = m_disp;
            state.secondary.display = s_disp;
        }

        if matchers_changed {
            let (master_cid, secondary_cid) = {
                let state = self.workspaces.get(&ws_id).unwrap();
                (state.master.container, state.secondary.container)
            };
            let mut tiling_windows = Self::pane_windows(hub, master_cid);
            tiling_windows.extend(Self::pane_windows(hub, secondary_cid));

            let focused = self.focused_tiling_window(ws_id);
            let previous_history = self.workspaces.get(&ws_id).unwrap().focus_history.clone();

            let state = self.workspaces.get_mut(&ws_id).unwrap();
            for &id in &state.master.matchers {
                self.slots.delete(id);
            }
            for &id in &state.secondary.matchers {
                self.slots.delete(id);
            }
            state.master.matchers = incoming_master
                .iter()
                .map(|m| {
                    self.slots.allocate(Slot {
                        matcher: m.clone(),
                        windows: Vec::new(),
                    })
                })
                .collect();
            state.secondary.matchers = incoming_secondary
                .iter()
                .map(|m| {
                    self.slots.allocate(Slot {
                        matcher: m.clone(),
                        windows: Vec::new(),
                    })
                })
                .collect();
            // Every attach runs scroll_into_view, which resolves the focused window against
            // the panes, so a full history over empty panes panics.
            state.clear_focus_history();
            state.master_count = new_count_opt;
            state.master_ratio = new_ratio_opt;

            // Clear both containers so the reattach loop rebuilds them without duplicates.
            hub.containers.get_mut(master_cid).children.clear();
            hub.containers.get_mut(secondary_cid).children.clear();

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
                self.workspaces.get_mut(&ws_id).unwrap().master_count = new_count_opt;
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
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        window_id: WindowId,
    ) -> Option<SlotId> {
        let (effective_count, master, secondary, master_matchers, secondary_matchers) = {
            let state = self.workspaces.get(&ws_id).unwrap();
            (
                state.master_count.unwrap_or(self.master_count),
                state.master.container,
                state.secondary.container,
                state.master.matchers.clone(),
                state.secondary.matchers.clone(),
            )
        };

        // Resolve the matching slot before touching a container, because the match borrows the
        // window metadata out of `hub` and the mutations below borrow `hub` exclusively.
        let metadata = hub.windows.get(window_id).metadata.as_ref();
        let master_match = master_matchers
            .iter()
            .copied()
            .find(|&sid| metadata.matches_window_matcher(&self.slots.get(sid).matcher));
        let secondary_match = secondary_matchers
            .iter()
            .copied()
            .find(|&sid| metadata.matches_window_matcher(&self.slots.get(sid).matcher));

        if let Some(sid) = master_match {
            if Self::pane_len(hub, master) < effective_count {
                self.join_slot_and_place(hub, master, &master_matchers, window_id, sid);
                return Some(sid);
            }
            // Master is full. Evict an unmatched window if one exists, otherwise let this
            // window fall through to the secondary stack.
            let evict = hub.containers.get(master).children().iter().rposition(|c| {
                matches!(c, Child::Window(w)
                    if self.window_states.get(w).is_some_and(|e| e.occupy.is_none()))
            });
            if let Some(evict_pos) = evict {
                let evicted = Self::remove_from_pane(hub, master, evict_pos);
                Self::insert_into_pane(hub, secondary, 0, evicted);
                self.join_slot_and_place(hub, master, &master_matchers, window_id, sid);
                return Some(sid);
            }
        }

        if let Some(sid) = secondary_match {
            self.join_slot_and_place(hub, secondary, &secondary_matchers, window_id, sid);
            return Some(sid);
        }

        // No slot matched.
        if Self::pane_len(hub, master) < effective_count {
            Self::push_to_pane(hub, master, window_id);
        } else {
            Self::push_to_pane(hub, secondary, window_id);
        }
        None
    }

    pub(super) fn remap_slot_on_pane_change(
        &mut self,
        hub: &mut HubAccess,
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

        let Some(sid) = matched else {
            return;
        };
        // dest_slots is the destination pane's matcher list, so the destination pane is the one
        // whose matcher list contains sid. Picking the other pane would order the window against
        // the wrong pane.
        let container = {
            let state = self.workspaces.get(&ws_id).unwrap();
            if state.master.matchers.contains(&sid) {
                state.master.container
            } else {
                state.secondary.container
            }
        };
        if let Some(pos) = Self::position_in_pane(hub, container, window_id) {
            Self::remove_from_pane(hub, container, pos);
        }
        self.join_slot_and_place(hub, container, dest_slots, window_id, sid);
    }

    fn join_slot_and_place(
        &mut self,
        hub: &mut HubAccess,
        container: ContainerId,
        pane_matchers: &[SlotId],
        window_id: WindowId,
        slot_id: SlotId,
    ) {
        self.slots.get_mut(slot_id).windows.push(window_id);
        let slot_position = pane_matchers.iter().position(|&x| x == slot_id).unwrap();
        // Insert in preferred-layout order. Moved windows break that order, so placing the
        // window right before the first later slot is acceptable.
        let insert_position = hub
            .containers
            .get(container)
            .children()
            .iter()
            .position(|c| {
                let Child::Window(w) = c else {
                    return false;
                };
                let Some(mid) = self.window_states.get(w).unwrap().occupy else {
                    return false;
                };
                pane_matchers
                    .iter()
                    .position(|&m| m == mid)
                    .is_some_and(|s| s > slot_position)
            })
            .unwrap_or_else(|| hub.containers.get(container).children().len());
        hub.containers
            .get_mut(container)
            .children
            .insert(insert_position, Child::Window(window_id));
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
