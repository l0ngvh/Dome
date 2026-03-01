use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;

use objc2::Message;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::{NSString, NSUInteger};
use objc2_io_surface::IOSurface;
use objc2_metal::*;
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

pub(super) struct MetalBackend {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    egui_pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    mirror_pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
}

impl MetalBackend {
    pub(super) fn new(device: &ProtocolObject<dyn MTLDevice>) -> Rc<Self> {
        let command_queue = device
            .newCommandQueue()
            .expect("failed to create command queue");

        let source = NSString::from_str(SHADER_SOURCE);
        let library = device
            .newLibraryWithSource_options_error(&source, None)
            .expect("failed to compile shader");
        let vertex_fn = library
            .newFunctionWithName(&NSString::from_str("vertex_main"))
            .expect("vertex_main not found");
        let fragment_egui_fn = library
            .newFunctionWithName(&NSString::from_str("fragment_egui"))
            .expect("fragment_egui not found");
        let fragment_mirror_fn = library
            .newFunctionWithName(&NSString::from_str("fragment_mirror"))
            .expect("fragment_mirror not found");

        let vertex_desc = MTLVertexDescriptor::vertexDescriptor();
        let attrs = vertex_desc.attributes();
        let attr0 = unsafe { attrs.objectAtIndexedSubscript(0) };
        attr0.setFormat(MTLVertexFormat::Float2);
        unsafe { attr0.setOffset(0) };
        unsafe { attr0.setBufferIndex(0) };
        let attr1 = unsafe { attrs.objectAtIndexedSubscript(1) };
        attr1.setFormat(MTLVertexFormat::Float2);
        unsafe { attr1.setOffset(8) };
        unsafe { attr1.setBufferIndex(0) };
        let attr2 = unsafe { attrs.objectAtIndexedSubscript(2) };
        attr2.setFormat(MTLVertexFormat::UChar4Normalized);
        unsafe { attr2.setOffset(16) };
        unsafe { attr2.setBufferIndex(0) };
        let layout0 = unsafe { vertex_desc.layouts().objectAtIndexedSubscript(0) };
        unsafe { layout0.setStride(VERTEX_STRIDE) };
        layout0.setStepFunction(MTLVertexStepFunction::PerVertex);

        // Two pipelines share the same vertex shader but differ in fragment processing:
        // egui premultiplies color by alpha for correct text/UI blending;
        // mirror passes through captured pixels as-is since they're already composited.
        let egui_pipeline =
            Self::create_pipeline(device, &vertex_fn, &fragment_egui_fn, &vertex_desc);
        let mirror_pipeline =
            Self::create_pipeline(device, &vertex_fn, &fragment_mirror_fn, &vertex_desc);

        // Linear smooths the mirror quad when capture resolution doesn't exactly match
        // the drawable (rounding between logical and physical pixels). egui works with
        // either filter mode, but its official backends also use linear.
        let sampler_desc = MTLSamplerDescriptor::new();
        sampler_desc.setMinFilter(MTLSamplerMinMagFilter::Linear);
        sampler_desc.setMagFilter(MTLSamplerMinMagFilter::Linear);
        let sampler = device
            .newSamplerStateWithDescriptor(&sampler_desc)
            .expect("failed to create sampler");

        Rc::new(Self {
            device: device.retain(),
            command_queue,
            egui_pipeline,
            mirror_pipeline,
            sampler,
        })
    }

    fn device(&self) -> &ProtocolObject<dyn MTLDevice> {
        &self.device
    }
}

/// For container overlays (tab bars). No mirror capability needed.
pub(super) struct ContainerRenderer {
    inner: EguiRenderer,
}

impl ContainerRenderer {
    pub(super) fn new(
        backend: Rc<MetalBackend>,
        scale: f64,
        logical_w: f64,
        logical_h: f64,
    ) -> Self {
        Self {
            inner: EguiRenderer::new(backend, scale, logical_w, logical_h),
        }
    }

    pub(super) fn layer(&self) -> Retained<CAMetalLayer> {
        self.inner.layer()
    }

    pub(super) fn resize(&self, logical_w: f64, logical_h: f64, scale: f64) {
        self.inner.resize(logical_w, logical_h, scale);
    }

