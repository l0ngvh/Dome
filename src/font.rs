use egui::{Context, FontFamily, FontId, TextStyle};
use serde::Deserialize;

// Minimum validated font size. Smaller values produce unreadable glyphs.
const MIN_FONT_SIZE: f32 = 4.0;
// Upper bound for validated font sizes. Above this the UI breaks layout
// (tabs overflow, picker rows overlap); catches obvious typos at load time.
const MAX_FONT_SIZE: f32 = 128.0;

// DTO: a pair of font sizes with no invariants beyond the validation range.
// pub(crate) fields are intentional (plain data, mirrors Flavor/Theme pattern).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FontConfig {
    #[serde(default = "default_text_size")]
    pub(crate) text_size: f32,
    #[serde(default = "default_subtext_size")]
    pub(crate) subtext_size: f32,
}

fn default_text_size() -> f32 {
    14.0
}

fn default_subtext_size() -> f32 {
    12.0
}

// Default preserves today's hardcoded appearance (14pt body, 12pt subtext).
impl Default for FontConfig {
    fn default() -> Self {
        Self {
            text_size: default_text_size(),
            subtext_size: default_subtext_size(),
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

    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        if !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&self.text_size) {
            anyhow::bail!(
                "font.text_size ({}) must be in [{}, {}]",
                self.text_size,
                MIN_FONT_SIZE,
                MAX_FONT_SIZE,
            );
        }
        if !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&self.subtext_size) {
            anyhow::bail!(
                "font.subtext_size ({}) must be in [{}, {}]",
                self.subtext_size,
                MIN_FONT_SIZE,
                MAX_FONT_SIZE,
            );
        }
        Ok(())
    }
}

/// Trivial wrapper around `!=` that names the invariant at call sites.
/// Mirrors `theme::theme_changed`.
pub(crate) fn font_changed(old: &FontConfig, new: &FontConfig) -> bool {
    old != new
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_defaults() {
        let fc = FontConfig::default();
        assert_eq!(fc.text_size, 14.0);
        assert_eq!(fc.subtext_size, 12.0);
    }

    #[test]
    fn font_config_deserializes_sizes() {
        let fc: FontConfig = toml::from_str("text_size = 18.0\nsubtext_size = 15.0").unwrap();
        assert_eq!(fc.text_size, 18.0);
        assert_eq!(fc.subtext_size, 15.0);
    }

    #[test]
    fn font_config_rejects_unknown_field() {
        assert!(toml::from_str::<FontConfig>("weight = \"bold\"").is_err());
    }

    #[test]
    fn font_validate_rejects_small_text_size() {
        let fc = FontConfig {
            text_size: 3.0,
            subtext_size: 12.0,
        };
        assert!(fc.validate().is_err());
    }

    #[test]
    fn font_validate_rejects_small_subtext() {
        let fc = FontConfig {
            text_size: 14.0,
            subtext_size: 3.0,
        };
        assert!(fc.validate().is_err());
    }

    #[test]
    fn font_validate_accepts_min() {
        let fc = FontConfig {
            text_size: MIN_FONT_SIZE,
            subtext_size: MIN_FONT_SIZE,
        };
        assert!(fc.validate().is_ok());
    }

    #[test]
    fn font_validate_rejects_large_text_size() {
        let fc = FontConfig {
            text_size: MAX_FONT_SIZE + 1.0,
            subtext_size: 12.0,
        };
        assert!(fc.validate().is_err());
    }

    #[test]
    fn font_validate_rejects_large_subtext() {
        let fc = FontConfig {
            text_size: 14.0,
            subtext_size: MAX_FONT_SIZE + 1.0,
        };
        assert!(fc.validate().is_err());
    }

    #[test]
    fn font_validate_accepts_max() {
        let fc = FontConfig {
            text_size: MAX_FONT_SIZE,
            subtext_size: MAX_FONT_SIZE,
        };
        assert!(fc.validate().is_ok());
    }

    #[test]
    fn apply_to_sets_body_and_small_sizes() {
        let ctx = egui::Context::default();
        let fc = FontConfig {
            text_size: 20.0,
            subtext_size: 11.0,
        };
        fc.apply_to(&ctx);
        let style = ctx.style();
        assert_eq!(style.text_styles[&TextStyle::Body].size, 20.0);
        assert_eq!(style.text_styles[&TextStyle::Small].size, 11.0);
    }

    #[test]
    fn font_changed_detects_changes() {
        let a = FontConfig {
            text_size: 14.0,
            subtext_size: 12.0,
        };
        let b = FontConfig {
            text_size: 14.0,
            subtext_size: 12.0,
        };
        assert!(!font_changed(&a, &b));

        let c = FontConfig {
            text_size: 16.0,
            subtext_size: 12.0,
        };
        assert!(font_changed(&a, &c));

        let d = FontConfig {
            text_size: 14.0,
            subtext_size: 10.0,
        };
        assert!(font_changed(&a, &d));
    }
}
