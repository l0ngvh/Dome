use std::sync::Arc;

use crate::font::{FontConfig, install_fonts};
use crate::theme::{Flavor, Theme, apply_catppuccin};

pub(crate) type FontResolver = Box<dyn Fn(&str) -> anyhow::Result<Vec<u8>>>;

pub(crate) struct WgpuContext {
    /// `instance` and `adapter` are used only to build the surface at construction.
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    /// `device` and `queue` outlive the surface, so the renderer keeps an `Arc` to each.
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl WgpuContext {
    pub(crate) fn new(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
    ) -> Self {
        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }
}

pub(crate) trait Compositor {
    /// The implementor must keep the OS object behind this target alive for its own lifetime.
    fn surface_target(&self) -> wgpu::SurfaceTargetUnsafe;

    fn format(&self) -> wgpu::TextureFormat;

    fn after_surface_created(&self, _surface: &wgpu::Surface<'static>) -> anyhow::Result<()> {
        Ok(())
    }

    fn before_configure(&self, _scale: f32) {}

    fn after_configure(&self, _surface: &wgpu::Surface<'static>) -> anyhow::Result<()> {
        Ok(())
    }

    fn begin_present(&self) {}
    fn end_present(&self) {}
}

pub(crate) struct Renderer {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    painter: egui_wgpu::Renderer,
    egui_ctx: egui::Context,
    /// Declared after `surface` so the surface's swap chain releases before the
    /// DirectComposition objects the compositor owns on Windows.
    compositor: Box<dyn Compositor>,
    resolve_font: FontResolver,
    flavor: Flavor,
    font: FontConfig,
}

impl Renderer {
    pub(crate) fn new(
        gpu: &WgpuContext,
        compositor: Box<dyn Compositor>,
        width: u32,
        height: u32,
        flavor: Flavor,
        font: &FontConfig,
        resolve_font: FontResolver,
    ) -> anyhow::Result<Self> {
        let target = compositor.surface_target();
        let surface = unsafe { gpu.instance.create_surface_unsafe(target)? };
        compositor.after_surface_created(&surface)?;

        // Metal advertises only PostMultiplied, DirectComposition prefers PreMultiplied.
        let caps = surface.get_capabilities(&gpu.adapter);
        let alpha_mode = [
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
        ]
        .into_iter()
        .find(|m| caps.alpha_modes.contains(m))
        .expect("surface must support a non-opaque alpha mode for translucent overlays");

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: compositor.format(),
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&gpu.device, &surface_config);
        compositor.after_configure(&surface)?;

        let painter = egui_wgpu::Renderer::new(
            &gpu.device,
            surface_config.format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                dithering: false,
                ..Default::default()
            },
        );

        // Clicks on tab bars must switch tabs, not enter egui text selection.
        let egui_ctx = egui::Context::default();
        egui_ctx.global_style_mut(|s| s.interaction.selectable_labels = false);
        apply_catppuccin(&egui_ctx, flavor);
        if let Some(family) = font.family.as_deref() {
            install_family(&resolve_font, &egui_ctx, family);
        }
        font.apply_to(&egui_ctx);

        Ok(Self {
            surface,
            surface_config,
            device: Arc::clone(&gpu.device),
            queue: Arc::clone(&gpu.queue),
            painter,
            egui_ctx,
            compositor,
            resolve_font,
            flavor,
            font: font.clone(),
        })
    }

    pub(crate) fn resize(&mut self, scale: f32, width: u32, height: u32) {
        self.surface_config.width = width.max(1);
        self.surface_config.height = height.max(1);
        self.compositor.before_configure(scale);
        self.surface.configure(&self.device, &self.surface_config);
        self.compositor
            .after_configure(&self.surface)
            .expect("compositor configure after resize");
    }

    pub(crate) fn theme(&self) -> Theme {
        Theme::from_flavor(self.flavor)
    }

    pub(crate) fn set_theme(&mut self, flavor: Flavor) {
        if self.flavor != flavor {
            apply_catppuccin(&self.egui_ctx, flavor);
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
            install_family(&self.resolve_font, &self.egui_ctx, family);
        }
        font.apply_to(&self.egui_ctx);
        self.font = font.clone();
    }

    #[tracing::instrument(skip_all)]
    pub(crate) fn render<R>(
        &mut self,
        pixels_per_point: f32,
        events: Vec<egui::Event>,
        mut ctx_fn: impl FnMut(&mut egui::Ui) -> R,
    ) -> R {
        // Acquire before running egui so a skipped frame still processes input.
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => Some(t),
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Outdated => None,
            wgpu::CurrentSurfaceTexture::Lost => panic!("surface lost"),
            wgpu::CurrentSurfaceTexture::Validation => panic!("surface validation error"),
        };

        let width_px = self.surface_config.width;
        let height_px = self.surface_config.height;
        let w_pts = width_px as f32 / pixels_per_point;
        let h_pts = height_px as f32 / pixels_per_point;
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(w_pts, h_pts),
            )),
            viewports: std::iter::once((
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    native_pixels_per_point: Some(pixels_per_point),
                    ..Default::default()
                },
            ))
            .collect(),
            events,
            ..Default::default()
        };

        // run_ui must fire every frame so textures_delta stays in sync with the painter's
        // ledger, even when the swap chain skipped the frame.
        let mut result = None;
        let output = self.egui_ctx.run_ui(raw_input, |ui| {
            result = Some(ctx_fn(ui));
        });

        for (id, delta) in &output.textures_delta.set {
            self.painter
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        if let Some(frame) = frame {
            self.compositor.begin_present();

            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let meshes = self
                .egui_ctx
                .tessellate(output.shapes, output.pixels_per_point);
            let screen = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [width_px, height_px],
                pixels_per_point: output.pixels_per_point,
            };

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            let user_cmds = self.painter.update_buffers(
                &self.device,
                &self.queue,
                &mut encoder,
                &meshes,
                &screen,
            );

            {
                let clear_color = wgpu::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                };
                let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                // egui_wgpu::Renderer::render requires 'static lifetime on the pass.
                self.painter
                    .render(&mut rpass.forget_lifetime(), &meshes, &screen);
            }

            self.queue.submit(
                user_cmds
                    .into_iter()
                    .chain(std::iter::once(encoder.finish())),
            );
            frame.present();

            self.compositor.end_present();
        }

        for id in &output.textures_delta.free {
            self.painter.free_texture(id);
        }

        result.unwrap()
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
