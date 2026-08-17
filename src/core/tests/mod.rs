mod export;
mod float_window;
mod focus_workspace;
mod fullscreen;
mod master;
mod minimize;
mod monitor;
mod move_to_workspace;
mod partition_tree;
mod pixel_rect;
mod preferred_layout;
mod query;
mod set_focus;
mod smoke;
mod strategy_switch;

use std::collections::HashSet;

use crate::config::{
    LayoutWorkspaceConfig, MasterConfig, PaneConfig, PartitionTreeConfig, SizeConstraint,
    SizeConstraints, Strategy, TreeLayoutNode, WindowMatcher,
};
use crate::core::GlobalLayoutConfig;
use crate::core::PaneDisplay;
use crate::core::allocator::NodeId;
use crate::core::hub::{Hub, MonitorLayout, SpawnIndicator};
use crate::core::node::{Direction, Logical, Pixels, WindowId};
use crate::core::strategy::TilingAction;
use crate::core::{
    ContainerPlacement, FloatWindowPlacement, PixelRect, TilingWindowPlacement, WindowMetadata,
};

const ASCII_WIDTH: usize = 150;
const ASCII_HEIGHT: usize = 30;
const TAB_BAR_HEIGHT: i32 = 2;
const BORDER_SIZE: i32 = 1;

