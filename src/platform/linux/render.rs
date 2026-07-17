use std::time::Duration;

use smithay::backend::renderer::{ImportAll, ImportMem};
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::desktop::layer_map_for_output;
use smithay::input::pointer::CursorImageStatus;
use smithay::utils::{Physical, Rectangle};

use crate::core::{Dimension, FloatWindowPlacement, Length, Logical, MonitorLayout, TilingWindowPlacement};
use crate::overlay;

use super::state::DomeState;

smithay::backend::renderer::element::render_elements! {
    pub(super) DomeRenderElement<R> where R: ImportAll + ImportMem;
    Surface=WaylandSurfaceRenderElement<R>,
    Memory=MemoryRenderBufferRenderElement<R>,
}

impl DomeState {
    pub(super) fn render_winit(&mut self) {
        let mut winit_data = match self.winit_data.take() {
            Some(d) => d,
            None => return,
        };

        let size = winit_data.backend.window_size();
        let damage: Rectangle<i32, Physical> = Rectangle::from_size(size);

        // Sync cursor icon with winit window
        match &self.cursor_status {
            CursorImageStatus::Named(icon) => {
                winit_data.backend.window().set_cursor_visible(true);
                winit_data.backend.window().set_cursor(smithay::reexports::winit::window::Cursor::Icon((*icon).into()));
            }
            _ => winit_data.backend.window().set_cursor_visible(false),
        }

        let mut all_elements: Vec<DomeRenderElement<_>> = Vec::new();

        if let CursorImageStatus::Surface(ref surface) = self.cursor_status {
            let pointer_loc = self.seat.get_pointer().unwrap().current_location();
            let hotspot = smithay::wayland::compositor::with_states(surface, |states| {
                states
                    .data_map
                    .get::<smithay::input::pointer::CursorImageSurfaceData>()
                    .map(|d| d.lock().unwrap().hotspot)
                    .unwrap_or_default()
            });
            let pos = (pointer_loc - hotspot.to_f64()).to_i32_round();
            let cursor_elements = render_elements_from_surface_tree(
                winit_data.backend.renderer(),
                surface,
                pos.to_physical(1),
                1.0,
                1.0,
                Kind::Cursor,
            );
            all_elements.extend(cursor_elements.into_iter().map(DomeRenderElement::Surface));
        }

        {
            let (renderer, mut framebuffer) = winit_data.backend.bind().expect("failed to bind backend");

            smithay::desktop::space::render_output::<_, DomeRenderElement<_>, _, _>(
                &winit_data.output,
                renderer,
                &mut framebuffer,
                1.0,
                0,
                [&self.space],
                &all_elements,
                &mut winit_data.damage_tracker,
                [0.1, 0.1, 0.1, 1.0],
            )
            .expect("failed to render output");
        }

        self.render_egui_overlays(size.w as u32, size.h as u32);

        winit_data.backend.submit(Some(&[damage])).expect("failed to submit");

        let elapsed = self.start_time.elapsed();
        self.space.elements().for_each(|window| {
            window.send_frame(
                &winit_data.output,
                elapsed,
                Some(Duration::ZERO),
                |_, _| Some(winit_data.output.clone()),
            );
        });

        {
            let layer_map = smithay::desktop::layer_map_for_output(&winit_data.output);
            for layer in layer_map.layers() {
                layer.send_frame(
                    &winit_data.output,
                    elapsed,
                    Some(Duration::ZERO),
                    |_, _| Some(winit_data.output.clone()),
                );
            }
        }

        self.space.refresh();
        self.popups.cleanup();
        layer_map_for_output(&winit_data.output).cleanup();
        if let Err(e) = self.display_handle.flush_clients() {
            tracing::warn!("failed to flush wayland clients: {e:#}");
        }

        self.winit_data = Some(winit_data);
    }

