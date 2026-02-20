#![allow(clippy::needless_range_loop)]

mod auto_tile;
mod delete_window;
mod float_window;
mod focus_direction;
mod focus_parent;
mod focus_workspace;
mod insert_window;
mod monitor;
mod move_in_direction;
mod move_to_workspace;
mod set_focus;
mod set_window_constraint;
mod smoke;
mod sync_config;
mod tabbed;
mod toggle_direction;
mod toggle_spawn_mode;
mod visible_placements;

use std::collections::HashSet;

use crate::config::SizeConstraint;
use crate::core::allocator::NodeId;
use crate::core::hub::{Hub, HubConfig};
use crate::core::node::{Child, Dimension, Direction, Parent, WindowId};

const ASCII_WIDTH: usize = 150;
const ASCII_HEIGHT: usize = 30;
const TAB_BAR_HEIGHT: f32 = 2.0;

pub(super) fn snapshot(hub: &Hub) -> String {
    validate_hub(hub);
    let mut s = snapshot_text(hub);

    // ASCII visualization uses screen coords from get_visible_placements
    let mut grid = vec![vec![' '; ASCII_WIDTH]; ASCII_HEIGHT];
    let all = hub.get_visible_placements();
    let mp = &all[0];

    // Draw tiling windows
    for wp in &mp.windows {
        if !wp.is_float {
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
    }

    // Draw tab bars
    for cp in &mp.containers {
        if cp.is_tabbed {
            let container = hub.get_container(cp.id);
            let labels: Vec<String> = container
                .children()
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
    let focused_float = mp.windows.iter().find(|p| p.is_focused && p.is_float);
    if focused_float.is_none() {
        if let Some(wp) = mp.windows.iter().find(|p| p.is_focused) {
            let d = wp.visible_frame;
            let clip = clip_edges(wp.frame, wp.visible_frame);
            draw_focused_border(&mut grid, d.x, d.y, d.width, d.height, clip);
        } else if let Some(cp) = mp.containers.iter().find(|p| p.is_focused) {
            let d = cp.visible_frame;
            let clip = clip_edges(cp.frame, cp.visible_frame);
            draw_focused_border(&mut grid, d.x, d.y, d.width, d.height, clip);
        }
    }

    // Draw float windows on top
    for wp in &mp.windows {
        if wp.is_float {
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
    let monitors = hub.all_monitors();
    let monitor_info = if monitors.len() > 1 {
        format!(", monitor={}", hub.focused_monitor())
    } else {
        String::new()
    };
    let screen = hub.get_monitor(hub.focused_monitor()).dimension();
    let mut s = format!(
        "Hub(focused={}{}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}),\n",
        hub.current_workspace(),
        monitor_info,
        screen.x,
        screen.y,
        screen.width,
        screen.height
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
    s
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
            let layout_info = if let Some(dir) = c.direction() {
                format!("direction={:?}", dir)
            } else {
                format!("tabbed=true, active_tab={}", c.active_tab_index())
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

fn fmt_float_str(hub: &Hub, s: &mut String, float_id: WindowId, indent: usize) {
    let prefix = "  ".repeat(indent);
    let f = hub.get_window(float_id);
    let dim = f.dimension();
    s.push_str(&format!(
        "{}Float(id={}, x={:.2}, y={:.2}, w={:.2}, h={:.2})\n",
        prefix, float_id, dim.x, dim.y, dim.width, dim.height
    ));
}

fn validate_hub(hub: &Hub) {
    // Validate monitors
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

    for (workspace_id, workspace) in hub.all_workspaces() {
        assert!(
            monitor_ids.contains(&workspace.monitor),
            "Workspace {workspace_id} has invalid monitor {}",
            workspace.monitor
        );
        match workspace.focused() {
            Some(Child::Window(wid)) => {
                let window = hub.get_window(wid);
                if window.is_float() {
                    assert!(
                        workspace.float_windows().contains(&wid),
                        "Workspace {workspace_id} focused on float {wid} but float not in workspace"
                    );
                } else if let Some(root) = workspace.root() {
                    match root {
                        Child::Window(_) => {
                            assert_eq!(
                                Child::Window(wid),
                                root,
                                "Workspace {workspace_id} focus ({wid:?}) doesn't match root ({root:?})"
                            );
                        }
                        Child::Container(cid) => {
                            let root_focus = hub.get_container(cid).focused;
                            assert!(
                                Child::Window(wid) == root || Child::Window(wid) == root_focus,
                                "Workspace {workspace_id} focus ({wid:?}) doesn't match root ({root:?}) or root's focus ({root_focus:?})"
                            );
                        }
                    }
                }
            }
            Some(Child::Container(cid)) => {
                if let Some(root) = workspace.root() {
                    match root {
                        Child::Window(_) => {
                            panic!(
                                "Workspace {workspace_id} focus is container {cid:?} but root is window"
                            );
                        }
                        Child::Container(root_cid) => {
                            let root_focus = hub.get_container(root_cid).focused;
                            assert!(
                                Child::Container(cid) == root
                                    || Child::Container(cid) == root_focus,
                                "Workspace {workspace_id} focus ({cid:?}) doesn't match root ({root:?}) or root's focus ({root_focus:?})"
                            );
                        }
                    }
                }
            }
            None => {}
        }

        // Validate all floats in workspace
        for &fid in workspace.float_windows() {
            let float = hub.get_window(fid);
            assert_eq!(
                float.workspace, workspace_id,
                "Float {fid} has wrong workspace"
            );
            assert_eq!(
                float.parent,
                Parent::Workspace(workspace_id),
                "Float {fid} has wrong parent"
            );
            assert!(
                float.is_float(),
                "Window {fid} in float_windows but mode is not Float"
            );
        }

        let Some(root) = workspace.root() else {
            continue;
        };
        let mut stack = vec![(root, Parent::Workspace(workspace_id))];
        for _ in super::bounded_loop() {
            let Some((child, expected_parent)) = stack.pop() else {
                break;
            };
            match child {
                Child::Window(wid) => {
                    let window = hub.get_window(wid);
                    assert!(!window.is_float(), "Window {wid} in tree but mode is Float");
                    assert_eq!(
                        window.parent, expected_parent,
                        "Window {wid} has wrong parent"
                    );
                    assert_eq!(
                        window.workspace, workspace_id,
                        "Window {wid} has wrong workspace"
                    );
                    // Validate window dimension >= min size
                    let dim = window.dimension();
                    let (min_w, min_h) = window.min_size();
                    assert!(
                        dim.width >= min_w - 0.01,
                        "Window {wid} width {:.2} < min_width {:.2}",
                        dim.width,
                        min_w
                    );
                    assert!(
                        dim.height >= min_h - 0.01,
                        "Window {wid} height {:.2} < min_height {:.2}",
                        dim.height,
                        min_h
                    );
                    // Validate window dimension <= max size (if set)
                    let (max_w, max_h) = window.max_size();
                    if max_w > 0.0 {
                        assert!(
                            dim.width <= max_w + 0.01,
                            "Window {wid} width {:.2} > max_width {:.2}",
                            dim.width,
                            max_w
                        );
                    }
                    if max_h > 0.0 {
                        assert!(
                            dim.height <= max_h + 0.01,
                            "Window {wid} height {:.2} > max_height {:.2}",
                            dim.height,
                            max_h
                        );
                    }
                    // Validate max >= min when both are set
                    if max_w > 0.0 {
                        assert!(
                            max_w >= min_w,
                            "Window {wid} max_width {:.2} < min_width {:.2}",
                            max_w,
                            min_w
                        );
                    }
                    if max_h > 0.0 {
                        assert!(
                            max_h >= min_h,
                            "Window {wid} max_height {:.2} < min_height {:.2}",
                            max_h,
                            min_h
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

                    // Validate container.focused is not a float
                    if let Child::Window(wid) = container.focused {
                        assert!(
                            !hub.get_window(wid).is_float(),
                            "Container {cid} focused on float {wid}"
                        );
                    }

                    if container.is_tabbed() {
                        assert!(
                            container.active_tab_index() < container.children().len(),
                            "Container {cid} active_tab out of bounds"
                        );
                        let active_tab = container.children()[container.active_tab_index()];
                        match active_tab {
                            Child::Window(_) => {
                                assert_eq!(
                                    container.focused, active_tab,
                                    "Container {cid} focused {:?} doesn't match active_tab {:?}",
                                    container.focused, active_tab
                                );
                            }
                            Child::Container(child_cid) => {
                                let child_focused = hub.get_container(child_cid).focused;
                                assert!(
                                    container.focused == child_focused
                                        || container.focused == active_tab,
                                    "Container {cid} focused {:?} doesn't match active_tab {:?} or its focused {:?}",
                                    container.focused,
                                    active_tab,
                                    child_focused
                                );
                            }
                        }
                    }

                    if let Parent::Container(parent_cid) = expected_parent
                        && let Some(parent_dir) = hub.get_container(parent_cid).direction()
                        && let Some(child_dir) = container.direction()
                    {
                        assert_ne!(
                            parent_dir, child_dir,
                            "Container {cid} has same direction as parent {parent_cid}"
                        );
                    }

                    // Validate container dimension = sum of children in layout direction
                    let dim = container.dimension();
                    let children = container.children();
                    let child_dims: Vec<_> = children
                        .iter()
                        .map(|&c| match c {
                            Child::Window(wid) => hub.get_window(wid).dimension(),
                            Child::Container(cid) => hub.get_container(cid).dimension(),
                        })
                        .collect();
                    let child_mins: Vec<_> = children
                        .iter()
                        .map(|&c| match c {
                            Child::Window(wid) => hub.get_window(wid).min_size(),
                            Child::Container(cid) => hub.get_container(cid).min_size(),
                        })
                        .collect();

                    match container.direction() {
                        Some(Direction::Horizontal) => {
                            let sum_width: f32 = child_dims.iter().map(|d| d.width).sum();
                            // With max_size, windows can be smaller than allocated space (centered)
                            // So sum of children widths <= container width
                            assert!(
                                sum_width <= dim.width + 0.01,
                                "Container {cid} children total width {:.2} > container width {:.2}",
                                sum_width,
                                dim.width
                            );
                            for (i, (d, (_, min_h))) in
                                child_dims.iter().zip(child_mins.iter()).enumerate()
                            {
                                // Child height can be smaller if max_height is set
                                let child_max_h = match children[i] {
                                    Child::Window(wid) => hub.get_window(wid).max_size().1,
                                    Child::Container(_) => 0.0,
                                };
                                let allows_smaller = child_max_h > 0.0 && child_max_h < dim.height;
                                assert!(
                                    d.height >= dim.height - 0.01
                                        || d.height >= *min_h - 0.01
                                        || allows_smaller,
                                    "Container {cid} child {i} height {:.2} < container height {:.2} and < min_height {:.2}",
                                    d.height,
                                    dim.height,
                                    min_h
                                );
                            }
                        }
                        Some(Direction::Vertical) => {
                            let sum_height: f32 = child_dims.iter().map(|d| d.height).sum();
                            // With max_size, windows can be smaller than allocated space (centered)
                            assert!(
                                sum_height <= dim.height + 0.01,
                                "Container {cid} children total height {:.2} > container height {:.2}",
                                sum_height,
                                dim.height
                            );
                            for (i, (d, (min_w, _))) in
                                child_dims.iter().zip(child_mins.iter()).enumerate()
                            {
                                // Child width can be smaller if max_width is set
                                let child_max_w = match children[i] {
                                    Child::Window(wid) => hub.get_window(wid).max_size().0,
                                    Child::Container(_) => 0.0,
                                };
                                let allows_smaller = child_max_w > 0.0 && child_max_w < dim.width;
                                assert!(
                                    d.width >= dim.width - 0.01
                                        || d.width >= *min_w - 0.01
                                        || allows_smaller,
                                    "Container {cid} child {i} width {:.2} < container width {:.2} and < min_width {:.2}",
                                    d.width,
                                    dim.width,
                                    min_w
                                );
                            }
                        }
                        None => {
                            // Tabbed: children can be smaller if max_size is set
                            let expected_height = dim.height - TAB_BAR_HEIGHT;
                            for (i, d) in child_dims.iter().enumerate() {
                                let (child_max_w, child_max_h) = match children[i] {
                                    Child::Window(wid) => hub.get_window(wid).max_size(),
                                    Child::Container(_) => (0.0, 0.0),
                                };
                                let allows_smaller_w = child_max_w > 0.0 && child_max_w < dim.width;
                                let allows_smaller_h =
                                    child_max_h > 0.0 && child_max_h < expected_height;
                                assert!(
                                    (d.width - dim.width).abs() < 0.01 || allows_smaller_w,
                                    "Container {cid} tabbed child {i} width {:.2} != container width {:.2}",
                                    d.width,
                                    dim.width
                                );
                                assert!(
                                    (d.height - expected_height).abs() < 0.01 || allows_smaller_h,
                                    "Container {cid} tabbed child {i} height {:.2} != expected {:.2}",
                                    d.height,
                                    expected_height
                                );
                            }
                        }
                    }

                    // Validate container dimension >= min size
                    let (min_w, min_h) = container.min_size();
                    assert!(
                        dim.width >= min_w - 0.01,
                        "Container {cid} width {:.2} < min_width {:.2}",
                        dim.width,
                        min_w
                    );
                    assert!(
                        dim.height >= min_h - 0.01,
                        "Container {cid} height {:.2} < min_height {:.2}",
                        dim.height,
                        min_h
                    );

                    // A container's focus must either match a child's focus or point directly to a child
                    let focused = container.focused;
                    let is_direct_child = container.children().contains(&focused);
                    let matches_child_focus = container.children().iter().any(|&c| {
                        if let Child::Container(child_cid) = c {
                            hub.get_container(child_cid).focused == focused
                        } else {
                            false
                        }
                    });
                    assert!(
                        is_direct_child || matches_child_focus,
                        "Container {cid} focus {focused:?} is neither a direct child nor matches a child's focus"
                    );

                    validate_child_exists(hub, container.focused);

                    for &c in container.children() {
                        stack.push((c, Parent::Container(cid)));
                    }
                }
            }
        }
    }

    // Validate visible placements
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

    for mp in &all_placements {
        let screen = hub.get_monitor(mp.monitor_id).dimension();

        for wp in &mp.windows {
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

        for cp in &mp.containers {
            assert_eq!(
                clip(cp.frame, screen),
                Some(cp.visible_frame),
                "Container {} visible_frame doesn't match clip(frame, screen)",
                cp.id
            );
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
    setup_logger_with_level("warn");
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
