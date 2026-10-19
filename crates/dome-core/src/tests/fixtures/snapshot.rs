use std::collections::BTreeMap;

use super::effects::RecordingEffects;
use crate::action::{MonitorDetails, WorkspaceInfo, WorkspaceState};
use crate::core::MonitorLayout;
use crate::core::{ContainerPlacement, Direction, MonitorId, PixelRect, WindowId};
use crate::core::{MinimizedWindowEntry, MonitorPlacements, VisiblePlacements};

pub fn of_hub(
    hub: &crate::core::Hub,
    effects: &RecordingEffects,
    modes: &[String],
    labels: &crate::tests::fixtures::window_labels::WindowLabels,
) -> String {
    let monitors = hub
        .query_monitors()
        .into_iter()
        .map(|details| {
            let id = hub
                .monitor_id_by_disambiguated_name(&details.unique_name)
                .expect("queried monitor");
            (id, details)
        })
        .collect::<Vec<_>>();
    render(
        &hub.get_visible_placements(),
        &hub.query_workspaces(),
        &monitors,
        &hub.minimized_window_entries(),
        effects,
        modes,
        labels.entries(),
    )
}

fn render(
    placements: &VisiblePlacements,
    workspaces: &[WorkspaceInfo],
    monitors: &[(MonitorId, MonitorDetails)],
    minimized: &[MinimizedWindowEntry],
    effects: &RecordingEffects,
    modes: &[String],
    labels: &BTreeMap<WindowId, String>,
) -> String {
    let paired: Vec<(&MonitorDetails, &MonitorPlacements)> = monitors
        .iter()
        .map(|(id, details)| (details, placements_on(placements, *id)))
        .collect();
    let mut lines = vec![
        focused_line(placements, &paired, labels),
        workspaces_line(workspaces),
    ];
    lines.extend(minimized_line(minimized, labels));
    lines.extend(effects_line(effects, modes, labels));
    for (details, monitor) in &paired {
        lines.push(monitor_line(details, monitor, labels));
        lines.extend(
            placement_lines(monitor, labels)
                .into_iter()
                .map(|line| format!("  {line}")),
        );
    }
    let grids: Vec<String> = paired
        .iter()
        .filter(|(_, monitor)| !monitor.work_area.is_empty())
        .map(|(_, monitor)| grid(monitor, labels))
        .collect();
    format!("{}\n{}", lines.join("\n"), grids.join("\n\n"))
}

fn placements_on(placements: &VisiblePlacements, id: MonitorId) -> &MonitorPlacements {
    placements
        .monitors
        .iter()
        .find(|monitor| monitor.monitor_id == id)
        .expect("queried monitor should have placements")
}

fn focused_line(
    placements: &VisiblePlacements,
    paired: &[(&MonitorDetails, &MonitorPlacements)],
    labels: &BTreeMap<WindowId, String>,
) -> String {
    let window = placements
        .focused_window
        .map_or_else(|| "none".to_owned(), |id| labels[&id].clone());
    let (details, _) = paired
        .iter()
        .find(|(_, monitor)| monitor.monitor_id == placements.focused_monitor)
        .expect("focused monitor should have placements");
    format!("focused {window} on {}", details.unique_name)
}

fn workspaces_line(workspaces: &[WorkspaceInfo]) -> String {
    let entries: Vec<String> = workspaces
        .iter()
        .map(|workspace| {
            let location = match workspace.state {
                WorkspaceState::Attached => "on",
                WorkspaceState::Parked => "parked from",
            };
            let marker = if workspace.is_focused {
                " (focused)"
            } else if workspace.is_visible {
                " (visible)"
            } else {
                ""
            };
            format!(
                "{} {location} {}{marker}",
                workspace.name, workspace.monitor
            )
        })
        .collect();
    format!("workspaces: {}", entries.join(", "))
}

fn minimized_line(
    minimized: &[MinimizedWindowEntry],
    labels: &BTreeMap<WindowId, String>,
) -> Option<String> {
    if minimized.is_empty() {
        return None;
    }
    let entries: Vec<String> = minimized
        .iter()
        .map(|entry| format!("{} {:?}", labels[&entry.id], entry.title))
        .collect();
    Some(format!("minimized: {}", entries.join(", ")))
}

fn effects_line(
    effects: &RecordingEffects,
    modes: &[String],
    labels: &BTreeMap<WindowId, String>,
) -> Option<String> {
    let mut entries: Vec<String> = effects
        .closed
        .iter()
        .map(|id| format!("close {}", labels[id]))
        .collect();
    entries.extend(
        effects
            .executed
            .iter()
            .map(|command| format!("execute {command:?}")),
    );
    entries.extend(modes.iter().map(|mode| format!("mode {mode}")));
    if effects.exited {
        entries.push("exit".to_owned());
    }
    entries.extend(
        effects
            .unminimized
            .iter()
            .map(|id| format!("unminimize {}", labels[id])),
    );
    (!entries.is_empty()).then(|| format!("effects: {}", entries.join(", ")))
}

