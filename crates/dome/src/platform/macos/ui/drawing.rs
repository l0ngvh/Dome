use crate::config::Appearance;
use crate::platform::macos::ui::window::Window;
use crate::platform::overlay_renderer::OverlayRenderer;

pub(in crate::platform::macos) async fn create_wgpu_context() -> anyhow::Result<WgpuContext> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::METAL;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await?;
    let (device, queue) = adapter.request_device(&Default::default()).await?;
    WgpuContext::new(instance, adapter, Arc::new(device), Arc::new(queue))
}

pub(in crate::platform::macos) fn create_drawing(
    gpu: &WgpuContext,
    scale: f64,
    size: Size<Physical>,
    appearance: &Appearance,
) -> (OverlayRenderer, NativeContent) {
    let content = NativeContent::new(scale);
    let renderer = Renderer::new(gpu, content.clone(), size).expect("overlay renderer init");
    (
        OverlayRenderer::new(
            renderer,
            appearance.theme,
            &appearance.font,
            Box::new(crate::platform::macos::font::resolve_system_font),
        ),
        content,
    )
}

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2_quartz_core::{CAMetalLayer, CATransaction};

/// Clones share one native backing. Create one backing per render surface and attach it
/// to only one window. The backing does not own that window or its event handler.
#[derive(Clone)]
pub struct NativeContent {
    layer: Retained<CAMetalLayer>,
}

impl NativeContent {
    pub(in crate::platform::macos) fn attach(&self, window: &Window) {
        window.set_content_layer(&self.layer());
    }

    fn new(scale: f64) -> Self {
        let layer: Retained<CAMetalLayer> = CAMetalLayer::new();
        // Set before any drawable exists so first-frame composition is at Retina density.
        layer.setContentsScale(scale);
        Self { layer }
    }

    fn layer(&self) -> Retained<CAMetalLayer> {
        self.layer.clone()
    }
}

impl NativeContent {
    pub(crate) fn surface_target(&self) -> wgpu::SurfaceTargetUnsafe {
        wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(Retained::as_ptr(&self.layer) as *mut c_void)
    }

    pub(crate) fn format(&self) -> wgpu::TextureFormat {
        // Non-sRGB keeps egui's premultiplied output byte-identical on the wire.
        wgpu::TextureFormat::Bgra8Unorm
    }

    pub(crate) fn before_configure(&self, scale: f32) {
        self.layer.setContentsScale(scale as f64);
    }

    pub(crate) fn after_configure(&self, surface: &wgpu::Surface<'static>) -> anyhow::Result<()> {
        configure_layer(surface);
        Ok(())
    }

    pub(crate) fn begin_present(&self) {
        // Disable Core Animation's implicit crossfade so the surface swap is atomic.
        CATransaction::begin();
        CATransaction::setDisableActions(true);
    }

    pub(crate) fn end_present(&self) {
        CATransaction::commit();
    }
}

pub struct Renderer {
    presentation: GpuPresentation,
    size: Size<Physical>,
    egui_ctx: egui::Context,
}

