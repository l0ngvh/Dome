// Logical points throughout. Each platform shell sets egui's pixels_per_point to its monitor
// scale, so strokes, corner radii and rects are rescaled to physical pixels at tessellation
// and must never be pre-multiplied here.

use egui::{
    Align, Color32, CornerRadius, Id, LayerId, Layout, Order, Rect, RichText, Sense, Stroke,
    StrokeKind, TextStyle, pos2, vec2,
};

use crate::core::{ContainerId, Dimension, Length, Logical, SpawnIndicator, WindowId};
use crate::theme::Theme;

/// Hardcoded corner radius for window borders and tabbed-container body
/// borders. Kept private: rendering knobs should not leak into the config
/// surface or into core, which has no view on pixels.
const WINDOW_BORDER_RADIUS_LOGICAL: f32 = 12.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct BorderMetrics {
    pub thickness: Length<Logical>,
    pub radius: Length<Logical>,
}

impl BorderMetrics {
    pub(crate) fn from_thickness(thickness: Length<Logical>) -> Self {
        Self {
            thickness,
            radius: Length::new(WINDOW_BORDER_RADIUS_LOGICAL),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LogicalTiledWindow {
    pub id: WindowId,
    pub frame: Dimension<Logical>,
    pub visible_frame: Dimension<Logical>,
    pub is_highlighted: bool,
    pub spawn_indicator: Option<SpawnIndicator>,
}

#[derive(Clone, Debug)]
pub(crate) struct LogicalTiledContainer {
    pub id: ContainerId,
    pub frame: Dimension<Logical>,
    pub visible_frame: Dimension<Logical>,
    /// Reserved by core at the top of `frame`. Reading config here instead reopens a sub-unit
    /// seam against the painted bar.
    pub tab_bar_height: Length<Logical>,
    pub is_highlighted: bool,
    pub spawn_indicator: Option<SpawnIndicator>,
    pub is_tabbed: bool,
    pub titles: Vec<String>,
}

/// Paints the per-monitor tiling overlay: window borders, highlighted-container
/// body border. Tab bars are owned by per-`ContainerId` windows the platform
/// shell hosts separately and reach the painter via `paint_tab_bar`, so this
/// entry point does not paint or hit-test tab bars. The returned click vector
/// is always empty and exists only to keep the renderer API uniform with
/// per-window overlays that do collect clicks.
pub(crate) fn paint_tiling_overlay(
    ctx: &egui::Context,
    monitor: Dimension<Logical>,
    windows: &[LogicalTiledWindow],
    containers: &[LogicalTiledContainer],
    theme: &Theme,
    border: BorderMetrics,
) -> Vec<(ContainerId, usize)> {
    for wp in windows {
        let vf = wp.visible_frame;
        let origin = vec2(
            vf.x.logical() - monitor.x.logical(),
            vf.y.logical() - monitor.y.logical(),
        );
        // layer_painter bypasses egui's Area sizing pass, which makes first-frame
        // output invisible. Window borders are pure painting with no interaction,
        // so Area is unnecessary.
        let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new(("border", wp.id))));
        let clip = Rect::from_min_size(
            origin.to_pos2(),
            vec2(vf.width.logical(), vf.height.logical()),
        );
        paint_window_border(
            &painter.with_clip_rect(clip),
            wp.frame,
            wp.visible_frame,
            wp.is_highlighted,
            wp.spawn_indicator,
            theme,
            border,
            origin,
        );
    }

    for cp in containers {
        let vf = cp.visible_frame;
        let origin = vec2(
            vf.x.logical() - monitor.x.logical(),
            vf.y.logical() - monitor.y.logical(),
        );
        egui::Area::new(egui::Id::new(("container", cp.id)))
            .order(egui::Order::Foreground)
            .fixed_pos(origin.to_pos2())
            .fade_in(false)
            .show(ctx, |ui| {
                // Without the discard, egui's Area emits Shape::Noop during the sizing pass, producing a black/invisible first frame on Windows.
                if ui.is_sizing_pass() {
                    ctx.request_discard("container first frame");
                    return;
                }
                ui.set_clip_rect(Rect::from_min_size(
                    origin.to_pos2(),
                    vec2(vf.width.logical(), vf.height.logical()),
                ));
                show_container(ui, cp, theme, border, origin);
            });
    }

    Vec::new()
}

/// `origin` is the visible_frame's top-left in canvas coordinates.
/// For per-window overlays (floats), pass `Vec2::ZERO`.
/// For the tiling overlay, pass `vec2(vf.x - monitor.x, vf.y - monitor.y)`.
#[expect(
    clippy::too_many_arguments,
    reason = "drawing params that must travel together"
)]
pub(crate) fn paint_window_border(
    painter: &egui::Painter,
    frame: Dimension<Logical>,
    visible_frame: Dimension<Logical>,
    is_highlighted: bool,
    spawn_indicator: Option<SpawnIndicator>,
    theme: &Theme,
    border: BorderMetrics,
    origin: egui::Vec2,
) {
    let colors = border_colors(is_highlighted, spawn_indicator, theme);
    paint_border_edges(
        painter,
        frame,
        visible_frame,
        border.thickness.logical(),
        border.radius.logical(),
        colors,
        theme.focused_border,
        origin,
    );
}

