#![allow(clippy::needless_range_loop)]

mod border;
mod delete_window;
mod focus_direction;
mod focus_parent;
mod focus_workspace;
mod insert_window;
mod move_to_workspace;
mod toggle_new_window_direction;

use crate::core::allocator::NodeId;
use crate::core::hub::Hub;
use crate::core::node::Child;

const ASCII_WIDTH: usize = 150;
const ASCII_HEIGHT: usize = 30;
const BORDER: f32 = 1.0;

pub(super) fn snapshot(hub: &Hub) -> String {
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
        if workspace.root().is_none() {
            s.push_str(&format!(
                "  Workspace(id={}, name={}{})\n",
                workspace_id, workspace.name, focused
            ));
        } else {
            s.push_str(&format!(
                "  Workspace(id={}, name={}{},\n",
                workspace_id, workspace.name, focused
            ));
            fmt_child_str(hub, &mut s, workspace.root().unwrap(), 2);
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

    if let Some(child) = focused {
        match child {
            Child::Window(id) => {
                let dim = hub.get_window(id).dimension();
                draw_focused_border(
                    &mut grid,
                    dim.x - BORDER,
                    dim.y - BORDER,
                    dim.width + 2.0 * BORDER,
                    dim.height + 2.0 * BORDER,
                );
            }
            Child::Container(id) => {
                let dim = hub.get_container(id).dimension();
                draw_focused_border(&mut grid, dim.x, dim.y, dim.width, dim.height);
            }
        }
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
            for &c in hub.get_container(id).children() {
                draw_windows(hub, grid, c, border);
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
            s.push_str(&format!(
                    "{}Container(id={}, parent={}, x={:.2}, y={:.2}, w={:.2}, h={:.2}, direction={:?},\n",
                    prefix,
                    id,
                    c.parent,
                    c.dimension.x,
                    c.dimension.y,
                    c.dimension.width,
                    c.dimension.height,
                    c.direction,
                ));
            for &child in c.children() {
                fmt_child_str(hub, s, child, indent + 1);
            }
            s.push_str(&format!("{})\n", prefix));
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
    )
}
