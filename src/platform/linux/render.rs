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
use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;

use crate::core::{Child, ContainerPlacement, Dimension, MonitorLayout, WindowPlacement};
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
        let _ = self.display_handle.flush_clients();

        self.winit_data = Some(winit_data);
    }

    fn render_egui_overlays(&mut self, width: u32, height: u32) {
        let mut painter = match self.egui_painter.take() {
            Some(p) => p,
            None => return,
        };

        let (meshes, textures_delta, pixels_per_point) = self.build_egui_shapes(width, height);
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
    ) -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta, f32) {
        let w = width as f32;
        let h = height as f32;
        let screen = Dimension { x: 0.0, y: 0.0, width: w, height: h };

        let placements = self.hub.get_visible_placements();

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(w, h),
            )),
            ..Default::default()
        };

        let output = self.egui_ctx.run(raw_input, |ctx| {
            egui::Area::new(egui::Id::new("overlay"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    for mp in &placements {
                        let MonitorLayout::Normal { windows, containers } = &mp.layout else {
                            continue;
                        };

                        for wp in windows {
                            let adjusted = WindowPlacement {
                                visible_frame: screen,
                                ..*wp
                            };
                            overlay::paint_window_border(ui.painter(), &adjusted, &self.config);
                        }

                        for cp in containers {
                            let adjusted = ContainerPlacement {
                                visible_frame: screen,
                                ..*cp
                            };
                            let titles = self.collect_tab_titles(cp.id);
                            overlay::show_container(ui, &adjusted, &titles, &self.config);
                        }
                    }
                });
        });

        let meshes = self.egui_ctx.tessellate(output.shapes, output.pixels_per_point);
        (meshes, output.textures_delta, output.pixels_per_point)
    }

    fn collect_tab_titles(&self, container_id: crate::core::ContainerId) -> Vec<String> {
        let container = self.hub.get_container(container_id);
        container
            .children()
            .iter()
            .map(|c| match c {
                Child::Window(wid) => self
                    .window_map
                    .get(wid)
                    .and_then(|w| w.toplevel())
                    .and_then(|t| {
                        smithay::wayland::compositor::with_states(t.wl_surface(), |states| {
                            states
                                .data_map
                                .get::<XdgToplevelSurfaceData>()
                                .and_then(|d| d.lock().unwrap().title.clone())
                        })
                    })
                    .unwrap_or_else(|| "Window".to_owned()),
                Child::Container(_) => "Container".to_owned(),
            })
            .collect()
    }
}
