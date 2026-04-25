use egui::{CentralPanel, ScrollArea, SelectableLabel};

use crate::core::WindowId;

pub(crate) enum PickerResult {
    None,
    Selected(WindowId),
}

/// Renders the minimized windows picker UI. Returns `Selected(id)` if a row was clicked.
/// Dark visuals must be set once at context creation time, not here.
pub(crate) fn paint_picker(
    ctx: &egui::Context,
    entries: &[(WindowId, String)],
    selected_index: usize,
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
            for (i, (id, title)) in entries.iter().enumerate() {
                let text = if title.is_empty() {
                    "Untitled"
                } else {
                    title.as_str()
                };
                let response = ui.add_sized(
                    [ui.available_width(), 28.0],
                    SelectableLabel::new(i == selected_index, text),
                );
                if response.clicked() {
                    result = PickerResult::Selected(*id);
                }
            }
        });
    });

    result
}
