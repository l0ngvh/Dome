use crate::core::hub::HubAccess;
use crate::core::node::Constraints;
use crate::core::node::{ContainerId, Dimension, Direction, Length, WorkspaceId};
use crate::core::partition_tree::{Child, SpawnMode};
use crate::core::strategy::{TilingPlacements, clip, distribute_space, translate};
use crate::core::{ContainerPlacement, SpawnIndicator, TilingWindowPlacement};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    /// Two-pass layout: bottom-up to compute minimum sizes (a container's min
    /// is the sum of its children's mins), then top-down to distribute space.
    /// A single pass can't do both because the total minimum must be known
    /// before distributing remaining space.
    pub(super) fn compute_placement_against_constraint(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) {
        let ws_state = self.workspaces.get(&ws_id).unwrap();
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

        self.adjust_placement(hub, ws_id);
        self.scroll_into_view(hub, ws_id);
    }

    /// Top-down placement pass: places the root and distributes space to
    /// each container's children using the current viewport_offset.
    pub(super) fn adjust_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let ws_state = self.workspaces.get(&ws_id).unwrap();
        let Some(root) = ws_state.root else { return };
        let viewport_offset = ws_state.viewport_offset;
        let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        let screen = monitor.dimension;
        let scale = monitor.scale;
        let (offset_x, offset_y) = viewport_offset;
        let viewport_rect = Dimension::new(offset_x, offset_y, screen.width, screen.height);

        self.set_root_dimension(hub, root, screen);

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

    pub(super) fn collect_placements(
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
        let screen = hub.monitors.get(ws.monitor).dimension;
        // Only highlight tiling focus when this is the current workspace AND
        // the workspace's effective focus is on tiling (not float). Fullscreen
        // workspaces never reach here (hub returns early with MonitorLayout::Fullscreen).
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
                    let frame = translate(self.child_dimension(child), offset_x, offset_y, screen);
                    if let Some(visible_frame) = clip(frame, screen) {
                        let is_highlighted = focused == Some(Child::Window(id));
                        windows.push(TilingWindowPlacement {
                            id,
                            frame,
                            visible_frame,
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
                    let frame = translate(self.child_dimension(child), offset_x, offset_y, screen);
                    let Some(visible_frame) = clip(frame, screen) else {
                        continue;
                    };
                    let is_highlighted = focused == Some(Child::Container(id));
                    containers.push(ContainerPlacement {
                        id,
                        frame,
                        visible_frame,
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

    /// Layout the children in the container.
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

    /// Place the root child within the screen. Base dimension is
    /// `screen.max(min)` on each axis: the root grows past the screen when a
    /// descendant's minimum exceeds the screen, and the viewport scrolls to it
    /// instead of clipping. Then applies the root's max constraint with the
    /// current viewport for centering.
    fn set_root_dimension(&mut self, hub: &HubAccess, root: Child, screen: Dimension) {
        let c = self.get_effective_constraints(hub, root);
        let base_dim: Dimension = Dimension::new(
            Length::ZERO,
            Length::ZERO,
            screen.width.max(c.min_width),
            screen.height.max(c.min_height),
        );
        let dim = place_in_visible(base_dim, (c.max_width, c.max_height), base_dim);

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