    pub(super) fn events(&self) -> Rc<RefCell<Vec<egui::Event>>> {
        self.inner.events()
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn render<R>(
        &mut self,
        pixels_per_point: f32,
        ui_fn: impl FnMut(&mut egui::Ui) -> R,
    ) -> R {
        let (meshes, delta, surface_size, result) = self.inner.prepare(pixels_per_point, ui_fn);
        if let Some(ctx) = self.inner.begin_frame(&delta, surface_size) {
            self.inner.draw_egui_meshes(&ctx, &meshes, pixels_per_point);
            ctx.finish();
        }
        result
    }
}

/// For per-window overlays. Draws borders via egui, and optionally shows a mirror of the
/// window content from screen capture.
pub(super) struct WindowRenderer {
    inner: EguiRenderer,
    mirror_texture: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
}

impl WindowRenderer {
    pub(super) fn new(
        backend: Rc<MetalBackend>,
        scale: f64,
        logical_w: f64,
        logical_h: f64,
    ) -> Self {
        Self {
            inner: EguiRenderer::new(backend, scale, logical_w, logical_h),
            mirror_texture: None,
        }
    }

    pub(super) fn layer(&self) -> Retained<CAMetalLayer> {
        self.inner.layer()
    }

    pub(super) fn resize(&self, logical_w: f64, logical_h: f64, scale: f64) {
        self.inner.resize(logical_w, logical_h, scale);
    }

    pub(super) fn events(&self) -> Rc<RefCell<Vec<egui::Event>>> {
        self.inner.events()
    }

    /// Wraps the IOSurface as a Metal texture for zero-copy GPU reads from the capture buffer.
    #[tracing::instrument(skip_all)]
    pub(super) fn set_mirror_surface(&mut self, surface: &IOSurface) {
        let w = surface.width() as usize;
        let h = surface.height() as usize;
        // BGRA8 to match SCStream's capture pixel format. No mipmaps — displayed at ~1:1.
        let desc = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                MTLPixelFormat::BGRA8Unorm,
                w,
                h,
                false,
            )
        };
        // ShaderRead-only — the GPU only samples this; capture writes to the IOSurface directly.
        desc.setUsage(MTLTextureUsage::ShaderRead);
        // objc2 has separate IOSurface and IOSurfaceRef types for the same underlying CF type.
        let surface_ref: &objc2_io_surface::IOSurfaceRef =
            unsafe { &*(surface as *const IOSurface as *const objc2_io_surface::IOSurfaceRef) };
        self.mirror_texture = self
            .inner
            .backend()
            .device
            // Plane 0 — BGRA is a single interleaved plane (unlike YCbCr which has separate planes).
            .newTextureWithDescriptor_iosurface_plane(&desc, surface_ref, 0);
        if self.mirror_texture.is_none() {
            tracing::trace!("failed to create mirror texture from IOSurface");
        }
    }

    pub(super) fn clear_mirror(&mut self) {
        self.mirror_texture = None;
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn render<R>(
        &mut self,
        pixels_per_point: f32,
        visible_content_bound: Option<[f32; 4]>,
        ui_fn: impl FnMut(&mut egui::Ui) -> R,
    ) -> R {
        let (meshes, delta, surface_size, result) = self.inner.prepare(pixels_per_point, ui_fn);
        if let Some(ctx) = self.inner.begin_frame(&delta, surface_size) {
            if let Some(tex) = &self.mirror_texture
                && let Some(visible_content_bound) = visible_content_bound
            {
                draw_mirror_quad(self.inner.backend(), &ctx, tex, visible_content_bound);
            }
            self.inner.draw_egui_meshes(&ctx, &meshes, pixels_per_point);
            ctx.finish();
        }
        result
    }
}

const SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct VertexIn {
    float2 pos [[attribute(0)]];
    float2 uv  [[attribute(1)]];
    float4 color [[attribute(2)]];
};

struct VertexOut {
    float4 position [[position]];
    float2 uv;
    float4 color;
};

