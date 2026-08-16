use crate::core::hub::HubAccess;
use crate::core::node::Constraints;
use crate::core::node::{
    ContainerId, Dimension, Direction, Length, PixelRect, Pixels, WorkspaceId,
};
use crate::core::partition_tree::{Child, SpawnMode};
use crate::core::strategy::{
    TilingPlacements, clip, distribute_space, translate, window_constraints,
};
use crate::core::{ContainerPlacement, SpawnIndicator, TilingWindowPlacement};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    /// Two-pass layout: bottom-up to compute minimum sizes, then top-down to
    /// distribute space. A single pass can't do both because the total minimum
    /// must be known before distributing remaining space.
    pub(super) fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let ws_state = self.workspaces.get(&ws_id).unwrap();
        let Some(root) = ws_state.root else { return };

        if let Child::Container(root_id) = root {
            let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
            let scale = monitor.scale;

            let order: Vec<_> = self.containers_preorder(root_id).collect();

            // Reversed pre-order visits children before parents.
            for &cid in order.iter().rev() {
                self.update_container_min_size(hub, cid, scale);
            }
        }

        self.adjust_placement(hub, ws_id);
        self.scroll_into_view(hub, ws_id);
    }

    /// Top-down half of the two-pass layout.
    pub(super) fn adjust_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let ws_state = self.workspaces.get(&ws_id).unwrap();
        let Some(root) = ws_state.root else { return };
        let viewport_offset = ws_state.viewport_offset;
        let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        let work_area = monitor.work_area;
        let screen_width = Length::from_pixels(work_area.width());
        let screen_height = Length::from_pixels(work_area.height());
        let scale = monitor.scale;
        let (offset_x, offset_y) = viewport_offset;
        let viewport_rect = Dimension::new(offset_x, offset_y, screen_width, screen_height);

        self.set_root_dimension(hub, root, screen_width, screen_height);

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

    pub(super) fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        focused: bool,
    ) -> TilingPlacements {
        let Some(ws_state) = self.workspaces.get(&ws_id) else {
            return TilingPlacements {
                windows: Vec::new(),
                containers: Vec::new(),
            };
        };
        let ws = hub.workspaces.get(ws_id);
        let (offset_x, offset_y) = ws_state.viewport_offset;
        let monitor = hub.monitors.get(ws.monitor);
        let screen = monitor.work_area;
        let scale = monitor.scale;
        let border = hub.border(ws.monitor);
        // Fullscreen workspaces never reach here (hub returns early with
        // MonitorLayout::Fullscreen).
        let focused = if focused && !ws.is_float_focused {
            ws_state.focused_tiling
        } else {
            None
        };
        let mut windows = Vec::new();
        let mut containers = Vec::new();

        // Hand-rolled DFS kept because tabbed containers push only the active
        // tab, not all children. This visible-only traversal differs from the
        // full pre-order that children_dfs provides.
        let mut stack: Vec<Child> = ws_state.root.into_iter().collect();
        for _ in crate::core::bounded_loop() {
            let Some(child) = stack.pop() else { break };
            match child {
                Child::Window(id) => {
                    let border_box = translate(
                        self.child_dimension(child),
                        offset_x,
                        offset_y,
                        screen.x(),
                        screen.y(),
                    );
                    if let Some(visible_border_box) = border_box.clip(screen) {
                        let is_highlighted = focused == Some(Child::Window(id));
                        let content_box = border_box.inset_by(border);
                        windows.push(TilingWindowPlacement {
                            id,
                            border_box,
                            visible_border_box,
                            content_box,
                            visible_content_box: content_box
                                .clip(screen)
                                .unwrap_or(PixelRect::ZERO),
                            is_highlighted,
                            spawn_indicator: if is_highlighted {
                                Some(SpawnIndicator::from(self.child_spawn_mode(child)))
                            } else {
                                None
                            },
                        });
                    }
                }
                Child::Container(id) => {
                    let container = self.containers.get(id);
                    let dim = self.child_dimension(child);
                    let border_box = translate(dim, offset_x, offset_y, screen.x(), screen.y());
                    let Some(visible_border_box) = border_box.clip(screen) else {
                        continue;
                    };
                    // Rounding the band height on its own would let round(y) + round(h) drift
                    // a unit from the round(y + h) the content box uses. The content top comes
                    // from the container's own dimension, not the active tab's, since a
                    // max-constrained tab is centred within the content.
                    let band_height = if container.is_tabbed() {
                        let content_top =
                            Pixels::round(dim.y + self.tab_bar_length(scale) - offset_y)
                                + screen.y();
                        content_top - border_box.y()
                    } else {
                        Pixels::ZERO
                    };
                    let is_highlighted = focused == Some(Child::Container(id));
                    containers.push(ContainerPlacement {
                        id,
                        border_box,
                        visible_border_box,
                        tab_bar_band: PixelRect::from_pixels(
                            border_box.x(),
                            border_box.y(),
                            border_box.width(),
                            band_height,
                        ),
                        is_highlighted,
                        spawn_indicator: if is_highlighted {
                            Some(SpawnIndicator::from(self.child_spawn_mode(child)))
                        } else {
                            None
                        },
                        is_tabbed: container.is_tabbed(),
                        active_tab_index: container.active_tab_index(),
                        titles: container
                            .children()
                            .iter()
                            .map(|c| match c {
                                Child::Window(wid) => hub.windows.get(*wid).title().to_owned(),
                                Child::Container(_) => "Container".to_string(),
                            })
                            .collect(),
                    });
                    if let Some(active) = container.active_tab() {
                        stack.push(active);
                    } else {
                        for &c in container.children() {
                            stack.push(c);
                        }
                    }
                }
            }
        }

        TilingPlacements {
            windows,
            containers,
        }
    }

    /// Max constrained children are centered inside of the visible portion of the container, or
    /// just centered inside the container if it's completely offscreen
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

    /// The root grows past the screen when a descendant's minimum exceeds it, so
    /// the viewport scrolls instead of clipping.
    fn set_root_dimension(
        &mut self,
        hub: &HubAccess,
        root: Child,
        screen_width: Length,
        screen_height: Length,
    ) {
        let c = self.get_effective_constraints(hub, root);
        let base_dim: Dimension = Dimension::new(
            Length::ZERO,
            Length::ZERO,
            screen_width.max(c.min_width),
            screen_height.max(c.min_height),
        );
        let dim = place_in_visible(base_dim, (c.max_width, c.max_height), base_dim);

        self.set_child_dimension(root, dim);
    }

    /// A tabbed container maxes both axes because its tabs share one area.
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

    /// The `!is_tab()` guard keeps tabbed children from being demoted to a split
    /// mode by a layout pass.
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

    fn get_effective_constraints(&self, hub: &HubAccess, child: Child) -> Constraints {
        match child {
            Child::Window(id) => window_constraints(hub, &self.size_constraints, id),
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
        Length::from_pixels(self.tab_bar_height).to_unit(scale)
    }
}

/// Returns (size, offset) for a max-constrained child.
///
/// `offset` is the placement offset relative to the container origin so the
/// child is centered inside the visible section of the container, clamped to
/// stay inside the container.
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
