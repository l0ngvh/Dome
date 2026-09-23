use crate::core::{
    ContainerPlacement, Dimension, Length, PixelRect, TilingWindowPlacement,
    hub::HubAccess,
    node::WorkspaceId,
    scrolling::ScrollingStrategy,
    strategy::{
        TilingPlacements, apply_max_constraint, container_titles, translate, window_constraints,
    },
};

impl ScrollingStrategy {
    pub(super) fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        if !self.workspaces.contains_key(&ws_id) {
            return;
        }
        let columns = self.column_dimensions(hub, ws_id);
        let state = &self.workspaces[&ws_id];
        for (column, rect) in state.columns.iter().zip(columns) {
            let wid = Self::column_window(hub, column.container);
            let c = window_constraints(hub, &self.size_constraints, wid);
            let (win_w, x_off) = apply_max_constraint(c.max_width, rect.width);
            let (win_h, y_off) = apply_max_constraint(c.max_height, rect.height);
            self.window_states.insert(
                wid,
                Dimension::new(rect.x + x_off, rect.y + y_off, win_w, win_h),
            );
        }
        self.clamp_scroll(hub, ws_id);
        self.scroll_into_view(hub, ws_id);
    }

    /// Column rectangles left to right, in unscrolled workspace space. A column's
    /// height exceeds the usable height when its window's minimum height does.
    pub(super) fn column_dimensions(&self, hub: &HubAccess, ws_id: WorkspaceId) -> Vec<Dimension> {
        let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        let screen_w = Length::from_pixels(monitor.work_area.width());
        let usable_h = Length::from_pixels(monitor.work_area.height());
        let mut x = Length::ZERO;
        self.workspaces[&ws_id]
            .columns
            .iter()
            .map(|column| {
                let wid = Self::column_window(hub, column.container);
                let c = window_constraints(hub, &self.size_constraints, wid);
                let width = column
                    .width
                    .resolve(screen_w, monitor.scale)
                    .max(c.min_width);
                let rect = Dimension::new(x, Length::ZERO, width, usable_h.max(c.min_height));
                x += width;
                rect
            })
            .collect()
    }

    fn clamp_scroll(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let (max_x, max_y) =
            max_offsets(&self.column_dimensions(hub, ws_id), work_area(hub, ws_id));
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.x_offset = state.x_offset.clamp(Length::ZERO, max_x);
        for (column, max) in state.columns.iter_mut().zip(max_y) {
            column.y_offset = column.y_offset.clamp(Length::ZERO, max);
        }
    }

    pub(super) fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
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
        let border = hub.border(ws.monitor);

        let focused_id = if highlighted && !ws.is_float_focused {
            state.focused_window()
        } else {
            None
        };

        let mut windows = Vec::new();
        let mut containers = Vec::new();
        for col in &state.columns {
            let cid = col.container;
            let wid = Self::column_window(hub, cid);
            let dim = self.window_states[&wid];
            let border_box = translate(dim, state.x_offset, col.y_offset, screen.x(), screen.y());
            let Some(visible_border_box) = border_box.clip(screen) else {
                continue;
            };
            let content_box = border_box.inset_by(border);
            windows.push(TilingWindowPlacement {
                id: wid,
                border_box,
                visible_border_box,
                content_box,
                visible_content_box: content_box.clip(screen).unwrap_or(PixelRect::ZERO),
                is_highlighted: focused_id == Some(wid),
                spawn_direction: None,
            });
            // One window fills its column, so the column border-box equals it.
            containers.push(ContainerPlacement {
                id: cid,
                border_box,
                visible_border_box,
                tab_bar_band: PixelRect::ZERO,
                visible_tab_bar_band: PixelRect::ZERO,
                is_highlighted: false,
                spawn_direction: None,
                is_tabbed: false,
                active_tab_index: 0,
                titles: container_titles(hub, cid),
            });
        }

        TilingPlacements {
            windows,
            containers,
        }
    }
}

pub(super) fn work_area(hub: &HubAccess, ws_id: WorkspaceId) -> PixelRect {
    hub.monitors
        .get(hub.workspaces.get(ws_id).monitor)
        .work_area
}

/// Largest horizontal offset, then the largest vertical offset for each column.
/// Each is zero when the content fits.
pub(super) fn max_offsets(columns: &[Dimension], work_area: PixelRect) -> (Length, Vec<Length>) {
    let total = columns.last().map_or(Length::ZERO, |c| c.x + c.width);
    let screen_w = Length::from_pixels(work_area.width());
    let screen_h = Length::from_pixels(work_area.height());
    let max_x = (total - screen_w).max(Length::ZERO);
    let max_y = columns
        .iter()
        .map(|c| (c.height - screen_h).max(Length::ZERO))
        .collect();
    (max_x, max_y)
}
