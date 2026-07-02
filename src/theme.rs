use egui::Color32;
use egui::epaint::Shadow;
use egui::style::{Selection, WidgetVisuals, Widgets};
use egui::{Stroke, Visuals};
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
        let p = palette(flavor);
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

/// Catppuccin palette, inlined from https://github.com/catppuccin/egui (MIT).
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) struct Palette {
    pub(crate) rosewater: Color32,
    pub(crate) flamingo: Color32,
    pub(crate) pink: Color32,
    pub(crate) mauve: Color32,
    pub(crate) red: Color32,
    pub(crate) maroon: Color32,
    pub(crate) peach: Color32,
    pub(crate) yellow: Color32,
    pub(crate) green: Color32,
    pub(crate) teal: Color32,
    pub(crate) sky: Color32,
    pub(crate) sapphire: Color32,
    pub(crate) blue: Color32,
    pub(crate) lavender: Color32,
    pub(crate) text: Color32,
    pub(crate) subtext1: Color32,
    pub(crate) subtext0: Color32,
    pub(crate) overlay2: Color32,
    pub(crate) overlay1: Color32,
    pub(crate) overlay0: Color32,
    pub(crate) surface2: Color32,
    pub(crate) surface1: Color32,
    pub(crate) surface0: Color32,
    pub(crate) base: Color32,
    pub(crate) mantle: Color32,
    pub(crate) crust: Color32,
}

pub(crate) const LATTE: Palette = Palette {
    rosewater: Color32::from_rgb(220, 138, 120),
    flamingo: Color32::from_rgb(221, 120, 120),
    pink: Color32::from_rgb(234, 118, 203),
    mauve: Color32::from_rgb(136, 57, 239),
    red: Color32::from_rgb(210, 15, 57),
    maroon: Color32::from_rgb(230, 69, 83),
    peach: Color32::from_rgb(254, 100, 11),
    yellow: Color32::from_rgb(223, 142, 29),
    green: Color32::from_rgb(64, 160, 43),
    teal: Color32::from_rgb(23, 146, 153),
    sky: Color32::from_rgb(4, 165, 229),
    sapphire: Color32::from_rgb(32, 159, 181),
    blue: Color32::from_rgb(30, 102, 245),
    lavender: Color32::from_rgb(114, 135, 253),
    text: Color32::from_rgb(76, 79, 105),
    subtext1: Color32::from_rgb(92, 95, 119),
    subtext0: Color32::from_rgb(108, 111, 133),
    overlay2: Color32::from_rgb(124, 127, 147),
    overlay1: Color32::from_rgb(140, 143, 161),
    overlay0: Color32::from_rgb(156, 160, 176),
    surface2: Color32::from_rgb(172, 176, 190),
    surface1: Color32::from_rgb(188, 192, 204),
    surface0: Color32::from_rgb(204, 208, 218),
    base: Color32::from_rgb(239, 241, 245),
    mantle: Color32::from_rgb(230, 233, 239),
    crust: Color32::from_rgb(220, 224, 232),
};

pub(crate) const FRAPPE: Palette = Palette {
    rosewater: Color32::from_rgb(242, 213, 207),
    flamingo: Color32::from_rgb(238, 190, 190),
    pink: Color32::from_rgb(244, 184, 228),
    mauve: Color32::from_rgb(202, 158, 230),
    red: Color32::from_rgb(231, 130, 132),
    maroon: Color32::from_rgb(234, 153, 156),
    peach: Color32::from_rgb(239, 159, 118),
    yellow: Color32::from_rgb(229, 200, 144),
    green: Color32::from_rgb(166, 209, 137),
    teal: Color32::from_rgb(129, 200, 190),
    sky: Color32::from_rgb(153, 209, 219),
    sapphire: Color32::from_rgb(133, 193, 220),
    blue: Color32::from_rgb(140, 170, 238),
    lavender: Color32::from_rgb(186, 187, 241),
    text: Color32::from_rgb(198, 208, 245),
    subtext1: Color32::from_rgb(181, 191, 226),
    subtext0: Color32::from_rgb(165, 173, 206),
    overlay2: Color32::from_rgb(148, 156, 187),
    overlay1: Color32::from_rgb(131, 139, 167),
    overlay0: Color32::from_rgb(115, 121, 148),
    surface2: Color32::from_rgb(98, 104, 128),
    surface1: Color32::from_rgb(81, 87, 109),
    surface0: Color32::from_rgb(65, 69, 89),
    base: Color32::from_rgb(48, 52, 70),
    mantle: Color32::from_rgb(41, 44, 60),
    crust: Color32::from_rgb(35, 38, 52),
};