pub(in crate::platform::macos) struct WgpuContext {
    /// `instance` and `adapter` are used only to build the surface at construction.
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    /// `device` and `queue` outlive the surface, so the renderer keeps an `Arc` to each.
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl Renderer {
    pub fn context(&self) -> &egui::Context {
        &self.egui_ctx
    }

    pub fn resize(&mut self, scale: f32, size: Size<Physical>) {
        self.size = Size {
            width: size.width().max(Length::new(1)),
            height: size.height().max(Length::new(1)),
        };
        self.presentation.resize(scale, self.size);
    }

    #[tracing::instrument(skip_all)]
    pub fn render<R>(
        &mut self,
        pixels_per_point: f32,
        events: Vec<egui::Event>,
        mut ctx_fn: impl FnMut(&mut egui::Ui) -> R,
    ) -> R {
        self.presentation.acquire();
        let w_pts = self.size.width().value() as f32 / pixels_per_point;
        let h_pts = self.size.height().value() as f32 / pixels_per_point;
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

        let meshes = self
            .egui_ctx
            .tessellate(output.shapes, output.pixels_per_point);
        self.presentation.present(
            &meshes,
            &output.textures_delta,
            self.size,
            output.pixels_per_point,
        );
        result.unwrap()
    }

    /// `content` must not already back another surface. The renderer retains it until
    /// after the GPU surface has been released.
    fn new(
        gpu: &WgpuContext,
        content: NativeContent,
        size: Size<Physical>,
    ) -> anyhow::Result<Self> {
        let target = content.surface_target();
        let surface = unsafe { gpu.instance.create_surface_unsafe(target)? };

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
            format: content.format(),
            width: size.width().max(Length::new(1)).value(),
            height: size.height().max(Length::new(1)).value(),
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&gpu.device, &surface_config);
        content.after_configure(&surface)?;

        let painter = egui_wgpu::Renderer::new(
            &gpu.device,
            surface_config.format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                dithering: false,
                ..Default::default()
            },
        );

        Ok(Self::with_presentation(
            GpuPresentation {
                surface,
                surface_config,
                device: Arc::clone(&gpu.device),
                queue: Arc::clone(&gpu.queue),
                painter,
                content,
                frame: None,
            },
            size,
        ))
    }

    fn with_presentation(presentation: GpuPresentation, size: Size<Physical>) -> Self {
        let egui_ctx = egui::Context::default();
        Self {
            presentation,
            size: Size {
                width: size.width().max(Length::new(1)),
                height: size.height().max(Length::new(1)),
            },
            egui_ctx,
        }
    }
}

fn configure_layer(surface: &wgpu::Surface<'static>) {
    unsafe {
        let Some(hal_surface) = surface.as_hal::<wgpu::hal::api::Metal>() else {
            return;
        };
        let layer = hal_surface.render_layer().lock();
        // Route present() through the caller's CATransaction so begin_present can suppress
        // the crossfade.
        layer.setPresentsWithTransaction(true);
        // wgpu-hal's PostMultiplied branch already sets this. Re-assert so a future
        // wgpu-hal change cannot silently break blending.
        layer.setOpaque(false);
    }
}

use std::sync::Arc;

use dome_core::{Length, Physical, Size};

impl WgpuContext {
    fn new(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}

struct GpuPresentation {
    frame: Option<wgpu::SurfaceTexture>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    painter: egui_wgpu::Renderer,
    // The swap chain must release before the content's native objects.
    content: NativeContent,
}

impl GpuPresentation {
    fn resize(&mut self, scale: f32, size: Size<Physical>) {
        self.surface_config.width = size.width().value();
        self.surface_config.height = size.height().value();
        self.content.before_configure(scale);
        self.surface.configure(&self.device, &self.surface_config);
        self.content
            .after_configure(&self.surface)
            .expect("content configure after resize");
    }

    fn acquire(&mut self) {
        self.frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => Some(t),
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Outdated => None,
            wgpu::CurrentSurfaceTexture::Lost => panic!("surface lost"),
            wgpu::CurrentSurfaceTexture::Validation => panic!("surface validation error"),
        };
    }

    fn present(
        &mut self,
        meshes: &[egui::ClippedPrimitive],
        textures: &egui::TexturesDelta,
        size: Size<Physical>,
        pixels_per_point: f32,
    ) {
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width().value(), size.height().value()],
            pixels_per_point,
        };
        for (id, delta) in &textures.set {
            self.painter
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        if let Some(frame) = self.frame.take() {
            self.content.begin_present();

            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            let user_cmds = self.painter.update_buffers(
                &self.device,
                &self.queue,
                &mut encoder,
                meshes,
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
                    .render(&mut rpass.forget_lifetime(), meshes, &screen);
            }

            self.queue.submit(
                user_cmds
                    .into_iter()
                    .chain(std::iter::once(encoder.finish())),
            );
            frame.present();

            self.content.end_present();
        }

        for id in &textures.free {
            self.painter.free_texture(id);
        }
    }
}