pub(super) fn snapshot(hub: &Hub) -> String {
    validate_hub(hub);
    let mut s = snapshot_text(hub);

    let mut grid = vec![vec![' '; ASCII_WIDTH]; ASCII_HEIGHT];
    let all = hub.get_visible_placements();
    let mp = &all.monitors[0];

    let (tiling_windows, float_windows, containers) = match &mp.layout {
        MonitorLayout::Normal {
            tiling_windows,
            float_windows,
            containers,
        } => (
            tiling_windows.as_slice(),
            float_windows.as_slice(),
            containers.as_slice(),
        ),
        MonitorLayout::Fullscreen(id) => {
            let screen = hub.access.monitors.get(mp.monitor_id).work_area;
            draw_rect(&mut grid, screen, &format!("W{}", id.get()), [false; 4]);
            s.push('\n');
            s.push_str(
                &grid
                    .iter()
                    .map(|row| row.iter().collect::<String>())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            return s;
        }
    };

    for wp in tiling_windows {
        let d = wp.visible_border_box;
        let clip = clip_edges(wp.border_box, wp.visible_border_box);
        draw_rect(&mut grid, d, &format!("W{}", wp.id.get()), clip);
    }

    for cp in containers {
        if cp.is_tabbed {
            let d = cp.visible_border_box;
            draw_tab_bar(&mut grid, d, &cp.titles, cp.active_tab_index);
        }
    }

    let focused_float = float_windows.iter().find(|p| p.is_highlighted);
    if focused_float.is_none() {
        if let Some(wp) = tiling_windows.iter().find(|p| p.is_highlighted) {
            let d = wp.visible_border_box;
            let clip = clip_edges(wp.border_box, wp.visible_border_box);
            draw_focused_border(&mut grid, d, clip);
        } else if let Some(cp) = containers.iter().find(|p| p.is_highlighted) {
            let d = cp.visible_border_box;
            let clip = clip_edges(cp.border_box, cp.visible_border_box);
            draw_focused_border(&mut grid, d, clip);
        }
    }

    for wp in float_windows {
        let d = wp.visible_border_box;
        let clip = clip_edges(wp.border_box, wp.visible_border_box);
        let grid_w = grid[0].len() as isize;
        let grid_h = grid.len() as isize;
        let x1 = d.x().value() as isize;
        let y1 = d.y().value() as isize;
        let x2 = d.right().value() as isize - 1;
        let y2 = d.bottom().value() as isize - 1;
        for row in (y1 + 1).max(0)..y2.min(grid_h) {
            for col in (x1 + 1).max(0)..x2.min(grid_w) {
                grid[row as usize][col as usize] = ' ';
            }
        }
        draw_rect(&mut grid, d, &format!("F{}", wp.id.get()), clip);
    }

    if let Some(wp) = focused_float {
        let d = wp.visible_border_box;
        let clip = clip_edges(wp.border_box, wp.visible_border_box);
        draw_focused_border(&mut grid, d, clip);
    }

    s.push('\n');
    s.push_str(
        &grid
            .iter()
            .map(|row| row.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    s
}

pub(super) fn snapshot_text(hub: &Hub) -> String {
    let vp = hub.get_visible_placements();
    let focused = match vp.focused_window {
        Some(id) => format!("focused={id}"),
        None => "focused=None".to_string(),
    };
    let mut s = format!("Hub({focused})\n");
    // Gated behind 2+ monitors so pre-existing single-monitor snapshots stay byte-identical.
    let monitor_count = vp.monitors.len();
    for mp in &vp.monitors {
        // `{:.2}` is a no-op on integer Display, so the printed screen goes through
        // `to_dimension` to keep the snapshot format stable.
        let screen = hub
            .access
            .monitors
            .get(mp.monitor_id)
            .work_area
            .to_dimension();
        let name_seg = if monitor_count > 1 {
            format!(
                ", name={:?}",
                hub.access.monitors.get(mp.monitor_id).unique_name
            )
        } else {
            String::new()
        };
        match &mp.layout {
            MonitorLayout::Normal {
                tiling_windows,
                float_windows,
                containers,
            } => {
                if tiling_windows.is_empty() && float_windows.is_empty() && containers.is_empty() {
                    s.push_str(&format!(
                        "  Monitor(id={}{}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}))\n",
                        mp.monitor_id, name_seg, screen.x, screen.y, screen.width, screen.height
                    ));
                } else {
                    s.push_str(&format!(
                        "  Monitor(id={}{}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}),\n",
                        mp.monitor_id, name_seg, screen.x, screen.y, screen.width, screen.height
                    ));
                    for wp in tiling_windows {
                        s.push_str(&fmt_tiling_placement(wp));
                    }
                    for wp in float_windows {
                        s.push_str(&fmt_float_placement(wp));
                    }
                    for cp in containers {
                        s.push_str(&fmt_container_placement(cp));
                    }
                    s.push_str("  )\n");
                }
            }
            MonitorLayout::Fullscreen(id) => {
                s.push_str(&format!(
                    "  Monitor(id={}{}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}),\n",
                    mp.monitor_id, name_seg, screen.x, screen.y, screen.width, screen.height
                ));
                s.push_str(&format!("    Fullscreen(id={})\n", id));
                s.push_str("  )\n");
            }
        }
    }
    let mut ids: Vec<WindowId> = hub
        .minimized_window_entries()
        .into_iter()
        .map(|e| e.id)
        .collect();
    if !ids.is_empty() {
        ids.sort();
        let id_strs: Vec<String> = ids.iter().map(|id| format!("{id}")).collect();
        s.push_str(&format!("  Minimized: [{}]\n", id_strs.join(", ")));
    }
    s
}

fn fmt_spawn(indicator: &SpawnIndicator) -> String {
    let dirs: Vec<&str> = [
        (indicator.top, "top"),
        (indicator.right, "right"),
        (indicator.bottom, "bottom"),
        (indicator.left, "left"),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, name)| *name)
    .collect();
    dirs.join("+")
}

fn fmt_tiling_placement(wp: &TilingWindowPlacement) -> String {
    let d = wp.visible_border_box.to_dimension();
    let mut parts = format!(
        "    Window(id={}, x={:.2}, y={:.2}, w={:.2}, h={:.2}",
        wp.id, d.x, d.y, d.width, d.height
    );
    if wp.is_highlighted {
        parts.push_str(", highlighted");
    }
    if let Some(ref si) = wp.spawn_indicator {
        parts.push_str(&format!(", spawn={}", fmt_spawn(si)));
    }
    parts.push_str(")\n");
    parts
}

fn fmt_float_placement(wp: &FloatWindowPlacement) -> String {
    let d = wp.visible_border_box.to_dimension();
    let mut parts = format!(
        "    Window(id={}, x={:.2}, y={:.2}, w={:.2}, h={:.2}",
        wp.id, d.x, d.y, d.width, d.height
    );
    parts.push_str(", float");
    if wp.is_highlighted {
        parts.push_str(", highlighted");
    }
    parts.push_str(")\n");
    parts
}

fn fmt_container_placement(cp: &ContainerPlacement) -> String {
    let d = cp.visible_border_box.to_dimension();
    let mut parts = format!(
        "    Container(id={}, x={:.2}, y={:.2}, w={:.2}, h={:.2}",
        cp.id, d.x, d.y, d.width, d.height
    );
    if cp.is_tabbed {
        parts.push_str(&format!(", tabbed, active_tab={}", cp.active_tab_index));
    }
    if cp.is_highlighted {
        parts.push_str(", highlighted");
    }
    if let Some(ref si) = cp.spawn_indicator {
        parts.push_str(&format!(", spawn={}", fmt_spawn(si)));
    }
    let titles = cp.titles.join(", ");
    parts.push_str(&format!(", titles=[{}]", titles));
    parts.push_str(")\n");
    parts
}

#[expect(
    clippy::needless_range_loop,
    reason = "grid indexing requires row/col indices"
)]
fn draw_tab_bar(grid: &mut [Vec<char>], rect: PixelRect, labels: &[String], active: usize) {
    let (x, y, width) = (rect.x().value(), rect.y().value(), rect.width().value());
    let x1 = x as usize;
    let y1 = y as usize;
    let y2 = y1 + TAB_BAR_HEIGHT as usize - 1;
    let x2 = (x + width) as usize - 1;
    let inner_width = x2 - x1 - 1;
    let tab_count = labels.len();

    for col in x1..=x2 {
        grid[y1][col] = '-';
    }
    grid[y1][x1] = '+';
    grid[y1][x2] = '+';

    for row in (y1 + 1)..=y2 {
        grid[row][x1] = '|';
        grid[row][x2] = '|';
    }

    if tab_count == 0 {
        return;
    }

    let label_row = y1 + 1 + (y2 - y1 - 1) / 2;
    let tab_width = inner_width / tab_count;
    for (i, label) in labels.iter().enumerate() {
        let tab_start = x1 + 1 + i * tab_width;
        let tab_end = if i == tab_count - 1 {
            x2 - 1
        } else {
            tab_start + tab_width - 1
        };
        let display = if i == active {
            format!("[{}]", label)
        } else {
            label.clone()
        };
        let mid = (tab_start + tab_end) / 2;
        let label_start = mid.saturating_sub(display.len() / 2);
        for (j, ch) in display.chars().enumerate() {
            let col = label_start + j;
            if col <= tab_end {
                grid[label_row][col] = ch;
            }
        }
        if i < tab_count - 1 {
            for row in (y1 + 1)..=y2 {
                grid[row][tab_end + 1] = '|';
            }
        }
    }
}

