use egui::{
    Align, Color32, CornerRadius, Id, LayerId, Layout, Order, Rect, RichText, Sense, Stroke,
    StrokeKind, TextStyle, pos2, vec2,
};

use crate::config::Config;
use crate::core::{
    ContainerId, ContainerPlacement, Dimension, SpawnIndicator, TilingWindowPlacement,
};
use crate::theme::Theme;

/// Hardcoded corner radius for window borders and tabbed-container body
/// borders. Kept private: rendering knobs should not leak into the config
/// surface or into core, which has no view on pixels.
const WINDOW_BORDER_RADIUS: f32 = 12.0;

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
        // layer_painter bypasses egui's Area sizing pass, which makes first-frame
        // output invisible. Window borders are pure painting with no interaction,
        // so Area is unnecessary.
        let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new(("border", wp.id))));
        let clip = Rect::from_min_size(origin.to_pos2(), vec2(vf.width, vf.height));
        paint_window_border(
            &painter.with_clip_rect(clip),
            wp.frame,
            wp.visible_frame,
            wp.is_highlighted,
            wp.spawn_indicator,
            config,
            origin,
        );
    }

    for (cp, titles) in containers {
        let vf = cp.visible_frame;
        let origin = vec2(vf.x - monitor.x, vf.y - monitor.y);
        egui::Area::new(egui::Id::new(("container", cp.id)))
            .order(egui::Order::Foreground)
            .fixed_pos(origin.to_pos2())
            .fade_in(false)
            .show(ctx, |ui| {
                // Skip the sizing pass and request a discard so the container
                // renders correctly on the first frame. Without this, egui's
                // Area emits Shape::Noop during the sizing pass, producing a
                // black/invisible first frame on Windows.
                if ui.is_sizing_pass() {
                    ctx.request_discard("container first frame");
                    return;
                }
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
    let theme = config.theme();
    let colors = border_colors(is_highlighted, spawn_indicator, &theme);
    paint_border_edges(
        painter,
        frame,
        visible_frame,
        config.border_size,
        WINDOW_BORDER_RADIUS,
        colors,
        theme.focused_border,
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
    let r = effective_radius(WINDOW_BORDER_RADIUS, w, h);
    let theme = config.theme();

    let border_c = if placement.is_highlighted {
        theme.focused_border
    } else {
        theme.unfocused_border
    };

    if placement.is_highlighted {
        let colors = border_colors(true, placement.spawn_indicator, &theme);
        let focused = theme.focused_border;
        let painter = ui.painter();

        if is_tabbed {
            let body_h = h - th;
            let r_body = effective_radius(r, w, body_h);

            // When r_body==0, clip rects collapse to zero dimensions (same bug as
            // paint_border_edges). Draw filled rects for the body border instead.
            if r_body == 0.0 {
                let corners = corner_colors(colors, focused);
                // Left/right edges inset by b at bottom to avoid overlap with corner squares
                painter.rect_filled(
                    Rect::from_min_size(pos2(ox, oy + th), vec2(b, body_h - b)),
                    CornerRadius::ZERO,
                    colors[3],
                );
                painter.rect_filled(
                    Rect::from_min_size(pos2(ox + w - b, oy + th), vec2(b, body_h - b)),
                    CornerRadius::ZERO,
                    colors[1],
                );
                painter.rect_filled(
                    Rect::from_min_size(pos2(ox + b, oy + h - b), vec2(w - 2.0 * b, b)),
                    CornerRadius::ZERO,
                    colors[2],
                );
                painter.rect_filled(
                    Rect::from_min_size(pos2(ox, oy + h - b), vec2(b, b)),
                    CornerRadius::ZERO,
                    corners[2],
                );
                painter.rect_filled(
                    Rect::from_min_size(pos2(ox + w - b, oy + h - b), vec2(b, b)),
                    CornerRadius::ZERO,
                    corners[3],
                );
            } else {
                let full_rect = Rect::from_min_size(pos2(ox, oy + th), vec2(w, body_h));
                let cr = CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: cr_u8(r_body),
                    se: cr_u8(r_body),
                };

                stroke_clipped(
                    painter,
                    Rect::from_min_size(pos2(ox, oy + th), vec2(r_body, body_h - r_body)),
                    full_rect,
                    cr,
                    (b, colors[3]),
                );
                stroke_clipped(
                    painter,
                    Rect::from_min_size(
                        pos2(ox + w - r_body, oy + th),
                        vec2(r_body, body_h - r_body),
                    ),
                    full_rect,
                    cr,
                    (b, colors[1]),
                );
                stroke_clipped(
                    painter,
                    Rect::from_min_size(
                        pos2(ox + r_body, oy + h - r_body),
                        vec2(w - 2.0 * r_body, r_body),
                    ),
                    full_rect,
                    cr,
                    (b, colors[2]),
                );
                // SW corner: top half = left edge, bottom half = bottom edge
                paint_split_corner(
                    painter,
                    Rect::from_min_size(pos2(ox, oy + h - r_body), vec2(r_body, r_body)),
                    full_rect,
                    cr,
                    b,
                    colors[3],
                    colors[2],
                );
                // SE corner: top half = right edge, bottom half = bottom edge
                paint_split_corner(
                    painter,
                    Rect::from_min_size(
                        pos2(ox + w - r_body, oy + h - r_body),
                        vec2(r_body, r_body),
                    ),
                    full_rect,
                    cr,
                    b,
                    colors[1],
                    colors[2],
                );
            }
        } else {
            paint_border_edges(
                painter,
                f,
                vf,
                b,
                WINDOW_BORDER_RADIUS,
                colors,
                focused,
                origin,
            );
        }
    }

    if !is_tabbed {
        return None;
    }

    let bg = theme.tab_bar_bg;
    let active_bg = theme.active_tab_bg;
    let tab_cr = tab_bar_corner_radius(th);
    let tab_bar_cr = CornerRadius::same(cr_u8(tab_cr));

    // Tab bar background
    let tab_bar_rect = Rect::from_min_size(pos2(ox, oy), vec2(w, th));
    ui.painter().rect_filled(tab_bar_rect, tab_bar_cr, bg);

    // Tab bar border
    ui.painter()
        .rect_stroke(tab_bar_rect, tab_bar_cr, (b, border_c), StrokeKind::Inside);

    // Tabs
    let tab_width = w / tab_titles.len() as f32;
    let mut clicked = None;
    let focused_c = theme.focused_border;

    for (i, title) in tab_titles.iter().enumerate() {
        let tab_x = ox + i as f32 * tab_width;
        let tab_rect = Rect::from_min_size(pos2(tab_x, oy), vec2(tab_width, th));
        let is_active = i == placement.active_tab_index;

        if is_active {
            let active_cr = active_tab_corner_radius(i, tab_titles.len(), tab_cr);
            ui.painter().rect_filled(tab_rect, active_cr, active_bg);

            if placement.is_highlighted {
                ui.painter()
                    .rect_stroke(tab_rect, active_cr, (b, focused_c), StrokeKind::Inside);
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
                .color(theme.tab_text)
                .text_style(TextStyle::Body),
            )
            .truncate()
            .halign(Align::Center),
        );
    }

    clicked
}

fn stroke_clipped(
    painter: &egui::Painter,
    clip: Rect,
    full_rect: Rect,
    cr: CornerRadius,
    stroke: impl Into<Stroke>,
) {
    painter
        .with_clip_rect(clip)
        .rect_stroke(full_rect, cr, stroke, StrokeKind::Inside);
}

/// Paints the inner stroke of `full_rect` twice, clipped to the top half and
/// bottom half of `corner_rect`. Lets a rounded corner display two colours
/// along its arc so a spawn-indicator edge tints only the half of the arc
/// adjacent to the flagged edge rather than the entire 90-degree sweep.
/// When `top == bottom` the two strokes coincide and produce pixel-identical
/// output to a single full-corner stroke. Kept for `r > 0` branches only;
/// the `r == 0` square-corner paths continue to use `corner_colors` since
/// there is no arc to split there.
/// Horizontal (top/bottom) split chosen over diagonal because
/// `egui::Painter::with_clip_rect` only accepts axis-aligned rects.
fn paint_split_corner(
    painter: &egui::Painter,
    corner_rect: Rect,
    full_rect: Rect,
    cr: CornerRadius,
    b: f32,
    top: Color32,
    bottom: Color32,
) {
    let mid_y = corner_rect.min.y + corner_rect.height() / 2.0;
    let top_half = Rect::from_min_max(corner_rect.min, pos2(corner_rect.max.x, mid_y));
    let bottom_half = Rect::from_min_max(pos2(corner_rect.min.x, mid_y), corner_rect.max);
    stroke_clipped(painter, top_half, full_rect, cr, (b, top));
    stroke_clipped(painter, bottom_half, full_rect, cr, (b, bottom));
}

/// Clamps radius to fit within the given dimensions.
/// When r == w/2 or h/2, corner clips cover everything and edges have zero width, which is fine.
fn effective_radius(r: f32, w: f32, h: f32) -> f32 {
    r.max(0.0).min(w / 2.0).min(h / 2.0)
}

/// Defensive clamp for converting f32 radius to u8 for CornerRadius fields.
fn cr_u8(r: f32) -> u8 {
    r.clamp(0.0, 255.0) as u8
}

/// Tab-bar corner radius, sized to a quarter of the tab bar's thickness and
/// clamped to `tab_bar_height / 2` so corners always fit. A quarter gives a
/// visibly softer corner than the main window border (6px vs 12px at the
/// default 24px tab-bar height) while scaling with the user-configured
/// tab-bar thickness. Used for both the tab-bar outline and the active-tab
/// highlight so they stay visually coherent.
fn tab_bar_corner_radius(tab_bar_height: f32) -> f32 {
    effective_radius(tab_bar_height * 0.25, tab_bar_height, tab_bar_height)
}

/// Returns the corner radius for the active-tab fill/highlight so its outer
/// corners match the tab bar wherever the tab sits on a tab-bar outer
/// corner. First tab rounds nw+sw, last tab rounds ne+se, a single tab
/// rounds all four, middle tabs stay square. Assumes the tab bar has all
/// four outer corners rounded with `tab_cr`; update in lockstep with the
/// tab bar outline if that changes.
fn active_tab_corner_radius(index: usize, tab_count: usize, tab_cr: f32) -> CornerRadius {
    let r = cr_u8(tab_cr);
    let is_first = index == 0;
    let is_last = index + 1 == tab_count;
    CornerRadius {
        nw: if is_first { r } else { 0 },
        sw: if is_first { r } else { 0 },
        ne: if is_last { r } else { 0 },
        se: if is_last { r } else { 0 },
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "drawing params that must travel together; a struct would add indirection without clarity"
)]
fn paint_border_edges(
    painter: &egui::Painter,
    frame: Dimension,
    visible_frame: Dimension,
    b: f32,
    r: f32,
    colors: [Color32; 4],
    focused: Color32,
    origin: egui::Vec2,
) {
    let ox = origin.x + frame.x - visible_frame.x;
    let oy = origin.y + frame.y - visible_frame.y;
    let w = frame.width;
    let h = frame.height;
    let r = effective_radius(r, w, h);

    // When r==0, clip rects for the 8-region approach collapse to zero dimensions
    // and egui skips them entirely. Draw simple filled rects instead.
    if r == 0.0 {
        let corners = corner_colors(colors, focused);
        // Edges (inset by b at corners to avoid overlap with corner squares)
        painter.rect_filled(
            Rect::from_min_size(pos2(ox + b, oy), vec2(w - 2.0 * b, b)),
            CornerRadius::ZERO,
            colors[0],
        );
        painter.rect_filled(
            Rect::from_min_size(pos2(ox + w - b, oy + b), vec2(b, h - 2.0 * b)),
            CornerRadius::ZERO,
            colors[1],
        );
        painter.rect_filled(
            Rect::from_min_size(pos2(ox + b, oy + h - b), vec2(w - 2.0 * b, b)),
            CornerRadius::ZERO,
            colors[2],
        );
        painter.rect_filled(
            Rect::from_min_size(pos2(ox, oy + b), vec2(b, h - 2.0 * b)),
            CornerRadius::ZERO,
            colors[3],
        );
        // Corners: [nw, ne, sw, se]
        painter.rect_filled(
            Rect::from_min_size(pos2(ox, oy), vec2(b, b)),
            CornerRadius::ZERO,
            corners[0],
        );
        painter.rect_filled(
            Rect::from_min_size(pos2(ox + w - b, oy), vec2(b, b)),
            CornerRadius::ZERO,
            corners[1],
        );
        painter.rect_filled(
            Rect::from_min_size(pos2(ox, oy + h - b), vec2(b, b)),
            CornerRadius::ZERO,
            corners[2],
        );
        painter.rect_filled(
            Rect::from_min_size(pos2(ox + w - b, oy + h - b), vec2(b, b)),
            CornerRadius::ZERO,
            corners[3],
        );
        return;
    }

    let full_rect = Rect::from_min_size(pos2(ox, oy), vec2(w, h));
    let cr = CornerRadius::from(r);

    stroke_clipped(
        painter,
        Rect::from_min_size(pos2(ox + r, oy), vec2(w - 2.0 * r, r)),
        full_rect,
        cr,
        (b, colors[0]),
    );
    stroke_clipped(
        painter,
        Rect::from_min_size(pos2(ox + w - r, oy + r), vec2(r, h - 2.0 * r)),
        full_rect,
        cr,
        (b, colors[1]),
    );
    stroke_clipped(
        painter,
        Rect::from_min_size(pos2(ox + r, oy + h - r), vec2(w - 2.0 * r, r)),
        full_rect,
        cr,
        (b, colors[2]),
    );
    stroke_clipped(
        painter,
        Rect::from_min_size(pos2(ox, oy + r), vec2(r, h - 2.0 * r)),
        full_rect,
        cr,
        (b, colors[3]),
    );

    // NW corner: top half = top edge colour, bottom half = left edge colour
    paint_split_corner(
        painter,
        Rect::from_min_size(pos2(ox, oy), vec2(r, r)),
        full_rect,
        cr,
        b,
        colors[0],
        colors[3],
    );
    // NE corner: top half = top edge colour, bottom half = right edge colour
    paint_split_corner(
        painter,
        Rect::from_min_size(pos2(ox + w - r, oy), vec2(r, r)),
        full_rect,
        cr,
        b,
        colors[0],
        colors[1],
    );
    // SW corner: top half = left edge colour, bottom half = bottom edge colour
    paint_split_corner(
        painter,
        Rect::from_min_size(pos2(ox, oy + h - r), vec2(r, r)),
        full_rect,
        cr,
        b,
        colors[3],
        colors[2],
    );
    // SE corner: top half = right edge colour, bottom half = bottom edge colour
    paint_split_corner(
        painter,
        Rect::from_min_size(pos2(ox + w - r, oy + h - r), vec2(r, r)),
        full_rect,
        cr,
        b,
        colors[1],
        colors[2],
    );
}

/// [top, right, bottom, left] border colors based on highlight state and spawn indicator.
fn border_colors(
    is_highlighted: bool,
    spawn_indicator: Option<SpawnIndicator>,
    theme: &Theme,
) -> [Color32; 4] {
    if !is_highlighted {
        return [theme.unfocused_border; 4];
    }
    let Some(si) = spawn_indicator else {
        return [theme.focused_border; 4];
    };
    let f = theme.focused_border;
    let s = theme.spawn_indicator;
    [
        if si.top { s } else { f },
        if si.right { s } else { f },
        if si.bottom { s } else { f },
        if si.left { s } else { f },
    ]
}

/// Returns [nw, ne, sw, se] corner colors. A corner gets the focused color only if both
/// adjacent edges are focused. Otherwise it takes the non-focused color from whichever
/// adjacent edge has it, with a fixed priority order per corner.
fn corner_colors(edge_colors: [Color32; 4], focused: Color32) -> [Color32; 4] {
    let c = edge_colors; // [top, right, bottom, left]
    [
        if c[0] != focused { c[0] } else { c[3] }, // NW: top first, then left
        if c[0] != focused { c[0] } else { c[1] }, // NE: top first, then right
        if c[2] != focused { c[2] } else { c[3] }, // SW: bottom first, then left
        if c[2] != focused { c[2] } else { c[1] }, // SE: bottom first, then right
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_radius_no_clamp() {
        assert_eq!(effective_radius(8.0, 100.0, 200.0), 8.0);
    }

    #[test]
    fn effective_radius_clamp_to_half_width() {
        assert_eq!(effective_radius(60.0, 100.0, 200.0), 50.0);
    }

    #[test]
    fn effective_radius_clamp_to_half_height() {
        assert_eq!(effective_radius(60.0, 200.0, 80.0), 40.0);
    }

    #[test]
    fn effective_radius_zero() {
        assert_eq!(effective_radius(0.0, 100.0, 100.0), 0.0);
    }

    #[test]
    fn effective_radius_tiny_window() {
        assert_eq!(effective_radius(10.0, 6.0, 4.0), 2.0);
    }

    #[test]
    fn effective_radius_negative_input() {
        assert_eq!(effective_radius(-5.0, 100.0, 100.0), 0.0);
    }

    #[test]
    fn corner_colors_uniform() {
        assert_eq!(
            corner_colors([Color32::GRAY; 4], Color32::GRAY),
            [Color32::GRAY; 4]
        );
    }

    #[test]
    fn corner_colors_spawn_right() {
        let focused = Color32::from_rgb(102, 153, 255);
        let spawn = Color32::from_rgb(255, 100, 100);
        let edge_colors = [focused, spawn, focused, focused];
        assert_eq!(
            corner_colors(edge_colors, focused),
            [focused, spawn, focused, spawn]
        );
    }

    #[test]
    fn corner_colors_spawn_top_and_right() {
        let focused = Color32::from_rgb(102, 153, 255);
        let spawn = Color32::from_rgb(255, 100, 100);
        let edge_colors = [spawn, spawn, focused, focused];
        assert_eq!(
            corner_colors(edge_colors, focused),
            [spawn, spawn, focused, spawn]
        );
    }

    #[test]
    fn corner_colors_spawn_bottom() {
        let focused = Color32::from_rgb(102, 153, 255);
        let spawn = Color32::from_rgb(255, 100, 100);
        let edge_colors = [focused, focused, spawn, focused];
        assert_eq!(
            corner_colors(edge_colors, focused),
            [focused, focused, spawn, spawn]
        );
    }

    #[test]
    fn corner_colors_all_spawn() {
        let focused = Color32::from_rgb(102, 153, 255);
        let spawn = Color32::from_rgb(255, 100, 100);
        assert_eq!(corner_colors([spawn; 4], focused), [spawn; 4]);
    }

    #[test]
    fn active_tab_corner_radius_first_of_many() {
        assert_eq!(
            active_tab_corner_radius(0, 4, 6.0),
            CornerRadius {
                nw: 6,
                sw: 6,
                ne: 0,
                se: 0
            }
        );
    }

    #[test]
    fn active_tab_corner_radius_last_of_many() {
        assert_eq!(
            active_tab_corner_radius(3, 4, 6.0),
            CornerRadius {
                nw: 0,
                sw: 0,
                ne: 6,
                se: 6
            }
        );
    }

    #[test]
    fn active_tab_corner_radius_middle() {
        assert_eq!(
            active_tab_corner_radius(1, 4, 6.0),
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: 0,
                se: 0
            }
        );
    }

    #[test]
    fn active_tab_corner_radius_single_tab() {
        assert_eq!(active_tab_corner_radius(0, 1, 6.0), CornerRadius::same(6));
    }

    #[test]
    fn active_tab_corner_radius_zero_radius() {
        assert_eq!(active_tab_corner_radius(0, 3, 0.0), CornerRadius::ZERO);
    }

    #[test]
    fn tab_bar_corner_radius_default_height() {
        assert_eq!(tab_bar_corner_radius(24.0), 6.0);
    }

    #[test]
    fn tab_bar_corner_radius_scales_linearly() {
        assert_eq!(tab_bar_corner_radius(40.0), 10.0);
        assert_eq!(tab_bar_corner_radius(12.0), 3.0);
    }

    #[test]
    fn tab_bar_corner_radius_clamps_to_half_height() {
        assert_eq!(tab_bar_corner_radius(4.0), 1.0);
    }

    #[test]
    fn tab_bar_corner_radius_zero_height() {
        assert_eq!(tab_bar_corner_radius(0.0), 0.0);
    }
}
