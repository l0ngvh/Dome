use crate::core::hub::HubAccess;
use crate::core::node::{Dimension, Direction, Length, PixelRect, WorkspaceId};
use crate::core::partition_tree::Child;
use crate::core::strategy::{container_titles, tab_bar_band, translate, visible_tab_bar_band};
use crate::core::{ContainerPlacement, TilingWindowPlacement};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    /// Divides the work area top-down. Window size limits play no part, so the tree always
    /// fits the work area.
    pub(super) fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let ws_state = self.workspaces.get(&ws_id).unwrap();
        let Some(root) = ws_state.root else { return };
        let work_area = ws_state.work_area;
        let scale = ws_state.scale;

        self.set_child_dimension(
            root,
            Dimension::new(
                Length::ZERO,
                Length::ZERO,
                Length::from_pixels(work_area.width()),
                Length::from_pixels(work_area.height()),
            ),
        );

        let Child::Container(root_id) = root else {
            return;
        };
        for cid in hub.containers_preorder(root_id) {
            let data = self.tiling_containers.get(&cid).unwrap();
            let dim = data.dimension;
            let direction = data.direction();
            let children = hub.containers.get(cid).children.clone();
            let dimensions = self.layout_children(children.len(), dim, direction, scale);
            for (child, child_dim) in children.into_iter().zip(dimensions) {
                self.set_child_dimension(child, child_dim);
            }
        }
    }

    /// When `highlighted` is true, the placement of the workspace's focused tiling child is
    /// highlighted. Otherwise no placement is.
    pub(super) fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
    ) -> (Vec<TilingWindowPlacement>, Vec<ContainerPlacement>) {
        let Some(ws_state) = self.workspaces.get(&ws_id) else {
            return (Vec::new(), Vec::new());
        };
        let screen = ws_state.work_area;
        let scale = ws_state.scale;
        let border = hub.border_for_scale(scale);
        let focused = if highlighted {
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
            let border_box = translate(dim, Length::ZERO, Length::ZERO, screen.x(), screen.y());
            // The tree lies inside the work area, so only a box of zero area has no visible
            // part, such as each tab of a container shorter than its tab bar.
            let Some(visible_border_box) = border_box.clip(screen) else {
                continue;
            };
            let is_highlighted = focused == Some(child);
            let spawn_direction = is_highlighted.then(|| self.child_spawn_direction(child));
            match child {
                Child::Window(id) => {
                    let content_box = border_box.inset_by(border);
                    windows.push(TilingWindowPlacement {
                        id,
                        border_box,
                        visible_border_box,
                        content_box,
                        visible_content_box: content_box.clip(screen).unwrap_or(PixelRect::ZERO),
                        is_highlighted,
                        spawn_direction,
                    });
                }
                Child::Container(id) => {
                    let container = hub.containers.get(id);
                    let data = self.tiling_containers.get(&id).unwrap();
                    let band = tab_bar_band(
                        border_box,
                        dim,
                        screen,
                        self.tab_bar_length(scale),
                        data.is_tabbed(),
                    );
                    containers.push(ContainerPlacement {
                        id,
                        border_box,
                        visible_border_box,
                        tab_bar_band: band,
                        visible_tab_bar_band: visible_tab_bar_band(band, visible_border_box),
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

        (windows, containers)
    }

    fn layout_children(
        &self,
        count: usize,
        dim: Dimension,
        direction: Option<Direction>,
        scale: f32,
    ) -> Vec<Dimension> {
        match direction {
            Some(dir) => layout_split_axis_children(count, dim, dir),
            None => vec![self.tab_content(dim, scale); count],
        }
    }

    /// The part of a tabbed container under its tab bar, which every tab shares. A container
    /// shorter than the bar leaves its tabs zero height.
    fn tab_content(&self, dim: Dimension, scale: f32) -> Dimension {
        let tab_bar = self.tab_bar_length(scale).min(dim.height);
        Dimension::new(dim.x, dim.y + tab_bar, dim.width, dim.height - tab_bar)
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

/// Splits `dim` into `count` equal runs along `direction`. The last run takes the leftover, so
/// the runs end exactly where `dim` does.
fn layout_split_axis_children(
    count: usize,
    dim: Dimension,
    direction: Direction,
) -> Vec<Dimension> {
    let axis = Axis::from_direction(direction);
    let origin = axis.along_origin(dim);
    let extent = axis.along_extent(dim);
    let cross_origin = axis.cross_origin(dim);
    let cross_extent = axis.cross_extent(dim);
    let even = extent / count as f32;

    let mut cursor = origin;
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let along = if i == count - 1 {
            origin + extent - cursor
        } else {
            even
        };
        result.push(axis.compose(cursor, along, cross_origin, cross_extent));
        cursor += along;
    }
    result
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
