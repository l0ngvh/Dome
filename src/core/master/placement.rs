use crate::core::{
    ContainerPlacement, Dimension, Length, PixelRect, TilingWindowPlacement, WindowId,
    hub::HubAccess,
    master::{MasterStrategy, PaneDisplay, PaneKind, WindowState},
    node::WorkspaceId,
    strategy::{
        TilingPlacements, container_titles, distribute_space, tab_bar_band, translate,
        window_constraints,
    },
};

impl MasterStrategy {
    pub(super) fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let master_ids: Vec<WindowId> = Self::pane_windows(hub, state.master.container);
        let stack_ids: Vec<WindowId> = Self::pane_windows(hub, state.secondary.container);
        if master_ids.is_empty() && stack_ids.is_empty() {
            return;
        }
        let master_ratio = state.master_ratio.unwrap_or(self.master_ratio);
        let master_display = state.master.display;
        let secondary_display = state.secondary.display;

        let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        let work_area = monitor.work_area;
        let scale = monitor.scale;
        let screen_width = Length::from_pixels(work_area.width());
        let h = Length::from_pixels(work_area.height());

        let ((master_x, master_w), (stack_x, stack_w)) =
            Self::split_widths(&master_ids, &stack_ids, screen_width, master_ratio);

        if !master_ids.is_empty() {
            if master_display == PaneDisplay::Tabbed && master_ids.len() >= 2 {
                self.do_tabbed_pane_layout(hub, &master_ids, master_x, master_w, h, scale);
            } else {
                self.do_pane_layout(hub, &master_ids, master_w, master_x, h);
            }
        }
        if !stack_ids.is_empty() {
            if secondary_display == PaneDisplay::Tabbed && stack_ids.len() >= 2 {
                self.do_tabbed_pane_layout(hub, &stack_ids, stack_x, stack_w, h, scale);
            } else {
                self.do_pane_layout(hub, &stack_ids, stack_w, stack_x, h);
            }
        }