pub(crate) const MACCHIATO: Palette = Palette {
    rosewater: Color32::from_rgb(244, 219, 214),
    flamingo: Color32::from_rgb(240, 198, 198),
    pink: Color32::from_rgb(245, 189, 230),
    mauve: Color32::from_rgb(198, 160, 246),
    red: Color32::from_rgb(237, 135, 150),
    maroon: Color32::from_rgb(238, 153, 160),
    peach: Color32::from_rgb(245, 169, 127),
    yellow: Color32::from_rgb(238, 212, 159),
    green: Color32::from_rgb(166, 218, 149),
    teal: Color32::from_rgb(139, 213, 202),
    sky: Color32::from_rgb(145, 215, 227),
    sapphire: Color32::from_rgb(125, 196, 228),
    blue: Color32::from_rgb(138, 173, 244),
    lavender: Color32::from_rgb(183, 189, 248),
    text: Color32::from_rgb(202, 211, 245),
    subtext1: Color32::from_rgb(184, 192, 224),
    subtext0: Color32::from_rgb(165, 173, 203),
    overlay2: Color32::from_rgb(147, 154, 183),
    overlay1: Color32::from_rgb(128, 135, 162),
    overlay0: Color32::from_rgb(110, 115, 141),
    surface2: Color32::from_rgb(91, 96, 120),
    surface1: Color32::from_rgb(73, 77, 100),
    surface0: Color32::from_rgb(54, 58, 79),
    base: Color32::from_rgb(36, 39, 58),
    mantle: Color32::from_rgb(30, 32, 48),
    crust: Color32::from_rgb(24, 25, 38),
};

pub(crate) const MOCHA: Palette = Palette {
    rosewater: Color32::from_rgb(245, 224, 220),
    flamingo: Color32::from_rgb(242, 205, 205),
    pink: Color32::from_rgb(245, 194, 231),
    mauve: Color32::from_rgb(203, 166, 247),
    red: Color32::from_rgb(243, 139, 168),
    maroon: Color32::from_rgb(235, 160, 172),
    peach: Color32::from_rgb(250, 179, 135),
    yellow: Color32::from_rgb(249, 226, 175),
    green: Color32::from_rgb(166, 227, 161),
    teal: Color32::from_rgb(148, 226, 213),
    sky: Color32::from_rgb(137, 220, 235),
    sapphire: Color32::from_rgb(116, 199, 236),
    blue: Color32::from_rgb(137, 180, 250),
    lavender: Color32::from_rgb(180, 190, 254),
    text: Color32::from_rgb(205, 214, 244),
    subtext1: Color32::from_rgb(186, 194, 222),
    subtext0: Color32::from_rgb(166, 173, 200),
    overlay2: Color32::from_rgb(147, 153, 178),
    overlay1: Color32::from_rgb(127, 132, 156),
    overlay0: Color32::from_rgb(108, 112, 134),
    surface2: Color32::from_rgb(88, 91, 112),
    surface1: Color32::from_rgb(69, 71, 90),
    surface0: Color32::from_rgb(49, 50, 68),
    base: Color32::from_rgb(30, 30, 46),
    mantle: Color32::from_rgb(24, 24, 37),
    crust: Color32::from_rgb(17, 17, 27),
};

