use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Dimension, Direction, Length, WorkspaceId};
use crate::core::partition_tree::{Child, SpawnMode};
use crate::core::strategy::clip;

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
            let mut stack = vec![root_id];
            let mut order = vec![];
            for _ in crate::core::bounded_loop() {
                let Some(cid) = stack.pop() else { break };
                order.push(cid);
                for child in &self.containers.get(cid).children {
                    if let Child::Container(child_cid) = child {
                        stack.push(*child_cid);
                    }
                }
            }

            // Update minimum sizes bottom-up, as parent's minimum size depends on children's
            for &cid in order.iter().rev() {
                self.update_container_min_size(hub, cid, scale);
            }
        }

        self.distribute_dimensions(hub, ws_id);
        self.scroll_into_view(hub, ws_id);
    }

    /// Top-down distribution: places the root and distributes space to each
    /// container's children using the current viewport_offset. Called from
    /// do_layout_workspace after the bottom-up min-size pass, and again from
    /// scroll_into_view when the viewport offset changes so that
    /// max-constrained windows re-center in the new visible section.
    pub(super) fn distribute_dimensions(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
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

        let mut stack = vec![root_id];
        let mut order = vec![];
        for _ in crate::core::bounded_loop() {
            let Some(cid) = stack.pop() else { break };
            order.push(cid);
            for child in &self.containers.get(cid).children {
                if let Child::Container(child_cid) = child {
                    stack.push(*child_cid);
                }
            }
        }

        for cid in order {
            let container = self.containers.get(cid);
            let dim = container.dimension;
            let children = container.children.clone();
            let direction = container.direction();
            for (child, child_dim) in children.iter().zip(self.layout_split_children(
                hub,
                &children,
                dim,
                direction,
                scale,
                viewport_rect,
            )) {
                self.set_split_child_dimension(hub, *child, child_dim);
            }
        }
    }

    /// Lays out children within the given dimension.
    ///
    /// For split containers, space is distributed along the split axis while
    /// respecting min/max constraints. If total size is less than available
    /// (due to max constraints), the group is centered. Windows hitting max
    /// size on the perpendicular axis are also centered. For tabbed containers,
    /// each child is assigned the full area below the tab bar, centered if
    /// constrained.
    fn layout_split_children(
        &self,
        hub: &HubAccess,
        children: &[Child],
        dim: Dimension,
        direction: Option<Direction>,
        scale: f32,
        viewport_rect: Dimension,
    ) -> Vec<Dimension> {
        let constraints = self.collect_constraints(hub, children);

        match direction {
            Some(Direction::Horizontal) => {
                let height = dim.height.max(
                    constraints
                        .iter()
                        .map(|c| c.2)
                        .fold(Length::ZERO, Length::max),
                );
                let width_constraints: Vec<_> = constraints.iter().map(|c| (c.0, c.1)).collect();
                let widths = distribute_space(&width_constraints, dim.width);

                let visible = clip(dim, viewport_rect).unwrap_or(dim);
                let group_total: Length = widths.iter().copied().sum();
                let half_gap = (visible.width - group_total).max(Length::ZERO) / 2.0;
                let raw_offset = (visible.x - dim.x) + half_gap;
                let max_offset = dim.width - group_total;
                let mut x = dim.x + raw_offset.clamp(Length::ZERO, max_offset);

                let mut result = Vec::with_capacity(children.len());
                for i in 0..children.len() {
                    let (_, _, _, max_h) = constraints[i];
                    let (h, y_off) =
                        apply_max_constraint(max_h, height, visible.height, visible.y - dim.y);
                    result.push(Dimension::new(x, dim.y + y_off, widths[i], h));
                    x += widths[i];
                }
                result
            }
            Some(Direction::Vertical) => {
                let width = dim.width.max(
                    constraints
                        .iter()
                        .map(|c| c.0)
                        .fold(Length::ZERO, Length::max),
                );
                let height_constraints: Vec<_> = constraints.iter().map(|c| (c.2, c.3)).collect();
                let heights = distribute_space(&height_constraints, dim.height);

                let visible = clip(dim, viewport_rect).unwrap_or(dim);
                let group_total: Length = heights.iter().copied().sum();
                let half_gap = (visible.height - group_total).max(Length::ZERO) / 2.0;
                let raw_offset = (visible.y - dim.y) + half_gap;
                let max_offset = dim.height - group_total;
                let mut y = dim.y + raw_offset.clamp(Length::ZERO, max_offset);

                let mut result = Vec::with_capacity(children.len());
                for i in 0..children.len() {
                    let (_, max_w, _, _) = constraints[i];
                    let (w, x_off) =
                        apply_max_constraint(max_w, width, visible.width, visible.x - dim.x);
                    result.push(Dimension::new(dim.x + x_off, y, w, heights[i]));
                    y += heights[i];
                }
                result
            }
            None => {
                let tab_bar = hub
                    .config
                    .layout
                    .partition_tree
                    .tab_bar_height
                    .to_unit(scale)
                    .value();
                let tab_bar_len = Length::new(tab_bar);
                let content_y = dim.y + tab_bar_len;
                let content_height = dim.height - tab_bar_len;

                let visible = clip(dim, viewport_rect).unwrap_or(dim);
                let visible_content_y = visible.y.max(content_y);
                let visible_content_height =
                    (visible.y + visible.height - visible_content_y).max(Length::ZERO);
                let visible_origin_y = visible_content_y - content_y;
                let visible_origin_x = visible.x - dim.x;

                let mut result = Vec::with_capacity(children.len());
                for (_, max_w, _, max_h) in constraints {
                    let (w, x_off) =
                        apply_max_constraint(max_w, dim.width, visible.width, visible_origin_x);
                    let (h, y_off) = apply_max_constraint(
                        max_h,
                        content_height,
                        visible_content_height,
                        visible_origin_y,
                    );
                    result.push(Dimension::new(dim.x + x_off, content_y + y_off, w, h));
                }
                result
            }
        }
    }

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
            let focused_dim = match focused {
                Child::Window(id) => self.tiling_data(id).dimension,
                Child::Container(id) => self.containers.get(id).dimension,
            };

            if focused_dim.x - offset_x + focused_dim.width > screen.width {
                offset_x = focused_dim.x + focused_dim.width - screen.width;
            }
            if focused_dim.x - offset_x < Length::ZERO {
                offset_x = focused_dim.x;
            }

            if focused_dim.y - offset_y + focused_dim.height > screen.height {
                offset_y = focused_dim.y + focused_dim.height - screen.height;
            }
            if focused_dim.y - offset_y < Length::ZERO {
                offset_y = focused_dim.y;
            }

            self.ws_state_mut(workspace_id).viewport_offset = (offset_x, offset_y);
        }

        if (offset_x, offset_y) != initial {
            self.distribute_dimensions(hub, workspace_id);
        }
    }

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
            Some(Child::Window(id)) => self.tiling_data(id).dimension,
            Some(Child::Container(id)) => self.containers.get(id).dimension,
            None => {
                self.ws_state_mut(workspace_id).viewport_offset = (Length::ZERO, Length::ZERO);
                return;
            }
        };

        offset_x = offset_x.clamp(
            Length::ZERO,
            (root_dim.width - screen.width).max(Length::ZERO),
        );
        offset_y = offset_y.clamp(
            Length::ZERO,
            (root_dim.height - screen.height).max(Length::ZERO),
        );
        self.ws_state_mut(workspace_id).viewport_offset = (offset_x, offset_y);
    }

    fn set_root_dimension(
        &mut self,
        hub: &mut HubAccess,
        root: Child,
        screen: Dimension,
        viewport_rect: Dimension,
    ) {
        let (min_w, min_h, max_w, max_h) = self.get_effective_constraints(hub, root);
        let base_dim: Dimension = Dimension::new(
            Length::ZERO,
            Length::ZERO,
            screen.width.max(min_w),
            screen.height.max(min_h),
        );
        let visible = clip(base_dim, viewport_rect).unwrap_or(base_dim);

        let (w, x_off) =
            apply_max_constraint(max_w, base_dim.width, visible.width, visible.x - base_dim.x);
        let (h, y_off) = apply_max_constraint(
            max_h,
            base_dim.height,
            visible.height,
            visible.y - base_dim.y,
        );
        let dim = Dimension::new(base_dim.x + x_off, base_dim.y + y_off, w, h);

        self.set_split_child_dimension(hub, root, dim);
    }

    fn update_container_min_size(
        &mut self,
        hub: &HubAccess,
        container_id: ContainerId,
        scale: f32,
    ) {
        let container = self.containers.get(container_id);
        let children = container.children.clone();
        let direction = container.direction();

        let child_mins: Vec<(Length, Length)> = children
            .iter()
            .map(|&c| {
                let (min_w, min_h, _, _) = self.get_effective_constraints(hub, c);
                (min_w, min_h)
            })
            .collect();

        let (min_w, min_h) = match direction {
            Some(Direction::Horizontal) => {
                let sum_w: Length = child_mins.iter().map(|(w, _)| *w).sum();
                let max_h = child_mins
                    .iter()
                    .map(|(_, h)| *h)
                    .fold(Length::ZERO, Length::max);
                (sum_w, max_h)
            }
            Some(Direction::Vertical) => {
                let max_w = child_mins
                    .iter()
                    .map(|(w, _)| *w)
                    .fold(Length::ZERO, Length::max);
                let sum_h: Length = child_mins.iter().map(|(_, h)| *h).sum();
                (max_w, sum_h)
            }
            None => {
                let max_w = child_mins
                    .iter()
                    .map(|(w, _)| *w)
                    .fold(Length::ZERO, Length::max);
                let max_h = child_mins
                    .iter()
                    .map(|(_, h)| *h)
                    .fold(Length::ZERO, Length::max);
                (
                    max_w,
                    max_h
                        + Length::new(
                            hub.config
                                .layout
                                .partition_tree
                                .tab_bar_height
                                .to_unit(scale)
                                .value(),
                        ),
                )
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

    fn collect_constraints(
        &self,
        hub: &HubAccess,
        children: &[Child],
    ) -> Vec<(Length, Length, Length, Length)> {
        children
            .iter()
            .map(|&c| {
                let (min_w, min_h, max_w, max_h) = self.get_effective_constraints(hub, c);
                (min_w, max_w, min_h, max_h)
            })
            .collect()
    }

    fn set_split_child_dimension(&mut self, hub: &mut HubAccess, child: Child, dim: Dimension) {
        let spawn_mode = if dim.width >= dim.height {
            SpawnMode::horizontal()
        } else {
            SpawnMode::vertical()
        };
        match child {
            Child::Window(wid) => {
                let td = self.tiling_data_mut(wid);
                td.dimension = dim;
                if hub.config.layout.partition_tree.automatic_tiling && !td.spawn_mode.is_tab() {
                    td.spawn_mode = SpawnMode::clean(spawn_mode);
                }
            }
            Child::Container(cid) => {
                let c = self.containers.get_mut(cid);
                c.dimension = dim;
                if hub.config.layout.partition_tree.automatic_tiling && !c.spawn_mode().is_tab() {
                    c.set_spawn_mode(spawn_mode);
                }
            }
        }
    }

    /// Returns (min_w, min_h, max_w, max_h). Window-specific max takes precedence over global min.
    fn get_effective_constraints(
        &self,
        hub: &HubAccess,
        child: Child,
    ) -> (Length, Length, Length, Length) {
        let ws_id = match child {
            Child::Window(id) => hub
                .windows
                .get(id)
                .workspace()
                .expect("tiling window has a workspace"),
            Child::Container(id) => self.containers.get(id).workspace,
        };
        let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        let screen = monitor.dimension;
        let scale = monitor.scale;
        let global_min_w = hub.config.min_width.resolve(screen.width, scale);
        let global_min_h = hub.config.min_height.resolve(screen.height, scale);

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

                let global_max_w = hub.config.max_width.resolve(screen.width, scale);
                let global_max_h = hub.config.max_height.resolve(screen.height, scale);

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

                (min_w, min_h, max_w, max_h)
            }
            Child::Container(id) => {
                let (min_w, min_h) = self.containers.get(id).min_size();
                (min_w, min_h, Length::ZERO, Length::ZERO)
            }
        }
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

/// Find a distribution of the available space for all split children of a container, so that all
/// windows that haven't approached its max/min constraints must share the same size.
fn distribute_space(constraints: &[(Length, Length)], container_size: Length) -> Vec<Length> {
    let constraints: Vec<(Length, Length)> = constraints
        .iter()
        .map(|&(min, max)| {
            let max = if max == Length::ZERO {
                Length::new(f32::INFINITY)
            } else {
                max
            };
            (min, max)
        })
        .collect();

    let sum_mins: Length = constraints.iter().map(|(min, _)| *min).sum();
    if sum_mins >= container_size {
        return constraints.iter().map(|(min, _)| *min).collect();
    }

    let all_finite = constraints.iter().all(|(_, max)| max.value().is_finite());
    if all_finite {
        let sum_maxes: Length = constraints.iter().map(|(_, max)| *max).sum();
        if sum_maxes <= container_size {
            return constraints.iter().map(|(_, max)| *max).collect();
        }
    }

    let mut low = 0.0_f32;
    let mut high = container_size.value();
    const EPSILON: f32 = 0.001;

    while high - low > EPSILON {
        let mid = (low + high) / 2.0;
        let total: f32 = constraints
            .iter()
            .map(|(min, max)| mid.clamp(min.value(), max.value()))
            .sum();
        if total > container_size.value() {
            high = mid;
        } else {
            low = mid;
        }
    }

    constraints
        .iter()
        .map(|(min, max)| Length::new(low.clamp(min.value(), max.value())))
        .collect()
}

// apply_max_constraint is module-private and exposing it just for tests would
// leak test-only public API. Using #[cfg(test)] keeps the helper internal.
#[cfg(test)]
mod tests {
    use super::apply_max_constraint;
    use crate::core::node::Length;

    #[test]
    fn apply_max_constraint_unchanged_when_visible_equals_container() {
        let container = Length::new(100.0);

        // max == 0: unconstrained, returns full container with no offset
        let (size, offset) = apply_max_constraint(Length::ZERO, container, container, Length::ZERO);
        assert_eq!(size, container);
        assert_eq!(offset, Length::ZERO);

        // max < container: returns max size, centered in container
        let max = Length::new(60.0);
        let (size, offset) = apply_max_constraint(max, container, container, Length::ZERO);
        assert_eq!(size, max);
        assert_eq!(offset, Length::new(20.0));

        // max == container: the strict `<` comparison means no constraint applied
        let (size, offset) = apply_max_constraint(container, container, container, Length::ZERO);
        assert_eq!(size, container);
        assert_eq!(offset, Length::ZERO);

        // max > container: no constraint applied
        let max = Length::new(200.0);
        let (size, offset) = apply_max_constraint(max, container, container, Length::ZERO);
        assert_eq!(size, container);
        assert_eq!(offset, Length::ZERO);
    }
}