        self.clamp_scroll(hub, ws_id);
        self.scroll_into_view(hub, ws_id);
    }

    pub(super) fn collect_tiling_placements(
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
        let monitor = hub.monitors.get(ws.monitor);
        let screen = monitor.work_area;
        let scale = monitor.scale;
        let border = hub.border(ws.monitor);

        let master_ids = Self::pane_windows(hub, state.master.container);
        let stack_ids = Self::pane_windows(hub, state.secondary.container);
        let screen_width = Length::from_pixels(screen.width());
        let master_ratio = state.master_ratio.unwrap_or(self.master_ratio);
        let ((master_x, master_w), (stack_x, stack_w)) =
            Self::split_widths(&master_ids, &stack_ids, screen_width, master_ratio);

        let focused_id = if focused && !ws.is_float_focused {
            state.focused_window()
        } else {
            None
        };

        let mut windows = Vec::new();
        let mut containers = Vec::new();

        for (kind, ids, pane_x, pane_w) in [
            (PaneKind::Master, &master_ids, master_x, master_w),
            (PaneKind::Secondary, &stack_ids, stack_x, stack_w),
        ] {
            if ids.is_empty() {
                continue;
            }
            let pane = state.pane(kind);
            if pane.display == PaneDisplay::Tabbed && ids.len() >= 2 {
                let active = self.last_focused_in(hub, ws_id, kind);
                let dim = self.window_states[&active].dimension;
                let border_box = translate(dim, Length::ZERO, Length::ZERO, screen.x(), screen.y());
                if let Some(visible_border_box) = border_box.clip(screen) {
                    let content_box = border_box.inset_by(border);
                    windows.push(TilingWindowPlacement {
                        id: active,
                        border_box,
                        visible_border_box,
                        content_box,
                        visible_content_box: content_box.clip(screen).unwrap_or(PixelRect::ZERO),
                        is_highlighted: focused_id == Some(active),
                        spawn_indicator: None,
                    });
                }
                let pane_dim = Dimension::new(
                    pane_x,
                    Length::ZERO,
                    pane_w,
                    Length::from_pixels(screen.height()),
                );
                let border_box =
                    translate(pane_dim, Length::ZERO, Length::ZERO, screen.x(), screen.y());
                if let Some(visible_border_box) = border_box.clip(screen) {
                    containers.push(ContainerPlacement {
                        id: pane.container,
                        border_box,
                        visible_border_box,
                        tab_bar_band: tab_bar_band(
                            border_box,
                            pane_dim,
                            Length::ZERO,
                            screen,
                            self.tab_bar_length(scale),
                            true,
                        ),
                        is_highlighted: false,
                        spawn_indicator: None,
                        is_tabbed: true,
                        active_tab_index: Self::position_in_pane(hub, pane.container, active)
                            .unwrap_or(0),
                        titles: container_titles(hub, pane.container),
                    });
                }
            } else {
                for &wid in ids.iter() {
                    let dim = self.window_states[&wid].dimension;
                    let border_box =
                        translate(dim, Length::ZERO, pane.y_offset, screen.x(), screen.y());
                    if let Some(visible_border_box) = border_box.clip(screen) {
                        let content_box = border_box.inset_by(border);
                        windows.push(TilingWindowPlacement {
                            id: wid,
                            border_box,
                            visible_border_box,
                            content_box,
                            visible_content_box: content_box
                                .clip(screen)
                                .unwrap_or(PixelRect::ZERO),
                            is_highlighted: focused_id == Some(wid),
                            spawn_indicator: None,
                        });
                    }
                }
            }
        }

        TilingPlacements {
            windows,
            containers,
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
        let constraints: Vec<(Length, Length)> = ids
            .iter()
            .map(|&id| {
                let c = window_constraints(hub, &self.size_constraints, id);
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
            let c = window_constraints(hub, &self.size_constraints, id);
            let (w, x_off) = apply_max_constraint(c.max_width, pane_width);
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

    /// Pane x offset and width, shared by layout and rendering so both agree on pane geometry.
    fn split_widths(
        master_ids: &[WindowId],
        stack_ids: &[WindowId],
        screen_width: Length,
        master_ratio: f32,
    ) -> ((Length, Length), (Length, Length)) {
        match (master_ids.len(), stack_ids.len()) {
            (_, 0) => ((Length::ZERO, screen_width), (screen_width, Length::ZERO)),
            (0, _) => ((Length::ZERO, Length::ZERO), (Length::ZERO, screen_width)),
            (_, _) => {
                // Master ignores per-window min width. With no horizontal scroll to absorb it,
                // an oversized min width would push the other pane off screen, so the split
                // follows master_ratio alone and each pane fills its share.
                let master_w = Length::new((screen_width.value() * master_ratio).floor());
                let stack_w = screen_width - master_w;
                ((Length::ZERO, master_w), (master_w, stack_w))
            }
        }
    }

    fn do_tabbed_pane_layout(
        &mut self,
        hub: &HubAccess,
        ids: &[WindowId],
        x_start: Length,
        pane_width: Length,
        screen_height: Length,
        scale: f32,
    ) {
        let band = self.tab_bar_length(scale);
        let content_h = (screen_height - band).max(Length::ZERO);
        // Every tab shares the content box, so all windows get a dimension even
        // though only the active one renders. The content box has zero height when
        // the tab bar is taller than the screen, so each window keeps its min height.
        for &wid in ids {
            let c = window_constraints(hub, &self.size_constraints, wid);
            let adjusted_w = c.min_width.max(pane_width);
            let (w, x_off) = apply_max_constraint(c.max_width, adjusted_w);
            let adjusted_h = c.min_height.max(content_h);
            let (slot_h, y_off) = apply_max_constraint(c.max_height, adjusted_h);
            let dim = Dimension::new(x_start + x_off, band + y_off, w, slot_h);
            self.window_states
                .entry(wid)
                .and_modify(|s| s.dimension = dim)
                .or_insert(WindowState {
                    occupy: None,
                    dimension: dim,
                });
        }
    }

    fn clamp_scroll(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let state = self.workspaces.get(&ws_id).unwrap();
        let pane_height = Length::from_pixels(
            hub.monitors
                .get(hub.workspaces.get(ws_id).monitor)
                .work_area
                .height(),
        );

        let master_ids: Vec<WindowId> = Self::pane_windows(hub, state.master.container);
        let master_tabbed = state.master.display == PaneDisplay::Tabbed && master_ids.len() >= 2;
        let master_max = if !master_ids.is_empty() && !master_tabbed {
            let content_h = self.pane_content_height(hub, &master_ids, pane_height);
            (content_h - pane_height).max(Length::ZERO)
        } else {
            Length::ZERO
        };

        let stack_ids: Vec<WindowId> = Self::pane_windows(hub, state.secondary.container);
        let stack_tabbed = state.secondary.display == PaneDisplay::Tabbed && stack_ids.len() >= 2;
        let stack_max = if !stack_ids.is_empty() && !stack_tabbed {
            let content_h = self.pane_content_height(hub, &stack_ids, pane_height);
            (content_h - pane_height).max(Length::ZERO)
        } else {
            Length::ZERO
        };

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.master.y_offset = state.master.y_offset.clamp(Length::ZERO, master_max);
        state.secondary.y_offset = state.secondary.y_offset.clamp(Length::ZERO, stack_max);
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