pub(crate) fn palette(flavor: Flavor) -> Palette {
    match flavor {
        Flavor::Latte => LATTE,
        Flavor::Frappe => FRAPPE,
        Flavor::Macchiato => MACCHIATO,
        Flavor::Mocha => MOCHA,
    }
}

/// Sets egui's built-in widget chrome to the Catppuccin palette for `flavor`.
/// Dome-specific painted colours (borders, tab bars, picker rows) come from
/// `Theme::from_flavor` instead.
pub(crate) fn apply_catppuccin(ctx: &egui::Context, flavor: Flavor) {
    let p = palette(flavor);
    let old = ctx.global_style().visuals.clone();
    ctx.set_visuals(visuals_from(&p, old));
}

fn visuals_from(p: &Palette, old: Visuals) -> Visuals {
    let is_latte = *p == LATTE;
    // Latte is light, so its drop shadow needs less alpha to read against a
    // pale base. Darker flavours use the original 96-alpha value.
    let shadow_color = if is_latte {
        Color32::from_black_alpha(25)
    } else {
        Color32::from_black_alpha(96)
    };
    Visuals {
        hyperlink_color: p.rosewater,
        faint_bg_color: p.surface0,
        extreme_bg_color: p.crust,
        code_bg_color: p.mantle,
        warn_fg_color: p.peach,
        error_fg_color: p.maroon,
        window_fill: p.base,
        panel_fill: p.base,
        window_stroke: Stroke {
            color: p.overlay1,
            ..old.window_stroke
        },
        widgets: Widgets {
            noninteractive: make_widget_visual(old.widgets.noninteractive, p, p.base),
            inactive: make_widget_visual(old.widgets.inactive, p, p.surface0),
            hovered: make_widget_visual(old.widgets.hovered, p, p.surface2),
            active: make_widget_visual(old.widgets.active, p, p.surface1),
            open: make_widget_visual(old.widgets.open, p, p.surface0),
        },
        selection: Selection {
            // Latte's blue is more saturated at the same alpha, so its
            // selection fill is scaled harder to stay readable.
            bg_fill: p.blue.linear_multiply(if is_latte { 0.4 } else { 0.2 }),
            stroke: Stroke {
                color: p.text,
                ..old.selection.stroke
            },
        },
        window_shadow: Shadow {
            color: shadow_color,
            ..old.window_shadow
        },
        popup_shadow: Shadow {
            color: shadow_color,
            ..old.popup_shadow
        },
        dark_mode: !is_latte,
        ..old
    }
}

fn make_widget_visual(old: WidgetVisuals, p: &Palette, bg_fill: Color32) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill,
        weak_bg_fill: bg_fill,
        bg_stroke: Stroke {
            color: p.overlay1,
            ..old.bg_stroke
        },
        fg_stroke: Stroke {
            color: p.text,
            ..old.fg_stroke
        },
        ..old
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
            // Passes if none panic. Catches palette field renames early.
            let _ = Theme::from_flavor(flavor);
        }
    }

    #[test]
    fn palette_latte_differs_from_mocha() {
        assert_ne!(palette(Flavor::Latte).base, palette(Flavor::Mocha).base);
    }

    // Catches a regression that re-introduces a global visuals override
    // (e.g. Visuals::dark()) that ignores the active flavor: both calls
    // would return the same fill and the assertion would fail.
    #[test]
    fn apply_catppuccin_panel_fill_tracks_flavor() {
        use egui::Context;

        fn panel_fill(flavor: Flavor) -> egui::Color32 {
            let ctx = Context::default();
            apply_catppuccin(&ctx, flavor);
            ctx.global_style().visuals.panel_fill
        }

        assert_ne!(panel_fill(Flavor::Latte), panel_fill(Flavor::Mocha));
    }
}
