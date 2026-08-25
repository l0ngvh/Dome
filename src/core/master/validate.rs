use rustc_hash::FxHashSet;

use crate::core::{
    Length, WindowId,
    hub::HubAccess,
    master::MasterStrategy,
    node::ContainerId,
    strategy::{VALIDATION_TOLERANCE, ValidateStrategy, window_constraints},
};

impl ValidateStrategy for MasterStrategy {
    fn reachable_containers(&self, _hub: &HubAccess) -> FxHashSet<ContainerId> {
        let mut reachable = FxHashSet::default();
        for state in self.workspaces.values() {
            reachable.insert(state.master.container);
            reachable.insert(state.secondary.container);
        }
        reachable
    }

    fn validate(&self, hub: &HubAccess) {
        for (&ws_id, state) in &self.workspaces {
            let master = Self::pane_windows(hub, state.master.container);
            let secondary = Self::pane_windows(hub, state.secondary.container);
            let mut seen = FxHashSet::default();
            for &wid in master.iter().chain(secondary.iter()) {
                hub.windows.get(wid);
                assert!(
                    seen.insert(wid),
                    "master-stack workspace {ws_id}: duplicate window {wid:?}"
                );
            }
            let effective_count = state.master_count.unwrap_or(self.master_count);
            assert!(
                master.len() <= effective_count,
                "master-stack workspace {ws_id}: master.len() {} > master_count {effective_count}",
                master.len()
            );

            assert_eq!(
                state.focus_history.len(),
                seen.len(),
                "master-stack workspace {ws_id}: focus_history has {} entries for {} windows, \
                 so it holds a duplicate or a stale window",
                state.focus_history.len(),
                seen.len()
            );
            let history_seen: FxHashSet<WindowId> = state.focus_history.iter().copied().collect();
            assert_eq!(
                history_seen, seen,
                "master-stack workspace {ws_id}: focus_history does not match master plus secondary \
                 (a duplicate entry also shows up here)"
            );

            for &wid in master.iter().chain(secondary.iter()) {
                assert!(
                    self.window_states.contains_key(&wid),
                    "master-stack workspace {ws_id}: window {wid:?} missing from window_states"
                );
            }

            for &wid in &master {
                if let Some(occupy) = self.window_states.get(&wid).and_then(|w| w.occupy) {
                    assert!(
                        state.master.matchers.contains(&occupy),
                        "master-stack workspace {ws_id}: master window {wid:?} occupies slot {occupy:?} outside master pane"
                    );
                }
            }
            for &wid in &secondary {
                if let Some(occupy) = self.window_states.get(&wid).and_then(|w| w.occupy) {
                    assert!(
                        state.secondary.matchers.contains(&occupy),
                        "master-stack workspace {ws_id}: secondary window {wid:?} occupies slot {occupy:?} outside secondary pane"
                    );
                }
            }
            for slot in &state.master.matchers {
                assert!(
                    !state.secondary.matchers.contains(slot),
                    "master-stack workspace {ws_id}: slot {slot:?} shared between master and secondary panes"
                );
            }

            if master.is_empty() && secondary.is_empty() {
                continue;
            }

            let pane_height = Length::from_pixels(
                hub.monitors
                    .get(hub.workspaces.get(ws_id).monitor)
                    .work_area
                    .height(),
            );

            for &wid in &master {
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
                let c = window_constraints(hub, &self.size_constraints, wid);
                assert!(
                    dim.height >= c.min_height - VALIDATION_TOLERANCE,
                    "master-stack workspace {ws_id}: window {wid:?} height {} < effective min_height {}",
                    dim.height,
                    c.min_height
                );
                if c.max_width > Length::ZERO {
                    assert!(
                        dim.width <= c.max_width + VALIDATION_TOLERANCE,
                        "master-stack workspace {ws_id}: window {wid:?} width {} > effective max_width {}",
                        dim.width,
                        c.max_width
                    );
                }
                if c.max_height > Length::ZERO {
                    assert!(
                        dim.height <= c.max_height + VALIDATION_TOLERANCE,
                        "master-stack workspace {ws_id}: window {wid:?} height {} > effective max_height {}",
                        dim.height,
                        c.max_height
                    );
                }
            }

            for &wid in &secondary {
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
                let c = window_constraints(hub, &self.size_constraints, wid);
                assert!(
                    dim.height >= c.min_height - VALIDATION_TOLERANCE,
                    "master-stack workspace {ws_id}: window {wid:?} height {} < effective min_height {}",
                    dim.height,
                    c.min_height
                );
                if c.max_width > Length::ZERO {
                    assert!(
                        dim.width <= c.max_width + VALIDATION_TOLERANCE,
                        "master-stack workspace {ws_id}: window {wid:?} width {} > effective max_width {}",
                        dim.width,
                        c.max_width
                    );
                }
                if c.max_height > Length::ZERO {
                    assert!(
                        dim.height <= c.max_height + VALIDATION_TOLERANCE,
                        "master-stack workspace {ws_id}: window {wid:?} height {} > effective max_height {}",
                        dim.height,
                        c.max_height
                    );
                }
            }

            let master_ids: Vec<WindowId> = master.clone();
            if !master_ids.is_empty() {
                let master_content_h = self.pane_content_height(hub, &master_ids, pane_height);
                let master_max_offset = (master_content_h - pane_height).max(Length::ZERO);
                assert!(
                    state.master.y_offset >= Length::ZERO
                        && state.master.y_offset <= master_max_offset,
                    "master-stack workspace {ws_id}: master_y_offset {} out of bounds [0, {}]",
                    state.master.y_offset,
                    master_max_offset
                );
            } else {
                assert!(
                    state.master.y_offset == Length::ZERO,
                    "master-stack workspace {ws_id}: master_y_offset should be zero (no master windows)"
                );
            }

            let stack_ids: Vec<WindowId> = secondary.clone();
            if !stack_ids.is_empty() {
                let stack_content_h = self.pane_content_height(hub, &stack_ids, pane_height);
                let stack_max_offset = (stack_content_h - pane_height).max(Length::ZERO);
                assert!(
                    state.secondary.y_offset >= Length::ZERO
                        && state.secondary.y_offset <= stack_max_offset,
                    "master-stack workspace {ws_id}: stack_y_offset {} out of bounds [0, {}]",
                    state.secondary.y_offset,
                    stack_max_offset
                );
            } else {
                assert!(
                    state.secondary.y_offset == Length::ZERO,
                    "master-stack workspace {ws_id}: stack_y_offset should be zero (no stack windows)"
                );
            }
        }
    }
}
