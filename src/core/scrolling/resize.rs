use crate::core::SizeConstraint;
use crate::core::hub::HubAccess;
use crate::core::node::{Length, WorkspaceId};
use crate::core::strategy::tighter_max;

use super::ScrollingStrategy;

const STEP_PERCENT: f32 = 5.0;

impl ScrollingStrategy {
    /// Widens or narrows the focused column by one step of the work area width. The step starts
    /// from the shown width, after the maximum-width clamp, because a step on the stored width
    /// alone could change nothing on screen.
    pub(super) fn resize_focused_column(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        grow: bool,
    ) {
        let Some(column) = self.focused_column_index(hub, ws_id) else {
            return;
        };
        let screen_w = Length::from_pixels(self.workspaces[&ws_id].work_area.width());
        // The column stores its new width as a share of the work area width, and an empty work
        // area has no width to take a share of.
        if screen_w <= Length::ZERO {
            return;
        }
        let shown = self.column_dimensions(hub, ws_id)[column].width;
        let container = self.workspaces[&ws_id].columns[column].container;
        let (min, max) =
            self.column_width_bounds(hub, ws_id, &Self::column_windows(hub, container));
        let step = screen_w * (STEP_PERCENT / 100.0);
        let target = if grow {
            let upper = tighter_max(screen_w, max);
            // A minimum can hold the column wider than `upper`. Storing `upper` then would
            // widen the column once the minimum goes away.
            if shown >= upper {
                return;
            }
            (shown + step).min(upper)
        } else {
            if shown <= min {
                return;
            }
            // A pixel width from `layout.lua` can exceed the work area.
            (shown - step).max(min).min(screen_w)
        };
        self.workspaces.get_mut(&ws_id).unwrap().columns[column].width =
            percent_of(target, screen_w);
        self.compute_placement(hub, ws_id);
    }
}

/// Rounds to two decimals, so a whole-step result exports as a whole percentage.
fn percent_of(target: Length, screen_w: Length) -> SizeConstraint {
    let percent = target.value() / screen_w.value() * 100.0;
    SizeConstraint::Percent((percent * 100.0).round() / 100.0)
}
