use rustc_hash::{FxHashMap, FxHashSet};

use crate::core::hub::HubAccess;
use crate::core::node::{Child, Length, WindowId};
use crate::core::slot::{SlotId, tiling_slots_by_workspace};
use crate::core::strategy::{Reachable, ValidateStrategy, validate_display_modes};

use super::ScrollingStrategy;
use super::placement::max_offsets;

impl ValidateStrategy for ScrollingStrategy {
    fn validate(&self, hub: &HubAccess) -> Reachable {
        let mut containers = FxHashSet::default();
        let mut windows = FxHashMap::default();
        let mut tiling_slots = tiling_slots_by_workspace(&hub.slots);
        let mut tiled: FxHashSet<WindowId> = FxHashSet::default();
        for (&ws_id, state) in &self.workspaces {
            let host = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
            assert_eq!(
                (state.work_area, state.scale),
                (host.work_area, host.scale),
                "scrolling {ws_id}: layout work area and scale differ from the host monitor's"
            );
            let listed: Vec<SlotId> = state
                .column_slots
                .iter()
                .flat_map(|column_slot| column_slot.slots.iter().copied())
                .collect();
            assert_eq!(
                listed,
                tiling_slots.remove(&ws_id).unwrap_or_default(),
                "scrolling {ws_id}: the column slots must list each tiling slot of the workspace \
                 once, in arena order"
            );

            let screen_w = Length::from_pixels(state.work_area.width());
            let mut remembered = FxHashSet::default();
            let mut tiling = Vec::new();
            for column in &state.columns {
                let container = column.container;
                containers.insert(container);
                let reported = hub.containers.get(container).workspace;
                assert_eq!(
                    reported, ws_id,
                    "scrolling {ws_id}: column {container} reports workspace {reported}"
                );
                let children = hub.containers.get(container).children();
                assert!(
                    !children.is_empty(),
                    "scrolling {ws_id}: column {container} holds no window"
                );
                assert!(
                    column.width.resolve(screen_w, state.scale) >= Length::ZERO,
                    "scrolling {ws_id}: column {container} width resolves to a negative length"
                );
                if let Some(index) = column.occupy {
                    assert!(
                        state
                            .column_slots
                            .get(index)
                            .is_some_and(|column_slot| !column_slot.slots.is_empty()),
                        "scrolling {ws_id}: column {container} remembers column slot {index}, \
                         which lists no slot"
                    );
                    assert!(
                        remembered.insert(index),
                        "scrolling {ws_id}: two columns remember column slot {index}"
                    );
                }
                for child in children {
                    let window_id = match child {
                        Child::Window(window_id) => *window_id,
                        Child::Container(_) => {
                            panic!("scrolling {ws_id}: column {container} holds a nested container")
                        }
                    };
                    hub.windows.get(window_id);
                    assert!(
                        tiled.insert(window_id),
                        "scrolling {ws_id}: window {window_id} appears in more than one column"
                    );
                    tiling.push(window_id);
                    let dimension = self
                        .window_states
                        .get(&window_id)
                        .unwrap_or_else(|| {
                            panic!("scrolling {ws_id}: window {window_id} has no window state")
                        })
                        .dimension;
                    // On an empty work area a window is legitimately zero-size.
                    if !state.work_area.is_empty() {
                        assert!(
                            dimension.width >= Length::ZERO && dimension.height > Length::ZERO,
                            "scrolling {ws_id}: window {window_id} has a negative width or a \
                             non-positive height"
                        );
                    }
                }
            }

            assert_eq!(
                state.focus_history.len(),
                tiling.len(),
                "scrolling {ws_id}: focus_history has {} entries for {} tiling windows, so it \
                 holds a duplicate or a stale window",
                state.focus_history.len(),
                tiling.len()
            );
            let history: FxHashSet<WindowId> = state.focus_history.iter().copied().collect();
            let in_columns: FxHashSet<WindowId> = tiling.iter().copied().collect();
            assert_eq!(
                history, in_columns,
                "scrolling {ws_id}: focus_history does not match the columns"
            );
            if let Some(selected) = state.selected_column {
                let holds_focus = state.focused_window().is_some_and(|focus| {
                    state
                        .columns
                        .iter()
                        .any(|column| column.container == selected)
                        && hub
                            .containers
                            .get(selected)
                            .children()
                            .contains(&Child::Window(focus))
                });
                assert!(
                    holds_focus,
                    "scrolling {ws_id}: selected column {selected} is not the column of the \
                     focused window"
                );
            }

            let (max_x, max_y) = max_offsets(&self.column_dimensions(hub, ws_id), state.work_area);
            assert!(
                state.x_offset >= Length::ZERO && state.x_offset <= max_x,
                "scrolling {ws_id}: x_offset {} out of bounds [0, {max_x}]",
                state.x_offset
            );
            for (column, max) in state.columns.iter().zip(max_y) {
                assert!(
                    column.y_offset >= Length::ZERO && column.y_offset <= max,
                    "scrolling {ws_id}: column {} y_offset {} out of bounds [0, {max}]",
                    column.container,
                    column.y_offset
                );
            }

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
        }
        let mut stale: Vec<WindowId> = self
            .window_states
            .keys()
            .filter(|window_id| !tiled.contains(window_id))
            .copied()
            .collect();
        stale.sort_unstable();
        assert!(
            stale.is_empty(),
            "scrolling: window states {stale:?} belong to no column"
        );
        Reachable {
            containers,
            windows,
        }
    }
}
