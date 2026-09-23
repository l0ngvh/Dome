use crate::core::{
    Dimension, Direction, Length, Pixels,
    hub::HubAccess,
    node::WorkspaceId,
    scrolling::{
        ScrollingStrategy,
        placement::{max_offsets, work_area},
    },
    strategy::translate,
};

impl ScrollingStrategy {
    pub(super) fn scroll_into_view(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some(idx) = self.focused_column_index(hub, ws_id) else {
            return;
        };
        let columns = self.column_dimensions(hub, ws_id);
        let screen = work_area(hub, ws_id);
        let (max_x, _) = max_offsets(&columns, screen);
        let viewport = Length::from_pixels(screen.width());
        let focused = columns[idx];
        let offset = self.workspaces[&ws_id].x_offset;
        let shows_part = focused.x < offset + viewport && focused.x + focused.width > offset;

        let target = if focused.width > viewport && shows_part {
            offset
        } else {
            nearest_edge(focused, offset, viewport)
        };
        self.workspaces.get_mut(&ws_id).unwrap().x_offset = target.clamp(Length::ZERO, max_x);
    }

    /// Pixels of the focused window past the work area edge on the `forward` side of
    /// `direction`. The measure uses the rendered border box in whole pixels, so an f32
    /// remainder in an offset never counts as a hidden part.
    pub(super) fn hidden_extent(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        direction: Direction,
        forward: bool,
    ) -> Pixels {
        let Some(idx) = self.focused_column_index(hub, ws_id) else {
            return Pixels::ZERO;
        };
        let state = &self.workspaces[&ws_id];
        let column = &state.columns[idx];
        let wid = Self::column_window(hub, column.container);
        let screen = work_area(hub, ws_id);
        let rect = translate(
            self.window_states[&wid],
            state.x_offset,
            column.y_offset,
            screen.x(),
            screen.y(),
        );
        let hidden = match (direction, forward) {
            (Direction::Horizontal, true) => rect.right() - screen.right(),
            (Direction::Horizontal, false) => screen.x() - rect.x(),
            (Direction::Vertical, true) => rect.bottom() - screen.bottom(),
            (Direction::Vertical, false) => screen.y() - rect.y(),
        };
        hidden.max(Pixels::ZERO)
    }

    /// Returns false, and changes nothing, when no part of the focused window lies
    /// past the work area edge on the `forward` side of `direction`.
    pub(super) fn reveal_hidden_part(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        direction: Direction,
        forward: bool,
    ) -> bool {
        let hidden = self.hidden_extent(hub, ws_id, direction, forward);
        if hidden == Pixels::ZERO {
            return false;
        }
        let Some(idx) = self.focused_column_index(hub, ws_id) else {
            return false;
        };
        let screen = work_area(hub, ws_id);
        let viewport = match direction {
            Direction::Horizontal => screen.width(),
            Direction::Vertical => screen.height(),
        };
        let step = Length::from_pixels(hidden.min(viewport));
        let (max_x, max_y) = max_offsets(&self.column_dimensions(hub, ws_id), screen);
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let (offset, max) = match direction {
            Direction::Horizontal => (&mut state.x_offset, max_x),
            Direction::Vertical => (&mut state.columns[idx].y_offset, max_y[idx]),
        };
        let moved = if forward {
            *offset + step
        } else {
            *offset - step
        };
        *offset = moved.clamp(Length::ZERO, max);
        true
    }
}

/// Least scroll that shows the whole column. A column wider than the viewport cannot
/// show whole, so the function aligns the column edge that faces the viewport.
fn nearest_edge(column: Dimension, offset: Length, viewport: Length) -> Length {
    let (start, end) = (column.x, column.x + column.width);
    if column.width > viewport {
        return if start >= offset + viewport {
            start
        } else {
            end - viewport
        };
    }
    if start < offset {
        start
    } else if end > offset + viewport {
        end - viewport
    } else {
        offset
    }
}
