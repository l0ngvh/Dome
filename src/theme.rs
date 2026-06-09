use egui::Color32;
use serde::Deserialize;

// Mocha is the darkest flavour and matches Dome's pre-theme default palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Flavor {
    Latte,
    Frappe,
    Macchiato,
    #[default]
    Mocha,
}

impl Flavor {
    /// Maps to the corresponding `catppuccin_egui` theme constant for
    /// `catppuccin_egui::set_theme`. Kept separate from `Theme::from_flavor`
    /// because that resolves Dome's own painted colours, while this drives
    /// egui's built-in widget chrome.
    pub(crate) fn catppuccin_egui(self) -> catppuccin_egui::Theme {
        match self {
            Flavor::Latte => catppuccin_egui::LATTE,
            Flavor::Frappe => catppuccin_egui::FRAPPE,
            Flavor::Macchiato => catppuccin_egui::MACCHIATO,
            Flavor::Mocha => catppuccin_egui::MOCHA,
        }
    }
}

// DTO: a resolved palette with no invariants. pub(crate) fields are intentional.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Theme {
    pub(crate) focused_border: Color32,
    pub(crate) unfocused_border: Color32,
    pub(crate) spawn_indicator: Color32,
    pub(crate) tab_bar_bg: Color32,
    pub(crate) active_tab_bg: Color32,
    pub(crate) tab_text: Color32,
    pub(crate) picker_selected_row: Color32,
    pub(crate) picker_hover_row: Color32,
    pub(crate) picker_separator: Color32,
    pub(crate) picker_title_text: Color32,
    pub(crate) picker_subtext: Color32,
    pub(crate) picker_empty_text: Color32,
}

impl Theme {
    pub(crate) fn from_flavor(flavor: Flavor) -> Self {
        let p = match flavor {
            Flavor::Latte => catppuccin_egui::LATTE,
            Flavor::Frappe => catppuccin_egui::FRAPPE,
            Flavor::Macchiato => catppuccin_egui::MACCHIATO,
            Flavor::Mocha => catppuccin_egui::MOCHA,
        };
        Self {
            focused_border: p.blue,
            unfocused_border: p.surface1,
            spawn_indicator: p.peach,
            tab_bar_bg: p.mantle,
            active_tab_bg: p.surface1,
            tab_text: p.text,
            picker_selected_row: p.surface2,
            picker_hover_row: p.surface1,
            picker_separator: p.surface0,
            picker_title_text: p.text,
            picker_subtext: p.subtext0,
            picker_empty_text: p.overlay1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flavor_default_is_mocha() {
        assert_eq!(Flavor::default(), Flavor::Mocha);
    }

    #[test]
    fn flavor_deserializes_lowercase() {
        #[derive(Deserialize)]
        struct W {
            theme: Flavor,
        }
        for (input, expected) in [
            ("latte", Flavor::Latte),
            ("frappe", Flavor::Frappe),
            ("macchiato", Flavor::Macchiato),
            ("mocha", Flavor::Mocha),
        ] {
            let toml_str = format!("theme = \"{input}\"");
            let w: W = toml::from_str(&toml_str).unwrap();
            assert_eq!(w.theme, expected);
        }
    }

    #[test]
    fn flavor_rejects_unknown() {
        #[derive(Deserialize)]
        struct W {
            #[expect(dead_code, reason = "only testing deserialization failure")]
            theme: Flavor,
        }
        assert!(toml::from_str::<W>(r#"theme = "dracula""#).is_err());
    }

    #[test]
    fn from_flavor_produces_distinct_themes() {
        let latte = Theme::from_flavor(Flavor::Latte);
        let mocha = Theme::from_flavor(Flavor::Mocha);
        // Latte is light, Mocha is dark: their blue values differ.
        assert_ne!(latte.focused_border, mocha.focused_border);
        // Within Mocha, focused_border (blue) differs from unfocused_border (surface1).
        assert_ne!(mocha.focused_border, mocha.unfocused_border);
    }

    #[test]
    fn all_flavors_resolve() {
        for flavor in [
            Flavor::Latte,
            Flavor::Frappe,
            Flavor::Macchiato,
            Flavor::Mocha,
        ] {
            // Passes if none panic. Catches palette field renames in catppuccin-egui early.
            let _ = Theme::from_flavor(flavor);
        }
    }

    #[test]
    fn flavor_catppuccin_egui_all_variants_resolve() {
        for flavor in [
            Flavor::Latte,
            Flavor::Frappe,
            Flavor::Macchiato,
            Flavor::Mocha,
        ] {
            // Passes if none panic. Catches palette constant renames early.
            let _ = flavor.catppuccin_egui();
        }
    }

    #[test]
    fn flavor_catppuccin_egui_latte_differs_from_mocha() {
        // catppuccin_egui::Theme does not derive PartialEq, so compare a single
        // Color32 field known to differ (Latte is light, Mocha is dark).
        assert_ne!(
            Flavor::Latte.catppuccin_egui().base,
            Flavor::Mocha.catppuccin_egui().base,
        );
    }

    // Catches a regression that re-introduces a global visuals override
    // (e.g. Visuals::dark()) that ignores the active flavor: both calls
    // would return the same fill and the assertion would fail.
    #[test]
    fn catppuccin_set_theme_panel_fill_tracks_flavor() {
        use egui::Context;

        fn panel_fill(flavor: Flavor) -> egui::Color32 {
            let ctx = Context::default();
            catppuccin_egui::set_theme(&ctx, flavor.catppuccin_egui());
            ctx.style().visuals.panel_fill
        }

        assert_ne!(panel_fill(Flavor::Latte), panel_fill(Flavor::Mocha));
    }
}