/// The window's canvas is exactly the visible frame, so the border draws at local
/// origin. `layer_painter` skips egui's Area sizing pass, which would blank the first
/// frame.
pub(crate) fn paint_float_border(
    ctx: &egui::Context,
    frame: Dimension<Logical>,
    visible_frame: Dimension<Logical>,
    is_highlighted: bool,
    theme: &Theme,
    border: BorderMetrics,
) {
    let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("border")));
    let clip = Rect::from_min_size(
        pos2(0.0, 0.0),
        vec2(
            visible_frame.width.logical(),
            visible_frame.height.logical(),
        ),
    );
    paint_window_border(
        &painter.with_clip_rect(clip),
        frame,
        visible_frame,
        is_highlighted,
        None,
        theme,
        border,
        vec2(0.0, 0.0),
    );
}

/// `tab_bar_frame` is the tab bar's rect in window-local logical points.
#[expect(
    clippy::too_many_arguments,
    reason = "drawing params that must travel together"
)]
pub(crate) fn paint_tab_bar(
    ctx: &egui::Context,
    container_id: ContainerId,
    tab_bar_frame: Dimension<Logical>,
    titles: &[String],
    active_index: usize,
    is_highlighted: bool,
    border: BorderMetrics,
    theme: &Theme,
) -> Option<(ContainerId, usize)> {
    let origin = vec2(tab_bar_frame.x.logical(), tab_bar_frame.y.logical());
    let mut clicked = None;
    egui::Area::new(egui::Id::new(("tab_bar", container_id)))
        .order(egui::Order::Foreground)
        .fixed_pos(origin.to_pos2())
        .fade_in(false)
        .show(ctx, |ui| {
            // Without the discard, the first frame paints Shape::Noop and the tab bar shows up blank on Windows.
            if ui.is_sizing_pass() {
                ctx.request_discard("tab bar first frame");
                return;
            }
            let rect = Rect::from_min_size(
                origin.to_pos2(),
                vec2(
                    tab_bar_frame.width.logical(),
                    tab_bar_frame.height.logical(),
                ),
            );
            ui.set_clip_rect(rect);
            clicked = paint_tab_bar_into_ui(
                ui,
                container_id,
                rect,
                titles,
                active_index,
                is_highlighted,
                border,
                theme,
            );
        });
    clicked.map(|tab_idx| (container_id, tab_idx))
}

