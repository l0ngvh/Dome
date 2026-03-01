use egui::{Align2, Color32, CornerRadius, FontId, Rect, Sense, pos2, vec2};

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
    if placement.is_focused {
        let colors = border_colors(true, false, placement.spawn_mode, config);
        paint_border_edges(
            ui.painter(),
            placement.frame,
            placement.visible_frame,
            config.border_size,
            colors,
        );
    }

    if !placement.is_tabbed || tab_titles.is_empty() {
        return None;
    }

    let vf = placement.visible_frame;
    let f = placement.frame;
    let ox = f.x - vf.x;
    let oy = f.y - vf.y;
    let b = config.border_size;
    let w = f.width;

    let bg = to_color32(config.tab_bar_background_color);
    let active_bg = to_color32(config.active_tab_background_color);

    let tab_bar_rect = Rect::from_min_size(pos2(ox, oy + b), vec2(w, config.tab_bar_height));
    ui.painter()
        .rect_filled(tab_bar_rect, CornerRadius::ZERO, bg);

    let tab_width = w / tab_titles.len() as f32;
    let mut clicked = None;

    for (i, title) in tab_titles.iter().enumerate() {
        let tab_rect = Rect::from_min_size(
            pos2(ox + i as f32 * tab_width, oy + b),
            vec2(tab_width, config.tab_bar_height),
        );
        if i == placement.active_tab_index {
            ui.painter()
                .rect_filled(tab_rect, CornerRadius::ZERO, active_bg);
        }
        let response = ui.allocate_rect(tab_rect, Sense::click());
        if response.clicked() {
            clicked = Some(i);
        }
        ui.painter().text(
            tab_rect.center(),
            Align2::CENTER_CENTER,
            title,
            FontId::proportional(12.0),
            Color32::WHITE,
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