/// Integral edges make these comparisons exact, where the `Dimension` form needed a
/// half-unit tolerance to avoid reporting a clip that rounding had already removed.
fn clip_edges(border_box: PixelRect, visible: PixelRect) -> [bool; 4] {
    [
        visible.x() > border_box.x(),
        visible.right() < border_box.right(),
        visible.y() > border_box.y(),
        visible.bottom() < border_box.bottom(),
    ]
}

fn draw_rect(grid: &mut [Vec<char>], rect: PixelRect, label: &str, clip: [bool; 4]) {
    let (x, y, w, h) = (
        rect.x().value(),
        rect.y().value(),
        rect.width().value(),
        rect.height().value(),
    );
    let grid_w = grid[0].len() as isize;
    let grid_h = grid.len() as isize;
    let [clip_l, clip_r, clip_t, clip_b] = clip;

    let x1 = x as isize;
    let y1 = y as isize;
    let x2 = (x + w) as isize - 1;
    let y2 = (y + h) as isize - 1;

    if !clip_t {
        for col in x1.max(0)..=x2.min(grid_w - 1) {
            if y1 >= 0 && y1 < grid_h {
                grid[y1 as usize][col as usize] = '-';
            }
        }
    }
    if !clip_b {
        for col in x1.max(0)..=x2.min(grid_w - 1) {
            if y2 >= 0 && y2 < grid_h {
                grid[y2 as usize][col as usize] = '-';
            }
        }
    }
    if !clip_l {
        for row in y1.max(0)..=y2.min(grid_h - 1) {
            if x1 >= 0 && x1 < grid_w {
                grid[row as usize][x1 as usize] = '|';
            }
        }
    }
    if !clip_r {
        for row in y1.max(0)..=y2.min(grid_h - 1) {
            if x2 >= 0 && x2 < grid_w {
                grid[row as usize][x2 as usize] = '|';
            }
        }
    }
    if !clip_l && !clip_t && x1 >= 0 && x1 < grid_w && y1 >= 0 && y1 < grid_h {
        grid[y1 as usize][x1 as usize] = '+';
    }
    if !clip_r && !clip_t && x2 >= 0 && x2 < grid_w && y1 >= 0 && y1 < grid_h {
        grid[y1 as usize][x2 as usize] = '+';
    }
    if !clip_l && !clip_b && x1 >= 0 && x1 < grid_w && y2 >= 0 && y2 < grid_h {
        grid[y2 as usize][x1 as usize] = '+';
    }
    if !clip_r && !clip_b && x2 >= 0 && x2 < grid_w && y2 >= 0 && y2 < grid_h {
        grid[y2 as usize][x2 as usize] = '+';
    }

    let mid_x = (x as f32 + w as f32 / 2.0).round() as isize;
    let mid_y = (y as f32 + h as f32 / 2.0).round() as isize;
    if mid_y >= 0 && mid_y < grid_h {
        let start_x = mid_x - (label.len() / 2) as isize;
        for (i, ch) in label.chars().enumerate() {
            let col = start_x + i as isize;
            if col > x1 && col < x2 && col >= 0 && col < grid_w {
                grid[mid_y as usize][col as usize] = ch;
            }
        }
    }
}

