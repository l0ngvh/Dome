use rustc_hash::FxHashSet;

use crate::core::{
    Length, WindowId,
    hub::HubAccess,
    node::{Child, ContainerId},
    scrolling::{
        ScrollingStrategy,
        placement::{max_offsets, work_area},
    },
    strategy::ValidateStrategy,
};

impl ValidateStrategy for ScrollingStrategy {
    fn validate(&self, hub: &HubAccess) -> FxHashSet<ContainerId> {
        let mut reachable = FxHashSet::default();
        for (&ws_id, state) in &self.workspaces {
            reachable.extend(state.columns.iter().map(|c| c.container));
            let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
            let screen_w = Length::from_pixels(monitor.work_area.width());
            let scale = monitor.scale;
            let mut seen = FxHashSet::default();
            for col in &state.columns {
                let cid = col.container;
                let children = hub.containers.get(cid).children();
                assert_eq!(
                    children.len(),
                    1,
                    "scrolling {ws_id}: column {cid:?} does not hold exactly one window"
                );
                let Child::Window(wid) = children[0] else {
                    panic!("scrolling {ws_id}: column {cid:?} holds a nested container")
                };
                hub.windows.get(wid);
                assert!(
                    seen.insert(wid),
                    "scrolling {ws_id}: window {wid:?} appears in more than one column"
                );
                let dim = self.window_states[&wid];
                assert!(
                    dim.width > Length::ZERO && dim.height > Length::ZERO,
                    "scrolling {ws_id}: window {wid:?} has a non-positive dimension"
                );
                assert!(
                    col.width.resolve(screen_w, scale) > Length::ZERO,
                    "scrolling {ws_id}: column {cid:?} width resolves to a non-positive length"
                );
                if let Some(slot) = col.occupy {
                    assert!(
                        state
                            .slots
                            .get(slot)
                            .is_some_and(|s| !s.children.is_empty()),
                        "scrolling {ws_id}: column {cid:?} occupies slot {slot}, which holds no matcher"
                    );
                }
            }
            let history: FxHashSet<WindowId> = state.focus_history.iter().copied().collect();
            assert_eq!(
                history, seen,
                "scrolling {ws_id}: focus_history does not match the columns"
            );
            let (max_x, max_y) =
                max_offsets(&self.column_dimensions(hub, ws_id), work_area(hub, ws_id));
            assert!(
                state.x_offset >= Length::ZERO && state.x_offset <= max_x,
                "scrolling {ws_id}: x_offset {} out of bounds [0, {max_x}]",
                state.x_offset
            );
            for (column, max) in state.columns.iter().zip(max_y) {
                assert!(
                    column.y_offset >= Length::ZERO && column.y_offset <= max,
                    "scrolling {ws_id}: column {:?} y_offset {} out of bounds [0, {max}]",
                    column.container,
                    column.y_offset
                );
            }
        }
        reachable
    }
}
