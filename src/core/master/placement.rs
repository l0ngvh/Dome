use crate::core::{
    Dimension, Length, TilingWindowPlacement, WindowId,
    hub::HubAccess,
    master::{MasterStrategy, WindowState, effective_constraints},
    node::WorkspaceId,
    strategy::{TilingPlacements, clip, distribute_space, translate},
};

impl MasterStrategy {
    pub(super) fn compute_placement_against_constraint(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let master_n = state.master.len();
        let stack_n = state.secondary.len();
        if master_n == 0 && stack_n == 0 {
            return;
        }

        let screen = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension;
        let h = screen.height;

        let master_ids: Vec<WindowId> = state.master.clone();
        let stack_ids: Vec<WindowId> = state.secondary.clone();

        match (master_n, stack_n) {
            (_, 0) => {
                self.do_pane_layout(hub, &master_ids, screen.width, Length::ZERO, h);
            }
            (0, _) => {
                self.do_pane_layout(hub, &stack_ids, screen.width, Length::ZERO, h);
            }
            (_, _) => {
                let master_min_w = master_ids
                    .iter()
                    .map(|&id| effective_constraints(hub, &self.size_constraints, id).min_width)
                    .fold(Length::ZERO, Length::max);
                let stack_min_w = stack_ids
                    .iter()
                    .map(|&id| effective_constraints(hub, &self.size_constraints, id).min_width)
                    .fold(Length::ZERO, Length::max);

                let desired_master_w = Length::new(
                    (screen.width.value() * state.master_ratio.unwrap_or(self.master_ratio))
                        .floor(),
                );
                let total_min = master_min_w + stack_min_w;

                let (master_w, stack_w) = if total_min >= screen.width {
                    (master_min_w, stack_min_w)
                } else if desired_master_w < master_min_w {
                    (master_min_w, screen.width - master_min_w)
                } else if screen.width - desired_master_w < stack_min_w {
                    (screen.width - stack_min_w, stack_min_w)
                } else {
                    (desired_master_w, screen.width - desired_master_w)
                };

                self.do_pane_layout(hub, &master_ids, master_w, Length::ZERO, h);
                self.do_pane_layout(hub, &stack_ids, stack_w, master_w, h);
            }
        }

        self.clamp_scroll(hub, ws_id);
        self.scroll_into_view(hub, ws_id);
    }

    pub(super) fn collect_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        focused: bool,
    ) -> TilingPlacements {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return TilingPlacements {
                windows: Vec::new(),
                containers: Vec::new(),
            };
        };

        let ws = hub.workspaces.get(ws_id);
        let screen = hub.monitors.get(ws.monitor).dimension;

        let mut windows = Vec::with_capacity(state.master.len() + state.secondary.len());

        let focused_id = if focused && !ws.is_float_focused {
            state.focus
        } else {
            None
        };

        let mut push_pane = |vec: &[WindowId], y_offset: Length| {
            for &wid in vec.iter() {
                let dim = self.window_states[&wid].dimension;
                let frame = translate(dim, Length::ZERO, y_offset, screen);
                if let Some(visible_frame) = clip(frame, screen) {
                    let is_highlighted = focused_id == Some(wid);
                    windows.push(TilingWindowPlacement {
                        id: wid,
                        frame,
                        visible_frame,
                        is_highlighted,
                        spawn_indicator: None,
                    });
                }
            }
        };

        push_pane(&state.master, state.master_y_offset);
        push_pane(&state.secondary, state.stack_y_offset);

        TilingPlacements {
            windows,
            containers: Vec::new(),
        }
    }

    fn do_pane_layout(
        &mut self,
        hub: &HubAccess,
        ids: &[WindowId],
        pane_width: Length,
        x_start: Length,
        screen_height: Length,
    ) {
        if ids.is_empty() {
            return;
        }
        let pane_min_w = ids
            .iter()
            .map(|&id| effective_constraints(hub, &self.size_constraints, id).min_width)
            .fold(Length::ZERO, Length::max);
        let adjusted_w = pane_min_w.max(pane_width);

        let constraints: Vec<(Length, Length)> = ids
            .iter()
            .map(|&id| {
                let c = effective_constraints(hub, &self.size_constraints, id);
                (c.min_height, c.max_height)
            })
            .collect();
        let heights = distribute_space(&constraints, screen_height);
        let sum_h: Length = heights.iter().copied().sum();
        let mut y = if sum_h < screen_height {
            (screen_height - sum_h) / 2.0
        } else {
            Length::ZERO
        };
        for (i, &id) in ids.iter().enumerate() {
            let c = effective_constraints(hub, &self.size_constraints, id);
            let (w, x_off) = apply_max_constraint(c.max_width, adjusted_w);
            let (slot_h, y_off) = apply_max_constraint(c.max_height, heights[i]);
            let dim = Dimension::new(x_start + x_off, y + y_off, w, slot_h);
            self.window_states
                .entry(id)
                .and_modify(|s| s.dimension = dim)
                .or_insert(WindowState {
                    occupy: None,
                    dimension: dim,
                });
            y += heights[i];
        }
    }

    fn clamp_scroll(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let state = self.workspaces.get(&ws_id).unwrap();
        let pane_height = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension
            .height;

        let master_ids: Vec<WindowId> = state.master.clone();
        let master_max = if !master_ids.is_empty() {
            let content_h = self.pane_content_height(hub, &master_ids, pane_height);
            (content_h - pane_height).max(Length::ZERO)
        } else {
            Length::ZERO
        };

        let stack_ids: Vec<WindowId> = state.secondary.clone();
        let stack_max = if !stack_ids.is_empty() {
            let content_h = self.pane_content_height(hub, &stack_ids, pane_height);
            (content_h - pane_height).max(Length::ZERO)
        } else {
            Length::ZERO
        };

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.master_y_offset = state.master_y_offset.clamp(Length::ZERO, master_max);
        state.stack_y_offset = state.stack_y_offset.clamp(Length::ZERO, stack_max);
    }
}

fn apply_max_constraint(max: Length, slot_extent: Length) -> (Length, Length) {
    let size = if max > Length::ZERO && max < slot_extent {
        max
    } else {
        slot_extent
    };
    let offset = (slot_extent - size) / 2.0;
    (size, offset.max(Length::ZERO))
}
