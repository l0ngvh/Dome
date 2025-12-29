#![allow(clippy::needless_range_loop)]

mod border;
mod delete_window;
mod float_window;
mod focus_direction;
mod focus_parent;
mod focus_workspace;
mod insert_window;
mod move_in_direction;
mod move_to_workspace;
mod set_focus;
mod tabbed;
mod toggle_spawn_direction;
mod window_at;

use crate::core::allocator::NodeId;
use crate::core::hub::Hub;
use crate::core::node::{Child, ContainerId, FloatWindowId, Focus, Parent, WorkspaceId};

const ASCII_WIDTH: usize = 150;
const ASCII_HEIGHT: usize = 30;
const BORDER: f32 = 1.0;
const TAB_BAR_HEIGHT: f32 = 2.0;

pub(super) fn snapshot(hub: &Hub) -> String {
    validate_hub(hub);
    let mut s = format!(
        "Hub(focused={}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}),\n",
        hub.current_workspace(),
        hub.screen().x,
        hub.screen().y,
        hub.screen().width,
        hub.screen().height
    );
    for (workspace_id, workspace) in hub.all_workspaces() {
        let focused = if let Some(current) = workspace.focused {
            format!(", focused={}", current)
        } else {
            String::new()
        };
        let has_content = workspace.root().is_some() || !workspace.float_windows().is_empty();
        if !has_content {
            s.push_str(&format!(
                "  Workspace(id={}, name={}{})\n",
                workspace_id, workspace.name, focused
            ));
        } else {
            s.push_str(&format!(
                "  Workspace(id={}, name={}{},\n",
                workspace_id, workspace.name, focused
            ));
            if let Some(root) = workspace.root() {
                fmt_child_str(hub, &mut s, root, 2);
            }
            for &float_id in workspace.float_windows() {
                fmt_float_str(hub, &mut s, float_id, 2);
            }
            s.push_str("  )\n");
        }
    }
    s.push_str(")\n");

    // ASCII visualization
    let mut grid = vec![vec![' '; ASCII_WIDTH]; ASCII_HEIGHT];
    let workspace = hub.get_workspace(hub.current_workspace());
    let focused = workspace.focused();

    if let Some(root) = workspace.root() {
        draw_windows(hub, &mut grid, root, BORDER);
    }

    // Draw float windows
    for &float_id in workspace.float_windows() {
        draw_float(hub, &mut grid, float_id, BORDER);
    }

    match focused {
        Some(Focus::Tiling(Child::Window(id))) => {
            let dim = hub.get_window(id).dimension();
            draw_focused_border(
                &mut grid,
                dim.x - BORDER,
                dim.y - BORDER,
                dim.width + 2.0 * BORDER,
                dim.height + 2.0 * BORDER,
            );
        }
        Some(Focus::Tiling(Child::Container(id))) => {
            let dim = hub.get_container(id).dimension();
            draw_focused_border(&mut grid, dim.x, dim.y, dim.width, dim.height);
        }
        Some(Focus::Float(id)) => {
            let dim = hub.get_float(id).dimension();
            draw_focused_border(
                &mut grid,
                dim.x - BORDER,
                dim.y - BORDER,
                dim.width + 2.0 * BORDER,
                dim.height + 2.0 * BORDER,
            );
        }
        None => {}
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

fn draw_float(hub: &Hub, grid: &mut [Vec<char>], float_id: FloatWindowId, border: f32) {
    let float = hub.get_float(float_id);
    let dim = float.dimension();
    draw_rect(
        grid,
        dim.x - border,
        dim.y - border,
        dim.width + 2.0 * border,
        dim.height + 2.0 * border,
        float.title(),
    );
}

fn draw_windows(hub: &Hub, grid: &mut [Vec<char>], child: Child, border: f32) {
    match child {
        Child::Window(id) => {
            let dim = hub.get_window(id).dimension();
            draw_rect(
                grid,
                dim.x - border,
                dim.y - border,
                dim.width + 2.0 * border,
                dim.height + 2.0 * border,
                &format!("W{}", id.get()),
            );
        }
        Child::Container(id) => {
            let c = hub.get_container(id);
            if c.is_tabbed() {
                let dim = c.dimension();
                let tab_labels: Vec<String> = c
                    .children()
                    .iter()
                    .map(|child| match child {
                        Child::Window(wid) => format!("W{}", wid.get()),
                        Child::Container(cid) => format!("C{}", cid.get()),
                    })
                    .collect();
                draw_tab_bar(grid, dim.x, dim.y, dim.width, &tab_labels, c.active_tab());

                if let Some(&active) = c.children().get(c.active_tab()) {
                    draw_windows(hub, grid, active, border);
                }
            } else {
                for &child in c.children() {
                    draw_windows(hub, grid, child, border);
                }
            }
        }
    }
}

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

fn draw_rect(grid: &mut [Vec<char>], x: f32, y: f32, w: f32, h: f32, label: &str) {
    let x1 = x.round() as usize;
    let y1 = y.round() as usize;
    let x2 = (x + w).round() as usize - 1;
    let y2 = (y + h).round() as usize - 1;

    for col in x1..=x2 {
        grid[y1][col] = '-';
        grid[y2][col] = '-';
    }
    for row in y1..=y2 {
        grid[row][x1] = '|';
        grid[row][x2] = '|';
    }
    grid[y1][x1] = '+';
    grid[y1][x2] = '+';
    grid[y2][x1] = '+';
    grid[y2][x2] = '+';

    let mid_x = (x + w / 2.0).round() as usize;
    let mid_y = (y + h / 2.0).round() as usize;
    let start_x = mid_x.saturating_sub(label.len() / 2);
    for (i, ch) in label.chars().enumerate() {
        let col = start_x + i;
        if col > x1 && col < x2 {
            grid[mid_y][col] = ch;
        }
    }
}

fn draw_focused_border(grid: &mut [Vec<char>], x: f32, y: f32, w: f32, h: f32) {
    let x1 = x.round() as usize;
    let y1 = y.round() as usize;
    let x2 = (x + w).round() as usize - 1;
    let y2 = (y + h).round() as usize - 1;

    for col in x1..=x2 {
        grid[y1][col] = '*';
        grid[y2][col] = '*';
    }
    for row in y1..=y2 {
        grid[row][x1] = '*';
        grid[row][x2] = '*';
    }
}

fn fmt_child_str(hub: &Hub, s: &mut String, child: Child, indent: usize) {
    let prefix = "  ".repeat(indent);
    match child {
        Child::Window(id) => {
            let w = hub.get_window(id);
            let dim = w.dimension();
            s.push_str(&format!(
                "{}Window(id={}, parent={}, x={:.2}, y={:.2}, w={:.2}, h={:.2})\n",
                prefix, id, w.parent, dim.x, dim.y, dim.width, dim.height
            ));
        }
        Child::Container(id) => {
            let c = hub.get_container(id);
            let layout_info = if c.is_tabbed() {
                format!("tabbed=true, active_tab={}", c.active_tab())
            } else {
                format!("direction={:?}", c.direction)
            };
            s.push_str(&format!(
                "{}Container(id={}, parent={}, x={:.2}, y={:.2}, w={:.2}, h={:.2}, {},\n",
                prefix,
                id,
                c.parent,
                c.dimension.x,
                c.dimension.y,
                c.dimension.width,
                c.dimension.height,
                layout_info,
            ));
            for &child in c.children() {
                fmt_child_str(hub, s, child, indent + 1);
            }
            s.push_str(&format!("{})\n", prefix));
        }
    }
}

fn fmt_float_str(hub: &Hub, s: &mut String, float_id: FloatWindowId, indent: usize) {
    let prefix = "  ".repeat(indent);
    let f = hub.get_float(float_id);
    let dim = f.dimension();
    s.push_str(&format!(
        "{}Float(id={}, title=\"{}\", x={:.2}, y={:.2}, w={:.2}, h={:.2})\n",
        prefix,
        float_id,
        f.title(),
        dim.x,
        dim.y,
        dim.width,
        dim.height
    ));
}

fn validate_hub(hub: &Hub) {
    for (workspace_id, workspace) in hub.all_workspaces() {
        if let Some(Focus::Tiling(child)) = workspace.focused() {
            validate_child_exists(hub, child);
        }
        let Some(root) = workspace.root() else {
            continue;
        };
        let mut stack = vec![(root, Parent::Workspace(workspace_id))];
        let mut iterations = 0;
        while let Some((child, expected_parent)) = stack.pop() {
            iterations += 1;
            if iterations > 10000 {
                panic!("validate_hub: cycle detected");
            }
            match child {
                Child::Window(wid) => {
                    let window = hub.get_window(wid);
                    assert_eq!(
                        window.parent, expected_parent,
                        "Window {wid} has wrong parent"
                    );
                    assert_eq!(
                        window.workspace, workspace_id,
                        "Window {wid} has wrong workspace"
                    );
                    for &cid in &window.focused_by {
                        let container = hub.get_container(cid);
                        assert_eq!(
                            container.focused, child,
                            "Window {wid} focused_by {cid} but container doesn't focus it"
                        );
                    }
                }
                Child::Container(cid) => {
                    let container = hub.get_container(cid);
                    assert_eq!(
                        container.parent, expected_parent,
                        "Container {cid} has wrong parent"
                    );
                    assert_eq!(
                        container.workspace, workspace_id,
                        "Container {cid} has wrong workspace"
                    );
                    assert!(
                        container.children.len() >= 2,
                        "Container {cid} has less than 2 children"
                    );

                    if container.is_tabbed() {
                        assert!(
                            container.active_tab() < container.children().len(),
                            "Container {cid} active_tab out of bounds"
                        );
                    }

                    if let Parent::Container(parent_cid) = expected_parent {
                        let parent = hub.get_container(parent_cid);
                        assert_ne!(
                            parent.direction, container.direction,
                            "Container {cid} has same direction as parent {parent_cid}"
                        );
                    }

                    let focused_title = match container.focused {
                        Child::Window(wid) => hub.get_window(wid).title().to_string(),
                        Child::Container(ccid) => hub.get_container(ccid).title().to_string(),
                    };
                    assert_eq!(
                        container.title(),
                        focused_title,
                        "Container {cid} title doesn't match focused child's title"
                    );

                    validate_child_exists(hub, container.focused);

                    for &cid_focusing in &container.focused_by {
                        let c = hub.get_container(cid_focusing);
                        assert_eq!(
                            c.focused, child,
                            "Container {cid} focused_by {cid_focusing} but that container doesn't focus it"
                        );
                    }

                    for &c in container.children() {
                        stack.push((c, Parent::Container(cid)));
                    }
                }
            }
        }
    }
}

fn validate_child_exists(hub: &Hub, child: Child) {
    match child {
        Child::Window(wid) => {
            hub.get_window(wid);
        }
        Child::Container(cid) => {
            hub.get_container(cid);
        }
    }
}

fn setup_logger() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .try_init();
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));
}

pub(super) fn setup() -> Hub {
    setup_logger();
    use crate::core::node::Dimension;
    Hub::new(
        Dimension {
            x: 0.0,
            y: 0.0,
            width: ASCII_WIDTH as f32,
            height: ASCII_HEIGHT as f32,
        },
        BORDER,
        TAB_BAR_HEIGHT,
    )
}