fn draw_focused_border(grid: &mut [Vec<char>], rect: PixelRect, clip: [bool; 4]) {
    let (x, y, w, h) = (
        rect.x().value(),
        rect.y().value(),
        rect.width().value(),
        rect.height().value(),
    );
    let grid_w = grid[0].len() as isize;
    let grid_h = grid.len() as isize;
    let [clip_l, clip_r, clip_t, clip_b] = clip;

    let x1 = x as isize;
    let y1 = y as isize;
    let x2 = (x + w) as isize - 1;
    let y2 = (y + h) as isize - 1;

    if !clip_t {
        for col in x1.max(0)..=x2.min(grid_w - 1) {
            if y1 >= 0 && y1 < grid_h {
                grid[y1 as usize][col as usize] = '*';
            }
        }
    }
    if !clip_b {
        for col in x1.max(0)..=x2.min(grid_w - 1) {
            if y2 >= 0 && y2 < grid_h {
                grid[y2 as usize][col as usize] = '*';
            }
        }
    }
    if !clip_l {
        for row in y1.max(0)..=y2.min(grid_h - 1) {
            if x1 >= 0 && x1 < grid_w {
                grid[row as usize][x1 as usize] = '*';
            }
        }
    }
    if !clip_r {
        for row in y1.max(0)..=y2.min(grid_h - 1) {
            if x2 >= 0 && x2 < grid_w {
                grid[row as usize][x2 as usize] = '*';
            }
        }
    }
}

fn validate_hub(hub: &Hub) {
    hub.validate();
    validate_visible_placements(hub);
    validate_minimized(hub);
}