vertex VertexOut vertex_main(
    VertexIn in [[stage_in]],
    constant float2 &surface_size [[buffer(1)]]
) {
    VertexOut out;
    // Convert from egui coords (top-left origin, Y-down, in points) to Metal NDC
    // (-1..1, center origin, Y-up). Y is negated to flip the axis.
    out.position = float4(
        2.0 * in.pos.x / surface_size.x - 1.0,
        -(2.0 * in.pos.y / surface_size.y - 1.0),
        0.0,
        1.0
    );
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

fragment float4 fragment_egui(
    VertexOut in [[stage_in]],
    texture2d<float> tex [[texture(0)]],
    sampler smp [[sampler(0)]]
) {
    float4 color = tex.sample(smp, in.uv) * in.color;
    color.rgb *= color.a;
    return color;
}

fragment float4 fragment_mirror(
    VertexOut in [[stage_in]],
    texture2d<float> tex [[texture(0)]],
    sampler smp [[sampler(0)]]
) {
    return tex.sample(smp, in.uv);
}
"#;

/// pos(f32×2) + uv(f32×2) + color(u8×4). Shared by egui meshes and mirror quad
/// so both can use the same vertex descriptor and pipeline layout.
const VERTEX_STRIDE: usize = 20;

impl MetalBackend {
    fn create_pipeline(
        device: &ProtocolObject<dyn MTLDevice>,
        vertex_fn: &ProtocolObject<dyn MTLFunction>,
        fragment_fn: &ProtocolObject<dyn MTLFunction>,
        vertex_desc: &MTLVertexDescriptor,
    ) -> Retained<ProtocolObject<dyn MTLRenderPipelineState>> {
        let desc = MTLRenderPipelineDescriptor::new();
        desc.setVertexFunction(Some(vertex_fn));
        desc.setFragmentFunction(Some(fragment_fn));
        desc.setVertexDescriptor(Some(vertex_desc));

        let color0 = unsafe { desc.colorAttachments().objectAtIndexedSubscript(0) };
        color0.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        color0.setBlendingEnabled(true);
        // Source is One (not SrcAlpha) because egui outputs premultiplied-alpha colors.
        color0.setSourceRGBBlendFactor(MTLBlendFactor::One);
        color0.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        color0.setSourceAlphaBlendFactor(MTLBlendFactor::OneMinusDestinationAlpha);
        color0.setDestinationAlphaBlendFactor(MTLBlendFactor::One);

        device
            .newRenderPipelineStateWithDescriptor_error(&desc)
            .expect("failed to create pipeline")
    }
}

#[must_use]
struct FrameContext {
    encoder: Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>,
    cmd_buf: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    drawable: Retained<ProtocolObject<dyn CAMetalDrawable>>,
    drawable_w: NSUInteger,
    drawable_h: NSUInteger,
}

impl FrameContext {
    /// Registers drawable presentation, then Drop handles the rest.
    fn finish(self) {
        unsafe {
            let _: () = objc2::msg_send![&*self.cmd_buf, presentDrawable: &*self.drawable];
        }
    }
}

impl Drop for FrameContext {
    /// Always commits the command buffer so the drawable returns to the pool.
    /// On the normal path (finish called), this also presents the new frame.
    /// On error paths, no present was registered — previous frame stays visible.
    fn drop(&mut self) {
        self.encoder.endEncoding();
        self.cmd_buf.commit();
        // Overlays are event-driven (not vsync), so we wait for the GPU to finish
        // before returning to avoid tearing.
        self.cmd_buf.waitUntilCompleted();
    }
}

/// Split into prepare → begin_frame → draw_egui_meshes so callers can inject custom
/// draw calls (e.g. mirror quad) between Metal setup and egui mesh drawing.
struct EguiRenderer {
    backend: Rc<MetalBackend>,
    layer: Retained<CAMetalLayer>,
    egui_ctx: egui::Context,
    egui_textures: HashMap<egui::TextureId, Retained<ProtocolObject<dyn MTLTexture>>>,
    events: Rc<RefCell<Vec<egui::Event>>>,
}

impl EguiRenderer {
    fn new(backend: Rc<MetalBackend>, scale: f64, logical_w: f64, logical_h: f64) -> Self {
        let layer = CAMetalLayer::layer();
        layer.setDevice(Some(backend.device()));
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        // Overlays composite over other windows, so non-drawn areas must be fully transparent.
        // Must be premultiplied (0,0,0,0) — non-zero RGB with zero alpha would leak color
        // through the premultiplied-alpha blend.
        layer.setOpaque(false);
        layer.setContentsScale(scale);
        layer.setDrawableSize(objc2_core_foundation::CGSize {
            width: logical_w * scale,
            height: logical_h * scale,
        });

        Self {
            backend,
            layer,
            egui_ctx: egui::Context::default(),
            egui_textures: HashMap::new(),
            events: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn layer(&self) -> Retained<CAMetalLayer> {
        self.layer.clone()
    }

    fn resize(&self, logical_w: f64, logical_h: f64, scale: f64) {
        self.layer.setDrawableSize(objc2_core_foundation::CGSize {
            width: logical_w * scale,
            height: logical_h * scale,
        });
        self.layer.setContentsScale(scale);
    }

    fn events(&self) -> Rc<RefCell<Vec<egui::Event>>> {
        self.events.clone()
    }

    fn backend(&self) -> &MetalBackend {
        &self.backend
    }

    fn prepare<R>(
        &mut self,
        pixels_per_point: f32,
        mut ui_fn: impl FnMut(&mut egui::Ui) -> R,
    ) -> (
        Vec<egui::ClippedPrimitive>,
        egui::TexturesDelta,
        [f32; 2],
        R,
    ) {
        let drawable_size = self.layer.drawableSize();
        let w_pts = drawable_size.width as f32 / pixels_per_point;
        let h_pts = drawable_size.height as f32 / pixels_per_point;

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
            events: std::mem::take(&mut *self.events.borrow_mut()),
            ..Default::default()
        };

        let mut result = None;
        let output = self.egui_ctx.run(raw_input, |ctx| {
            egui::Area::new(egui::Id::new("overlay"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    result = Some(ui_fn(ui));
                });
        });
        let meshes = self
            .egui_ctx
            .tessellate(output.shapes, output.pixels_per_point);

        (
            meshes,
            output.textures_delta,
            [w_pts, h_pts],
            result.unwrap(),
        )
    }

    #[tracing::instrument(skip_all)]
    fn begin_frame(
        &mut self,
        textures_delta: &egui::TexturesDelta,
        surface_size_points: [f32; 2],
    ) -> Option<FrameContext> {
        self.update_textures(textures_delta);

        let Some(drawable) = self.layer.nextDrawable() else {
            tracing::trace!("no drawable available");
            return None;
        };
        let b = &self.backend;
        let Some(cmd_buf) = b.command_queue.commandBuffer() else {
            tracing::trace!("failed to create command buffer");
            return None;
        };

        let pass_desc = MTLRenderPassDescriptor::renderPassDescriptor();
        let color0 = unsafe { pass_desc.colorAttachments().objectAtIndexedSubscript(0) };
        color0.setTexture(Some(&drawable.texture()));
        color0.setLoadAction(MTLLoadAction::Clear);
        // Premultiplied transparent — see setOpaque(false) above.
        color0.setClearColor(MTLClearColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        });
        color0.setStoreAction(MTLStoreAction::Store);

        let Some(encoder) = cmd_buf.renderCommandEncoderWithDescriptor(&pass_desc) else {
            tracing::trace!("failed to create render encoder");
            return None;
        };

        let drawable_w = drawable.texture().width();
        let drawable_h = drawable.texture().height();

        encoder.setCullMode(MTLCullMode::None);
        encoder.setViewport(MTLViewport {
            originX: 0.0,
            originY: 0.0,
            width: drawable_w as f64,
            height: drawable_h as f64,
            znear: 0.0,
            zfar: 1.0,
        });
        unsafe {
            encoder.setFragmentSamplerState_atIndex(Some(&b.sampler), 0);
            encoder.setVertexBytes_length_atIndex(
                NonNull::new(surface_size_points.as_ptr() as *mut c_void).unwrap(),
                8,
                1,
            );
        }
        encoder.setScissorRect(MTLScissorRect {
            x: 0,
            y: 0,
            width: drawable_w,
            height: drawable_h,
        });

        Some(FrameContext {
            encoder,
            cmd_buf,
            drawable,
            drawable_w,
            drawable_h,
        })
    }

    #[tracing::instrument(skip_all)]
    fn draw_egui_meshes(
        &self,
        ctx: &FrameContext,
        meshes: &[egui::ClippedPrimitive],
        pixels_per_point: f32,
    ) {
        let b = &self.backend;
        let encoder = &ctx.encoder;

        encoder.setRenderPipelineState(&b.egui_pipeline);
        for prim in meshes {
            let egui::ClippedPrimitive {
                clip_rect,
                primitive,
            } = prim;
            let egui::epaint::Primitive::Mesh(mesh) = primitive else {
                continue;
            };
            if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                continue;
            }
            let Some(tex) = self.egui_textures.get(&mesh.texture_id) else {
                continue;
            };

            // Clamp to drawable bounds — Metal crashes if scissor rect exceeds drawable.
            let ppp = pixels_per_point;
            let sx = (clip_rect.min.x * ppp).round() as NSUInteger;
            let sy = (clip_rect.min.y * ppp).round() as NSUInteger;
            let sw = ((clip_rect.max.x - clip_rect.min.x) * ppp).round() as NSUInteger;
            let sh = ((clip_rect.max.y - clip_rect.min.y) * ppp).round() as NSUInteger;
            let dw = ctx.drawable_w;
            let dh = ctx.drawable_h;
            let sx = sx.min(dw);
            let sy = sy.min(dh);
            let sw = sw.min(dw - sx);
            let sh = sh.min(dh - sy);
            if sw == 0 || sh == 0 {
                continue;
            }

            let Some(vbuf) = (unsafe {
                b.device.newBufferWithBytes_length_options(
                    NonNull::new(mesh.vertices.as_ptr() as *mut c_void).unwrap(),
                    (mesh.vertices.len() * VERTEX_STRIDE) as NSUInteger,
                    MTLResourceOptions::StorageModeShared,
                )
            }) else {
                tracing::trace!("failed to create vertex buffer");
                continue;
            };
            let Some(ibuf) = (unsafe {
                b.device.newBufferWithBytes_length_options(
                    NonNull::new(mesh.indices.as_ptr() as *mut c_void).unwrap(),
                    (mesh.indices.len() * 4) as NSUInteger,
                    MTLResourceOptions::StorageModeShared,
                )
            }) else {
                tracing::trace!("failed to create index buffer");
                continue;
            };

            encoder.setScissorRect(MTLScissorRect {
                x: sx,
                y: sy,
                width: sw,
                height: sh,
            });
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(Some(&vbuf), 0, 0);
                encoder.setFragmentTexture_atIndex(Some(tex), 0);
                encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
                    MTLPrimitiveType::Triangle,
                    mesh.indices.len() as NSUInteger,
                    MTLIndexType::UInt32,
                    &ibuf,
                    0,
                );
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn update_textures(&mut self, delta: &egui::TexturesDelta) {
        for (id, image_delta) in &delta.set {
            let pixels: Vec<u8> = match &image_delta.image {
                egui::ImageData::Color(img) => {
                    img.pixels.iter().flat_map(|c| c.to_array()).collect()
                }
                egui::ImageData::Font(img) => {
                    img.srgba_pixels(None).flat_map(|c| c.to_array()).collect()
                }
            };
            let [w, h] = image_delta.image.size();

            if let Some(pos) = image_delta.pos {
                if let Some(tex) = self.egui_textures.get(id) {
                    let region = MTLRegion {
                        origin: MTLOrigin {
                            x: pos[0] as NSUInteger,
                            y: pos[1] as NSUInteger,
                            z: 0,
                        },
                        size: MTLSize {
                            width: w as NSUInteger,
                            height: h as NSUInteger,
                            depth: 1,
                        },
                    };
                    unsafe {
                        tex.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                            region,
                            0,
                            NonNull::new(pixels.as_ptr() as *mut c_void).unwrap(),
                            (w * 4) as NSUInteger,
                        );
                    }
                }
            } else {
                let desc = unsafe {
                    MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                        MTLPixelFormat::RGBA8Unorm,
                        w as NSUInteger,
                        h as NSUInteger,
                        false,
                    )
                };
                desc.setUsage(MTLTextureUsage::ShaderRead);
                desc.setStorageMode(MTLStorageMode::Shared);
                let Some(tex) = self.backend.device.newTextureWithDescriptor(&desc) else {
                    tracing::trace!("failed to create egui texture");
                    continue;
                };
                let region = MTLRegion {
                    origin: MTLOrigin { x: 0, y: 0, z: 0 },
                    size: MTLSize {
                        width: w as NSUInteger,
                        height: h as NSUInteger,
                        depth: 1,
                    },
                };
                unsafe {
                    tex.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                        region,
                        0,
                        NonNull::new(pixels.as_ptr() as *mut c_void).unwrap(),
                        (w * 4) as NSUInteger,
                    );
                }
                self.egui_textures.insert(*id, tex);
            }
        }
        for id in &delta.free {
            self.egui_textures.remove(id);
        }
    }
}

