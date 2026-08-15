use std::collections::HashSet;

use crate::core::{
    Length, WindowId,
    hub::HubAccess,
    master::MasterStrategy,
    strategy::{VALIDATION_TOLERANCE, ValidateStrategy, window_constraints},
};

impl ValidateStrategy for MasterStrategy {
    fn validate(&self, hub: &HubAccess) {
        for (&ws_id, state) in &self.workspaces {
            let mut seen = HashSet::new();
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

            assert_eq!(
                state.focus_history.len(),
                seen.len(),
                "master-stack workspace {ws_id}: focus_history has {} entries for {} windows, \
                 so it holds a duplicate or a stale window",
                state.focus_history.len(),
                seen.len()
            );
            let history_seen: HashSet<WindowId> = state.focus_history.iter().copied().collect();
            assert_eq!(
                history_seen, seen,
                "master-stack workspace {ws_id}: focus_history does not match master plus secondary \
                 (a duplicate entry also shows up here)"
            );

            for &wid in state.master.iter().chain(state.secondary.iter()) {
                assert!(
                    self.window_states.contains_key(&wid),
                    "master-stack workspace {ws_id}: window {wid:?} missing from window_states"
                );
            }

            for &wid in &state.master {
                if let Some(occupy) = self.window_states.get(&wid).and_then(|w| w.occupy) {
                    assert!(
                        state.master_matchers.contains(&occupy),
                        "master-stack workspace {ws_id}: master window {wid:?} occupies slot {occupy:?} outside master pane"
                    );
                }
            }
            for &wid in &state.secondary {
                if let Some(occupy) = self.window_states.get(&wid).and_then(|w| w.occupy) {
                    assert!(
                        state.secondary_matchers.contains(&occupy),
                        "master-stack workspace {ws_id}: secondary window {wid:?} occupies slot {occupy:?} outside secondary pane"
                    );
                }
            }
            for slot in &state.master_matchers {
                assert!(
                    !state.secondary_matchers.contains(slot),
                    "master-stack workspace {ws_id}: slot {slot:?} shared between master and secondary panes"
                );
            }

            if state.master.is_empty() && state.secondary.is_empty() {
                continue;
            }

            let pane_height = Length::from_pixels(
                hub.monitors
                    .get(hub.workspaces.get(ws_id).monitor)
                    .work_area
                    .height(),
            );

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
                let c = window_constraints(hub, &self.size_constraints, wid);
                assert!(
                    dim.width >= c.min_width - VALIDATION_TOLERANCE,
                    "master-stack workspace {ws_id}: window {wid:?} width {} < effective min_width {}",
                    dim.width,
                    c.min_width
                );
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
                let c = window_constraints(hub, &self.size_constraints, wid);
                assert!(
                    dim.width >= c.min_width - VALIDATION_TOLERANCE,
                    "master-stack workspace {ws_id}: window {wid:?} width {} < effective min_width {}",
                    dim.width,
                    c.min_width
                );
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

            let master_ids: Vec<WindowId> = state.master.clone();
            if !master_ids.is_empty() {
                let master_content_h = self.pane_content_height(hub, &master_ids, pane_height);
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
                let stack_content_h = self.pane_content_height(hub, &stack_ids, pane_height);
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
}
