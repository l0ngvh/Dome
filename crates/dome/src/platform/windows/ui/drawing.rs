use crate::config::Appearance;
use crate::platform::overlay_renderer::OverlayRenderer;
use crate::platform::windows::ui::window::Window;

pub(in crate::platform::windows) async fn create_wgpu_context() -> WgpuContext {
    let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_descriptor.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(instance_descriptor);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("No DX12 adapter");
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .expect("Failed to create wgpu device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);

    WgpuContext::new(instance, adapter, device, queue).expect("GPU presentation context init")
}

impl NativeContent {
    pub(in crate::platform::windows) fn attach(&self, window: &Window) -> anyhow::Result<()> {
        window.set_content_visual(self.device(), self.visual())
    }
}
impl WgpuContext {
    pub(in crate::platform::windows) fn create_drawing(
        &self,
        size: Size<Physical>,
        appearance: &Appearance,
    ) -> anyhow::Result<(OverlayRenderer, NativeContent)> {
        let content = NativeContent::new(self)?;
        let renderer = Renderer::new(self, content.clone(), size)?;
        Ok((
            OverlayRenderer::new(
                renderer,
                appearance.theme,
                &appearance.font,
                Box::new(crate::platform::windows::font::resolve_system_font),
            ),
            content,
        ))
    }
}

use windows::Win32::Graphics::DirectComposition::{IDCompositionDevice, IDCompositionVisual};
use windows::core::Interface;

/// Clones share one native backing. Create one backing per render surface and attach it
/// to only one window. The backing does not own that window or its event handler.
#[derive(Clone)]
pub(in crate::platform::windows) struct NativeContent {
    device: IDCompositionDevice,
    visual: IDCompositionVisual,
}

pub(crate) struct Renderer {
    presentation: GpuPresentation,
    size: Size<Physical>,
    egui_ctx: egui::Context,
}

pub(in crate::platform::windows) struct WgpuContext {
    /// `instance` and `adapter` are used only to build the surface at construction.
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    /// `device` and `queue` outlive the surface, so the renderer keeps an `Arc` to each.
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    composition_device: ::windows::Win32::Graphics::DirectComposition::IDCompositionDevice,
}

impl Renderer {
    pub(crate) fn context(&self) -> &egui::Context {
        &self.egui_ctx
    }

    pub(crate) fn resize(&mut self, scale: f32, size: Size<Physical>) {
        self.size = Size {
            width: size.width().max(Length::new(1)),
            height: size.height().max(Length::new(1)),
        };
        self.presentation.resize(scale, self.size);
    }

    #[tracing::instrument(skip_all)]
    pub(crate) fn render<R>(
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

impl NativeContent {
    fn new(context: &WgpuContext) -> anyhow::Result<Self> {
        let device = context.composition_device.clone();
        let visual = unsafe { device.CreateVisual()? };
        Ok(Self { device, visual })
    }

    fn device(&self) -> &IDCompositionDevice {
        &self.device
    }

    fn visual(&self) -> &IDCompositionVisual {
        &self.visual
    }

    fn surface_target(&self) -> wgpu::SurfaceTargetUnsafe {
        wgpu::SurfaceTargetUnsafe::CompositionVisual(self.visual.as_raw() as *mut _)
    }

    fn format(&self) -> wgpu::TextureFormat {
        wgpu::TextureFormat::Bgra8UnormSrgb
    }

    fn before_configure(&self, _scale: f32) {}

    fn after_configure(&self, _surface: &wgpu::Surface<'static>) -> anyhow::Result<()> {
        unsafe { self.device.Commit()? };
        Ok(())
    }

    fn begin_present(&self) {}
    fn end_present(&self) {}
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
            composition_device: unsafe {
                ::windows::Win32::Graphics::DirectComposition::DCompositionCreateDevice2(None)?
            },
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