fn validate_visible_placements(hub: &Hub) {
    // Deliberately independent of `PixelRect::clip`. Production derives every
    // `visible_border_box` with that method, so asserting against it would compare it
    // with itself and the invariant could never fail.
    fn clip(rect: PixelRect, bounds: PixelRect) -> Option<PixelRect> {
        let x1 = rect.x().value().max(bounds.x().value());
        let y1 = rect.y().value().max(bounds.y().value());
        let x2 = rect.right().value().min(bounds.right().value());
        let y2 = rect.bottom().value().min(bounds.bottom().value());
        if x1 >= x2 || y1 >= y2 {
            return None;
        }
        Some(PixelRect::new(x1, y1, x2 - x1, y2 - y1))
    }

    let all_placements = hub.get_visible_placements();
    let mut seen_window_ids = HashSet::new();

    for mp in &all_placements.monitors {
        let screen = hub.access.monitors.get(mp.monitor_id).work_area;
        let (tiling_windows, float_windows, containers) = match &mp.layout {
            MonitorLayout::Normal {
                tiling_windows,
                float_windows,
                containers,
            } => (
                tiling_windows.as_slice(),
                float_windows.as_slice(),
                containers.as_slice(),
            ),
            MonitorLayout::Fullscreen(_) => continue,
        };
        for wp in tiling_windows {
            assert!(
                seen_window_ids.insert(wp.id),
                "Duplicate window {} in visible placements",
                wp.id
            );
            assert_eq!(
                clip(wp.border_box, screen),
                Some(wp.visible_border_box),
                "Window {} visible_border_box doesn't match clip(border_box, screen)",
                wp.id
            );
        }
        for wp in float_windows {
            assert!(
                seen_window_ids.insert(wp.id),
                "Duplicate window {} in visible placements",
                wp.id
            );
            assert_eq!(
                clip(wp.border_box, screen),
                Some(wp.visible_border_box),
                "Window {} visible_border_box doesn't match clip(border_box, screen)",
                wp.id
            );
        }
        for cp in containers {
            assert_eq!(
                clip(cp.border_box, screen),
                Some(cp.visible_border_box),
                "Container {} visible_border_box doesn't match clip(border_box, screen)",
                cp.id
            );
        }
    }
}

fn validate_minimized(hub: &Hub) {
    let minimized_ids: Vec<WindowId> = hub
        .minimized_window_entries()
        .into_iter()
        .map(|e| e.id)
        .collect();

    for &id in &minimized_ids {
        let w = hub.access.windows.get(id);
        assert!(
            w.is_minimized(),
            "Window {id} in minimized_windows but is_minimized is false"
        );
        assert!(
            w.workspace().is_none(),
            "{id} is minimized but has a workspace",
        );
    }
    for (wid, window) in hub.access.windows.all_active() {
        if window.workspace().is_none() {
            assert!(
                window.is_minimized(),
                "{wid} has no workspace but is_minimized is false"
            );
            assert!(
                minimized_ids.contains(&wid),
                "{wid} has no workspace but is not in minimized_windows"
            );
        }
    }
}

impl Hub {
    pub(crate) fn focus_left(&mut self) {
        self.handle_tiling_action(TilingAction::FocusDirection {
            direction: Direction::Horizontal,
            forward: false,
        });
    }

    pub(crate) fn focus_right(&mut self) {
        self.handle_tiling_action(TilingAction::FocusDirection {
            direction: Direction::Horizontal,
            forward: true,
        });
    }

    pub(crate) fn focus_up(&mut self) {
        self.handle_tiling_action(TilingAction::FocusDirection {
            direction: Direction::Vertical,
            forward: false,
        });
    }

    pub(crate) fn focus_down(&mut self) {
        self.handle_tiling_action(TilingAction::FocusDirection {
            direction: Direction::Vertical,
            forward: true,
        });
    }

    pub(crate) fn focus_parent(&mut self) {
        self.handle_tiling_action(TilingAction::FocusParent);
    }

    pub(crate) fn focus_next_tab(&mut self) {
        self.handle_tiling_action(TilingAction::FocusTab { forward: true });
    }

    pub(crate) fn focus_prev_tab(&mut self) {
        self.handle_tiling_action(TilingAction::FocusTab { forward: false });
    }

    pub(crate) fn move_left(&mut self) {
        self.handle_tiling_action(TilingAction::MoveDirection {
            direction: Direction::Horizontal,
            forward: false,
        });
    }

    pub(crate) fn move_right(&mut self) {
        self.handle_tiling_action(TilingAction::MoveDirection {
            direction: Direction::Horizontal,
            forward: true,
        });
    }

