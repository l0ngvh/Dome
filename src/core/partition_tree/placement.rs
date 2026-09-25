use crate::core::hub::HubAccess;
use crate::core::node::{Dimension, Direction, Length, PixelRect, WorkspaceId};
use crate::core::partition_tree::Child;
use crate::core::strategy::{TilingPlacements, container_titles, visible_tab_bar_band};
use crate::core::{ContainerPlacement, TilingWindowPlacement};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    /// Lays the tree out in screen-absolute coordinates. Window size limits play no part.
    pub(super) fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let ws_state = self.workspaces.get(&ws_id).unwrap();
        let Some(root) = ws_state.root else { return };

        let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        let scale = monitor.scale;

        self.set_child_dimension(root, monitor.work_area.to_dimension());

        let Child::Container(root_id) = root else {
            return;
        };
        for cid in hub.containers_preorder(root_id) {
            let data = self.tiling_containers.get(&cid).unwrap();
            let dim = data.dimension;
            let direction = data.direction();
            let children = hub.containers.get(cid).children.clone();
            for (child, child_dim) in children
                .iter()
                .zip(self.layout_children(&children, dim, direction, scale))
            {
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
        let monitor = hub.monitors.get(ws.monitor);
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

        let mut stack: Vec<Child> = ws_state.root.into_iter().collect();
        for _ in crate::core::bounded_loop() {
            let Some(child) = stack.pop() else { break };
            let dim = self.child_dimension(child);
            let border_box = PixelRect::from_dimension(dim);
            // The tree lies inside the usable area, so a border box is its own visible part
            // and only a zero-area box, such as a tab covered by its ancestors' tab bars,
            // has nothing to show.
            if border_box.is_empty() {
                continue;
            }
            let is_highlighted = focused == Some(child);
            let spawn_direction = is_highlighted.then(|| self.child_spawn_direction(child));
            match child {
                Child::Window(id) => {
                    let content_box = border_box.inset_by(border);
                    windows.push(TilingWindowPlacement {
                        id,
                        border_box,
                        visible_border_box: border_box,
                        content_box,
                        visible_content_box: content_box,
                        is_highlighted,
                        spawn_direction,
                        is_mirrored: false,
                    });
                }
                Child::Container(id) => {
                    let container = hub.containers.get(id);
                    let data = self.tiling_containers.get(&id).unwrap();
                    let tab_bar = if data.is_tabbed() {
                        self.tab_bar_length(scale)
                    } else {
                        Length::ZERO
                    };
                    // Rounded from the container's top, so the band's bottom edge lands on
                    // the rounded top of the active tab.
                    let band =
                        PixelRect::from_dimension(Dimension::new(dim.x, dim.y, dim.width, tab_bar));
                    containers.push(ContainerPlacement {
                        id,
                        border_box,
                        visible_border_box: border_box,
                        tab_bar_band: band,
                        visible_tab_bar_band: visible_tab_bar_band(band, border_box),
                        is_highlighted,
                        spawn_direction,
                        is_tabbed: data.is_tabbed(),
                        active_tab_index: data.active_tab_index(),
                        titles: container_titles(hub, id),
                    });
                    if let Some(active) = self.active_tab(hub, id) {
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

    fn layout_children(
        &self,
        children: &[Child],
        dim: Dimension,
        direction: Option<Direction>,
        scale: f32,
    ) -> Vec<Dimension> {
        match direction {
            Some(dir) => self.layout_split_axis_children(children, dim, dir),
            None => self.layout_tabbed_children(children, dim, scale),
        }
    }

    fn layout_split_axis_children(
        &self,
        children: &[Child],
        dim: Dimension,
        direction: Direction,
    ) -> Vec<Dimension> {
        let axis = Axis::from_direction(direction);
        let origin = axis.along_origin(dim);
        let extent = axis.along_extent(dim);
        let cross_origin = axis.cross_origin(dim);
        let cross_extent = axis.cross_extent(dim);
        let n = children.len();
        let even = extent / n as f32;

        let mut cursor = origin;
        let mut result = Vec::with_capacity(n);
        for i in 0..n {
            // The last child takes the leftover, so the run ends exactly at
            // origin + extent.
            let along = if i == n - 1 {
                origin + extent - cursor
            } else {
                even
            };
            result.push(axis.compose(cursor, along, cross_origin, cross_extent));
            cursor += along;
        }
        result
    }

    fn layout_tabbed_children(
        &self,
        children: &[Child],
        dim: Dimension,
        scale: f32,
    ) -> Vec<Dimension> {
        let tab_bar = self.tab_bar_length(scale).min(dim.height);
        let content = Dimension::new(dim.x, dim.y + tab_bar, dim.width, dim.height - tab_bar);
        vec![content; children.len()]
    }

    fn set_child_dimension(&mut self, child: Child, dim: Dimension) {
        let spawn_direction = if dim.width >= dim.height {
            Direction::Horizontal
        } else {
            Direction::Vertical
        };
        let automatic_tiling = self.automatic_tiling;
        match child {
            Child::Window(wid) => {
                let td = self.tiling_windows.get_mut(&wid).unwrap();
                td.dimension = dim;
                if automatic_tiling {
                    td.spawn_direction = spawn_direction;
                }
            }
            Child::Container(cid) => {
                let c = self.tiling_containers.get_mut(&cid).unwrap();
                c.dimension = dim;
                if automatic_tiling {
                    c.set_spawn_direction(spawn_direction);
                }
            }
        }
    }

    pub(super) fn tab_bar_length(&self, scale: f32) -> Length {
        Length::from_pixels(self.tab_bar_height).to_unit(scale)
    }
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
