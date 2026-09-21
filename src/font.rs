use std::sync::Arc;

use egui::{Context, FontData, FontDefinitions, FontFamily, FontId, TextStyle};

// Minimum validated font size. Smaller values produce unreadable glyphs.
pub(crate) const MIN_FONT_SIZE: f32 = 4.0;
// Above this the tab bar overflows.
pub(crate) const MAX_FONT_SIZE: f32 = 128.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FontConfig {
    pub(crate) size: f32,
    pub(crate) family: Option<String>,
}

impl FontConfig {
    /// Sets egui's body text size. `self.size` goes in unmultiplied, because
    /// egui scales it by `pixels_per_point` at render.
    pub(crate) fn apply_to(&self, ctx: &Context) {
        ctx.global_style_mut(|s| {
            s.text_styles.insert(
                TextStyle::Body,
                FontId::new(self.size, FontFamily::Proportional),
            );
        });
    }
}

pub(crate) fn install_fonts(bytes: Vec<u8>, ctx: &Context) {
    let mut defs = FontDefinitions::default();
    let key = "user_font".to_string();
    defs.font_data
        .insert(key.clone(), Arc::new(FontData::from_owned(bytes)));
    defs.families
        .entry(FontFamily::Proportional)
        .or_default()
        .push(key);
    ctx.set_fonts(defs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_to_sets_body_size() {
        let ctx = egui::Context::default();
        let fc = FontConfig {
            size: 20.0,
            family: None,
        };
        fc.apply_to(&ctx);
        let style = ctx.global_style();
        assert_eq!(style.text_styles[&TextStyle::Body].size, 20.0);
    }
}
