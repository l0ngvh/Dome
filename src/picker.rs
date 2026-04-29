use std::collections::HashMap;

use egui::{
    Align, CentralPanel, Color32, CornerRadius, Image, Label, Layout, Rect, RichText, ScrollArea,
    Sense, TextureHandle, UiBuilder, load::SizedTexture, pos2, vec2,
};

use crate::core::WindowId;

#[derive(Clone, Debug)]
pub(crate) struct PickerEntry {
    pub(crate) id: WindowId,
    pub(crate) title: String,
    pub(crate) app_id: Option<String>,
    pub(crate) app_name: Option<String>,
}

/// The closure returns `(app_id, app_name)` for each window: `app_id` is the
/// icon-cache key, `app_name` is a human-readable display string.
pub(crate) fn build_picker_entries(
    minimized: &[(WindowId, String)],
    lookup: impl Fn(WindowId) -> (Option<String>, Option<String>),
) -> Vec<PickerEntry> {
    minimized
        .iter()
        .map(|(id, title)| {
            let (app_id, app_name) = lookup(*id);
            PickerEntry {
                id: *id,
                title: title.clone(),
                app_id,
                app_name,
            }
        })
        .collect()
}

pub(crate) enum PickerResult {
    None,
    Selected(WindowId),
}

const ICON_SIZE: f32 = 24.0;
const ROW_HEIGHT: f32 = 40.0;
const H_PAD: f32 = 12.0;

/// Returns `egui::Visuals::dark()` with `panel_fill` overridden to a slightly
/// darker gray (30,30,30); row chrome (selection, hover, separator) is painted
/// directly in `paint_picker` and is not derived from these visuals.
pub(crate) fn picker_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_gray(30);
    v
}

// Separator is drawn at the top of each row except (a) the first row and
// (b) the rows immediately above and below the selected row, so the
// selected-row highlight is not bisected by a separator line.
fn should_draw_separator(index: usize, selected_index: usize) -> bool {
    index > 0 && index != selected_index && index != selected_index + 1
}

