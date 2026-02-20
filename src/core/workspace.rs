use crate::core::{
    Child, ContainerId, Dimension, Hub, SpawnMode,
    node::{Direction, Parent, WorkspaceId},
};

impl Hub {
    pub(super) fn adjust_workspace(&mut self, ws_id: WorkspaceId) {
        let ws = self.workspaces.get(ws_id);
        let Some(root) = ws.root else { return };
        let screen = self.monitors.get(ws.monitor).dimension;

        let Child::Container(root_id) = root else {
            self.set_root_dimension(root, screen);
            return;
        };

        // Collect containers in pre-order
        let mut stack = vec![root_id];
        let mut order = vec![];
        for _ in super::bounded_loop() {
            let Some(cid) = stack.pop() else { break };
            order.push(cid);
            for child in &self.containers.get(cid).children {
                if let Child::Container(child_cid) = child {
                    stack.push(*child_cid);
                }
            }
        }

        // Update minimum sizes bottom-up, as parent's minimum size depends on children's minimum size
        for &cid in order.iter().rev() {
            self.update_container_min_size(cid);
        }

        self.set_root_dimension(root, screen);
        for cid in order {
            let container = self.containers.get(cid);
            let dim = container.dimension;
            let children = container.children.clone();
            let direction = container.direction();
            for (child, child_dim) in children
                .iter()
                .zip(self.layout_split_children(&children, dim, direction))
            {
                self.set_split_child_dimension(*child, child_dim);
            }
        }

        // Focused window can go out of view due to resizing other windows
        self.scroll_into_view(ws_id);
    }

    pub(super) fn focus_workspace_with_id(&mut self, workspace_id: WorkspaceId) {
        tracing::debug!("Focusing workspace {workspace_id}");
        let current_ws = self.current_workspace();
        if workspace_id == current_ws {
            return;
        }
        let monitor_id = self.workspaces.get(workspace_id).monitor;
        self.focused_monitor = monitor_id;
        self.monitors.get_mut(monitor_id).active_workspace = workspace_id;
        self.prune_workspace(current_ws);
    }

    /// Update primary focus, i.e. focus of the whole workspace
    pub(super) fn set_workspace_focus(&mut self, child: Child) {
        let mut current = child;
        for _ in super::bounded_loop() {
            match self.get_parent(current) {
                Parent::Container(cid) => {
                    let container = self.containers.get_mut(cid);
                    if container.is_tabbed {
                        container.set_active_tab(current);
                    }
                    container.focused = child;
                    current = Child::Container(cid);
                }
                Parent::Workspace(ws) => {
                    self.workspaces.get_mut(ws).focused = Some(child);
                    self.scroll_into_view(ws);
                    break;
                }
            }
        }
    }

