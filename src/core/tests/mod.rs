mod float_window;
mod focus_workspace;
mod fullscreen;
mod master;
mod minimize;
mod monitor;
mod move_to_workspace;
mod partition_tree;
mod preferred_layout;
mod query;
mod set_focus;
mod smoke;
mod strategy_switch;

use std::collections::HashSet;

use crate::config::{
    LayoutWorkspaceConfig, MasterConfig, PartitionTreeConfig, SizeConstraint, Strategy,
    TreeLayoutNode, WindowMatcher,
};
use crate::core::GlobalLayoutConfig;
use crate::core::allocator::NodeId;
use crate::core::hub::{Hub, MonitorLayout, SpawnIndicator};
use crate::core::node::{Dimension, Direction, Length, Logical, WindowId};
use crate::core::strategy::TilingAction;
use crate::core::{
    ContainerPlacement, FloatWindowPlacement, TilingWindowPlacement, WindowMetadata,
};

const ASCII_WIDTH: usize = 150;
const ASCII_HEIGHT: usize = 30;
const TAB_BAR_HEIGHT: f32 = 2.0;

pub(super) fn snapshot(hub: &Hub) -> String {
    validate_hub(hub);
    let mut s = snapshot_text(hub);

    // ASCII visualization uses screen coords from get_visible_placements
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
            let screen = hub.access.monitors.get(mp.monitor_id).dimension;
            draw_rect(
                &mut grid,
                screen.x.value(),
                screen.y.value(),
                screen.width.value(),
                screen.height.value(),
                &format!("W{}", id.get()),
                [false; 4],
            );
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

    // Draw tiling windows
    for wp in tiling_windows {
        let d = wp.visible_frame;
        let clip = clip_edges(wp.frame, wp.visible_frame);
        draw_rect(
            &mut grid,
            d.x.value(),
            d.y.value(),
            d.width.value(),
            d.height.value(),
            &format!("W{}", wp.id.get()),
            clip,
        );
    }

    // Draw tab bars
    for cp in containers {
        if cp.is_tabbed {
            let d = cp.visible_frame;
            draw_tab_bar(
                &mut grid,
                d.x.value(),
                d.y.value(),
                d.width.value(),
                &cp.titles,
                cp.active_tab_index,
            );
        }
    }

    // Draw focus border for non-float focused
    let focused_float = float_windows.iter().find(|p| p.is_highlighted);
    if focused_float.is_none() {
        if let Some(wp) = tiling_windows.iter().find(|p| p.is_highlighted) {
            let d = wp.visible_frame;
            let clip = clip_edges(wp.frame, wp.visible_frame);
            draw_focused_border(
                &mut grid,
                d.x.value(),
                d.y.value(),
                d.width.value(),
                d.height.value(),
                clip,
            );
        } else if let Some(cp) = containers.iter().find(|p| p.is_highlighted) {
            let d = cp.visible_frame;
            let clip = clip_edges(cp.frame, cp.visible_frame);
            draw_focused_border(
                &mut grid,
                d.x.value(),
                d.y.value(),
                d.width.value(),
                d.height.value(),
                clip,
            );
        }
    }

    // Draw float windows on top
    for wp in float_windows {
        let d = wp.visible_frame;
        let clip = clip_edges(wp.frame, wp.visible_frame);
        let grid_w = grid[0].len() as isize;
        let grid_h = grid.len() as isize;
        let x1 = d.x.round().value() as isize;
        let y1 = d.y.round().value() as isize;
        let x2 = (d.x + d.width).round().value() as isize - 1;
        let y2 = (d.y + d.height).round().value() as isize - 1;
        for row in (y1 + 1).max(0)..y2.min(grid_h) {
            for col in (x1 + 1).max(0)..x2.min(grid_w) {
                grid[row as usize][col as usize] = ' ';
            }
        }
        draw_rect(
            &mut grid,
            d.x.value(),
            d.y.value(),
            d.width.value(),
            d.height.value(),
            &format!("F{}", wp.id.get()),
            clip,
        );
    }

    // Draw focus border for float focused (on top of everything)
    if let Some(wp) = focused_float {
        let d = wp.visible_frame;
        let clip = clip_edges(wp.frame, wp.visible_frame);
        draw_focused_border(
            &mut grid,
            d.x.value(),
            d.y.value(),
            d.width.value(),
            d.height.value(),
            clip,
        );
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
    for mp in &vp.monitors {
        let screen = hub.access.monitors.get(mp.monitor_id).dimension;
        match &mp.layout {
            MonitorLayout::Normal {
                tiling_windows,
                float_windows,
                containers,
            } => {
                if tiling_windows.is_empty() && float_windows.is_empty() && containers.is_empty() {
                    s.push_str(&format!(
                        "  Monitor(id={}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}))\n",
                        mp.monitor_id, screen.x, screen.y, screen.width, screen.height
                    ));
                } else {
                    s.push_str(&format!(
                        "  Monitor(id={}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}),\n",
                        mp.monitor_id, screen.x, screen.y, screen.width, screen.height
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
                    "  Monitor(id={}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}),\n",
                    mp.monitor_id, screen.x, screen.y, screen.width, screen.height
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
    let d = wp.visible_frame;
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
    let d = wp.visible_frame;
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
    let d = cp.visible_frame;
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
fn draw_tab_bar(
    grid: &mut [Vec<char>],
    x: f32,
    y: f32,
    width: f32,
    labels: &[String],
    active: usize,
) {
    let x1 = x.round() as usize;
    let y1 = y.round() as usize;
    let y2 = y1 + TAB_BAR_HEIGHT as usize - 1;
    let x2 = (x + width).round() as usize - 1;
    let inner_width = x2 - x1 - 1;
    let tab_count = labels.len();

    // Draw top border
    for col in x1..=x2 {
        grid[y1][col] = '-';
    }
    grid[y1][x1] = '+';
    grid[y1][x2] = '+';

    // Draw side borders
    for row in (y1 + 1)..=y2 {
        grid[row][x1] = '|';
        grid[row][x2] = '|';
    }

    if tab_count == 0 {
        return;
    }

    // Draw tab labels evenly spread with separators (centered vertically in content area)
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

fn clip_edges(frame: Dimension, visible: Dimension) -> [bool; 4] {
    let half = Length::new(0.5);
    [
        visible.x > frame.x + half,
        (visible.x + visible.width) < (frame.x + frame.width) - half,
        visible.y > frame.y + half,
        (visible.y + visible.height) < (frame.y + frame.height) - half,
    ]
}

fn draw_rect(grid: &mut [Vec<char>], x: f32, y: f32, w: f32, h: f32, label: &str, clip: [bool; 4]) {
    let grid_w = grid[0].len() as isize;
    let grid_h = grid.len() as isize;
    let [clip_l, clip_r, clip_t, clip_b] = clip;

    let x1 = x.round() as isize;
    let y1 = y.round() as isize;
    let x2 = (x + w).round() as isize - 1;
    let y2 = (y + h).round() as isize - 1;

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

    let mid_x = (x + w / 2.0).round() as isize;
    let mid_y = (y + h / 2.0).round() as isize;
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

fn draw_focused_border(grid: &mut [Vec<char>], x: f32, y: f32, w: f32, h: f32, clip: [bool; 4]) {
    let grid_w = grid[0].len() as isize;
    let grid_h = grid.len() as isize;
    let [clip_l, clip_r, clip_t, clip_b] = clip;

    let x1 = x.round() as isize;
    let y1 = y.round() as isize;
    let x2 = (x + w).round() as isize - 1;
    let y2 = (y + h).round() as isize - 1;

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
    hub.validate_tree();
    validate_visible_placements(hub);
    validate_minimized(hub);
}

fn validate_visible_placements(hub: &Hub) {
    fn clip(dim: Dimension, bounds: Dimension) -> Option<Dimension> {
        let x1 = dim.x.max(bounds.x);
        let y1 = dim.y.max(bounds.y);
        let x2 = (dim.x + dim.width).min(bounds.x + bounds.width);
        let y2 = (dim.y + dim.height).min(bounds.y + bounds.height);
        if x1 >= x2 || y1 >= y2 {
            return None;
        }
        Some(Dimension::new(x1, y1, x2 - x1, y2 - y1))
    }

    let all_placements = hub.get_visible_placements();
    let mut seen_window_ids = HashSet::new();

    for mp in &all_placements.monitors {
        let screen = hub.access.monitors.get(mp.monitor_id).dimension;
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
                clip(wp.frame, screen),
                Some(wp.visible_frame),
                "Window {} visible_frame doesn't match clip(frame, screen)",
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
                clip(wp.frame, screen),
                Some(wp.visible_frame),
                "Window {} visible_frame doesn't match clip(frame, screen)",
                wp.id
            );
        }
        for cp in containers {
            assert_eq!(
                clip(cp.frame, screen),
                Some(cp.visible_frame),
                "Container {} visible_frame doesn't match clip(frame, screen)",
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
    // Converse: any window with workspace = None must be in minimized_windows.
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

/// Test convenience methods that wrap handle_tiling_action with the appropriate
/// TilingAction variant. Keeps test call sites readable (e.g. hub.focus_left()
/// instead of hub.handle_tiling_action(TilingAction::FocusDirection { ... })).
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

    /// Inserts a tiling window and seeds its title to `W<id>` so the ASCII
    /// tab bar in `snapshot()` (which reads `ContainerPlacement::titles`)
    /// shows readable per-tab labels. Call this instead of `insert_tiling` in
    /// `#[test]` functions whose inline snapshot contains `tabbed, active_tab=`.
    /// Non-tabbed tests should keep calling `insert_tiling` directly to avoid
    /// churning the `titles=[...]` textual line in their snapshots.
    pub(crate) fn insert_tiling_titled(&mut self) -> WindowId {
        let id = self.insert_tiling(self.current_workspace(), titled("w0"));
        self.set_window_title(id, format!("W{}", id.get()));
        id
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
    workspace_overrides: Vec<LayoutWorkspaceConfig>,
}

impl TestHubBuilder {
    fn new() -> Self {
        Self {
            layout: LayoutConfigBuilder::new().build(),
            workspace_overrides: Vec::new(),
        }
    }

    fn with_layout(self, layout: GlobalLayoutConfig) -> Self {
        Self { layout, ..self }
    }

    fn with_workspace_overrides(
        self,
        workspace_overrides: Vec<LayoutWorkspaceConfig>,
    ) -> Self {
        Self {
            workspace_overrides,
            ..self
        }
    }

    fn build(self) -> Hub {
        Hub::new(
            Dimension::new(
                Length::new(0.0),
                Length::new(0.0),
                Length::new(ASCII_WIDTH as f32),
                Length::new(ASCII_HEIGHT as f32),
            ),
            1.0,
            self.layout,
            self.workspace_overrides,
            Vec::new(),
        )
    }
}

struct LayoutConfigBuilder {
    strategy: Strategy,
    master: MasterConfig,
    partition_tree: PartitionTreeConfig,
    min_width: SizeConstraint,
    min_height: SizeConstraint,
    max_width: SizeConstraint,
    max_height: SizeConstraint,
    float: Vec<WindowMatcher>,
    fullscreen: Vec<WindowMatcher>,
}

impl LayoutConfigBuilder {
    fn new() -> Self {
        Self {
            strategy: Strategy::PartitionTree,
            master: MasterConfig {
                master_ratio: 0.5,
                master_count: 1,
            },
            partition_tree: PartitionTreeConfig {
                tab_bar_height: Length::<Logical>::new(TAB_BAR_HEIGHT),
                automatic_tiling: false,
            },
            min_width: SizeConstraint::Pixels(Length::new(1.0)),
            min_height: SizeConstraint::Pixels(Length::new(1.0)),
            max_width: SizeConstraint::Pixels(Length::new(0.0)),
            max_height: SizeConstraint::Pixels(Length::new(0.0)),
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

    fn with_min_width(self, min_width: SizeConstraint) -> Self {
        Self { min_width, ..self }
    }

    fn with_min_height(self, min_height: SizeConstraint) -> Self {
        Self { min_height, ..self }
    }

    fn with_partition_tree_config(self, partition_tree: PartitionTreeConfig) -> Self {
        Self {
            partition_tree,
            ..self
        }
    }

    fn with_max_width(self, max_width: SizeConstraint) -> Self {
        Self { max_width, ..self }
    }

    fn with_max_height(self, max_height: SizeConstraint) -> Self {
        Self { max_height, ..self }
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
            partition_tree: self.partition_tree,
            master: self.master,
            min_width: self.min_width,
            min_height: self.min_height,
            max_width: self.max_width,
            max_height: self.max_height,
            float: self.float,
            fullscreen: self.fullscreen,
        }
    }
}

struct PartitionTreeConfigBuilder {
    tab_bar_height: Length<Logical>,
    automatic_tiling: bool,
}

impl PartitionTreeConfigBuilder {
    fn new() -> Self {
        Self {
            tab_bar_height: Length::<Logical>::new(TAB_BAR_HEIGHT),
            automatic_tiling: false,
        }
    }

    fn with_tab_bar_height(self, tab_bar_height: Length<Logical>) -> Self {
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

    fn with_master(self, master: Vec<WindowMatcher>) -> Self {
        Self { master, ..self }
    }

    fn with_secondary(self, secondary: Vec<WindowMatcher>) -> Self {
        Self { secondary, ..self }
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
                master: self.master,
                secondary: self.secondary,
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

/// Minimal test metadata with no structure — title set via `titled` or
/// left blank.
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
    fn icon_key(&self) -> Option<String> {
        None
    }
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
        if let Some(ref title) = self.title
            && let Some(ref mtitle) = matcher.title
            && title == mtitle
        {
            return true;
        }
        if let Some(ref process) = self.process
            && let Some(ref mproc) = matcher.process
            && process == mproc
        {
            return true;
        }
        false
    }
}

/// Convenience: create a boxed `TestMetadata` with the given title.
pub(crate) fn titled(t: &str) -> Box<dyn WindowMetadata> {
    Box::new(TestMetadata {
        title: Some(t.to_owned()),
        ..Default::default()
    })
}
