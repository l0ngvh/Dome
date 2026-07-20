use crate::core::{
    Length, WindowId,
    hub::HubAccess,
    master::{MasterStrategy, effective_constraints},
    strategy::ValidateStrategy,
};

impl ValidateStrategy for MasterStrategy {
    fn validate(&self, hub: &HubAccess) {
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
