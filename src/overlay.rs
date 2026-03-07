use egui::{Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, pos2, vec2};

use crate::config::{Color, Config};
use crate::core::SpawnMode;
use crate::core::{ContainerPlacement, WindowPlacement};

/// Draws 4 border edges for a window overlay.
/// Coordinates are relative to the overlay window origin. The overlay window is sized to
/// `placement.visible_frame` (the clipped portion). Borders are drawn at positions offset
/// from the full `placement.frame`, so edges clipped by the monitor are naturally outside
/// the window bounds and get clipped by egui.
pub(crate) fn paint_window_border(
    painter: &egui::Painter,
    placement: &WindowPlacement,
    config: &Config,
) {
    let colors = border_colors(
        placement.is_focused,
        placement.is_float,
        placement.spawn_mode,
        config,
    );
    paint_border_edges(
        painter,
        placement.frame,
        placement.visible_frame,
        config.border_size,
        colors,
    );
}

/// Draws container borders and an inline tab bar. Returns `Some(tab_index)` if a tab was clicked.
/// The overlay window is sized to `placement.visible_frame`.
pub(crate) fn show_container(
    ui: &mut egui::Ui,
    placement: &ContainerPlacement,
    tab_titles: &[String],
    config: &Config,
) -> Option<usize> {
    let vf = placement.visible_frame;
    let f = placement.frame;
    let ox = f.x - vf.x;
    let oy = f.y - vf.y;
    let b = config.border_size;
    let w = f.width;
    let h = f.height;
    let is_tabbed = placement.is_tabbed && !tab_titles.is_empty();
    let th = config.tab_bar_height;

    let border_c = to_color32(if placement.is_focused {
        config.focused_color
    } else {
        config.border_color
    });

    if placement.is_focused {
        let colors = border_colors(true, false, placement.spawn_mode, config);
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
            paint_border_edges(painter, f, vf, b, colors);
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

            if placement.is_focused {
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

        let response = ui.allocate_rect(tab_rect, Sense::click());
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
            egui::Label::new(RichText::new(title).color(Color32::WHITE).size(12.0))
                .truncate()
                .halign(Align::Center),
        );
    }

    clicked
}

use crate::core::Dimension;

fn paint_border_edges(
    painter: &egui::Painter,
    frame: Dimension,
    visible_frame: Dimension,
    b: f32,
    colors: [Color32; 4],
) {
    // Offset: frame origin relative to visible_frame origin
    let ox = frame.x - visible_frame.x;
    let oy = frame.y - visible_frame.y;
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

/// [top, right, bottom, left] border colors based on focus state and spawn mode.
fn border_colors(
    focused: bool,
    is_float: bool,
    spawn_mode: SpawnMode,
    config: &Config,
) -> [Color32; 4] {
    if !focused {
        [to_color32(config.border_color); 4]
    } else if is_float {
        [to_color32(config.focused_color); 4]
    } else {
        let f = to_color32(config.focused_color);
        let s = to_color32(config.spawn_indicator_color);
        [
            if spawn_mode.is_tab() { s } else { f },
            if spawn_mode.is_horizontal() { s } else { f },
            if spawn_mode.is_vertical() { s } else { f },
            f,
        ]
    }
}

fn to_color32(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        (c.a * 255.0) as u8,
    )
}
