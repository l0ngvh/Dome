// Logical points throughout. Each platform renderer sets egui's pixels_per_point to its monitor
// scale, so strokes, corner radii and rects are rescaled to physical pixels at tessellation
// and must never be pre-multiplied here.

use egui::{
    Align, Color32, CornerRadius, Id, LayerId, Layout, Order, Rect, RichText, Sense, Stroke,
    StrokeKind, TextStyle, pos2, vec2,
};

use crate::theme::Theme;
use dome_core::{ContainerId, Dimension, Direction, FractionalLength, Logical, WindowId};

const CONTAINER_BORDER_RADIUS_LOGICAL: f64 = 12.0;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::platform::keymap::{KeymapPublisher, KeymapView};
    use crate::theme::Flavor;
    use dome_core::config::{LuaRuntime, PreferredLayouts};
    use dome_core::{
        FractionalCoordinate, Hub, MonitorLayout, PixelRect, ReportedMonitor, WindowMatcher,
        WindowMetadata, WindowRestrictions,
    };
    use egui::{Context, Event, Modifiers, PointerButton, RawInput};

    pub(crate) fn tab_bar() -> (Context, ContainerId) {
        let (sender, _receiver) = std::sync::mpsc::channel();
        let mut keymap = KeymapPublisher::new(KeymapView::new(), sender);
        let (runtime, config) = LuaRuntime::load(
            "paint-test.lua".into(),
            Ok(r#"return {
            layout = "partition_tree",
            minimum_width = 1,
            minimum_height = 1,
            partition_tree = { automatic_tiling = false },
            reserved_area = function(monitor) return nil end,
        }"#
            .into()),
            &mut keymap,
        )
        .unwrap();
        let mut hub = Hub::new(
            ReportedMonitor {
                device_name: "Paint test".into(),
                work_area: PixelRect::new(0, 0, 100, 20),
                scale: 1.0,
                cg_display_id: None,
                gdi_device: None,
            },
            config.tiling,
            PreferredLayouts::default(),
            runtime,
        );
        for (index, title) in ["First", "Second"].into_iter().enumerate() {
            hub.insert_window(
                WindowId::new(index + 1),
                Box::new(PaintWindow(title.into())),
                PixelRect::new(0, 0, 100, 20),
                WindowRestrictions::None,
            );
        }
        let placements = hub.get_visible_placements();
        let MonitorLayout::Normal { containers, .. } = &placements.monitors[0].layout else {
            panic!("normal layout");
        };
        let container = containers[0].id;
        let context = Context::default();
        context.set_pixels_per_point(2.0);
        context.global_style_mut(|style| style.interaction.selectable_labels = false);
        for _ in 0..2 {
            assert_eq!(tab_bar_frame(&context, container, Vec::new()), None);
        }
        (context, container)
    }

    pub(crate) fn tab_bar_frame(
        context: &Context,
        container: ContainerId,
        events: Vec<Event>,
    ) -> Option<(ContainerId, usize)> {
        let mut clicked = None;
        let _output = context.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 20.0))),
                events,
                ..Default::default()
            },
            |ui| {
                clicked = paint_tab_bar(
                    ui.ctx(),
                    container,
                    Dimension::<Logical>::new(
                        FractionalCoordinate::ZERO,
                        FractionalCoordinate::ZERO,
                        FractionalLength::new(100.0),
                        FractionalLength::new(20.0),
                    ),
                    &["First".into(), "Second".into()],
                    1,
                    false,
                    BorderMetrics::from_thickness(FractionalLength::ZERO),
                    &Theme::from_flavor(Flavor::Mocha),
                );
            },
        );
        clicked
    }

    pub(crate) fn pointer_button(position: egui::Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn release_primary_pointer_after_separate_press_frame_selects_tab() {
        let (context, container) = tab_bar();
        let position = pos2(40.0, 10.0);
        assert_eq!(
            tab_bar_frame(&context, container, vec![pointer_button(position, true)]),
            None
        );
        assert_eq!(
            tab_bar_frame(&context, container, vec![pointer_button(position, false)]),
            Some((container, 0))
        );
    }

    #[test]
    fn click_tab_with_primary_press_and_release_in_same_frame_selects_tab() {
        let (context, container) = tab_bar();
        let position = pos2(40.0, 10.0);
        assert_eq!(
            tab_bar_frame(
                &context,
                container,
                vec![
                    Event::PointerMoved(position),
                    pointer_button(position, true),
                    pointer_button(position, false),
                ]
            ),
            Some((container, 0))
        );
    }

    #[test]
    fn click_tab_with_secondary_button_does_not_select_tab() {
        let (context, container) = tab_bar();
        let events = [true, false]
            .map(|pressed| Event::PointerButton {
                pos: pos2(40.0, 10.0),
                button: PointerButton::Secondary,
                pressed,
                modifiers: Modifiers::NONE,
            })
            .to_vec();
        assert_eq!(tab_bar_frame(&context, container, events), None);
    }

    fn logical_frame(width: f64, height: f64) -> Dimension<Logical> {
        Dimension::new(
            FractionalCoordinate::ZERO,
            FractionalCoordinate::ZERO,
            FractionalLength::new(width),
            FractionalLength::new(height),
        )
    }

    fn paint_rectangles(mut paint: impl FnMut(&Context)) -> Vec<(Rect, egui::epaint::RectShape)> {
        let context = Context::default();
        context.set_pixels_per_point(2.0);
        context
            .run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(2000.0, 2000.0))),
                    ..Default::default()
                },
                |ui| paint(ui.ctx()),
            )
            .shapes
            .into_iter()
            .filter_map(|shape| match shape.shape {
                egui::Shape::Rect(rectangle) => Some((shape.clip_rect, rectangle)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn paint_float_borders_preserves_square_corners_and_bounds_rounded_rings() {
        let theme = Theme::from_flavor(Flavor::Mocha);
        let frame = logical_frame(100.0, 80.0);
        let square = paint_rectangles(|context| {
            paint_float_border(
                context,
                frame,
                frame,
                true,
                FractionalLength::ZERO,
                &theme,
                BorderMetrics::from_thickness(FractionalLength::new(2.0)),
            );
        });
        assert!(!square.is_empty());
        for (index, (_, rectangle)) in square.iter().enumerate() {
            assert_eq!(rectangle.corner_radius, CornerRadius::ZERO);
            assert_eq!(rectangle.fill, theme.focused_border);
            for (_, other) in &square[index + 1..] {
                assert!(!rectangle.rect.intersect(other.rect).is_positive());
            }
        }
        assert_eq!(
            square
                .iter()
                .map(|(_, shape)| shape.rect.area())
                .sum::<f32>(),
            100.0 * 80.0 - 96.0 * 76.0
        );
        for (frame, radius, expected_radius) in
            [(frame, 8.5, 11), (logical_frame(40.0, 20.0), 100.0, 10)]
        {
            let rounded = paint_rectangles(|context| {
                paint_float_border(
                    context,
                    frame,
                    frame,
                    true,
                    FractionalLength::new(radius),
                    &theme,
                    BorderMetrics::from_thickness(FractionalLength::new(2.25)),
                );
            });
            assert!(!rounded.is_empty());
            let bounds = Rect::from_min_size(
                pos2(0.0, 0.0),
                vec2(frame.width.logical() as f32, frame.height.logical() as f32),
            );
            for (clip, rectangle) in rounded {
                assert_eq!(rectangle.rect, bounds);
                assert!(bounds.contains_rect(clip));
                assert_eq!(rectangle.corner_radius, CornerRadius::same(expected_radius));
                assert_eq!(rectangle.stroke, Stroke::new(2.25, theme.focused_border));
                assert_eq!(rectangle.stroke_kind, StrokeKind::Inside);
            }
        }
    }

    #[test]
    fn paint_tiled_spawn_indicators_colors_only_selected_edges_and_adjacent_half_corners() {
        let theme = Theme::from_flavor(Flavor::Mocha);
        let frame = logical_frame(100.0, 80.0);
        for direction in [None, Some(Direction::Horizontal), Some(Direction::Vertical)] {
            let shapes = paint_rectangles(|context| {
                paint_tiling_overlay(
                    context,
                    frame,
                    &[LogicalTiledWindow {
                        id: WindowId::new(1),
                        frame,
                        visible_frame: frame,
                        is_highlighted: true,
                        spawn_direction: direction,
                        corner_radius: FractionalLength::new(10.0),
                    }],
                    &[],
                    &theme,
                    BorderMetrics::from_thickness(FractionalLength::new(2.0)),
                );
            });
            assert!(!shapes.is_empty());
            let colored_clips: Vec<_> = shapes
                .iter()
                .filter(|(_, shape)| shape.stroke.color == theme.spawn_indicator)
                .map(|(clip, _)| *clip)
                .collect();
            let expected = match direction {
                None => vec![],
                Some(Direction::Horizontal) => vec![
                    Rect::from_min_max(pos2(88.0, 12.0), pos2(100.0, 68.0)),
                    Rect::from_min_max(pos2(88.0, 6.0), pos2(100.0, 12.0)),
                    Rect::from_min_max(pos2(88.0, 68.0), pos2(100.0, 74.0)),
                ],
                Some(Direction::Vertical) => vec![
                    Rect::from_min_max(pos2(12.0, 68.0), pos2(88.0, 80.0)),
                    Rect::from_min_max(pos2(0.0, 74.0), pos2(12.0, 80.0)),
                    Rect::from_min_max(pos2(88.0, 74.0), pos2(100.0, 80.0)),
                ],
            };
            assert!(expected.iter().all(|clip| colored_clips.contains(clip)));
            assert!(colored_clips.iter().all(|clip| expected.contains(clip)));
        }
    }

    #[test]
    fn paint_tabs_rounds_outer_corners_and_keeps_body_below_bar() {
        let (_, container) = tab_bar();
        let theme = Theme::from_flavor(Flavor::Mocha);
        for (count, active, expected) in [
            (1, 0, CornerRadius::same(6)),
            (
                3,
                0,
                CornerRadius {
                    nw: 6,
                    sw: 6,
                    ne: 0,
                    se: 0,
                },
            ),
            (3, 1, CornerRadius::ZERO),
            (
                3,
                2,
                CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: 6,
                    se: 6,
                },
            ),
        ] {
            let titles = vec!["Tab".to_string(); count];
            let shapes = paint_rectangles(|context| {
                paint_tab_bar(
                    context,
                    container,
                    logical_frame(120.0, 22.0),
                    &titles,
                    active,
                    true,
                    BorderMetrics::from_thickness(FractionalLength::new(2.0)),
                    &theme,
                );
            });
            let background = shapes
                .iter()
                .find(|(_, shape)| shape.fill == theme.tab_bar_bg)
                .expect("tab bar background");
            assert_eq!(background.1.corner_radius, CornerRadius::same(6));
            let active_shape = shapes
                .iter()
                .find(|(_, shape)| shape.fill == theme.active_tab_bg)
                .expect("active tab background");
            assert_eq!(active_shape.1.corner_radius, expected);
            assert_eq!(
                active_shape.1.rect,
                Rect::from_min_size(
                    pos2(active as f32 * 120.0 / count as f32, 0.0),
                    vec2(120.0 / count as f32, 22.0)
                )
            );
        }
        let frame = logical_frame(100.0, 33.0);
        let titles = ["Tab".to_string()];
        let shapes = paint_rectangles(|context| {
            paint_tiling_overlay(
                context,
                frame,
                &[],
                &[LogicalTiledContainer {
                    id: container,
                    frame,
                    visible_frame: frame,
                    tab_bar_height: FractionalLength::new(22.0),
                    is_highlighted: true,
                    spawn_direction: None,
                    is_tabbed: true,
                    titles: &titles,
                }],
                &theme,
                BorderMetrics::from_thickness(FractionalLength::new(2.0)),
            );
        });
        assert!(!shapes.is_empty());
        for (clip, shape) in shapes {
            assert!(clip.min.y >= 22.0);
            assert_eq!(
                shape.rect,
                Rect::from_min_max(pos2(0.0, 22.0), pos2(100.0, 33.0))
            );
            assert_eq!(
                shape.corner_radius,
                CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 6,
                    se: 6
                }
            );
            assert_eq!(shape.stroke, Stroke::new(2.0, theme.focused_border));
        }
    }

    #[derive(Clone, Debug)]
    struct PaintWindow(String);

    impl std::fmt::Display for PaintWindow {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(&self.0)
        }
    }

    impl WindowMetadata for PaintWindow {
        fn app_name(&self) -> Option<String> {
            None
        }
        fn title(&self) -> Option<&str> {
            Some(&self.0)
        }
        fn set_title(&mut self, title: String) {
            self.0 = title;
        }
        fn clone_box(&self) -> Box<dyn WindowMetadata> {
            Box::new(self.clone())
        }
        fn matches_window_matcher(&self, _: &WindowMatcher) -> bool {
            false
        }
        fn to_window_matcher(&self) -> WindowMatcher {
            WindowMatcher::default()
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BorderMetrics {
    pub thickness: FractionalLength<Logical>,
    pub radius: FractionalLength<Logical>,
}

impl BorderMetrics {
    pub(crate) fn from_thickness(thickness: FractionalLength<Logical>) -> Self {
        Self {
            thickness,
            radius: FractionalLength::new(CONTAINER_BORDER_RADIUS_LOGICAL),
        }
    }

    /// The ring sits outside the window, so its outer radius adds the thickness
    /// to keep the inner edge concentric with the window's corner.
    fn around_window(self, window_corner_radius: FractionalLength<Logical>) -> Self {
        let radius = if window_corner_radius > FractionalLength::ZERO {
            window_corner_radius + self.thickness
        } else {
            FractionalLength::ZERO
        };
        Self { radius, ..self }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LogicalTiledWindow {
    pub id: WindowId,
    pub frame: Dimension<Logical>,
    pub visible_frame: Dimension<Logical>,
    pub is_highlighted: bool,
    pub spawn_direction: Option<Direction>,
    pub corner_radius: FractionalLength<Logical>,
}

#[derive(Clone, Debug)]
pub(crate) struct LogicalTiledContainer<'a> {
    pub id: ContainerId,
    pub frame: Dimension<Logical>,
    pub visible_frame: Dimension<Logical>,
    /// Reserved by core at the top of `frame`. Reading config here instead reopens a sub-unit
    /// seam against the painted bar.
    pub tab_bar_height: FractionalLength<Logical>,
    pub is_highlighted: bool,
    pub spawn_direction: Option<Direction>,
    pub is_tabbed: bool,
    pub titles: &'a [String],
}

/// Paints window borders and the highlighted-container body border in the per-monitor
/// tiling overlay. Tab bars are owned by separate per-`ContainerId` platform windows
/// and reach the painter via `paint_tab_bar`, so this
/// entry point does not paint or hit-test tab bars. The returned click vector
/// is always empty.
pub(crate) fn paint_tiling_overlay(
    ctx: &egui::Context,
    monitor: Dimension<Logical>,
    windows: &[LogicalTiledWindow],
    containers: &[LogicalTiledContainer<'_>],
    theme: &Theme,
    border: BorderMetrics,
) -> Vec<(ContainerId, usize)> {
    for wp in windows {
        let vf = wp.visible_frame;
        let origin = vec2(
            (vf.x.logical() - monitor.x.logical()) as f32,
            (vf.y.logical() - monitor.y.logical()) as f32,
        );
        // layer_painter bypasses egui's Area sizing pass, which makes first-frame
        // output invisible. Window borders are pure painting with no interaction,
        // so Area is unnecessary.
        let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new(("border", wp.id))));
        let clip = Rect::from_min_size(
            origin.to_pos2(),
            vec2(vf.width.logical() as f32, vf.height.logical() as f32),
        );
        paint_window_border(
            &painter.with_clip_rect(clip),
            wp.frame,
            wp.visible_frame,
            wp.is_highlighted,
            wp.spawn_direction,
            theme,
            border.around_window(wp.corner_radius),
            origin,
        );
    }

    for cp in containers {
        let vf = cp.visible_frame;
        let origin = vec2(
            (vf.x.logical() - monitor.x.logical()) as f32,
            (vf.y.logical() - monitor.y.logical()) as f32,
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
                    vec2(vf.width.logical() as f32, vf.height.logical() as f32),
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
    spawn_direction: Option<Direction>,
    theme: &Theme,
    border: BorderMetrics,
    origin: egui::Vec2,
) {
    let colors = border_colors(is_highlighted, spawn_direction, theme);
    paint_border_edges(
        painter,
        frame,
        visible_frame,
        border.thickness.logical() as f32,
        border.radius.logical() as f32,
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
    corner_radius: FractionalLength<Logical>,
    theme: &Theme,
    border: BorderMetrics,
) {
    let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("border")));
    let clip = Rect::from_min_size(
        pos2(0.0, 0.0),
        vec2(
            visible_frame.width.logical() as f32,
            visible_frame.height.logical() as f32,
        ),
    );
    paint_window_border(
        &painter.with_clip_rect(clip),
        frame,
        visible_frame,
        is_highlighted,
        None,
        theme,
        border.around_window(corner_radius),
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
    let origin = vec2(
        tab_bar_frame.x.logical() as f32,
        tab_bar_frame.y.logical() as f32,
    );
    let mut clicked = None;
    egui::Area::new(egui::Id::new(("tab_bar", container_id)))
        .order(egui::Order::Foreground)
        .fixed_pos(origin.to_pos2())
        .constrain(false)
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
                    tab_bar_frame.width.logical() as f32,
                    tab_bar_frame.height.logical() as f32,
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
    placement: &LogicalTiledContainer<'_>,
    theme: &Theme,
    border: BorderMetrics,
    origin: egui::Vec2,
) {
    let vf = placement.visible_frame;
    let f = placement.frame;
    let ox = origin.x + (f.x.logical() - vf.x.logical()) as f32;
    let oy = origin.y + (f.y.logical() - vf.y.logical()) as f32;
    let b = border.thickness.logical() as f32;
    let w = f.width.logical() as f32;
    let h = f.height.logical() as f32;
    let is_tabbed = placement.is_tabbed && !placement.titles.is_empty();
    let th = placement.tab_bar_height.logical() as f32;
    let r = effective_radius(border.radius.logical() as f32, w, h);

    if placement.is_highlighted {
        let colors = border_colors(true, placement.spawn_direction, theme);
        let focused = theme.focused_border;
        let painter = ui.painter();

        if is_tabbed {
            let body_h = h - th;
            if body_h <= 0.0 {
                return;
            }
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
                CONTAINER_BORDER_RADIUS_LOGICAL as f32,
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
    let b = border.thickness.logical() as f32;
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
    if top == bottom {
        stroke_clipped(painter, corner_rect, full_rect, cr, (b, top));
        return;
    }
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
    r.clamp(0.0, 255.0).round() as u8
}

/// Proportional because the user configures the bar height, so the corner keeps its shape at
/// any height.
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
    let ox = origin.x + (frame.x.logical() - visible_frame.x.logical()) as f32;
    let oy = origin.y + (frame.y.logical() - visible_frame.y.logical()) as f32;
    let w = frame.width.logical() as f32;
    let h = frame.height.logical() as f32;
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

/// [top, right, bottom, left] border colors based on highlight state and spawn direction.
fn border_colors(
    is_highlighted: bool,
    spawn_direction: Option<Direction>,
    theme: &Theme,
) -> [Color32; 4] {
    if !is_highlighted {
        return [theme.unfocused_border; 4];
    }
    let Some(direction) = spawn_direction else {
        return [theme.focused_border; 4];
    };
    let f = theme.focused_border;
    let s = theme.spawn_indicator;
    match direction {
        Direction::Horizontal => [f, s, f, f],
        Direction::Vertical => [f, f, s, f],
    }
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