/// `visible_content_bound` is `[x, y, w, h]` in logical points, overlay-local (top-left origin).
/// Vertices share the same coordinate space as egui meshes, so `vertex_main` handles both.
#[tracing::instrument(skip_all)]
fn draw_mirror_quad(
    b: &MetalBackend,
    ctx: &FrameContext,
    texture: &ProtocolObject<dyn MTLTexture>,
    visible_content_bound: [f32; 4],
) {
    let [mx, my, mw, mh] = visible_content_bound;
    let encoder = &ctx.encoder;
    encoder.setRenderPipelineState(&b.mirror_pipeline);
    // [pos_x, pos_y, u, v, _]. UVs 0→1 sample the full texture, which already
    // contains only the visible portion (capture source rect matches mirror_rect).
    let verts: [[f32; 5]; 4] = [
        [mx, my, 0.0, 0.0, 0.0],
        [mx + mw, my, 1.0, 0.0, 0.0],
        [mx, my + mh, 0.0, 1.0, 0.0],
        [mx + mw, my + mh, 1.0, 1.0, 0.0],
    ];
    // Pack into vertex layout: 4 floats (pos + uv) + 4 color bytes.
    // Color is white — unused by mirror fragment shader but fills the shared layout.
    let mut vert_data = Vec::with_capacity(4 * VERTEX_STRIDE);
    for v in &verts {
        vert_data.extend_from_slice(&v[0].to_le_bytes());
        vert_data.extend_from_slice(&v[1].to_le_bytes());
        vert_data.extend_from_slice(&v[2].to_le_bytes());
        vert_data.extend_from_slice(&v[3].to_le_bytes());
        vert_data.extend_from_slice(&[255, 255, 255, 255]);
    }
    // Two triangles forming a quad: top-left (0,1,2) and bottom-right (2,1,3).
    let indices: [u32; 6] = [0, 1, 2, 2, 1, 3];
    // Shared storage — CPU writes, GPU reads. Fine for small per-frame buffers.
    // Vertex buffer: per-corner data (pos, uv, color).
    // Index buffer: which vertices form each triangle, so shared corners aren't duplicated.
    let Some(vbuf) = (unsafe {
        b.device.newBufferWithBytes_length_options(
            NonNull::new(vert_data.as_ptr() as *mut c_void).unwrap(),
            vert_data.len() as NSUInteger,
            MTLResourceOptions::StorageModeShared,
        )
    }) else {
        tracing::trace!("failed to create mirror vertex buffer");
        return;
    };
    let Some(ibuf) = (unsafe {
        b.device.newBufferWithBytes_length_options(
            NonNull::new(indices.as_ptr() as *mut c_void).unwrap(),
            (indices.len() * 4) as NSUInteger,
            MTLResourceOptions::StorageModeShared,
        )
    }) else {
        tracing::trace!("failed to create mirror index buffer");
        return;
    };

    unsafe {
        encoder.setVertexBuffer_offset_atIndex(Some(&vbuf), 0, 0);
        encoder.setFragmentTexture_atIndex(Some(texture), 0);
        encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
            MTLPrimitiveType::Triangle,
            6,
            MTLIndexType::UInt32,
            &ibuf,
            0,
        );
    }
}
