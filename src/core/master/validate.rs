use rustc_hash::{FxHashMap, FxHashSet};

use crate::core::{
    Length, WindowId,
    hub::HubAccess,
    master::{MasterStrategy, PaneDisplay, PaneKind},
    slot::{SlotId, tiling_slots_by_workspace},
    strategy::{
        Reachable, VALIDATION_TOLERANCE, ValidateStrategy, validate_display_modes,
        window_constraints,
    },
};

impl ValidateStrategy for MasterStrategy {
    fn validate(&self, hub: &HubAccess) -> Reachable {
        let mut reachable = FxHashSet::default();
        let mut windows = FxHashMap::default();
        let mut tiling_slots = tiling_slots_by_workspace(&hub.slots);
        for (&ws_id, state) in &self.workspaces {
            let host = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
            assert_eq!(
                (state.work_area, state.scale),
                (host.work_area, host.scale),
                "master-stack workspace {ws_id}: layout work area and scale differ from the host \
                 monitor's"
            );
            reachable.insert(state.master.container);
            reachable.insert(state.secondary.container);
            for pane in [&state.master, &state.secondary] {
                let reported = hub.containers.get(pane.container).workspace;
                assert_eq!(
                    reported, ws_id,
                    "master-stack workspace {ws_id}: pane container {} reports workspace {reported}",
                    pane.container
                );
            }
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
            let tiling: Vec<WindowId> = master.iter().chain(&secondary).copied().collect();
            validate_display_modes(
                hub,
                ws_id,
                &tiling,
                &state.float_windows,
                &state.fullscreen_windows,
            );
            let mut all = tiling;
            all.extend(state.float_windows.windows());
            all.extend(state.fullscreen_windows.windows());
            windows.insert(ws_id, all);
            let pane_slots: Vec<SlotId> = state
                .master
                .slots
                .iter()
                .chain(&state.secondary.slots)
                .copied()
                .collect();
            assert_eq!(
                pane_slots,
                tiling_slots.remove(&ws_id).unwrap_or_default(),
                "master-stack workspace {ws_id}: the panes must list each tiling slot of the \
                 workspace once, in arena order, master slots first"
            );

            if master.is_empty() && secondary.is_empty() {
                continue;
            }

            let pane_height = Length::from_pixels(host.work_area.height());

            for &wid in &master {
                let dim = self.window_states[&wid].dimension;
                // On an empty work area a window is legitimately zero-size.
                if !host.work_area.is_empty() {
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
                }
                let c = window_constraints(
                    hub,
                    &self.size_constraints,
                    wid,
                    host.work_area,
                    host.scale,
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

            for &wid in &secondary {
                let dim = self.window_states[&wid].dimension;
                if !host.work_area.is_empty() {
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
                }
                let c = window_constraints(
                    hub,
                    &self.size_constraints,
                    wid,
                    host.work_area,
                    host.scale,
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

            for kind in [PaneKind::Master, PaneKind::Secondary] {
                let pane = state.pane(kind);
                if pane.display == PaneDisplay::Tabbed && Self::pane_len(hub, pane.container) >= 2 {
                    assert!(
                        pane.y_offset == Length::ZERO,
                        "master-stack workspace {ws_id}: tabbed {kind:?} pane y_offset {} is not zero",
                        pane.y_offset
                    );
                }
            }
        }
        Reachable {
            containers: reachable,
            windows,
        }
    }
}
