mod float_window;
mod focus_workspace;
mod fullscreen;
mod master_stack;
mod monitor;
mod move_to_workspace;
mod partition_tree;
mod query;
mod set_focus;
mod smoke;

use std::collections::HashSet;

use crate::config::SizeConstraint;
use crate::core::allocator::NodeId;
use crate::core::hub::{Hub, HubConfig, MonitorLayout, SpawnIndicator};
use crate::core::node::{Dimension, Direction, Workspace, WorkspaceId};
use crate::core::partition_tree::Child;
use crate::core::strategy::TilingAction;

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
                screen.x,
                screen.y,
                screen.width,
                screen.height,
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
            d.x,
            d.y,
            d.width,
            d.height,
            &format!("W{}", wp.id.get()),
            clip,
        );
    }

    // Draw tab bars
    for cp in containers {
        if cp.is_tabbed {
            let labels: Vec<String> = cp
                .children
                .iter()
                .map(|child| match child {
                    Child::Window(wid) => format!("W{}", wid.get()),
                    Child::Container(cid) => format!("C{}", cid.get()),
                })
                .collect();
            let d = cp.visible_frame;
            draw_tab_bar(&mut grid, d.x, d.y, d.width, &labels, cp.active_tab_index);
        }
    }

    // Draw focus border for non-float focused
    let focused_float = float_windows.iter().find(|p| p.is_highlighted);
    if focused_float.is_none() {
        if let Some(wp) = tiling_windows.iter().find(|p| p.is_highlighted) {
            let d = wp.visible_frame;
            let clip = clip_edges(wp.frame, wp.visible_frame);
            draw_focused_border(&mut grid, d.x, d.y, d.width, d.height, clip);
        } else if let Some(cp) = containers.iter().find(|p| p.is_highlighted) {
            let d = cp.visible_frame;
            let clip = clip_edges(cp.frame, cp.visible_frame);
            draw_focused_border(&mut grid, d.x, d.y, d.width, d.height, clip);
        }
    }

    // Draw float windows on top
    for wp in float_windows {
        let d = wp.visible_frame;
        let clip = clip_edges(wp.frame, wp.visible_frame);
        let grid_w = grid[0].len() as isize;
        let grid_h = grid.len() as isize;
        let x1 = d.x.round() as isize;
        let y1 = d.y.round() as isize;
        let x2 = (d.x + d.width).round() as isize - 1;
        let y2 = (d.y + d.height).round() as isize - 1;
        for row in (y1 + 1).max(0)..y2.min(grid_h) {
            for col in (x1 + 1).max(0)..x2.min(grid_w) {
                grid[row as usize][col as usize] = ' ';
            }
        }
        draw_rect(
            &mut grid,
            d.x,
            d.y,
            d.width,
            d.height,
            &format!("F{}", wp.id.get()),
            clip,
        );
    }

    // Draw focus border for float focused (on top of everything)
    if let Some(wp) = focused_float {
        let d = wp.visible_frame;
        let clip = clip_edges(wp.frame, wp.visible_frame);
        draw_focused_border(&mut grid, d.x, d.y, d.width, d.height, clip);
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
    s
}

/// Formats the full Hub state using its Debug impl, for smoke test error diagnostics.
/// Unlike snapshot_text (which only shows visible placements), this dumps everything
/// including non-focused workspaces.
pub(super) fn hub_debug_text(hub: &Hub) -> String {
    format!("{:#?}", hub)
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

fn fmt_tiling_placement(wp: &crate::core::hub::TilingWindowPlacement) -> String {
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

fn fmt_float_placement(wp: &crate::core::hub::FloatWindowPlacement) -> String {
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

fn fmt_container_placement(cp: &crate::core::hub::ContainerPlacement) -> String {
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
    [
        visible.x > frame.x + 0.5,
        (visible.x + visible.width) < (frame.x + frame.width) - 0.5,
        visible.y > frame.y + 0.5,
        (visible.y + visible.height) < (frame.y + frame.height) - 0.5,
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
    validate_monitors(hub);
    let monitor_ids: Vec<_> = hub.all_monitors().iter().map(|(id, _)| *id).collect();

    for (workspace_id, workspace) in hub.all_workspaces() {
        assert!(
            monitor_ids.contains(&workspace.monitor),
            "Workspace {workspace_id} has invalid monitor {}",
            workspace.monitor
        );
        validate_floats(hub, workspace_id, &workspace);
        validate_fullscreens(hub, workspace_id, &workspace);
    }

    hub.validate_tree();
    validate_visible_placements(hub);
}

fn validate_monitors(hub: &Hub) {
    let monitors = hub.all_monitors();
    let monitor_ids: Vec<_> = monitors.iter().map(|(id, _)| *id).collect();
    assert!(
        monitor_ids.contains(&hub.focused_monitor()),
        "Focused monitor {} not in monitors",
        hub.focused_monitor()
    );
    for (monitor_id, monitor) in &monitors {
        let ws = hub.get_workspace(monitor.active_workspace);
        assert_eq!(
            ws.monitor, *monitor_id,
            "Monitor {monitor_id} active_workspace {} points to workspace with monitor {}",
            monitor.active_workspace, ws.monitor
        );
    }
}

fn validate_floats(hub: &Hub, workspace_id: WorkspaceId, workspace: &Workspace) {
    for &(fid, _) in workspace.float_windows() {
        let float = hub.get_window(fid);
        assert_eq!(
            float.workspace, workspace_id,
            "Float {fid} has wrong workspace"
        );
        assert!(
            float.is_float(),
            "Window {fid} in float_windows but mode is not Float"
        );
    }
}

fn validate_fullscreens(hub: &Hub, workspace_id: WorkspaceId, workspace: &Workspace) {
    for &fid in workspace.fullscreen_windows() {
        let window = hub.get_window(fid);
        assert_eq!(
            window.workspace, workspace_id,
            "Fullscreen {fid} has wrong workspace"
        );
        assert!(
            window.is_fullscreen(),
            "Window {fid} in fullscreen_windows but mode is not Fullscreen"
        );
    }
    if let Some(&top) = workspace.fullscreen_windows().last() {
        assert_eq!(
            workspace.focused_non_tiling(),
            Some(top),
            "Workspace {workspace_id} has fullscreen windows but focus is not on topmost fullscreen window {top}"
        );
    }
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
        Some(Dimension {
            x: x1,
            y: y1,
            width: x2 - x1,
            height: y2 - y1,
        })
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

fn setup_logger() {
    setup_logger_with_level("warn");
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
}

pub(super) fn setup_logger_with_level(level: &str) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            tab_bar_height: TAB_BAR_HEIGHT,
            auto_tile: false,
            min_width: SizeConstraint::Pixels(0.0),
            min_height: SizeConstraint::Pixels(0.0),
            max_width: SizeConstraint::Pixels(0.0),
            max_height: SizeConstraint::Pixels(0.0),
        }
    }
}

pub(super) fn setup_hub() -> Hub {
    Hub::new(
        Dimension {
            x: 0.0,
            y: 0.0,
            width: ASCII_WIDTH as f32,
            height: ASCII_HEIGHT as f32,
        },
        HubConfig::default(),
    )
}

pub(super) fn setup() -> Hub {
    setup_logger();
    setup_hub()
}

pub(super) fn setup_with_auto_tile() -> Hub {
    setup_logger();
    Hub::new(
        Dimension {
            x: 0.0,
            y: 0.0,
            width: ASCII_WIDTH as f32,
            height: ASCII_HEIGHT as f32,
        },
        HubConfig {
            auto_tile: true,
            ..Default::default()
        },
    )
}
