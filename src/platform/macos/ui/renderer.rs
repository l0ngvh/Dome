use std::sync::Arc;

use objc2::rc::Retained;
use objc2_quartz_core::{CAMetalLayer, CATransaction};

use crate::font::FontConfig;
use crate::theme::{Flavor, apply_catppuccin};

/// wgpu-backed renderer for a single CAMetalLayer-hosted overlay.
///
/// `device` and `queue` are `Arc`-shared with `painter` and `surface`. Drop
/// order across fields is not load-bearing.
pub(super) struct Renderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    painter: egui_wgpu::Renderer,
    egui_ctx: egui::Context,
    layer: Retained<CAMetalLayer>,
    logical: (f64, f64),
    scale: f64,
    corner_radius: Option<f64>,
}

impl Renderer {
    pub(super) fn new(
        factory: &WgpuFactory,
        scale: f64,
        logical_w: f64,
        logical_h: f64,
        flavor: Flavor,
        font: &FontConfig,
        corner_radius: Option<f64>,
    ) -> Self {
        let layer: Retained<CAMetalLayer> = CAMetalLayer::new();
        // Set before any drawable exists so first-frame composition is at Retina density.
        layer.setContentsScale(scale);

        let target = wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
            Retained::as_ptr(&layer) as *mut core::ffi::c_void
        );
        let surface = unsafe {
            factory
                .instance()
                .create_surface_unsafe(target)
                .expect("create_surface_unsafe")
        };

        // Metal advertises only PostMultiplied, which sets CAMetalLayer.opaque = false
        // for Core Animation to composite egui's premultiplied output.
        let caps = surface.get_capabilities(factory.adapter());
        let alpha_mode = [
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
        ]
        .into_iter()
        .find(|m| caps.alpha_modes.contains(m))
        .expect("surface must support a non-opaque alpha mode for translucent overlays");

        let (physical_w, physical_h) = physical_size(logical_w, logical_h, scale);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            // Non-sRGB keeps egui's premultiplied output byte-identical on the wire.
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: physical_w,
            height: physical_h,
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(factory.device(), &surface_config);

        let device = Arc::clone(factory.device());
        let queue = Arc::clone(factory.queue());

        let painter = egui_wgpu::Renderer::new(
            &device,
            surface_config.format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                dithering: false,
                ..Default::default()
            },
        );

        // Disable selectable labels so clicks on tab bars register as tab switches
        // instead of triggering egui's text selection behavior.
        let egui_ctx = egui::Context::default();
        egui_ctx.global_style_mut(|s| s.interaction.selectable_labels = false);
        apply_catppuccin(&egui_ctx, flavor);
        install_font(&egui_ctx, font.family.as_deref());
        font.apply_to(&egui_ctx);

        let renderer = Self {
            device,
            queue,
            surface,
            surface_config,
            painter,
            egui_ctx,
            layer,
            logical: (logical_w, logical_h),
            scale,
            corner_radius,
        };
        configure_layer(&renderer.surface, renderer.corner_radius);
        renderer
    }

    pub(super) fn resize(&mut self, scale: f64, logical_w: f64, logical_h: f64) {
        self.scale = scale;
        self.logical = (logical_w, logical_h);
        self.layer.setContentsScale(scale);
        let (physical_w, physical_h) = physical_size(logical_w, logical_h, scale);
        self.surface_config.width = physical_w;
        self.surface_config.height = physical_h;
        self.surface.configure(&self.device, &self.surface_config);
        configure_layer(&self.surface, self.corner_radius);
    }

    pub(super) fn layer(&self) -> Retained<CAMetalLayer> {
        self.layer.clone()
    }

    pub(super) fn apply_theme(&mut self, flavor: Flavor) {
        apply_catppuccin(&self.egui_ctx, flavor);
    }

    pub(super) fn reinstall_fonts(&mut self, family: Option<&str>) {
        install_font(&self.egui_ctx, family);
    }

    pub(super) fn apply_font(&mut self, font: &FontConfig) {
        font.apply_to(&self.egui_ctx);
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn render<R>(
        &mut self,
        pixels_per_point: f32,
        events: Vec<egui::Event>,
        mut ctx_fn: impl FnMut(&mut egui::Ui) -> R,
    ) -> R {
        // Acquire before running egui so a skipped frame still processes input.
        // Transient statuses skip only the GPU paint. Lost and Validation panic.
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

        // run_ui must fire every frame so textures_delta stays in sync with
        // the painter's ledger, even when the swap chain skipped the frame.
        let mut result = None;
        let output = self.egui_ctx.run_ui(raw_input, |ui| {
            result = Some(ctx_fn(ui));
        });

        for (id, delta) in &output.textures_delta.set {
            self.painter
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        if let Some(frame) = frame {
            // presentsWithTransaction (see configure_layer) attaches present() to
            // this CATransaction. setDisableActions suppresses the implicit
            // Core Animation crossfade on contents change.
            CATransaction::begin();
            CATransaction::setDisableActions(true);

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

            CATransaction::commit();
        }

        for id in &output.textures_delta.free {
            self.painter.free_texture(id);
        }

        result.unwrap()
    }
}

pub(super) struct WgpuFactory {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl WgpuFactory {
    pub(super) fn new() -> anyhow::Result<Self> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
        Ok(Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
        })
    }

    pub(super) fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    pub(super) fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    pub(super) fn device(&self) -> &Arc<wgpu::Device> {
        &self.device
    }

    pub(super) fn queue(&self) -> &Arc<wgpu::Queue> {
        &self.queue
    }
}

fn physical_size(logical_w: f64, logical_h: f64, scale: f64) -> (u32, u32) {
    let w = (logical_w * scale).round() as u32;
    let h = (logical_h * scale).round() as u32;
    (w.max(1), h.max(1))
}

fn install_font(ctx: &egui::Context, family: Option<&str>) {
    let Some(family) = family else {
        return;
    };
    match crate::platform::macos::font::resolve_system_font(family) {
        Ok(bytes) => crate::font::install_fonts(bytes, ctx),
        Err(e) => tracing::warn!(
            family = %family,
            error = %e,
            "font resolution failed. using egui defaults"
        ),
    }
}

fn configure_layer(surface: &wgpu::Surface<'static>, corner_radius: Option<f64>) {
    unsafe {
        let Some(hal_surface) = surface.as_hal::<wgpu::hal::api::Metal>() else {
            return;
        };
        let layer = hal_surface.render_layer().lock();
        // Attach drawable.present() to the caller's CATransaction so
        // setDisableActions there can suppress the implicit crossfade.
        layer.setPresentsWithTransaction(true);
        // Belt and braces. wgpu-hal's PostMultiplied branch already calls this.
        // Re-assert so a future wgpu-hal change does not silently break blending.
        layer.setOpaque(false);
        if let Some(r) = corner_radius {
            layer.setCornerRadius(r);
            layer.setMasksToBounds(true);
        }
    }
}
