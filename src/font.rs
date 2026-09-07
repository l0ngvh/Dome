use std::sync::Arc;

use egui::{Context, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use serde::Deserialize;

// Minimum validated font size. Smaller values produce unreadable glyphs.
pub(crate) const MIN_FONT_SIZE: f32 = 4.0;
// Upper bound for validated font sizes. Above this the UI breaks layout,
// tabs overflow. Catches obvious typos at load time.
pub(crate) const MAX_FONT_SIZE: f32 = 128.0;

// pub(crate) fields are intentional (plain data, mirrors Flavor/Theme pattern).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct FontConfig {
    #[serde(rename = "font_size")]
    pub(crate) size: f32,
    #[serde(rename = "font_family")]
    pub(crate) family: Option<String>,
}

pub(crate) fn default_font_size() -> f32 {
    14.0
}

// Default preserves today's hardcoded appearance (14pt body).
impl Default for FontConfig {
    fn default() -> Self {
        Self {
            size: default_font_size(),
            family: None,
        }
    }
}

impl FontConfig {
    /// Sets egui's body text size. egui scales it by `pixels_per_point` at
    /// render, so pass the configured size unmultiplied.
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
    fn font_defaults() {
        let fc = FontConfig::default();
        assert_eq!(fc.size, 14.0);
        assert_eq!(fc.family, None);
    }

    #[test]
    fn font_config_deserializes_sizes() {
        let fc: FontConfig = toml::from_str("font_size = 18.0").unwrap();
        assert_eq!(fc.size, 18.0);
    }

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

    #[test]
    fn font_config_deserializes_family() {
        let fc: FontConfig =
            toml::from_str("font_size = 14.0\nfont_family = \"Microsoft YaHei UI\"").unwrap();
        assert_eq!(fc.family, Some("Microsoft YaHei UI".into()));
    }

    #[test]
    fn font_config_default_family_is_none() {
        assert_eq!(FontConfig::default().family, None);
    }
}
