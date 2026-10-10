use crate::core::{
    ContainerPlacement, Dimension, Direction, Length, PixelRect, TilingWindowPlacement,
    hub::HubAccess,
    node::{Constraints, ContainerId, WindowId, WorkspaceId},
    strategy::{
        apply_max_constraint, container_titles, distribute_space, tighter_max, translate,
        window_constraints,
    },
};

use super::ScrollingStrategy;

impl ScrollingStrategy {
    pub(super) fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let usable_h = Length::from_pixels(state.work_area.height());
        let containers: Vec<ContainerId> = state.columns.iter().map(|c| c.container).collect();
        let rects = self.column_dimensions(hub, ws_id);
        for (container, rect) in containers.into_iter().zip(&rects) {
            let windows = Self::column_windows(hub, container);
            let constraints: Vec<Constraints> = windows
                .iter()
                .map(|&window_id| self.effective_constraints(hub, ws_id, window_id))
                .collect();
            let heights: Vec<Length> = self
                .window_heights(hub, ws_id, &windows, usable_h)
                .into_iter()
                .zip(&constraints)
                .map(|(share, c)| apply_max_constraint(c.max_height, share).0)
                .collect();
            // The windows stay together, so the height a capped window frees is split between
            // the top and the bottom of the column.
            let stack_h: Length = heights.iter().copied().sum();
            let mut y = rect.y + ((rect.height - stack_h) / 2.0).max(Length::ZERO);
            for ((window_id, c), height) in windows.into_iter().zip(&constraints).zip(heights) {
                let (width, x_offset) = apply_max_constraint(c.max_width, rect.width);
                self.window_states.get_mut(&window_id).unwrap().dimension =
                    Dimension::new(rect.x + x_offset, y, width, height);
                y += height;
            }
        }
        self.clamp_scroll(ws_id, &rects);
        self.scroll_into_view(hub, ws_id, &rects);
    }

    /// Each window's share of the column height, top to bottom, with maximum heights ignored.
    /// The windows share the height equally, except that a window with a larger minimum keeps
    /// its minimum.
    pub(super) fn window_heights(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        windows: &[WindowId],
        usable_h: Length,
    ) -> Vec<Length> {
        let constraints: Vec<(Length, Length)> = windows
            .iter()
            .map(|&window_id| {
                let c = self.effective_constraints(hub, ws_id, window_id);
                (c.min_height, Length::ZERO)
            })
            .collect();
        distribute_space(&constraints, usable_h)
    }

    /// Column rectangles left to right, in unscrolled workspace space. A row narrower than the
    /// work area starts at half the free width. A column's height exceeds the usable height when
    /// its windows' minimum heights add up past it.
    pub(super) fn column_dimensions(&self, hub: &HubAccess, ws_id: WorkspaceId) -> Vec<Dimension> {
        let state = &self.workspaces[&ws_id];
        let screen_w = Length::from_pixels(state.work_area.width());
        let usable_h = Length::from_pixels(state.work_area.height());
        let sizes: Vec<(Length, Length)> = state
            .columns
            .iter()
            .map(|column| {
                let windows = Self::column_windows(hub, column.container);
                let (min_width, max_width) = self.column_width_bounds(hub, ws_id, &windows);
                let stored = column.width.resolve(screen_w, state.scale);
                let capped = if max_width > Length::ZERO {
                    stored.min(max_width)
                } else {
                    stored
                };
                // A minimum beats a smaller maximum, because a window never shrinks past the
                // minimum it declared.
                let width = capped.max(min_width);
                let sum_h: Length = self
                    .window_heights(hub, ws_id, &windows, usable_h)
                    .iter()
                    .copied()
                    .sum();
                (width, usable_h.max(sum_h))
            })
            .collect();
        let row_width: Length = sizes.iter().map(|&(width, _)| width).sum();
        // A row as wide as the work area or wider starts at the left edge, so the horizontal
        // scroll still reaches every column.
        let mut x = ((screen_w - row_width) / 2.0).max(Length::ZERO);
        sizes
            .into_iter()
            .map(|(width, height)| {
                let rect = Dimension::new(x, Length::ZERO, width, height);
                x += width;
                rect
            })
            .collect()
    }

    /// The narrowest and widest the column may be. The widest is zero when no window caps it.
    pub(super) fn column_width_bounds(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        windows: &[WindowId],
    ) -> (Length, Length) {
        windows
            .iter()
            .fold((Length::ZERO, Length::ZERO), |(min, max), &window_id| {
                let c = self.effective_constraints(hub, ws_id, window_id);
                (min.max(c.min_width), tighter_max(max, c.max_width))
            })
    }

    fn clamp_scroll(&mut self, ws_id: WorkspaceId, columns: &[Dimension]) {
        let (max_x, max_y) = max_offsets(columns, self.workspaces[&ws_id].work_area);
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.x_offset = state.x_offset.clamp(Length::ZERO, max_x);
        for (column, max) in state.columns.iter_mut().zip(max_y) {
            column.y_offset = column.y_offset.clamp(Length::ZERO, max);
        }
    }

    /// When `highlighted` is true, the placement of the selected column, or else of the
    /// workspace's tiling focus, is highlighted and carries its spawn direction. A selected column
    /// carries a vertical one, because a new window opens at its bottom. A window or a column that
    /// lies wholly outside the work area gets no placement.
    pub(super) fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
    ) -> (Vec<TilingWindowPlacement>, Vec<ContainerPlacement>) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return (Vec::new(), Vec::new());
        };
        let screen = state.work_area;
        let border = hub.border_for_scale(state.scale);
        let (focused_id, selected_column) = match (highlighted, state.selected_column) {
            (false, _) => (None, None),
            (true, Some(container)) => (None, Some(container)),
            (true, None) => (state.focused_window(), None),
        };

        let mut windows = Vec::new();
        let mut containers = Vec::new();
        for (column, column_rect) in state.columns.iter().zip(self.column_dimensions(hub, ws_id)) {
            for window_id in Self::column_windows(hub, column.container) {
                let border_box = translate(
                    self.window_states[&window_id].dimension,
                    state.x_offset,
                    column.y_offset,
                    screen.x(),
                    screen.y(),
                );
                let Some(visible_border_box) = border_box.clip(screen) else {
                    continue;
                };
                let content_box = border_box.inset_by(border);
                let is_highlighted = focused_id == Some(window_id);
                windows.push(TilingWindowPlacement {
                    id: window_id,
                    border_box,
                    visible_border_box,
                    content_box,
                    visible_content_box: content_box.clip(screen).unwrap_or(PixelRect::ZERO),
                    is_highlighted,
                    spawn_direction: is_highlighted
                        .then(|| self.window_states[&window_id].spawn_direction),
                });
            }
            let border_box = translate(
                column_rect,
                state.x_offset,
                column.y_offset,
                screen.x(),
                screen.y(),
            );
            let Some(visible_border_box) = border_box.clip(screen) else {
                continue;
            };
            let is_highlighted = selected_column == Some(column.container);
            containers.push(ContainerPlacement {
                id: column.container,
                border_box,
                visible_border_box,
                tab_bar_band: PixelRect::ZERO,
                visible_tab_bar_band: PixelRect::ZERO,
                is_highlighted,
                spawn_direction: is_highlighted.then_some(Direction::Vertical),
                is_tabbed: false,
                active_tab_index: 0,
                titles: container_titles(hub, column.container),
            });
        }
        (windows, containers)
    }

    /// Resolves the window's constraints against the work area and scale of the workspace.
    fn effective_constraints(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        window_id: WindowId,
    ) -> Constraints {
        let state = &self.workspaces[&ws_id];
        window_constraints(
            hub,
            &self.size_constraints,
            window_id,
            state.work_area,
            state.scale,
        )
    }
}

/// The largest horizontal offset, then the largest vertical offset of each column. Each is zero
/// when the content fits.
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
