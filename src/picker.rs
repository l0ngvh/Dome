use std::collections::HashMap;

use egui::{Button, CentralPanel, Image, ScrollArea, TextureHandle, load::SizedTexture};

use crate::core::WindowId;

#[derive(Clone, Debug)]
pub(crate) struct PickerEntry {
    pub(crate) id: WindowId,
    pub(crate) title: String,
    pub(crate) app_id: Option<String>,
}

pub(crate) fn build_picker_entries(
    minimized: &[(WindowId, String)],
    lookup_app_id: impl Fn(WindowId) -> Option<String>,
) -> Vec<PickerEntry> {
    minimized
        .iter()
        .map(|(id, title)| PickerEntry {
            id: *id,
            title: title.clone(),
            app_id: lookup_app_id(*id),
        })
        .collect()
}

pub(crate) enum PickerResult {
    None,
    Selected(WindowId),
}

const ICON_SIZE: f32 = 24.0;

/// Renders the minimized windows picker UI. Returns `Selected(id)` if a row was clicked.
/// Dark visuals must be set once at context creation time, not here.
pub(crate) fn paint_picker(
    ctx: &egui::Context,
    entries: &[PickerEntry],
    selected_index: usize,
    icon_textures: &HashMap<String, Option<TextureHandle>>,
) -> PickerResult {
    let mut result = PickerResult::None;

    CentralPanel::default().show(ctx, |ui| {
        if entries.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No minimized windows");
            });
            return;
        }
        ScrollArea::vertical().show(ui, |ui| {
            for (i, entry) in entries.iter().enumerate() {
                let text = if entry.title.is_empty() {
                    "Untitled"
                } else {
                    entry.title.as_str()
                };
                let icon = entry
                    .app_id
                    .as_ref()
                    .and_then(|id| icon_textures.get(id))
                    .and_then(|opt| opt.as_ref());
                let _response = ui.allocate_ui([ui.available_width(), 28.0].into(), |ui| {
                    ui.horizontal(|ui| {
                        if let Some(texture) = icon {
                            ui.add(Image::from_texture(SizedTexture::new(
                                texture.id(),
                                [ICON_SIZE, ICON_SIZE],
                            )));
                        } else {
                            ui.allocate_space([ICON_SIZE, ICON_SIZE].into());
                        }
                        let label = ui.add(Button::new(text).selected(i == selected_index));
                        if label.clicked() {
                            result = PickerResult::Selected(entry.id);
                        }
                    });
                });
            }
        });
    });

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
            id if id == w1 => Some("com.app.one".to_string()),
            id if id == w3 => Some("com.app.three".to_string()),
            _ => None,
        });
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].id, w1);
        assert_eq!(entries[0].title, "Window One");
        assert_eq!(entries[0].app_id.as_deref(), Some("com.app.one"));
        assert_eq!(entries[1].id, w2);
        assert_eq!(entries[1].title, "Window Two");
        assert!(entries[1].app_id.is_none());
        assert_eq!(entries[2].id, w3);
        assert_eq!(entries[2].title, "Window Three");
        assert_eq!(entries[2].app_id.as_deref(), Some("com.app.three"));
    }

    #[test]
    fn build_entries_empty_input() {
        let entries = build_picker_entries(&[], |_| None);
        assert!(entries.is_empty());
    }

    #[test]
    fn build_entries_duplicate_app_id() {
        let mut hub = test_hub();
        let w1 = hub.insert_tiling();
        let w2 = hub.insert_tiling();
        let minimized = vec![(w1, "Chrome 1".to_string()), (w2, "Chrome 2".to_string())];
        let entries = build_picker_entries(&minimized, |_| Some("com.google.Chrome".to_string()));
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
            },
            PickerEntry {
                id: w2,
                title: "Win B".to_string(),
                app_id: Some("app-b".to_string()),
            },
            PickerEntry {
                id: w3,
                title: "Win C".to_string(),
                app_id: None,
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
        let output = ctx.run(raw, |ctx| {
            result = paint_picker(ctx, &entries, selected_index, &empty);
        });
        assert!(!output.shapes.is_empty());
        assert!(matches!(result, PickerResult::None));
    }
}
