use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Dimension, Direction, WorkspaceId};
use crate::core::partition_tree::{Child, SpawnMode};

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
        let screen = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension;

        let Child::Container(root_id) = root else {
            self.set_root_dimension(hub, root, screen);
            return;
        };

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
            self.update_container_min_size(hub, cid);
        }

        self.set_root_dimension(hub, root, screen);
        for cid in order {
            let container = self.containers.get(cid);
            let dim = container.dimension;
            let children = container.children.clone();
            let direction = container.direction();
            for (child, child_dim) in children
                .iter()
                .zip(self.layout_split_children(hub, &children, dim, direction))
            {
                self.set_split_child_dimension(hub, *child, child_dim);
            }
        }

        // Focused window can go out of view due to resizing other windows
        self.scroll_into_view(hub, ws_id);
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
    ) -> Vec<Dimension> {
        let constraints = self.collect_constraints(hub, children);

        match direction {
            Some(Direction::Horizontal) => {
                let height = dim
                    .height
                    .max(constraints.iter().map(|c| c.2).fold(0.0, f32::max));
                let width_constraints: Vec<_> = constraints.iter().map(|c| (c.0, c.1)).collect();
                let widths = distribute_space(&width_constraints, dim.width);
                let mut x = dim.x + (dim.width - widths.iter().sum::<f32>()) / 2.0;

                let mut result = Vec::with_capacity(children.len());
                for i in 0..children.len() {
                    let (_, _, _, max_h) = constraints[i];
                    let (h, y_off) = apply_max_constraint(max_h, height);
                    result.push(Dimension {
                        x,
                        y: dim.y + y_off,
                        width: widths[i],
                        height: h,
                    });
                    x += widths[i];
                }
                result
            }
            Some(Direction::Vertical) => {
                let width = dim
                    .width
                    .max(constraints.iter().map(|c| c.0).fold(0.0, f32::max));
                let height_constraints: Vec<_> = constraints.iter().map(|c| (c.2, c.3)).collect();
                let heights = distribute_space(&height_constraints, dim.height);
                let mut y = dim.y + (dim.height - heights.iter().sum::<f32>()) / 2.0;

                let mut result = Vec::with_capacity(children.len());
                for i in 0..children.len() {
                    let (_, max_w, _, _) = constraints[i];
                    let (w, x_off) = apply_max_constraint(max_w, width);
                    result.push(Dimension {
                        x: dim.x + x_off,
                        y,
                        width: w,
                        height: heights[i],
                    });
                    y += heights[i];
                }
                result
            }
            None => {
                let content_y = dim.y + hub.config.tab_bar_height;
                let content_height = dim.height - hub.config.tab_bar_height;

                let mut result = Vec::with_capacity(children.len());
                for (_, max_w, _, max_h) in constraints {
                    let (w, x_off) = apply_max_constraint(max_w, dim.width);
                    let (h, y_off) = apply_max_constraint(max_h, content_height);
                    result.push(Dimension {
                        x: dim.x + x_off,
                        y: content_y + y_off,
                        width: w,
                        height: h,
                    });
                }
                result
            }
        }
    }

    pub(super) fn scroll_into_view(&mut self, hub: &mut HubAccess, workspace_id: WorkspaceId) {
        self.clamp_viewport_offset(hub, workspace_id);

        let Some(ws_state) = self.workspaces.get(&workspace_id) else {
            return;
        };
        let monitor_id = hub.workspaces.get(workspace_id).monitor;
        let screen = hub.monitors.get(monitor_id).dimension;
        let (mut offset_x, mut offset_y) = ws_state.viewport_offset;

        let focused_dim = match ws_state.focused_tiling {
            Some(Child::Window(id)) => self.tiling_data(id).dimension,
            Some(Child::Container(id)) => self.containers.get(id).dimension,
            None => return,
        };

        if focused_dim.x - offset_x + focused_dim.width > screen.width {
            offset_x = focused_dim.x + focused_dim.width - screen.width;
        }
        if focused_dim.x - offset_x < 0.0 {
            offset_x = focused_dim.x;
        }

        if focused_dim.y - offset_y + focused_dim.height > screen.height {
            offset_y = focused_dim.y + focused_dim.height - screen.height;
        }
        if focused_dim.y - offset_y < 0.0 {
            offset_y = focused_dim.y;
        }

        self.ws_state_mut(workspace_id).viewport_offset = (offset_x, offset_y);
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
                self.ws_state_mut(workspace_id).viewport_offset = (0.0, 0.0);
                return;
            }
        };

        offset_x = offset_x.clamp(0.0, (root_dim.width - screen.width).max(0.0));
        offset_y = offset_y.clamp(0.0, (root_dim.height - screen.height).max(0.0));
        self.ws_state_mut(workspace_id).viewport_offset = (offset_x, offset_y);
    }

    fn set_root_dimension(&mut self, hub: &mut HubAccess, root: Child, screen: Dimension) {
        let (min_w, min_h, max_w, max_h) = self.get_effective_constraints(hub, root);
        let base_dim = Dimension {
            x: 0.0,
            y: 0.0,
            width: screen.width.max(min_w),
            height: screen.height.max(min_h),
        };

        let (w, x_off) = apply_max_constraint(max_w, base_dim.width);
        let (h, y_off) = apply_max_constraint(max_h, base_dim.height);
        let dim = Dimension {
            x: base_dim.x + x_off,
            y: base_dim.y + y_off,
            width: w,
            height: h,
        };

        self.set_split_child_dimension(hub, root, dim);
    }

    fn update_container_min_size(&mut self, hub: &HubAccess, container_id: ContainerId) {
        let container = self.containers.get(container_id);
        let children = container.children.clone();
        let direction = container.direction();

        let child_mins: Vec<(f32, f32)> = children
            .iter()
            .map(|&c| {
                let (min_w, min_h, _, _) = self.get_effective_constraints(hub, c);
                (min_w, min_h)
            })
            .collect();

        let (min_w, min_h) = match direction {
            Some(Direction::Horizontal) => {
                let sum_w: f32 = child_mins.iter().map(|(w, _)| *w).sum();
                let max_h = child_mins.iter().map(|(_, h)| *h).fold(0.0, f32::max);
                (sum_w, max_h)
            }
            Some(Direction::Vertical) => {
                let max_w = child_mins.iter().map(|(w, _)| *w).fold(0.0, f32::max);
                let sum_h: f32 = child_mins.iter().map(|(_, h)| *h).sum();
                (max_w, sum_h)
            }
            None => {
                let max_w = child_mins.iter().map(|(w, _)| *w).fold(0.0, f32::max);
                let max_h = child_mins.iter().map(|(_, h)| *h).fold(0.0, f32::max);
                (max_w, max_h + hub.config.tab_bar_height)
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
    ) -> Vec<(f32, f32, f32, f32)> {
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
                if hub.config.auto_tile && !td.spawn_mode.is_tab() {
                    td.spawn_mode = SpawnMode::clean(spawn_mode);
                }
            }
            Child::Container(cid) => {
                let c = self.containers.get_mut(cid);
                c.dimension = dim;
                if hub.config.auto_tile && !c.spawn_mode().is_tab() {
                    c.set_spawn_mode(spawn_mode);
                }
            }
        }
    }

    /// Returns (min_w, min_h, max_w, max_h). Window-specific max takes precedence over global min.
    fn get_effective_constraints(&self, hub: &HubAccess, child: Child) -> (f32, f32, f32, f32) {
        let ws_id = match child {
            Child::Window(id) => hub.windows.get(id).workspace,
            Child::Container(id) => self.containers.get(id).workspace,
        };
        let screen = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension;
        let global_min_w = hub.config.min_width.resolve(screen.width);
        let global_min_h = hub.config.min_height.resolve(screen.height);

        match child {
            Child::Window(id) => {
                let window = hub.windows.get(id);
                let (win_min_w, win_min_h) = window.min_size();
                let (win_max_w, win_max_h) = window.max_size();

                let global_max_w = hub.config.max_width.resolve(screen.width);
                let global_max_h = hub.config.max_height.resolve(screen.height);

                let max_w = if win_max_w > 0.0 {
                    win_max_w
                } else {
                    global_max_w
                };
                let max_h = if win_max_h > 0.0 {
                    win_max_h
                } else {
                    global_max_h
                };

                // Window-specific max caps the effective min
                let min_w = if max_w > 0.0 {
                    win_min_w.max(global_min_w).min(max_w)
                } else {
                    win_min_w.max(global_min_w)
                };
                let min_h = if max_h > 0.0 {
                    win_min_h.max(global_min_h).min(max_h)
                } else {
                    win_min_h.max(global_min_h)
                };

                (min_w, min_h, max_w, max_h)
            }
            Child::Container(id) => {
                let (min_w, min_h) = self.containers.get(id).min_size();
                (min_w, min_h, 0.0, 0.0)
            }
        }
    }
}