    fn render_egui_overlays(&mut self, width: u32, height: u32) {
        let mut painter = match self.egui_painter.take() {
            Some(p) => p,
            None => return,
        };

        let (meshes, textures_delta, pixels_per_point) = self.build_egui_shapes(width, height, None);
        painter.paint_and_update_textures(
            [width, height],
            pixels_per_point,
            &meshes,
            &textures_delta,
        );

        self.egui_painter = Some(painter);
    }

    pub(super) fn build_egui_shapes(
        &mut self,
        width: u32,
        height: u32,
        monitor_filter: Option<crate::core::MonitorId>,
    ) -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta, f32) {
        let w = width as f32;
        let h = height as f32;
        // Overlay draws in canvas-wide coordinates. Each placement's visible_frame
        // stays in its real screen position; `origin` positions the painter at
        // that same coordinate so `paint_window_border` and `show_container`
        // compute the same offsets they would inside a per-window egui Area.
        let _screen: Dimension<Logical> = Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(w),
            Length::new(h),
        );

        let placements = self.hub.get_visible_placements();
        let theme = self.config.theme();
        let metrics = overlay::OverlayMetrics {
            border: overlay::BorderMetrics::from_thickness(Length::<Logical>::new(
                self.config.border_size,
            )),
            tab_bar_height: self.config.partition_tree.tab_bar_height,
        };

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(w, h),
            )),
            ..Default::default()
        };

        let output = self.egui_ctx.run_ui(raw_input, |ui| {
            for mp in &placements.monitors {
                if let Some(filter) = monitor_filter {
                    if mp.monitor_id != filter {
                        continue;
                    }
                }
                let MonitorLayout::Normal { tiling_windows, float_windows, containers } = &mp.layout else {
                    continue;
                };

                for wp in tiling_windows {
                    let vf = wp.visible_frame;
                    let clip = egui::Rect::from_min_size(
                        egui::pos2(vf.x.value(), vf.y.value()),
                        egui::vec2(vf.width.value(), vf.height.value()),
                    );
                    let origin = egui::vec2(vf.x.value(), vf.y.value());
                    overlay::paint_window_border(
                        &ui.painter().with_clip_rect(clip),
                        wp.frame,
                        wp.visible_frame,
                        wp.is_highlighted,
                        wp.spawn_indicator,
                        &theme,
                        metrics.border,
                        origin,
                    );
                }

                for wp in float_windows {
                    let vf = wp.visible_frame;
                    let clip = egui::Rect::from_min_size(
                        egui::pos2(vf.x.value(), vf.y.value()),
                        egui::vec2(vf.width.value(), vf.height.value()),
                    );
                    let origin = egui::vec2(vf.x.value(), vf.y.value());
                    overlay::paint_window_border(
                        &ui.painter().with_clip_rect(clip),
                        wp.frame,
                        wp.visible_frame,
                        wp.is_highlighted,
                        None,
                        &theme,
                        metrics.border,
                        origin,
                    );
                }

                for cp in containers {
                    let vf = cp.visible_frame;
                    let clip = egui::Rect::from_min_size(
                        egui::pos2(vf.x.value(), vf.y.value()),
                        egui::vec2(vf.width.value(), vf.height.value()),
                    );
                    let origin = egui::vec2(vf.x.value(), vf.y.value());
                    let logical = overlay::LogicalTiledContainer {
                        id: cp.id,
                        frame: cp.frame,
                        visible_frame: cp.visible_frame,
                        is_highlighted: cp.is_highlighted,
                        spawn_indicator: cp.spawn_indicator,
                        is_tabbed: cp.is_tabbed,
                        titles: cp.titles.clone(),
                    };
                    let mut child = ui.new_child(
                        egui::UiBuilder::new().max_rect(clip),
                    );
                    overlay::show_container(&mut child, &logical, &theme, metrics, origin);
                }
            }
        });

        let meshes = self.egui_ctx.tessellate(output.shapes, output.pixels_per_point);
        (meshes, output.textures_delta, output.pixels_per_point)
    }

}
