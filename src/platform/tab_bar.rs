use crate::config::Config;
use crate::core::{ContainerId, Dimension, Length, Logical};
use crate::overlay::{self, BorderMetrics};
use crate::platform::render::Renderer;

pub(crate) struct TabBarWidget {
    renderer: Renderer,
    events: Vec<egui::Event>,
    container_id: ContainerId,
    bar_size: (Length<Logical>, Length<Logical>),
    border_thickness: Length<Logical>,
    titles: Vec<String>,
    active_index: usize,
    is_highlighted: bool,
    /// Physical pixels per logical point, sizing the egui render surface.
    /// Backing scale factor on macOS, DPI scale on Windows.
    scale: f32,
    surface_size: (u32, u32),
}

impl TabBarWidget {
    pub(crate) fn new(
        renderer: Renderer,
        container_id: ContainerId,
        scale: f32,
        surface_size: (u32, u32),
    ) -> Self {
        Self {
            renderer,
            events: Vec::new(),
            container_id,
            bar_size: (Length::ZERO, Length::ZERO),
            border_thickness: Length::ZERO,
            titles: Vec::new(),
            active_index: 0,
            is_highlighted: false,
            scale,
            surface_size,
        }
    }

    /// Does not paint. The caller renders after placing the window so the present
    /// matches the new geometry.
    pub(crate) fn set_content(
        &mut self,
        scale: f32,
        bar_size: (Length<Logical>, Length<Logical>),
        border_thickness: Length<Logical>,
        titles: Vec<String>,
        active_index: usize,
        is_highlighted: bool,
    ) {
        self.scale = scale;
        self.bar_size = bar_size;
        self.border_thickness = border_thickness;
        self.titles = titles;
        self.active_index = active_index;
        self.is_highlighted = is_highlighted;
        let (width, height) = self.bar_size;
        let surface_size = (
            (width.logical() * scale).round().max(1.0) as u32,
            (height.logical() * scale).round().max(1.0) as u32,
        );
        if surface_size != self.surface_size {
            self.renderer.resize(scale, surface_size.0, surface_size.1);
            self.surface_size = surface_size;
        }
    }

    pub(crate) fn set_config(&mut self, config: &Config) {
        self.renderer.set_theme(config.theme);
        self.renderer.set_font(&config.font);
    }

    /// DPI scale of the last `set_content`. Windows divides physical pointer
    /// coordinates by it to reach logical points.
    #[cfg(target_os = "windows")]
    pub(crate) fn scale(&self) -> f32 {
        self.scale
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn push_pointer_moved(&mut self, pos: egui::Pos2) {
        self.events.push(egui::Event::PointerMoved(pos));
    }

    pub(crate) fn push_pointer_button(&mut self, pos: egui::Pos2, pressed: bool) {
        self.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }

    /// `paint_tab_bar`'s `Sense::click()` resolves only when the press and release
    /// are both in the drained events, so a click needs a render on the release
    /// edge with the press still queued.
    pub(crate) fn render(&mut self) -> Option<(ContainerId, usize)> {
        let events = std::mem::take(&mut self.events);
        let border = BorderMetrics::from_thickness(self.border_thickness);
        let theme = self.renderer.theme();
        let container_id = self.container_id;
        let titles = self.titles.clone();
        let active_index = self.active_index;
        let is_highlighted = self.is_highlighted;
        let scale = self.scale;
        let (width, height) = self.bar_size;
        let canvas = Dimension::<Logical>::new(Length::ZERO, Length::ZERO, width, height);
        self.renderer.render(scale, events, |ui| {
            overlay::paint_tab_bar(
                ui.ctx(),
                container_id,
                canvas,
                &titles,
                active_index,
                is_highlighted,
                border,
                &theme,
            )
        })
    }
}
