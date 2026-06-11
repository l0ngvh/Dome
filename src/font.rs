// Font-size invariant: FontConfig::apply_to writes config.font.text_size into
// egui::Style::text_styles unchanged. egui rasterises glyphs at text_size * pixels_per_point
// physical pixels (text_size * monitor.scale on Windows, text_size * backingScaleFactor on macOS).
// Same mechanism that rescales overlay strokes and corner radii -- do not multiply text_size here.

use std::sync::Arc;

use egui::{Context, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use serde::Deserialize;

// Minimum validated font size. Smaller values produce unreadable glyphs.
pub(crate) const MIN_FONT_SIZE: f32 = 4.0;
// Upper bound for validated font sizes. Above this the UI breaks layout
// (tabs overflow, picker rows overlap); catches obvious typos at load time.
pub(crate) const MAX_FONT_SIZE: f32 = 128.0;

// DTO: a pair of font sizes with no invariants beyond the validation range.
// pub(crate) fields are intentional (plain data, mirrors Flavor/Theme pattern).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct FontConfig {
    #[serde(default = "default_text_size")]
    pub(crate) text_size: f32,
    #[serde(default = "default_subtext_size")]
    pub(crate) subtext_size: f32,
    #[serde(default)]
    pub(crate) family: Option<String>,
}

pub(crate) fn default_text_size() -> f32 {
    14.0
}

pub(crate) fn default_subtext_size() -> f32 {
    12.0
}

// Default preserves today's hardcoded appearance (14pt body, 12pt subtext).
impl Default for FontConfig {
    fn default() -> Self {
        Self {
            text_size: default_text_size(),
            subtext_size: default_subtext_size(),
            family: None,
        }
    }
}

impl FontConfig {
    /// Pins egui's `TextStyle::Body` and `TextStyle::Small` to the configured
    /// sizes. Must land atomically with the call-site switch from `.size(N)` to
    /// `.text_style(TextStyle::Body|Small)`, otherwise picker subtext would
    /// shrink to egui's default Small (10pt) instead of our 12pt.
    pub(crate) fn apply_to(&self, ctx: &Context) {
        ctx.style_mut(|s| {
            s.text_styles.insert(
                TextStyle::Body,
                FontId::new(self.text_size, FontFamily::Proportional),
            );
            s.text_styles.insert(
                TextStyle::Small,
                FontId::new(self.subtext_size, FontFamily::Proportional),
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
    fn font_defaults() {
        let fc = FontConfig::default();
        assert_eq!(fc.text_size, 14.0);
        assert_eq!(fc.subtext_size, 12.0);
        assert_eq!(fc.family, None);
    }

    #[test]
    fn font_config_deserializes_sizes() {
        let fc: FontConfig = toml::from_str("text_size = 18.0\nsubtext_size = 15.0").unwrap();
        assert_eq!(fc.text_size, 18.0);
        assert_eq!(fc.subtext_size, 15.0);
    }

    #[test]
    fn apply_to_sets_body_and_small_sizes() {
        let ctx = egui::Context::default();
        let fc = FontConfig {
            text_size: 20.0,
            subtext_size: 11.0,
            family: None,
        };
        fc.apply_to(&ctx);
        let style = ctx.style();
        assert_eq!(style.text_styles[&TextStyle::Body].size, 20.0);
        assert_eq!(style.text_styles[&TextStyle::Small].size, 11.0);
    }

    #[test]
    fn font_config_deserializes_family() {
        let fc: FontConfig = toml::from_str(
            "text_size = 14.0\nsubtext_size = 12.0\nfamily = \"Microsoft YaHei UI\"",
        )
        .unwrap();
        assert_eq!(fc.family, Some("Microsoft YaHei UI".into()));
    }

    #[test]
    fn font_config_default_family_is_none() {
        assert_eq!(FontConfig::default().family, None);
    }
}