    pub(crate) fn move_up(&mut self) {
        self.handle_tiling_action(TilingAction::MoveDirection {
            direction: Direction::Vertical,
            forward: false,
        });
    }

    pub(crate) fn move_down(&mut self) {
        self.handle_tiling_action(TilingAction::MoveDirection {
            direction: Direction::Vertical,
            forward: true,
        });
    }

    pub(crate) fn toggle_spawn_mode(&mut self) {
        self.handle_tiling_action(TilingAction::ToggleSpawnMode);
    }

    pub(crate) fn toggle_direction(&mut self) {
        self.handle_tiling_action(TilingAction::ToggleDirection);
    }

    pub(crate) fn toggle_container_layout(&mut self) {
        self.handle_tiling_action(TilingAction::ToggleContainerLayout);
    }
}

pub(super) fn setup_logger_with_level(level: &str) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

#[derive(Clone)]
struct TestHubBuilder {
    layout: GlobalLayoutConfig,
    preferred_layout: Vec<LayoutWorkspaceConfig>,
    scale: f32,
}

impl TestHubBuilder {
    fn new() -> Self {
        Self {
            layout: LayoutConfigBuilder::new().build(),
            preferred_layout: Vec::new(),
            scale: 1.0,
        }
    }

    fn with_layout(self, layout: GlobalLayoutConfig) -> Self {
        Self { layout, ..self }
    }

    fn with_preferred_layout(self, preferred_layout: Vec<LayoutWorkspaceConfig>) -> Self {
        Self {
            preferred_layout,
            ..self
        }
    }

    #[cfg_attr(
        not(target_os = "windows"),
        expect(
            dead_code,
            reason = "phantom marker used only as a type parameter on Windows"
        )
    )]
    fn with_scale(self, scale: f32) -> Self {
        Self { scale, ..self }
    }

    fn build(self) -> Hub {
        Hub::new(
            PixelRect::new(0, 0, ASCII_WIDTH as i32, ASCII_HEIGHT as i32),
            self.scale,
            self.layout,
            self.preferred_layout,
        )
    }
}

struct LayoutConfigBuilder {
    strategy: Strategy,
    border_size: Pixels<Logical>,
    master: MasterConfig,
    partition_tree: PartitionTreeConfig,
    size_constraints: SizeConstraints,
    float: Vec<WindowMatcher>,
    fullscreen: Vec<WindowMatcher>,
}

impl LayoutConfigBuilder {
    fn new() -> Self {
        Self {
            strategy: Strategy::PartitionTree,
            border_size: Pixels::new(BORDER_SIZE),
            master: MasterConfig {
                master_ratio: 0.5,
                master_count: 1,
            },
            partition_tree: PartitionTreeConfig {
                tab_bar_height: Pixels::new(TAB_BAR_HEIGHT),
                automatic_tiling: false,
            },
            size_constraints: SizeConstraints {
                minimum_width: SizeConstraint::Pixels(Pixels::new(1)),
                minimum_height: SizeConstraint::Pixels(Pixels::new(1)),
                maximum_width: SizeConstraint::Pixels(Pixels::new(0)),
                maximum_height: SizeConstraint::Pixels(Pixels::new(0)),
            },
            float: vec![],
            fullscreen: vec![],
        }
    }
    fn with_strategy(self, strategy: Strategy) -> Self {
        Self { strategy, ..self }
    }

    fn with_master_config(self, master: MasterConfig) -> Self {
        Self { master, ..self }
    }

    fn with_border_size(self, border_size: Pixels<Logical>) -> Self {
        Self {
            border_size,
            ..self
        }
    }

    fn with_min_width(self, min_width: SizeConstraint) -> Self {
        Self {
            size_constraints: SizeConstraints {
                minimum_width: min_width,
                ..self.size_constraints
            },
            ..self
        }
    }

    fn with_min_height(self, min_height: SizeConstraint) -> Self {
        Self {
            size_constraints: SizeConstraints {
                minimum_height: min_height,
                ..self.size_constraints
            },
            ..self
        }
    }