/// `origin` is the visible_frame's top-left in canvas coordinates (same as `paint_window_border`).
fn show_container(
    ui: &mut egui::Ui,
    placement: &LogicalTiledContainer,
    theme: &Theme,
    border: BorderMetrics,
    origin: egui::Vec2,
) {
    let vf = placement.visible_frame;
    let f = placement.frame;
    let ox = origin.x + f.x.logical() - vf.x.logical();
    let oy = origin.y + f.y.logical() - vf.y.logical();
    let b = border.thickness.logical();
    let w = f.width.logical();
    let h = f.height.logical();
    let is_tabbed = placement.is_tabbed && !placement.titles.is_empty();
    let th = placement.tab_bar_height.logical();
    let r = effective_radius(border.radius.logical(), w, h);

    if placement.is_highlighted {
        let colors = border_colors(true, placement.spawn_indicator, theme);
        let focused = theme.focused_border;
        let painter = ui.painter();

        if is_tabbed {
            let body_h = h - th;
            let r_body = effective_radius(r, w, body_h);

            // When r_body==0, clip rects collapse to zero dimensions and egui skips them entirely.
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
                paint_split_corner(
                    painter,
                    Rect::from_min_size(pos2(ox, oy + h - r_body), vec2(r_body, r_body)),
                    full_rect,
                    cr,
                    b,
                    colors[3],
                    colors[2],
                );
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
                WINDOW_BORDER_RADIUS_LOGICAL,
                colors,
                focused,
                origin,
            );
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "drawing params that must travel together"
)]
fn paint_tab_bar_into_ui(
    ui: &mut egui::Ui,
    container_id: ContainerId,
    tab_bar_rect: Rect,
    titles: &[String],
    active_index: usize,
    is_highlighted: bool,
    border: BorderMetrics,
    theme: &Theme,
) -> Option<usize> {
    let ox = tab_bar_rect.min.x;
    let oy = tab_bar_rect.min.y;
    let w = tab_bar_rect.width();
    let th = tab_bar_rect.height();
    let b = border.thickness.logical();
    let border_c = if is_highlighted {
        theme.focused_border
    } else {
        theme.unfocused_border
    };

    let bg = theme.tab_bar_bg;
    let active_bg = theme.active_tab_bg;
    let tab_cr = tab_bar_corner_radius(th);
    let tab_bar_cr = CornerRadius::same(cr_u8(tab_cr));

    ui.painter().rect_filled(tab_bar_rect, tab_bar_cr, bg);

    ui.painter()
        .rect_stroke(tab_bar_rect, tab_bar_cr, (b, border_c), StrokeKind::Inside);

    let tab_width = w / titles.len() as f32;
    let mut clicked = None;
    let focused_c = theme.focused_border;

    for (i, title) in titles.iter().enumerate() {
        let tab_x = ox + i as f32 * tab_width;
        let tab_rect = Rect::from_min_size(pos2(tab_x, oy), vec2(tab_width, th));
        let is_active = i == active_index;

        if is_active {
            let active_cr = active_tab_corner_radius(i, titles.len(), tab_cr);
            ui.painter().rect_filled(tab_rect, active_cr, active_bg);

            if is_highlighted {
                ui.painter()
                    .rect_stroke(tab_rect, active_cr, (b, focused_c), StrokeKind::Inside);
            }
        }

        if i > 0 && !is_active && i != active_index + 1 {
            ui.painter().rect_filled(
                Rect::from_min_size(pos2(tab_rect.min.x - b / 2.0, oy), vec2(b, th)),
                CornerRadius::ZERO,
                border_c,
            );
        }

        let response = ui.interact(
            tab_rect,
            egui::Id::new(("tab", container_id, i)),
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

/// Lets a rounded corner display two colours along its arc so a
/// spawn-indicator edge tints only the half of the arc adjacent to the flagged
/// edge rather than the entire 90-degree sweep.
/// When `top == bottom` the two strokes coincide and produce pixel-identical
/// output to a single full-corner stroke.
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

/// When r == w/2 or h/2, corner clips cover everything and edges have zero width, which is fine.
fn effective_radius(r: f32, w: f32, h: f32) -> f32 {
    r.max(0.0).min(w / 2.0).min(h / 2.0)
}

fn cr_u8(r: f32) -> u8 {
    r.clamp(0.0, 255.0) as u8
}

/// A quarter of the tab-bar thickness gives a visibly softer corner than
/// `WINDOW_BORDER_RADIUS_LOGICAL` while still scaling with the
/// user-configured bar thickness.
fn tab_bar_corner_radius(tab_bar_height: f32) -> f32 {
    effective_radius(tab_bar_height * 0.25, tab_bar_height, tab_bar_height)
}

/// Keeps the active tab's outer corners matching the tab bar wherever the tab
/// sits on a tab-bar outer corner. Assumes the tab bar has all four outer
/// corners rounded with `tab_cr`, so this needs updating in lockstep with the
/// tab-bar outline.
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
    frame: Dimension<Logical>,
    visible_frame: Dimension<Logical>,
    b: f32,
    r: f32,
    colors: [Color32; 4],
    focused: Color32,
    origin: egui::Vec2,
) {
    let ox = origin.x + frame.x.logical() - visible_frame.x.logical();
    let oy = origin.y + frame.y.logical() - visible_frame.y.logical();
    let w = frame.width.logical();
    let h = frame.height.logical();
    let r = effective_radius(r, w, h);

    // When r==0, clip rects for the 8-region approach collapse to zero dimensions
    // and egui skips them entirely.
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

    paint_split_corner(
        painter,
        Rect::from_min_size(pos2(ox, oy), vec2(r, r)),
        full_rect,
        cr,
        b,
        colors[0],
        colors[3],
    );
    paint_split_corner(
        painter,
        Rect::from_min_size(pos2(ox + w - r, oy), vec2(r, r)),
        full_rect,
        cr,
        b,
        colors[0],
        colors[1],
    );
    paint_split_corner(
        painter,
        Rect::from_min_size(pos2(ox, oy + h - r), vec2(r, r)),
        full_rect,
        cr,
        b,
        colors[3],
        colors[2],
    );
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
        if c[0] != focused { c[0] } else { c[3] },
        if c[0] != focused { c[0] } else { c[1] },
        if c[2] != focused { c[2] } else { c[3] },
        if c[2] != focused { c[2] } else { c[1] },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Length;

    #[test]
    fn effective_radius_cases() {
        let cases = [
            (8.0, 100.0, 200.0, 8.0),
            (60.0, 100.0, 200.0, 50.0),
            (60.0, 200.0, 80.0, 40.0),
            (0.0, 100.0, 100.0, 0.0),
            (10.0, 6.0, 4.0, 2.0),
            (-5.0, 100.0, 100.0, 0.0),
        ];
        for (r, w, h, expected) in cases {
            assert_eq!(
                effective_radius(r, w, h),
                expected,
                "case (r={r}, w={w}, h={h})"
            );
        }
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

    #[test]
    fn logical_dimension_constructor_sanity() {
        let ld = Dimension::<Logical>::new(
            Length::new(1.0),
            Length::new(2.0),
            Length::new(3.0),
            Length::new(4.0),
        );
        assert_eq!(ld.x, Length::new(1.0));
        assert_eq!(ld.y, Length::new(2.0));
        assert_eq!(ld.width, Length::new(3.0));
        assert_eq!(ld.height, Length::new(4.0));
    }
}