/// Renders the minimized windows picker UI. Returns `Selected(id)` if a row was clicked.
/// Dark visuals must be set once at context creation time, not here.
pub(crate) fn paint_picker(
    ctx: &egui::Context,
    entries: &[PickerEntry],
    selected_index: usize,
    icon_textures: &HashMap<String, Option<TextureHandle>>,
) -> PickerResult {
    let corner_radius: CornerRadius = CornerRadius::same(6);
    let mem_id = egui::Id::new("picker_last_selected_index");
    let mut result = PickerResult::None;

    CentralPanel::default().show(ctx, |ui| {
        if entries.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("No minimized windows")
                        .size(14.0)
                        .color(Color32::from_gray(100)),
                );
            });
            return;
        }

        let prev_selected = ctx.data(|d| d.get_temp::<Option<usize>>(mem_id)).flatten();

        ScrollArea::vertical().show(ui, |ui| {
            for (i, entry) in entries.iter().enumerate() {
                let row_rect =
                    Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), ROW_HEIGHT));
                let response = ui.allocate_rect(row_rect, Sense::click());

                if response.clicked() {
                    result = PickerResult::Selected(entry.id);
                }

                // Background: selected or hovered
                if i == selected_index {
                    ui.painter().rect_filled(
                        row_rect,
                        corner_radius,
                        Color32::from_rgb(60, 100, 180),
                    );
                } else if response.hovered() {
                    ui.painter()
                        .rect_filled(row_rect, CornerRadius::ZERO, Color32::from_gray(50));
                }

                // Scroll into view when selection changes
                if i == selected_index && prev_selected != Some(selected_index) {
                    ui.scroll_to_rect(row_rect, Some(Align::Center));
                }

                // Separator line between rows (skipped around the selected row)
                if should_draw_separator(i, selected_index) {
                    let sep_rect = Rect::from_min_size(
                        pos2(row_rect.min.x + H_PAD, row_rect.min.y),
                        vec2(row_rect.width() - H_PAD * 2.0, 1.0),
                    );
                    ui.painter()
                        .rect_filled(sep_rect, CornerRadius::ZERO, Color32::from_gray(45));
                }

                // Icon
                let icon = entry
                    .app_id
                    .as_ref()
                    .and_then(|id| icon_textures.get(id))
                    .and_then(|o| o.as_ref());

                let inner = row_rect.shrink2(vec2(H_PAD, 0.0));

                // Icon pass (left-aligned)
                let mut icon_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                if let Some(tex) = icon {
                    icon_ui.add(Image::from_texture(SizedTexture::new(
                        tex.id(),
                        [ICON_SIZE, ICON_SIZE],
                    )));
                } else {
                    icon_ui.allocate_space(vec2(ICON_SIZE, ICON_SIZE));
                }
                icon_ui.add_space(8.0);

                // Text pass (right-to-left so app name sits on the right)
                let text_rect =
                    Rect::from_min_max(pos2(inner.min.x + ICON_SIZE + 8.0, inner.min.y), inner.max);
                let mut text_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(text_rect)
                        .layout(Layout::right_to_left(Align::Center)),
                );

                // App name on the right (added first in RTL layout)
                if entry.app_name.as_deref().is_some_and(|s| !s.is_empty()) {
                    text_ui.add(Label::new(
                        RichText::new(entry.app_name.as_deref().unwrap_or(""))
                            .size(12.0)
                            .color(Color32::from_gray(100)),
                    ));
                    text_ui.add_space(8.0);
                }

                // Title (truncated, fills remaining space)
                let title_text = if entry.title.is_empty() {
                    "Untitled"
                } else {
                    &entry.title
                };
                text_ui.add(
                    Label::new(
                        RichText::new(title_text)
                            .size(14.0)
                            .color(Color32::from_gray(230)),
                    )
                    .truncate(),
                );
            }
        });
    });

    ctx.data_mut(|d| d.insert_temp(mem_id, Some(selected_index)));

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::core::{Dimension, Hub};

    fn test_hub() -> Hub {
        Hub::new(
            Dimension {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            Config::default().into(),
        )
    }

    #[test]
    fn build_entries_resolves_app_id() {
        let mut hub = test_hub();
        let w1 = hub.insert_tiling();
        let w2 = hub.insert_tiling();
        let w3 = hub.insert_tiling();
        let minimized = vec![
            (w1, "Window One".to_string()),
            (w2, "Window Two".to_string()),
            (w3, "Window Three".to_string()),
        ];
        let entries = build_picker_entries(&minimized, |id| match id {
            id if id == w1 => (Some("com.app.one".to_string()), Some("App One".to_string())),
            id if id == w3 => (
                Some("com.app.three".to_string()),
                Some("App Three".to_string()),
            ),
            _ => (None, None),
        });
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].id, w1);
        assert_eq!(entries[0].title, "Window One");
        assert_eq!(entries[0].app_id.as_deref(), Some("com.app.one"));
        assert_eq!(entries[0].app_name.as_deref(), Some("App One"));
        assert_eq!(entries[1].id, w2);
        assert_eq!(entries[1].title, "Window Two");
        assert!(entries[1].app_id.is_none());
        assert!(entries[1].app_name.is_none());
        assert_eq!(entries[2].id, w3);
        assert_eq!(entries[2].title, "Window Three");
        assert_eq!(entries[2].app_id.as_deref(), Some("com.app.three"));
        assert_eq!(entries[2].app_name.as_deref(), Some("App Three"));
    }

    #[test]
    fn build_entries_empty_input() {
        let entries = build_picker_entries(&[], |_| (None, None));
        assert!(entries.is_empty());
    }

    #[test]
    fn build_entries_duplicate_app_id() {
        let mut hub = test_hub();
        let w1 = hub.insert_tiling();
        let w2 = hub.insert_tiling();
        let minimized = vec![(w1, "Chrome 1".to_string()), (w2, "Chrome 2".to_string())];
        let entries = build_picker_entries(&minimized, |_| {
            (
                Some("com.google.Chrome".to_string()),
                Some("Google Chrome".to_string()),
            )
        });
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].app_id.as_deref(), Some("com.google.Chrome"));
        assert_eq!(entries[1].app_id.as_deref(), Some("com.google.Chrome"));
    }

    /// Smoke test: paint_picker does not panic across all icon states and produces
    /// renderable output. Headless egui cannot verify pixel-level rendering; visual
    /// correctness is verified by manual testing.
    #[test]
    fn paint_picker_with_icons() {
        use egui::{Color32, ColorImage, RawInput};

        let mut hub = test_hub();
        let w1 = hub.insert_tiling();
        let w2 = hub.insert_tiling();
        let w3 = hub.insert_tiling();
        let entries = vec![
            PickerEntry {
                id: w1,
                title: "Win A".to_string(),
                app_id: Some("app-a".to_string()),
                app_name: Some("App A".into()),
            },
            PickerEntry {
                id: w2,
                title: "Win B".to_string(),
                app_id: Some("app-b".to_string()),
                app_name: Some("App B".into()),
            },
            PickerEntry {
                id: w3,
                title: "Win C".to_string(),
                app_id: None,
                app_name: None,
            },
        ];
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "test",
            ColorImage::new([2, 2], vec![Color32::RED; 4]),
            Default::default(), // TextureOptions default is fine for a test texture
        );
        let selected_index = 0;

        // Frame 1: all loaded
        let mut all_loaded = HashMap::new();
        all_loaded.insert("app-a".to_string(), Some(texture.clone()));
        all_loaded.insert("app-b".to_string(), Some(texture.clone()));
        let raw = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(400.0, 300.0),
            )),
            ..Default::default()
        };
        let mut result = PickerResult::None;
        let output = ctx.run(raw.clone(), |ctx| {
            result = paint_picker(ctx, &entries, selected_index, &all_loaded);
        });
        assert!(!output.shapes.is_empty());
        assert!(matches!(result, PickerResult::None));

        // Frame 2: mixed (loaded + in-flight None)
        let mut mixed = HashMap::new();
        mixed.insert("app-a".to_string(), Some(texture.clone()));
        mixed.insert("app-b".to_string(), None);
        let mut result = PickerResult::None;
        let output = ctx.run(raw.clone(), |ctx| {
            result = paint_picker(ctx, &entries, selected_index, &mixed);
        });
        assert!(!output.shapes.is_empty());
        assert!(matches!(result, PickerResult::None));

        // Frame 3: empty map
        let empty: HashMap<String, Option<TextureHandle>> = HashMap::new();
        let mut result = PickerResult::None;
        let output = ctx.run(raw.clone(), |ctx| {
            result = paint_picker(ctx, &entries, selected_index, &empty);
        });
        assert!(!output.shapes.is_empty());
        assert!(matches!(result, PickerResult::None));

        // Frame 4: selected_index changes to 1, exercises scroll_to_rect branch
        let mut result = PickerResult::None;
        let output = ctx.run(raw.clone(), |ctx| {
            result = paint_picker(ctx, &entries, 1, &empty);
        });
        assert!(!output.shapes.is_empty());
        assert!(matches!(result, PickerResult::None));

        // Frame 5: selected_index stays at 1, exercises the skip branch
        let mut result = PickerResult::None;
        let output = ctx.run(raw.clone(), |ctx| {
            result = paint_picker(ctx, &entries, 1, &empty);
        });
        assert!(!output.shapes.is_empty());
        assert!(matches!(result, PickerResult::None));
    }

    #[test]
    fn should_draw_separator_cases() {
        assert!(!should_draw_separator(0, 3)); // never before the first row
        assert!(!should_draw_separator(3, 3)); // the selected row itself
        assert!(!should_draw_separator(4, 3)); // immediately below selected
        assert!(!should_draw_separator(1, 0)); // immediately below selected
        assert!(should_draw_separator(5, 1)); // far from selected
        assert!(should_draw_separator(2, 5)); // far from selected, before it
    }
}
