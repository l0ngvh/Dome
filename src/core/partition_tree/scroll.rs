use crate::core::{
    Length,
    hub::HubAccess,
    node::{Child, WorkspaceId},
};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    /// Adjust the workspace's viewport offset so the focused node is fully
    /// visible.
    pub(super) fn scroll_into_view(&mut self, hub: &HubAccess, workspace_id: WorkspaceId) {
        let initial = self.workspaces.get(&workspace_id).unwrap().viewport_offset;

        self.clamp_viewport_offset(hub, workspace_id);

        let Some(ws_state) = self.workspaces.get(&workspace_id) else {
            return;
        };
        let monitor_id = hub.workspaces.get(workspace_id).monitor;
        let screen = hub.monitors.get(monitor_id).dimension;
        let (mut offset_x, mut offset_y) = ws_state.viewport_offset;

        if let Some(focused) = ws_state.focused_tiling {
            let focused_dim = self.child_dimension(focused);
            let scale = hub.monitors.get(monitor_id).scale;
            let reserved_top = self.enclosing_tabbed_strip_total(focused, scale);

            offset_x =
                nudge_offset_into_view(offset_x, focused_dim.x, focused_dim.width, screen.width);
            offset_y =
                nudge_offset_into_view(offset_y, focused_dim.y, focused_dim.height, screen.height);
            // Keep enclosing tab strips visible at the top of the viewport.
            // After this clamp, focused.y - offset_y >= reserved_top, so each
            // enclosing strip sits on or below the top of the screen.
            offset_y = offset_y.min(focused_dim.y - reserved_top);

            self.workspaces
                .get_mut(&workspace_id)
                .unwrap()
                .viewport_offset = (offset_x, offset_y);
        }

        if (offset_x, offset_y) != initial {
            self.adjust_placement(hub, workspace_id);
        }
    }

    /// Clamp the workspace's viewport offset against the root's dimension.
    /// With no root, resets to `(ZERO, ZERO)` so a later attach starts from a
    /// known origin instead of inheriting a stale offset from a previous tree.
    fn clamp_viewport_offset(&mut self, hub: &HubAccess, workspace_id: WorkspaceId) {
        let Some(ws_state) = self.workspaces.get(&workspace_id) else {
            return;
        };
        let screen = hub
            .monitors
            .get(hub.workspaces.get(workspace_id).monitor)
            .dimension;
        let (mut offset_x, mut offset_y) = ws_state.viewport_offset;

        let root_dim = match ws_state.root {
            Some(child) => self.child_dimension(child),
            None => {
                self.workspaces
                    .get_mut(&workspace_id)
                    .unwrap()
                    .viewport_offset = (Length::ZERO, Length::ZERO);
                return;
            }
        };

        offset_x = clamp_offset(offset_x, root_dim.width, screen.width);
        offset_y = clamp_offset(offset_y, root_dim.height, screen.height);
        self.workspaces
            .get_mut(&workspace_id)
            .unwrap()
            .viewport_offset = (offset_x, offset_y);
    }

    /// Sum of `tab_bar_length` over each strict ancestor of `focused` that is
    /// tabbed. Used by `scroll_into_view` to reserve space for tab strips when
    /// clamping the viewport offset.
    fn enclosing_tabbed_strip_total(&self, focused: Child, scale: f32) -> Length {
        let tb = self.tab_bar_length(scale);
        let mut total = Length::ZERO;
        for (_, parent_id) in self.ancestors_of(focused) {
            if self.containers.get(parent_id).is_tabbed() {
                total += tb;
            }
        }
        total
    }
}

fn nudge_offset_into_view(
    offset: Length,
    dim_origin: Length,
    dim_extent: Length,
    screen_extent: Length,
) -> Length {
    let mut offset = offset;
    if dim_origin - offset + dim_extent > screen_extent {
        offset = dim_origin + dim_extent - screen_extent;
    }
    if dim_origin - offset < Length::ZERO {
        offset = dim_origin;
    }
    offset
}

fn clamp_offset(offset: Length, root_extent: Length, screen_extent: Length) -> Length {
    offset.clamp(
        Length::ZERO,
        (root_extent - screen_extent).max(Length::ZERO),
    )
}
