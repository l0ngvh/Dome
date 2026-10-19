#[cfg(target_os = "macos")]
use crate::platform::macos as native;
#[cfg(target_os = "windows")]
use crate::platform::windows as native;
use native::ui::drawing::Renderer;

use crate::theme::{Flavor, Theme, apply_catppuccin};
use dome_core::font::FontConfig;
use dome_core::{Physical, Size};

pub(crate) type FontResolver = Box<dyn Fn(&str) -> anyhow::Result<Vec<u8>>>;

pub(crate) struct OverlayRenderer {
    renderer: Renderer,
    resolve_font: FontResolver,
    flavor: Flavor,
    font: FontConfig,
}

impl OverlayRenderer {
    pub(crate) fn new(
        renderer: Renderer,
        flavor: Flavor,
        font: &FontConfig,
        resolve_font: FontResolver,
    ) -> Self {
        let context = renderer.context();
        context.global_style_mut(|style| style.interaction.selectable_labels = false);
        apply_catppuccin(context, flavor);
        if let Some(family) = font.family.as_deref() {
            install_family(&resolve_font, context, family);
        }
        // egui applies pixels_per_point during rendering, so keep the size unscaled.
        context.global_style_mut(|style| {
            style.text_styles.insert(
                TextStyle::Body,
                FontId::new(font.size, FontFamily::Proportional),
            );
        });
        Self {
            renderer,
            resolve_font,
            flavor,
            font: font.clone(),
        }
    }

    pub(crate) fn resize(&mut self, scale: f32, size: Size<Physical>) {
        self.renderer.resize(scale, size);
    }

    pub(crate) fn theme(&self) -> Theme {
        Theme::from_flavor(self.flavor)
    }

    pub(crate) fn set_theme(&mut self, flavor: Flavor) {
        if self.flavor != flavor {
            apply_catppuccin(self.renderer.context(), flavor);
            self.flavor = flavor;
        }
    }

    pub(crate) fn set_font(&mut self, font: &FontConfig) {
        if self.font == *font {
            return;
        }
        if self.font.family != font.family
            && let Some(family) = font.family.as_deref()
        {
            install_family(&self.resolve_font, self.renderer.context(), family);
        }
        self.renderer.context().global_style_mut(|style| {
            style.text_styles.insert(
                TextStyle::Body,
                FontId::new(font.size, FontFamily::Proportional),
            );
        });
        self.font = font.clone();
    }

    pub(crate) fn render<R>(
        &mut self,
        pixels_per_point: f32,
        events: Vec<egui::Event>,
        paint: impl FnMut(&mut egui::Ui) -> R,
    ) -> R {
        self.renderer.render(pixels_per_point, events, paint)
    }
}

fn install_family(resolve: &FontResolver, ctx: &egui::Context, family: &str) {
    match resolve(family) {
        Ok(bytes) => install_fonts(bytes, ctx),
        Err(e) => tracing::warn!(
            family = %family,
            error = %e,
            "font resolution failed, using egui defaults"
        ),
    }
}

use std::sync::Arc;

use egui::{Context, FontData, FontDefinitions, FontFamily, FontId, TextStyle};

fn install_fonts(bytes: Vec<u8>, ctx: &Context) {
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