    fn with_partition_tree_config(self, partition_tree: PartitionTreeConfig) -> Self {
        Self {
            partition_tree,
            ..self
        }
    }

    fn with_max_width(self, max_width: SizeConstraint) -> Self {
        Self {
            size_constraints: SizeConstraints {
                maximum_width: max_width,
                ..self.size_constraints
            },
            ..self
        }
    }

    fn with_max_height(self, max_height: SizeConstraint) -> Self {
        Self {
            size_constraints: SizeConstraints {
                maximum_height: max_height,
                ..self.size_constraints
            },
            ..self
        }
    }

    fn with_float(self, float: Vec<WindowMatcher>) -> Self {
        Self { float, ..self }
    }

    fn with_fullscreen(self, fullscreen: Vec<WindowMatcher>) -> Self {
        Self { fullscreen, ..self }
    }

    fn build(self) -> GlobalLayoutConfig {
        GlobalLayoutConfig {
            strategy: self.strategy,
            border_size: self.border_size,
            partition_tree: self.partition_tree,
            master: self.master,
            size_constraints: self.size_constraints,
            float: self.float,
            fullscreen: self.fullscreen,
            ignore: Vec::new(),
        }
    }
}

struct PartitionTreeConfigBuilder {
    tab_bar_height: Pixels<Logical>,
    automatic_tiling: bool,
}

impl PartitionTreeConfigBuilder {
    fn new() -> Self {
        Self {
            tab_bar_height: Pixels::new(TAB_BAR_HEIGHT),
            automatic_tiling: false,
        }
    }

    fn with_tab_bar_height(self, tab_bar_height: Pixels<Logical>) -> Self {
        Self {
            tab_bar_height,
            ..self
        }
    }

    fn with_automatic_tiling(self, automatic_tiling: bool) -> Self {
        Self {
            automatic_tiling,
            ..self
        }
    }

    fn build(self) -> PartitionTreeConfig {
        PartitionTreeConfig {
            tab_bar_height: self.tab_bar_height,
            automatic_tiling: self.automatic_tiling,
        }
    }
}

struct LayoutWorkspaceConfigBuilder {
    strategy: Strategy,
    name: String,
    master_ratio: Option<f32>,
    master_count: Option<usize>,
    master: Vec<WindowMatcher>,
    secondary: Vec<WindowMatcher>,
    master_display: PaneDisplay,
    secondary_display: PaneDisplay,
    tree: Option<TreeLayoutNode>,
    float: Vec<WindowMatcher>,
    fullscreen: Vec<WindowMatcher>,
}

impl LayoutWorkspaceConfigBuilder {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            strategy: Strategy::PartitionTree,
            master_ratio: None,
            master_count: None,
            master: vec![],
            secondary: vec![],
            master_display: PaneDisplay::Tiled,
            secondary_display: PaneDisplay::Tiled,
            tree: None,
            float: vec![],
            fullscreen: vec![],
        }
    }

    fn with_strategy(self, strategy: Strategy) -> Self {
        Self { strategy, ..self }
    }

    fn with_master_count(self, master_count: usize) -> Self {
        Self {
            master_count: Some(master_count),
            ..self
        }
    }

    fn with_master_ratio(self, master_ratio: f32) -> Self {
        Self {
            master_ratio: Some(master_ratio),
            ..self
        }
    }

    fn with_master(self, master: Vec<WindowMatcher>) -> Self {
        Self { master, ..self }
    }

    fn with_secondary(self, secondary: Vec<WindowMatcher>) -> Self {
        Self { secondary, ..self }
    }

    fn with_master_display(self, master_display: PaneDisplay) -> Self {
        Self {
            master_display,
            ..self
        }
    }

    fn with_secondary_display(self, secondary_display: PaneDisplay) -> Self {
        Self {
            secondary_display,
            ..self
        }
    }

    fn with_float(self, float: Vec<WindowMatcher>) -> Self {
        Self { float, ..self }
    }

    fn with_fullscreen(self, fullscreen: Vec<WindowMatcher>) -> Self {
        Self { fullscreen, ..self }
    }

    fn with_tree(self, tree: TreeLayoutNode) -> Self {
        Self {
            tree: Some(tree),
            ..self
        }
    }

    fn build(self) -> LayoutWorkspaceConfig {
        match self.strategy {
            Strategy::Master => LayoutWorkspaceConfig::Master {
                name: self.name,
                master_count: self.master_count,
                master_ratio: self.master_ratio,
                master: PaneConfig {
                    display: self.master_display,
                    children: self.master,
                },
                secondary: PaneConfig {
                    display: self.secondary_display,
                    children: self.secondary,
                },
                float: self.float,
                fullscreen: self.fullscreen,
            },
            Strategy::PartitionTree => LayoutWorkspaceConfig::PartitionTree {
                name: self.name,
                tree: self.tree,
                float: self.float,
                fullscreen: self.fullscreen,
            },
        }
    }
}

