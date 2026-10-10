use crate::core::hub::HubAccess;
use crate::core::node::{Dimension, Direction, Length, Pixels, WorkspaceId};
use crate::core::strategy::translate;

use super::ScrollingStrategy;
use super::placement::max_offsets;

impl ScrollingStrategy {
    /// Scrolls the focused column into view across the row and the focused window down its
    /// column. `columns` must be the workspace's `column_dimensions`.
    pub(super) fn scroll_into_view(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        columns: &[Dimension],
    ) {
        let Some(column) = self.focused_column_index(hub, ws_id) else {
            return;
        };
        let state = &self.workspaces[&ws_id];
        let screen = state.work_area;
        let (max_x, max_y) = max_offsets(columns, screen);
        let focus = state
            .focused_window()
            .expect("a focused column implies a focused window");
        // The vertical axis measures the focused window, because focus in a stack can land on a
        // window below the work area.
        let focused_window = self.window_states[&focus].dimension;
        let new_x = scroll_axis(
            columns[column].x,
            columns[column].width,
            state.x_offset,
            Length::from_pixels(screen.width()),
            max_x,
        );
        let new_y = scroll_axis(
            focused_window.y,
            focused_window.height,
            state.columns[column].y_offset,
            Length::from_pixels(screen.height()),
            max_y[column],
        );
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.x_offset = new_x;
        state.columns[column].y_offset = new_y;
    }

    /// Pixels of the focused window, or of the whole column while it is selected, past the work
    /// area edge on the `forward` side of `direction`. The measure uses the rendered border box
    /// in whole pixels, so an f32 remainder in an offset never counts as a hidden part.
    fn hidden_extent(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        direction: Direction,
        forward: bool,
    ) -> Pixels {
        let Some(column) = self.focused_column_index(hub, ws_id) else {
            return Pixels::ZERO;
        };
        let state = &self.workspaces[&ws_id];
        let Some(focus) = state.focused_window() else {
            return Pixels::ZERO;
        };
        let screen = state.work_area;
        let dimension = if state.selected_column.is_some() {
            self.column_dimensions(hub, ws_id)[column]
        } else {
            self.window_states[&focus].dimension
        };
        let rect = translate(
            dimension,
            state.x_offset,
            state.columns[column].y_offset,
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

    /// Scrolls toward the part of the focused window, or of the selected column, that lies past
    /// the work area edge on the `forward` side of `direction`, by at most one viewport length.
    /// Returns false, and changes nothing, when no such part exists.
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
        let Some(column) = self.focused_column_index(hub, ws_id) else {
            return false;
        };
        let screen = self.workspaces[&ws_id].work_area;
        let viewport = match direction {
            Direction::Horizontal => screen.width(),
            Direction::Vertical => screen.height(),
        };
        let step = Length::from_pixels(hidden.min(viewport));
        let (max_x, max_y) = max_offsets(&self.column_dimensions(hub, ws_id), screen);
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let (offset, max) = match direction {
            Direction::Horizontal => (&mut state.x_offset, max_x),
            Direction::Vertical => (&mut state.columns[column].y_offset, max_y[column]),
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

/// The offset that brings the span `[start, start + extent)` into a `viewport` starting at
/// `offset`, clamped to `[0, max]`. A span longer than the viewport that already shows a part
/// keeps `offset`, so a partly revealed span is not snapped back to its start.
fn scroll_axis(
    start: Length,
    extent: Length,
    offset: Length,
    viewport: Length,
    max: Length,
) -> Length {
    let shows_part = start < offset + viewport && start + extent > offset;
    let target = if extent > viewport && shows_part {
        offset
    } else {
        nearest_edge(start, extent, offset, viewport)
    };
    target.clamp(Length::ZERO, max)
}

/// Least scroll that shows the whole span `[start, start + extent)`. A span longer than the
/// viewport cannot show whole. When it lies after the viewport, its start is aligned with the
/// viewport's start, and otherwise its end is aligned with the viewport's end.
fn nearest_edge(start: Length, extent: Length, offset: Length, viewport: Length) -> Length {
    let end = start + extent;
    if extent > viewport {
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