/// Returns (size, offset) where offset is for centering within available space.
fn apply_max_constraint(max: f32, available: f32) -> (f32, f32) {
    if max > 0.0 && max < available {
        (max, (available - max) / 2.0)
    } else {
        (available, 0.0)
    }
}

fn distribute_space(constraints: &[(f32, f32)], container_size: f32) -> Vec<f32> {
    let constraints: Vec<(f32, f32)> = constraints
        .iter()
        .map(|&(min, max)| {
            let max = if max == 0.0 { f32::INFINITY } else { max };
            (min, max)
        })
        .collect();

    let sum_mins: f32 = constraints.iter().map(|(min, _)| min).sum();
    if sum_mins >= container_size {
        return constraints.iter().map(|(min, _)| *min).collect();
    }

    let all_finite = constraints.iter().all(|(_, max)| max.is_finite());
    if all_finite {
        let sum_maxes: f32 = constraints.iter().map(|(_, max)| max).sum();
        if sum_maxes <= container_size {
            return constraints.iter().map(|(_, max)| *max).collect();
        }
    }

    let mut low = 0.0;
    let mut high = container_size;
    const EPSILON: f32 = 0.001;

    while high - low > EPSILON {
        let mid = (low + high) / 2.0;
        let total: f32 = constraints
            .iter()
            .map(|(min, max)| mid.clamp(*min, *max))
            .sum();
        if total > container_size {
            high = mid;
        } else {
            low = mid;
        }
    }

    constraints
        .iter()
        .map(|(min, max)| low.clamp(*min, *max))
        .collect()
}