fn monitor_line(
    details: &MonitorDetails,
    monitor: &MonitorPlacements,
    labels: &BTreeMap<WindowId, String>,
) -> String {
    let mut line = format!(
        "{} {} border {}",
        details.unique_name,
        rectangle(monitor.work_area),
        monitor.border_thickness.value()
    );
    if let MonitorLayout::Fullscreen(id) = monitor.layout {
        line.push_str(&format!(" fullscreen {}", labels[&id]));
    }
    if let Some(id) = details.cg_display_id {
        line.push_str(&format!(" cg_display_id {id}"));
    }
    if let Some(device) = &details.gdi_device {
        line.push_str(&format!(" gdi_device {device}"));
    }
    line
}

fn rectangle(rect: PixelRect) -> String {
    format!(
        "{},{} {}x{}",
        rect.x().value(),
        rect.y().value(),
        rect.width().value(),
        rect.height().value()
    )
}

fn markers(highlighted: bool, spawn: Option<Direction>) -> String {
    let mut result = String::new();
    if highlighted {
        result.push_str(" highlighted");
    }
    if let Some(direction) = spawn {
        result.push_str(match direction {
            Direction::Horizontal => " spawn=right",
            Direction::Vertical => " spawn=bottom",
        });
    }
    result
}

fn placement_lines(
    monitor: &MonitorPlacements,
    labels: &BTreeMap<WindowId, String>,
) -> Vec<String> {
    let MonitorLayout::Normal {
        tiling_windows,
        float_windows,
        containers,
    } = &monitor.layout
    else {
        return Vec::new();
    };
    let mut windows: Vec<_> = tiling_windows
        .iter()
        .map(|window| {
            let off_screen = if window.is_partially_off_screen() {
                " off-screen"
            } else {
                ""
            };
            (
                &labels[&window.id],
                format!(
                    "{} {}{}{off_screen}",
                    labels[&window.id],
                    rectangle(window.border_box),
                    markers(window.is_highlighted, window.spawn_direction)
                ),
            )
        })
        .collect();
    windows.extend(float_windows.iter().map(|window| {
        (
            &labels[&window.id],
            format!(
                "{} {} float{}",
                labels[&window.id],
                rectangle(window.border_box),
                markers(window.is_highlighted, None)
            ),
        )
    }));
    windows.sort_by_key(|(label, _)| *label);
    let mut lines: Vec<_> = windows.into_iter().map(|(_, line)| line).collect();
    let mut container_lines = Vec::new();
    for container in containers
        .iter()
        .filter(|container| container.is_tabbed || container.is_highlighted)
    {
        let mut line = format!("container {}", rectangle(container.border_box));
        if container.is_tabbed {
            line.push_str(&format!(
                " tabbed active={} titles=[{}]",
                container.active_tab_index,
                container.titles.join(", ")
            ));
        }
        line.push_str(&markers(
            container.is_highlighted,
            container.spawn_direction,
        ));
        container_lines.push(line);
    }
    container_lines.sort();
    lines.extend(container_lines);
    lines
}

