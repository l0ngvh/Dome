use egui::{Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, pos2, vec2};

use crate::config::{Color, Config};
use crate::core::{
    ContainerId, ContainerPlacement, Dimension, SpawnIndicator, TilingWindowPlacement,
};

/// Draws all tiling window borders and container overlays for a single monitor.
/// Returns `(ContainerId, tab_index)` for each tab that was clicked.
pub(crate) fn paint_tiling_overlay(
    ctx: &egui::Context,
    monitor: Dimension,
    windows: &[TilingWindowPlacement],
    containers: &[(ContainerPlacement, Vec<String>)],
    config: &Config,
) -> Vec<(ContainerId, usize)> {
    let mut clicked = Vec::new();

    for wp in windows {
        let vf = wp.visible_frame;
        let origin = vec2(vf.x - monitor.x, vf.y - monitor.y);
        egui::Area::new(egui::Id::new(("border", wp.id)))
            .fixed_pos(origin.to_pos2())
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(Rect::from_min_size(
                    origin.to_pos2(),
                    vec2(vf.width, vf.height),
                ));
                paint_window_border(
                    ui.painter(),
                    wp.frame,
                    wp.visible_frame,
                    wp.is_highlighted,
                    wp.spawn_indicator,
                    config,
                    origin,
                );
            });
    }

    for (cp, titles) in containers {
        let vf = cp.visible_frame;
        let origin = vec2(vf.x - monitor.x, vf.y - monitor.y);
        egui::Area::new(egui::Id::new(("container", cp.id)))
            .order(egui::Order::Foreground)
            .fixed_pos(origin.to_pos2())
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(Rect::from_min_size(
                    origin.to_pos2(),
                    vec2(vf.width, vf.height),
                ));
                if let Some(tab) = show_container(ui, cp, titles, config, origin) {
                    clicked.push((cp.id, tab));
                }
            });
    }

    clicked
}

/// Draws 4 border edges for a window overlay.
/// `origin` is the visible_frame's top-left in canvas coordinates.
/// For per-window overlays (floats), pass `Vec2::ZERO`.
/// For the tiling overlay, pass `vec2(vf.x - monitor.x, vf.y - monitor.y)`.
pub(crate) fn paint_window_border(
    painter: &egui::Painter,
    frame: Dimension,
    visible_frame: Dimension,
    is_highlighted: bool,
    spawn_indicator: Option<SpawnIndicator>,
    config: &Config,
    origin: egui::Vec2,
) {
    let colors = border_colors(is_highlighted, spawn_indicator, config);
    paint_border_edges(
        painter,
        frame,
        visible_frame,
        config.border_size,
        colors,
        origin,
    );
}

