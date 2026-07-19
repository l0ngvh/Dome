//! Layout pipeline for the partition tree.
//!
//! `do_layout_workspace` runs two passes:
//! 1. Bottom-up `update_container_min_size` walk: a container's minimum
//!    is the sum (along its split axis) or max (across) of its
//!    children's minimums. Must complete before pass 2 because the
//!    root's minimum can exceed the screen.
//! 2. Top-down `do_layout_top_down`: places the root, then
//!    distributes each container's space to its children using the
//!    current viewport offset.
//!
//! `scroll_into_view` then clamps and adjusts the viewport offset to
//! keep the focused node visible. When the offset moves it re-runs
//! `do_layout_top_down` so max-constrained windows recenter inside
//! the new visible section. The `initial` capture in
//! `scroll_into_view` gates this re-run: layout converges in one extra
//! pass at most because the second `do_layout_top_down` does not
//! move the focused node.

use crate::core::hub::HubAccess;
use crate::core::node::Constraints;
use crate::core::node::{ContainerId, Dimension, Direction, Length, WorkspaceId};
use crate::core::partition_tree::{Child, SpawnMode};
use crate::core::strategy::{clip, distribute_space};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    /// Two-pass layout: bottom-up to compute minimum sizes (a container's min
    /// is the sum of its children's mins), then top-down to distribute space.
    /// A single pass can't do both because the total minimum must be known
    /// before distributing remaining space.
    pub(super) fn do_layout_workspace(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        let Some(ws_state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let Some(root) = ws_state.root else { return };

        if let Child::Container(root_id) = root {
            let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
            let scale = monitor.scale;

            // Collect containers in pre-order
            let order: Vec<_> = self.containers_preorder(root_id).collect();

            // Update minimum sizes bottom-up, as parent's minimum size depends on children's
            for &cid in order.iter().rev() {
                self.update_container_min_size(hub, cid, scale);
            }
        }

        self.do_layout_top_down(hub, ws_id);
        self.scroll_into_view(hub, ws_id);
    }

    /// Top-down placement pass: places the root and distributes space to
    /// each container's children using the current viewport_offset.
    pub(super) fn do_layout_top_down(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        let Some(ws_state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let Some(root) = ws_state.root else { return };
        let viewport_offset = ws_state.viewport_offset;
        let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        let screen = monitor.dimension;
        let scale = monitor.scale;
        let (offset_x, offset_y) = viewport_offset;
        let viewport_rect = Dimension::new(offset_x, offset_y, screen.width, screen.height);

        self.set_root_dimension(hub, root, screen, viewport_rect);

        let Child::Container(root_id) = root else {
            return;
        };

        let order: Vec<_> = self.containers_preorder(root_id).collect();

        for cid in order {
            let container = self.containers.get(cid);
            let dim = container.dimension;
            let children = container.children.clone();
            let direction = container.direction();
            for (child, child_dim) in children.iter().zip(self.layout_children(
                hub,
                &children,
                dim,
                direction,
                scale,
                viewport_rect,
            )) {
                self.set_child_dimension(*child, child_dim);
            }
        }
    }

    /// Adjust the workspace's viewport offset so the focused node is fully
    /// visible.
    ///
    /// Captures `initial` before clamping so the trailing
    /// `do_layout_top_down` only re-runs when the offset actually moved.
    /// The re-run lets max-constrained children recenter in the new visible
    /// section.
    pub(super) fn scroll_into_view(&mut self, hub: &mut HubAccess, workspace_id: WorkspaceId) {
        let Some(initial) = self
            .workspaces
            .get(&workspace_id)
            .map(|s| s.viewport_offset)
        else {
            return;
        };

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
            self.do_layout_top_down(hub, workspace_id);
        }
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

    fn layout_children(
        &self,
        hub: &HubAccess,
        children: &[Child],
        dim: Dimension,
        direction: Option<Direction>,
        scale: f32,
        viewport_rect: Dimension,
    ) -> Vec<Dimension> {
        match direction {
            Some(dir) => self.layout_split_axis_children(hub, children, dim, dir, viewport_rect),
            None => self.layout_tabbed_children(hub, children, dim, scale, viewport_rect),
        }
    }

    fn layout_split_axis_children(
        &self,
        hub: &HubAccess,
        children: &[Child],
        dim: Dimension,
        direction: Direction,
        viewport_rect: Dimension,
    ) -> Vec<Dimension> {
        let constraints: Vec<Constraints> = children
            .iter()
            .map(|&c| self.get_effective_constraints(hub, c))
            .collect();
        let axis = Axis::from_direction(direction);

        let cross_extent = axis.cross_extent(dim).max(
            constraints
                .iter()
                .map(|c| axis.cross_min(c))
                .fold(Length::ZERO, Length::max),
        );
        let along_pairs: Vec<_> = constraints.iter().map(|c| axis.along_min_max(c)).collect();
        let along_sizes = distribute_space(&along_pairs, axis.along_extent(dim));

        let visible = clip(dim, viewport_rect).unwrap_or(dim);
        let group_total: Length = along_sizes.iter().copied().sum();
        let (_, group_off) = apply_max_constraint(
            group_total,
            axis.along_extent(dim),
            axis.along_extent(visible),
            axis.along_origin(visible) - axis.along_origin(dim),
        );

        let mut along_cursor = axis.along_origin(dim) + group_off;
        let mut result = Vec::with_capacity(children.len());
        for (i, &along_size) in along_sizes.iter().enumerate() {
            let (cross_size, cross_off) = apply_max_constraint(
                axis.cross_max(&constraints[i]),
                cross_extent,
                axis.cross_extent(visible),
                axis.cross_origin(visible) - axis.cross_origin(dim),
            );
            result.push(axis.compose(
                along_cursor,
                along_size,
                axis.cross_origin(dim) + cross_off,
                cross_size,
            ));
            along_cursor += along_size;
        }
        result
    }

    fn layout_tabbed_children(
        &self,
        hub: &HubAccess,
        children: &[Child],
        dim: Dimension,
        scale: f32,
        viewport_rect: Dimension,
    ) -> Vec<Dimension> {
        let constraints: Vec<Constraints> = children
            .iter()
            .map(|&c| self.get_effective_constraints(hub, c))
            .collect();
        let tab_bar = self.tab_bar_length(scale);
        let content = Dimension::new(dim.x, dim.y + tab_bar, dim.width, dim.height - tab_bar);

        let outer_visible = clip(dim, viewport_rect).unwrap_or(dim);
        let visible_content_y = outer_visible.y.max(content.y);
        let visible_content_height =
            (outer_visible.y + outer_visible.height - visible_content_y).max(Length::ZERO);
        let visible_content = Dimension::new(
            outer_visible.x,
            visible_content_y,
            outer_visible.width,
            visible_content_height,
        );

        constraints
            .iter()
            .map(|c| place_in_visible(content, (c.max_width, c.max_height), visible_content))
            .collect()
    }

    /// Clamp the workspace's viewport offset against the root's dimension.
    /// With no root, resets to `(ZERO, ZERO)` so a later attach starts from a
    /// known origin instead of inheriting a stale offset from a previous tree.
    fn clamp_viewport_offset(&mut self, hub: &mut HubAccess, workspace_id: WorkspaceId) {
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

    /// Place the root child within the screen. Base dimension is
    /// `screen.max(min)` on each axis: the root grows past the screen when a
    /// descendant's minimum exceeds the screen, and the viewport scrolls to it
    /// instead of clipping. Then applies the root's max constraint with the
    /// current viewport for centering.
    fn set_root_dimension(
        &mut self,
        hub: &mut HubAccess,
        root: Child,
        screen: Dimension,
        viewport_rect: Dimension,
    ) {
        let c = self.get_effective_constraints(hub, root);
        let base_dim: Dimension = Dimension::new(
            Length::ZERO,
            Length::ZERO,
            screen.width.max(c.min_width),
            screen.height.max(c.min_height),
        );
        let visible = clip(base_dim, viewport_rect).unwrap_or(base_dim);
        let dim = place_in_visible(base_dim, (c.max_width, c.max_height), visible);

        self.set_child_dimension(root, dim);
    }

    /// Bottom-up minimum size for one container. For split containers, sums
    /// along the split axis and maxes across. For tabbed containers, maxes both
    /// axes (tabs share area) and adds `tab_bar_length` to the height for the
    /// tab strip above the active child.
    fn update_container_min_size(
        &mut self,
        hub: &HubAccess,
        container_id: ContainerId,
        scale: f32,
    ) {
        let container = self.containers.get(container_id);
        let children = container.children.clone();
        let direction = container.direction();

        let child_constraints: Vec<Constraints> = children
            .iter()
            .map(|&c| self.get_effective_constraints(hub, c))
            .collect();

        let (min_w, min_h) = match direction {
            Some(Direction::Horizontal) => {
                let sum_w: Length = child_constraints.iter().map(|c| c.min_width).sum();
                let max_h = child_constraints
                    .iter()
                    .map(|c| c.min_height)
                    .fold(Length::ZERO, Length::max);
                (sum_w, max_h)
            }
            Some(Direction::Vertical) => {
                let max_w = child_constraints
                    .iter()
                    .map(|c| c.min_width)
                    .fold(Length::ZERO, Length::max);
                let sum_h: Length = child_constraints.iter().map(|c| c.min_height).sum();
                (max_w, sum_h)
            }
            None => {
                let max_w = child_constraints
                    .iter()
                    .map(|c| c.min_width)
                    .fold(Length::ZERO, Length::max);
                let max_h = child_constraints
                    .iter()
                    .map(|c| c.min_height)
                    .fold(Length::ZERO, Length::max);
                (max_w, max_h + self.tab_bar_length(scale))
            }
        };

        let container = self.containers.get_mut(container_id);
        container.min_width = min_w;
        container.min_height = min_h;

        if container.dimension.width < min_w {
            container.dimension.width = min_w;
        }
        if container.dimension.height < min_h {
            container.dimension.height = min_h;
        }
    }

    /// Write `dim` to `child` and, when `automatic_tiling` is on, set the
    /// child's `spawn_mode` to match the new aspect ratio (Horizontal if wider
    /// than tall, otherwise Vertical). The `!is_tab()` guard keeps tabbed
    /// children from being demoted to a split mode by a layout pass.
    fn set_child_dimension(&mut self, child: Child, dim: Dimension) {
        let spawn_mode = if dim.width >= dim.height {
            SpawnMode::horizontal()
        } else {
            SpawnMode::vertical()
        };
        let automatic_tiling = self.automatic_tiling;
        match child {
            Child::Window(wid) => {
                let td = self.tiling_windows.get_mut(&wid).unwrap();
                td.dimension = dim;
                if automatic_tiling && !td.spawn_mode.is_tab() {
                    td.spawn_mode = SpawnMode::without_history(spawn_mode);
                }
            }
            Child::Container(cid) => {
                let c = self.containers.get_mut(cid);
                c.dimension = dim;
                if automatic_tiling && !c.spawn_mode().is_tab() {
                    c.set_spawn_mode_reset(spawn_mode);
                }
            }
        }
    }

    /// Resolve the effective constraints for a child.
    ///
    /// Window: per-instance max (`Window::max_size`) wins when non-zero,
    /// otherwise the global `max_*` config applies. The resolved max also caps
    /// the effective min so a window's min cannot exceed its max.
    ///
    /// Container: returns its tracked `min_size` and `(ZERO, ZERO)` for max.
    /// Containers have no max constraint. `ZERO` is the sentinel that
    /// downstream layout reads as "unconstrained".
    fn get_effective_constraints(&self, hub: &HubAccess, child: Child) -> Constraints {
        let ws_id = self.child_workspace(hub, child);
        let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        let screen = monitor.dimension;
        let scale = monitor.scale;
        let global_min_w = self
            .size_constraints
            .minimum_width
            .resolve(screen.width, scale);
        let global_min_h = self
            .size_constraints
            .minimum_height
            .resolve(screen.height, scale);

        match child {
            Child::Window(id) => {
                let window = hub.windows.get(id);
                // Window.min_*/max_* are raw f32 (pre-scaled platform hints); wrap at this Dimension seam.
                let (win_min_w_raw, win_min_h_raw) = window.min_size();
                let (win_max_w_raw, win_max_h_raw) = window.max_size();
                let win_min_w = Length::new(win_min_w_raw);
                let win_min_h = Length::new(win_min_h_raw);
                let win_max_w = Length::new(win_max_w_raw);
                let win_max_h = Length::new(win_max_h_raw);

                let global_max_w = self
                    .size_constraints
                    .maximum_width
                    .resolve(screen.width, scale);
                let global_max_h = self
                    .size_constraints
                    .maximum_height
                    .resolve(screen.height, scale);

                let max_w = if win_max_w > Length::ZERO {
                    win_max_w
                } else {
                    global_max_w
                };
                let max_h = if win_max_h > Length::ZERO {
                    win_max_h
                } else {
                    global_max_h
                };

                // Window-specific max caps the effective min
                let min_w = if max_w > Length::ZERO {
                    win_min_w.max(global_min_w).min(max_w)
                } else {
                    win_min_w.max(global_min_w)
                };
                let min_h = if max_h > Length::ZERO {
                    win_min_h.max(global_min_h).min(max_h)
                } else {
                    win_min_h.max(global_min_h)
                };

                Constraints {
                    min_width: min_w,
                    min_height: min_h,
                    max_width: max_w,
                    max_height: max_h,
                }
            }
            Child::Container(id) => {
                let (min_w, min_h) = self.containers.get(id).min_size();
                Constraints {
                    min_width: min_w,
                    min_height: min_h,
                    max_width: Length::ZERO,
                    max_height: Length::ZERO,
                }
            }
        }
    }

    pub(super) fn tab_bar_length(&self, scale: f32) -> Length {
        self.tab_bar_height.to_unit(scale)
    }
}

/// Returns (size, offset) for a max-constrained child.
///
/// `size` is the constrained extent on this axis. `offset` is the placement
/// offset relative to the container origin so the child is centered inside the
/// visible section of the container, clamped to stay inside the container.
///
/// When `visible_extent == container_extent` and `visible_origin == 0` (the
/// container is fully on screen), this reduces to simple centering within the
/// container.
fn apply_max_constraint(
    max: Length,
    container_extent: Length,
    visible_extent: Length,
    visible_origin: Length,
) -> (Length, Length) {
    let size = if max > Length::ZERO && max < container_extent {
        max
    } else {
        container_extent
    };
    let half_gap = (visible_extent - size).max(Length::ZERO) / 2.0;
    let raw_offset = visible_origin + half_gap;
    let max_offset = container_extent - size;
    (size, raw_offset.clamp(Length::ZERO, max_offset))
}

fn place_in_visible(container: Dimension, max: (Length, Length), visible: Dimension) -> Dimension {
    let (max_w, max_h) = max;
    let (w, x_off) = apply_max_constraint(
        max_w,
        container.width,
        visible.width,
        visible.x - container.x,
    );
    let (h, y_off) = apply_max_constraint(
        max_h,
        container.height,
        visible.height,
        visible.y - container.y,
    );
    Dimension::new(container.x + x_off, container.y + y_off, w, h)
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

#[derive(Copy, Clone)]
enum Axis {
    X,
    Y,
}

impl Axis {
    fn from_direction(direction: Direction) -> Self {
        match direction {
            Direction::Horizontal => Axis::X,
            Direction::Vertical => Axis::Y,
        }
    }

    fn along_extent(self, dim: Dimension) -> Length {
        match self {
            Axis::X => dim.width,
            Axis::Y => dim.height,
        }
    }

    fn along_origin(self, dim: Dimension) -> Length {
        match self {
            Axis::X => dim.x,
            Axis::Y => dim.y,
        }
    }

    fn cross_extent(self, dim: Dimension) -> Length {
        match self {
            Axis::X => dim.height,
            Axis::Y => dim.width,
        }
    }

    fn cross_origin(self, dim: Dimension) -> Length {
        match self {
            Axis::X => dim.y,
            Axis::Y => dim.x,
        }
    }

    fn along_min_max(self, c: &Constraints) -> (Length, Length) {
        match self {
            Axis::X => (c.min_width, c.max_width),
            Axis::Y => (c.min_height, c.max_height),
        }
    }

    fn cross_min(self, c: &Constraints) -> Length {
        match self {
            Axis::X => c.min_height,
            Axis::Y => c.min_width,
        }
    }

    fn cross_max(self, c: &Constraints) -> Length {
        match self {
            Axis::X => c.max_height,
            Axis::Y => c.max_width,
        }
    }

    fn compose(
        self,
        along_origin: Length,
        along_size: Length,
        cross_origin: Length,
        cross_size: Length,
    ) -> Dimension {
        match self {
            Axis::X => Dimension::new(along_origin, cross_origin, along_size, cross_size),
            Axis::Y => Dimension::new(cross_origin, along_origin, cross_size, along_size),
        }
    }
}