fn grid(monitor: &MonitorPlacements, labels: &BTreeMap<WindowId, String>) -> String {
    let work_area = monitor.work_area;
    let mut cells =
        vec![vec![' '; work_area.width().value() as usize]; work_area.height().value() as usize];
    match &monitor.layout {
        MonitorLayout::Fullscreen(id) => {
            draw_box(
                &mut cells,
                relative(work_area, work_area),
                &labels[id],
                [false; 4],
            );
        }
        MonitorLayout::Normal {
            tiling_windows,
            float_windows,
            containers,
        } => {
            for window in tiling_windows {
                draw_box(
                    &mut cells,
                    relative(window.visible_border_box, work_area),
                    &labels[&window.id],
                    clip_edges(window.border_box, window.visible_border_box),
                );
            }
            for container in containers.iter().filter(|container| container.is_tabbed) {
                draw_tab_bar(&mut cells, container, work_area);
            }
            let highlighted_float = float_windows.iter().find(|window| window.is_highlighted);
            if highlighted_float.is_none() {
                if let Some(window) = tiling_windows.iter().find(|window| window.is_highlighted) {
                    draw_border(
                        &mut cells,
                        relative(window.visible_border_box, work_area),
                        clip_edges(window.border_box, window.visible_border_box),
                        true,
                    );
                } else if let Some(container) =
                    containers.iter().find(|container| container.is_highlighted)
                {
                    draw_border(
                        &mut cells,
                        relative(container.visible_border_box, work_area),
                        clip_edges(container.border_box, container.visible_border_box),
                        true,
                    );
                }
            }
            for window in float_windows {
                let rect = relative(window.visible_border_box, work_area);
                for row in rect.y().value() + 1..rect.bottom().value() - 1 {
                    for column in rect.x().value() + 1..rect.right().value() - 1 {
                        put(&mut cells, column, row, ' ');
                    }
                }
                draw_box(
                    &mut cells,
                    rect,
                    &labels[&window.id],
                    clip_edges(window.border_box, window.visible_border_box),
                );
            }
            if let Some(window) = highlighted_float {
                draw_border(
                    &mut cells,
                    relative(window.visible_border_box, work_area),
                    clip_edges(window.border_box, window.visible_border_box),
                    true,
                );
            }
        }
    }
    cells
        .iter()
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn relative(rect: PixelRect, work_area: PixelRect) -> PixelRect {
    PixelRect::new(
        rect.x().value() - work_area.x().value(),
        rect.y().value() - work_area.y().value(),
        rect.width().value(),
        rect.height().value(),
    )
}

fn clip_edges(border_box: PixelRect, visible: PixelRect) -> [bool; 4] {
    [
        visible.x() > border_box.x(),
        visible.right() < border_box.right(),
        visible.y() > border_box.y(),
        visible.bottom() < border_box.bottom(),
    ]
}

fn put(grid: &mut [Vec<char>], x: i32, y: i32, character: char) {
    if let Ok(row) = usize::try_from(y)
        && let Ok(column) = usize::try_from(x)
        && let Some(cell) = grid.get_mut(row).and_then(|row| row.get_mut(column))
    {
        *cell = character;
    }
}

fn draw_border(grid: &mut [Vec<char>], rect: PixelRect, clip: [bool; 4], highlighted: bool) {
    if rect.is_empty() {
        return;
    }
    let left = rect.x().value();
    let right = rect.right().value() - 1;
    let top = rect.y().value();
    let bottom = rect.bottom().value() - 1;
    let [clip_left, clip_right, clip_top, clip_bottom] = clip;
    let horizontal = if highlighted { '*' } else { '-' };
    let vertical = if highlighted { '*' } else { '|' };
    let corner = if highlighted { '*' } else { '+' };
    for column in left..=right {
        if !clip_top {
            put(grid, column, top, horizontal);
        }
        if !clip_bottom {
            put(grid, column, bottom, horizontal);
        }
    }
    for row in top..=bottom {
        if !clip_left {
            put(grid, left, row, vertical);
        }
        if !clip_right {
            put(grid, right, row, vertical);
        }
    }
    for (x, y, clipped) in [
        (left, top, clip_left || clip_top),
        (right, top, clip_right || clip_top),
        (left, bottom, clip_left || clip_bottom),
        (right, bottom, clip_right || clip_bottom),
    ] {
        if !clipped {
            put(grid, x, y, corner);
        }
    }
}

fn draw_box(grid: &mut [Vec<char>], rect: PixelRect, label: &str, clip: [bool; 4]) {
    if rect.is_empty() {
        return;
    }
    draw_border(grid, rect, clip, false);
    let middle = (rect.x().value() as f64 + rect.width().value() as f64 / 2.0).round() as i32;
    let row = (rect.y() + (rect.height() - crate::Length::new(1)).to_length() / 2).value();
    let start = middle - label.chars().count() as i32 / 2;
    for (index, character) in label.chars().enumerate() {
        let column = start + index as i32;
        if column > rect.x().value() && column < rect.right().value() - 1 {
            put(grid, column, row, character);
        }
    }
}

fn draw_tab_bar(grid: &mut [Vec<char>], container: &ContainerPlacement, work_area: PixelRect) {
    let band = relative(container.visible_tab_bar_band, work_area);
    if band.is_empty() {
        return;
    }
    let left = band.x().value();
    let right = band.right().value() - 1;
    let top = band.y().value();
    let bottom = band.bottom().value() - 1;
    for column in left..=right {
        put(grid, column, top, '-');
    }
    put(grid, left, top, '+');
    put(grid, right, top, '+');
    for row in top + 1..=bottom {
        put(grid, left, row, '|');
        put(grid, right, row, '|');
    }
    if container.titles.is_empty() {
        return;
    }
    let width = (right - left - 1).max(0) / container.titles.len() as i32;
    let label_row = (band.y() + band.height() / 2).value();
    for (index, title) in container.titles.iter().enumerate() {
        let start = left + 1 + index as i32 * width;
        let end = if index + 1 == container.titles.len() {
            right - 1
        } else {
            start + width - 1
        };
        let display = if index == container.active_tab_index {
            format!("[{title}]")
        } else {
            title.clone()
        };
        let label_start = (start + end) / 2 - display.chars().count() as i32 / 2;
        for (offset, character) in display.chars().enumerate() {
            let column = label_start + offset as i32;
            if column >= start && column <= end {
                put(grid, column, label_row, character);
            }
        }
        if index + 1 < container.titles.len() && end + 1 > left && end + 1 < right {
            for row in top + 1..=bottom {
                put(grid, end + 1, row, '|');
            }
        }
    }
}