    /// Deletes workspace if empty and not active on its monitor
    pub(super) fn prune_workspace(&mut self, ws_id: WorkspaceId) {
        let ws = self.workspaces.get(ws_id);
        if ws.root.is_some() || !ws.float_windows.is_empty() {
            return;
        }
        if self.monitors.get(ws.monitor).active_workspace != ws_id {
            self.workspaces.delete(ws_id);
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
        children: &[Child],
        dim: Dimension,
        direction: Option<Direction>,
    ) -> Vec<Dimension> {
        let constraints = self.collect_constraints(children);

        match direction {
            Some(Direction::Horizontal) => {
                let height = dim.height.max(
                    constraints.iter().map(|c| c.2).fold(0.0, f32::max), // max of min_h
                );
                let width_constraints: Vec<_> = constraints.iter().map(|c| (c.0, c.1)).collect(); // (min_w, max_w)
                let widths = distribute_space(&width_constraints, dim.width);
                let mut x = dim.x + (dim.width - widths.iter().sum::<f32>()) / 2.0;

                let mut result = Vec::with_capacity(children.len());
                for i in 0..children.len() {
                    let (_, _, _, max_h) = constraints[i];
                    let (h, y_off) = Self::apply_max_constraint(max_h, height);
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
                let width = dim.width.max(
                    constraints.iter().map(|c| c.0).fold(0.0, f32::max), // max of min_w
                );
                let height_constraints: Vec<_> = constraints.iter().map(|c| (c.2, c.3)).collect(); // (min_h, max_h)
                let heights = distribute_space(&height_constraints, dim.height);
                let mut y = dim.y + (dim.height - heights.iter().sum::<f32>()) / 2.0;

                let mut result = Vec::with_capacity(children.len());
                for i in 0..children.len() {
                    let (_, max_w, _, _) = constraints[i];
                    let (w, x_off) = Self::apply_max_constraint(max_w, width);
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
                let content_y = dim.y + self.config.tab_bar_height;
                let content_height = dim.height - self.config.tab_bar_height;

                let mut result = Vec::with_capacity(children.len());
                for (_, max_w, _, max_h) in constraints {
                    let (w, x_off) = Self::apply_max_constraint(max_w, dim.width);
                    let (h, y_off) = Self::apply_max_constraint(max_h, content_height);
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

    fn scroll_into_view(&mut self, workspace_id: WorkspaceId) {
        self.clamp_viewport_offset(workspace_id);

        let (monitor_id, current_offset, focused) = {
            let ws = self.workspaces.get(workspace_id);
            (ws.monitor, ws.viewport_offset, ws.focused)
        };
        let screen = self.monitors.get(monitor_id).dimension;
        let (mut offset_x, mut offset_y) = current_offset;

        let focused_dim = match focused {
            Some(Child::Window(id)) => self.windows.get(id).dimension,
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

        self.workspaces.get_mut(workspace_id).viewport_offset = (offset_x, offset_y);
    }

    fn clamp_viewport_offset(&mut self, workspace_id: WorkspaceId) {
        let ws = self.workspaces.get(workspace_id);
        let screen = self.monitors.get(ws.monitor).dimension;
        let (mut offset_x, mut offset_y) = ws.viewport_offset;

        let root_dim = match ws.root {
            Some(Child::Window(id)) => self.windows.get(id).dimension,
            Some(Child::Container(id)) => self.containers.get(id).dimension,
            None => {
                self.workspaces.get_mut(workspace_id).viewport_offset = (0.0, 0.0);
                return;
            }
        };

        offset_x = offset_x.clamp(0.0, (root_dim.width - screen.width).max(0.0));
        offset_y = offset_y.clamp(0.0, (root_dim.height - screen.height).max(0.0));
        self.workspaces.get_mut(workspace_id).viewport_offset = (offset_x, offset_y);
    }

    // Windows can extend beyond screen visible area
    fn set_root_dimension(&mut self, root: Child, screen: Dimension) {
        let (min_w, min_h) = self.get_effective_min_size(root);
        let base_dim = Dimension {
            x: 0.0,
            y: 0.0,
            width: screen.width.max(min_w),
            height: screen.height.max(min_h),
        };

        // Apply max_size centering for single window at root
        let dim = {
            let (max_w, max_h) = self.get_effective_max_size(root);
            let (w, x_off) = Self::apply_max_constraint(max_w, base_dim.width);
            let (h, y_off) = Self::apply_max_constraint(max_h, base_dim.height);
            Dimension {
                x: base_dim.x + x_off,
                y: base_dim.y + y_off,
                width: w,
                height: h,
            }
        };

        self.set_split_child_dimension(root, dim);
    }

    fn update_container_min_size(&mut self, container_id: ContainerId) {
        let container = self.containers.get(container_id);
        let children = container.children.clone();
        let direction = container.direction();

        let child_mins: Vec<(f32, f32)> = children
            .iter()
            .map(|&c| self.get_effective_min_size(c))
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
                // Tabbed
                let max_w = child_mins.iter().map(|(w, _)| *w).fold(0.0, f32::max);
                let max_h = child_mins.iter().map(|(_, h)| *h).fold(0.0, f32::max);
                (max_w, max_h + self.config.tab_bar_height)
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

    fn collect_constraints(&self, children: &[Child]) -> Vec<(f32, f32, f32, f32)> {
        children
            .iter()
            .map(|&c| {
                let (min_w, min_h) = self.get_effective_min_size(c);
                let (max_w, max_h) = self.get_effective_max_size(c);
                (min_w, max_w, min_h, max_h)
            })
            .collect()
    }

    fn set_split_child_dimension(&mut self, child: Child, dim: Dimension) {
        let spawn_mode = if dim.width >= dim.height {
            SpawnMode::horizontal()
        } else {
            SpawnMode::vertical()
        };
        match child {
            Child::Window(wid) => {
                let w = self.windows.get_mut(wid);
                w.dimension = dim;
                if self.config.auto_tile && !w.spawn_mode().is_tab() {
                    w.set_spawn_mode(spawn_mode);
                }
            }
            Child::Container(cid) => {
                let c = self.containers.get_mut(cid);
                c.dimension = dim;
                if self.config.auto_tile && !c.spawn_mode().is_tab() {
                    c.set_spawn_mode(spawn_mode);
                }
            }
        }
    }

    fn get_effective_min_size(&self, child: Child) -> (f32, f32) {
        let ws_id = match child {
            Child::Window(id) => self.windows.get(id).workspace,
            Child::Container(id) => self.containers.get(id).workspace,
        };
        let screen = self
            .monitors
            .get(self.workspaces.get(ws_id).monitor)
            .dimension;
        let global_min_w = self.config.min_width.resolve(screen.width);
        let global_min_h = self.config.min_height.resolve(screen.height);

        match child {
            Child::Window(id) => {
                let (w, h) = self.windows.get(id).min_size();
                (w.max(global_min_w), h.max(global_min_h))
            }
            Child::Container(id) => self.containers.get(id).min_size(),
        }
    }

    fn get_effective_max_size(&self, child: Child) -> (f32, f32) {
        match child {
            Child::Window(id) => {
                let window = self.windows.get(id);
                let (max_w, max_h) = window.max_size();
                let screen = self
                    .monitors
                    .get(self.workspaces.get(window.workspace).monitor)
                    .dimension;
                let global_max_w = self.config.max_width.resolve(screen.width);
                let global_max_h = self.config.max_height.resolve(screen.height);
                let w = if max_w > 0.0 { max_w } else { global_max_w };
                let h = if max_h > 0.0 { max_h } else { global_max_h };
                (w, h)
            }
            Child::Container(_) => (0.0, 0.0),
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
