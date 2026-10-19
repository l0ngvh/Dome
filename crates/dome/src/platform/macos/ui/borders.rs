use super::drawing::{WgpuContext, create_drawing};
use super::{
    ContainerShow, FloatBorderPlacement, TilingOverlayPlacement, frame_attrs, physical_size,
};
use crate::config::Appearance;
use crate::overlay::{self, BorderMetrics, LogicalTiledContainer, LogicalTiledWindow};
use crate::platform::macos::ui::window::Window;
use crate::platform::macos::ui::window::WindowAttributes;
use crate::platform::macos::ui::window::WindowLevel;
use crate::platform::overlay_renderer::OverlayRenderer;
use dome_core::{
    Dimension, FloatWindowPlacement, FractionalLength, Logical, TilingWindowPlacement,
};

#[derive(Clone, Copy)]
pub(in crate::platform::macos) struct TilingWindowShow {
    pub(in crate::platform::macos) placement: TilingWindowPlacement,
    pub(in crate::platform::macos) corner_radius: FractionalLength<Logical>,
}

pub(in crate::platform::macos) struct FloatBorderOverlay {
    window: Window,
    renderer: OverlayRenderer,
    placement: FloatWindowPlacement,
    scale: f64,
    border_thickness: FractionalLength<Logical>,
    corner_radius: FractionalLength<Logical>,
}

impl FloatBorderOverlay {
    pub(in crate::platform::macos) fn new(
        gpu: &WgpuContext,
        appearance: &Appearance,
        show: &FloatBorderPlacement,
    ) -> Self {
        let frame = show.cocoa_frame;
        let scale = show.scale;
        let size = physical_size(frame.size.width, frame.size.height, scale);
        let (renderer, surface) = create_drawing(gpu, scale, size, appearance);
        let (position, size) = frame_attrs(frame);
        let window = Window::new(
            &WindowAttributes {
                position,
                size,
                click_through: true,
                focusable: false,
            },
            None,
        )
        .expect("auxiliary window on main thread");
        window.set_level(WindowLevel::Floating);
        surface.attach(&window);
        Self {
            window,
            renderer,
            placement: show.placement,
            scale,
            border_thickness: show.border_thickness,
            corner_radius: show.corner_radius,
        }
    }

    pub(in crate::platform::macos) fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible);
    }

    pub(in crate::platform::macos) fn paint(&mut self) {
        let placement = self.placement;
        let corner_radius = self.corner_radius;
        let border = BorderMetrics::from_thickness(self.border_thickness);
        let theme = self.renderer.theme();
        self.renderer.render(self.scale as f32, Vec::new(), |ui| {
            overlay::paint_float_border(
                ui.ctx(),
                placement.border_box.to_dimension(),
                placement.visible_border_box.to_dimension(),
                placement.is_highlighted,
                corner_radius,
                &theme,
                border,
            );
        });
    }
}

impl FloatBorderOverlay {
    pub(in crate::platform::macos) fn update(
        &mut self,
        show: &FloatBorderPlacement,
        appearance: &Appearance,
    ) {
        let cocoa_frame = show.cocoa_frame;
        let scale = show.scale;
        let placement = show.placement;
        self.placement = placement;
        self.scale = scale;
        self.border_thickness = show.border_thickness;
        self.corner_radius = show.corner_radius;

        let (position, size) = frame_attrs(cocoa_frame);
        self.window.set_frame(position, size);
        let size = physical_size(cocoa_frame.size.width, cocoa_frame.size.height, scale);
        self.renderer.resize(scale as f32, size);
        self.renderer.set_theme(appearance.theme);
    }
}

pub(in crate::platform::macos) struct TilingOverlay {
    window: Window,
    renderer: OverlayRenderer,
    monitor: Dimension,
    windows: Vec<TilingWindowShow>,
    containers: Vec<ContainerShow>,
    border_thickness: FractionalLength<Logical>,
    scale: f64,
}

impl TilingOverlay {
    pub(in crate::platform::macos) fn new(
        gpu: &WgpuContext,
        appearance: &Appearance,
        placement: &TilingOverlayPlacement,
    ) -> Self {
        let scale = placement.scale;
        let cocoa_frame = placement.cocoa_frame;
        let (renderer, surface) = create_drawing(
            gpu,
            scale,
            physical_size(cocoa_frame.size.width, cocoa_frame.size.height, scale),
            appearance,
        );
        let (position, size) = frame_attrs(cocoa_frame);
        let window = Window::new(
            &WindowAttributes {
                position,
                size,
                click_through: true,
                focusable: true,
            },
            None,
        )
        .expect("auxiliary window on main thread");
        window.set_level(WindowLevel::Bottom);
        surface.attach(&window);
        window.set_visible(true);
        Self {
            window,
            renderer,
            monitor: placement.monitor,
            windows: Vec::new(),
            containers: Vec::new(),
            border_thickness: placement.border_thickness,
            scale,
        }
    }

    fn render_now(&mut self) {
        let monitor_logical = self.monitor;
        let windows_logical: Vec<LogicalTiledWindow> = self
            .windows
            .iter()
            .map(|show| LogicalTiledWindow {
                id: show.placement.id,
                frame: show.placement.border_box.to_dimension(),
                visible_frame: show.placement.visible_border_box.to_dimension(),
                is_highlighted: show.placement.is_highlighted,
                spawn_direction: show.placement.spawn_direction,
                corner_radius: show.corner_radius,
            })
            .collect();
        let containers_logical: Vec<LogicalTiledContainer> = self
            .containers
            .iter()
            .map(|cs| LogicalTiledContainer {
                id: cs.placement.id,
                frame: cs.placement.border_box.to_dimension(),
                visible_frame: cs.placement.visible_border_box.to_dimension(),
                tab_bar_height: FractionalLength::from_pixels(
                    cs.placement.visible_tab_bar_band.height(),
                ),
                is_highlighted: cs.placement.is_highlighted,
                spawn_direction: cs.placement.spawn_direction,
                is_tabbed: cs.placement.is_tabbed,
                titles: &cs.placement.titles,
            })
            .collect();
        let border = BorderMetrics::from_thickness(self.border_thickness);
        let theme = self.renderer.theme();
        let scale = self.scale;

        self.renderer.render(scale as f32, Vec::new(), |ui| {
            overlay::paint_tiling_overlay(
                ui.ctx(),
                monitor_logical,
                &windows_logical,
                &containers_logical,
                &theme,
                border,
            )
        });
    }
}

impl TilingOverlay {
    pub(in crate::platform::macos) fn update(
        &mut self,
        placement: &TilingOverlayPlacement,
        appearance: &Appearance,
    ) {
        let (position, size) = frame_attrs(placement.cocoa_frame);
        self.window.set_frame(position, size);
        self.monitor = placement.monitor;
        self.scale = placement.scale;
        self.windows = placement.windows.clone();
        self.containers = placement.containers.clone();
        self.border_thickness = placement.border_thickness;
        self.renderer.set_theme(appearance.theme);
        let size = physical_size(
            self.monitor.width.logical(),
            self.monitor.height.logical(),
            self.scale,
        );
        self.renderer.resize(self.scale as f32, size);
        self.render_now();
    }

    pub(in crate::platform::macos) fn focus(&self) {
        super::activation::activate_self();
        self.window.focus();
    }
}