/// Draws container borders and an inline tab bar. Returns `Some(tab_index)` if a tab was clicked.
/// `origin` is the visible_frame's top-left in canvas coordinates (same as `paint_window_border`).
pub(crate) fn show_container(
    ui: &mut egui::Ui,
    placement: &ContainerPlacement,
    tab_titles: &[String],
    config: &Config,
    origin: egui::Vec2,
) -> Option<usize> {
    let vf = placement.visible_frame;
    let f = placement.frame;
    let ox = origin.x + f.x - vf.x;
    let oy = origin.y + f.y - vf.y;
    let b = config.border_size;
    let w = f.width;
    let h = f.height;
    let is_tabbed = placement.is_tabbed && !tab_titles.is_empty();
    let th = config.tab_bar_height;

    let border_c = to_color32(if placement.is_highlighted {
        config.focused_color
    } else {
        config.border_color
    });

    if placement.is_highlighted {
        let colors = border_colors(true, placement.spawn_indicator, config);
        let painter = ui.painter();

        if is_tabbed {
            // Left border: from tab bar bottom to container bottom
            painter.rect_filled(
                Rect::from_min_size(pos2(ox, oy + th), vec2(b, h - th - b)),
                CornerRadius::ZERO,
                colors[3],
            );
            // Right border: from tab bar bottom to container bottom
            painter.rect_filled(
                Rect::from_min_size(pos2(ox + w - b, oy + th), vec2(b, h - th - b)),
                CornerRadius::ZERO,
                colors[1],
            );
            // Bottom border
            painter.rect_filled(
                Rect::from_min_size(pos2(ox, oy + h - b), vec2(w, b)),
                CornerRadius::ZERO,
                colors[2],
            );
        } else {
            paint_border_edges(painter, f, vf, b, colors, origin);
        }
    }

    if !is_tabbed {
        return None;
    }

    let bg = to_color32(config.tab_bar_background_color);
    let active_bg = to_color32(config.active_tab_background_color);

    // Tab bar background
    let tab_bar_rect = Rect::from_min_size(pos2(ox, oy), vec2(w, th));
    ui.painter()
        .rect_filled(tab_bar_rect, CornerRadius::ZERO, bg);

    // Tab bar borders: top, bottom, left, right
    ui.painter().rect_filled(
        Rect::from_min_size(pos2(ox, oy), vec2(w, b)),
        CornerRadius::ZERO,
        border_c,
    );
    ui.painter().rect_filled(
        Rect::from_min_size(pos2(ox, oy + th - b), vec2(w, b)),
        CornerRadius::ZERO,
        border_c,
    );
    ui.painter().rect_filled(
        Rect::from_min_size(pos2(ox, oy + b), vec2(b, th - 2.0 * b)),
        CornerRadius::ZERO,
        border_c,
    );
    ui.painter().rect_filled(
        Rect::from_min_size(pos2(ox + w - b, oy + b), vec2(b, th - 2.0 * b)),
        CornerRadius::ZERO,
        border_c,
    );

    // Tabs
    let tab_width = w / tab_titles.len() as f32;
    let mut clicked = None;
    let focused_c = to_color32(config.focused_color);

    for (i, title) in tab_titles.iter().enumerate() {
        let tab_x = ox + i as f32 * tab_width;
        let tab_rect = Rect::from_min_size(pos2(tab_x, oy), vec2(tab_width, th));
        let is_active = i == placement.active_tab_index;

        if is_active {
            ui.painter()
                .rect_filled(tab_rect, CornerRadius::ZERO, active_bg);

            if placement.is_highlighted {
                // Active tab border: top, bottom, left, right
                ui.painter().rect_filled(
                    Rect::from_min_size(pos2(tab_x, oy), vec2(tab_width, b)),
                    CornerRadius::ZERO,
                    focused_c,
                );
                ui.painter().rect_filled(
                    Rect::from_min_size(pos2(tab_x, oy + th - b), vec2(tab_width, b)),
                    CornerRadius::ZERO,
                    focused_c,
                );
                ui.painter().rect_filled(
                    Rect::from_min_size(pos2(tab_x, oy + b), vec2(b, th - 2.0 * b)),
                    CornerRadius::ZERO,
                    focused_c,
                );
                ui.painter().rect_filled(
                    Rect::from_min_size(pos2(tab_x + tab_width - b, oy + b), vec2(b, th - 2.0 * b)),
                    CornerRadius::ZERO,
                    focused_c,
                );
            }
        }

        if i > 0 && !is_active && i != placement.active_tab_index + 1 {
            ui.painter().rect_filled(
                Rect::from_min_size(pos2(tab_rect.min.x - b / 2.0, oy), vec2(b, th)),
                CornerRadius::ZERO,
                border_c,
            );
        }

        let response = ui.interact(
            tab_rect,
            egui::Id::new(("tab", placement.id, i)),
            Sense::click(),
        );
        if response.clicked() {
            clicked = Some(i);
        }
        let inner = tab_rect.shrink2(vec2(b * 2.0, 0.0));
        let mut tab_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::left_to_right(Align::Center)),
        );
        tab_ui.add(
            egui::Label::new(
                RichText::new(if title.is_empty() {
                    "Untitled"
                } else {
                    title.as_str()
                })
                .color(Color32::WHITE)
                .size(12.0),
            )
            .truncate()
            .halign(Align::Center),
        );
    }

    clicked
}

fn paint_border_edges(
    painter: &egui::Painter,
    frame: Dimension,
    visible_frame: Dimension,
    b: f32,
    colors: [Color32; 4],
    origin: egui::Vec2,
) {
    // Offset: origin positions the visible_frame in canvas space, then frame is offset
    // relative to visible_frame. For per-window overlays (origin=ZERO), this reduces to
    // the original frame.x - visible_frame.x calculation.
    let ox = origin.x + frame.x - visible_frame.x;
    let oy = origin.y + frame.y - visible_frame.y;
    let w = frame.width;
    let h = frame.height;

    // [top, right, bottom, left]
    painter.rect_filled(
        Rect::from_min_size(pos2(ox, oy), vec2(w, b)),
        CornerRadius::ZERO,
        colors[0],
    );
    painter.rect_filled(
        Rect::from_min_size(pos2(ox + w - b, oy + b), vec2(b, h - 2.0 * b)),
        CornerRadius::ZERO,
        colors[1],
    );
    painter.rect_filled(
        Rect::from_min_size(pos2(ox, oy + h - b), vec2(w, b)),
        CornerRadius::ZERO,
        colors[2],
    );
    painter.rect_filled(
        Rect::from_min_size(pos2(ox, oy + b), vec2(b, h - 2.0 * b)),
        CornerRadius::ZERO,
        colors[3],
    );
}

/// [top, right, bottom, left] border colors based on highlight state and spawn indicator.
fn border_colors(
    is_highlighted: bool,
    spawn_indicator: Option<SpawnIndicator>,
    config: &Config,
) -> [Color32; 4] {
    if !is_highlighted {
        return [to_color32(config.border_color); 4];
    }
    let Some(si) = spawn_indicator else {
        return [to_color32(config.focused_color); 4];
    };
    let f = to_color32(config.focused_color);
    let s = to_color32(config.spawn_indicator_color);
    [
        if si.top { s } else { f },
        if si.right { s } else { f },
        if si.bottom { s } else { f },
        if si.left { s } else { f },
    ]
}

fn to_color32(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        (c.a * 255.0) as u8,
    )
}