pub(super) fn setup_hub() -> Hub {
    TestHubBuilder::new().build()
}

pub(super) fn setup() -> Hub {
    setup_logger_with_level("warn");
    setup_hub()
}

/// `setup()` with a caller-supplied layout config. An open-coded
/// `TestHubBuilder` chain would lose the logger initialisation.
pub(super) fn setup_with_layout(layout: GlobalLayoutConfig) -> Hub {
    setup_logger_with_level("warn");
    TestHubBuilder::new().with_layout(layout).build()
}

pub(super) fn titled_matcher(title: &str) -> WindowMatcher {
    WindowMatcher {
        title: Some(title.to_string()),
        ..Default::default()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TestMetadata {
    pub title: Option<String>,
    pub process: Option<String>,
}

impl std::fmt::Display for TestMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title.as_deref().unwrap_or(""))
    }
}

impl WindowMetadata for TestMetadata {
    fn app_name(&self) -> Option<String> {
        None
    }
    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }
    fn clone_box(&self) -> Box<dyn WindowMetadata> {
        Box::new(self.clone())
    }

    fn matches_window_matcher(&self, matcher: &crate::config::WindowMatcher) -> bool {
        let title = self.title.as_deref();
        let process = self.process.as_deref();

        if let Some(_p) = matcher.app.as_deref() {
            return false;
        }
        if let Some(_p) = matcher.bundle_id.as_deref() {
            return false;
        }
        if let Some(_p) = matcher.class.as_deref() {
            return false;
        }
        if let Some(_p) = matcher.aumid.as_deref() {
            return false;
        }
        if let Some(p) = matcher.process.as_deref()
            && !process.is_some_and(|s| crate::config::pattern_matches(p, s))
        {
            return false;
        }
        if let Some(p) = matcher.title.as_deref()
            && !title.is_some_and(|t| crate::config::pattern_matches(p, t))
        {
            return false;
        }
        matcher.process.is_some() || matcher.title.is_some()
    }

    fn to_window_matcher(&self) -> crate::config::WindowMatcher {
        crate::config::WindowMatcher {
            title: self.title.clone(),
            process: self.process.clone(),
            ..Default::default()
        }
    }
}

/// Rect for test inserts where geometry is not under assertion. Tiling ignores it.
pub(crate) fn default_rect() -> PixelRect {
    PixelRect::new(0, 0, 100, 100)
}

/// Convenience: a 100x30 monitor work area at the given origin.
pub(super) fn work_area_at(x: i32, y: i32) -> PixelRect {
    PixelRect::new(x, y, 100, 30)
}

/// Convenience: create a boxed `TestMetadata` with the given title.
pub(crate) fn titled(t: &str) -> Box<dyn WindowMetadata> {
    Box::new(TestMetadata {
        title: Some(t.to_owned()),
        ..Default::default()
    })
}

pub(crate) fn process_meta(p: &str) -> Box<dyn WindowMetadata> {
    Box::new(TestMetadata {
        process: Some(p.into()),
        ..Default::default()
    })
}

pub(crate) fn titled_process(title: &str, process: &str) -> Box<dyn WindowMetadata> {
    Box::new(TestMetadata {
        title: Some(title.into()),
        process: Some(process.into()),
    })
}
