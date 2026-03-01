use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSNormalWindowLevel, NSResponder, NSView, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::CGFloat;
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSString, NSUInteger};
use objc2_io_surface::IOSurface;
use objc2_metal::*;
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

use super::dome::HubEvent;
use crate::config::Config;
use crate::core::{ContainerId, ContainerPlacement};
use crate::overlay;

const SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct VertexIn {
    float2 pos [[attribute(0)]];
    float2 uv  [[attribute(1)]];
    float4 color [[attribute(2)]]; // uchar4Normalized → float4
};

struct VertexOut {
    float4 position [[position]];
    float2 uv;
    float4 color;
};

vertex VertexOut vertex_main(
    VertexIn in [[stage_in]],
    constant float2 &screen_size [[buffer(1)]]
) {
    VertexOut out;
    out.position = float4(
        2.0 * in.pos.x / screen_size.x - 1.0,
        -(2.0 * in.pos.y / screen_size.y - 1.0),
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

struct MetalEguiRenderer {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    egui_pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    mirror_pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    egui_textures: HashMap<egui::TextureId, Retained<ProtocolObject<dyn MTLTexture>>>,
}

impl MetalEguiRenderer {
    fn new(device: &ProtocolObject<dyn MTLDevice>) -> Self {
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
        // pos: float2, offset 0
        let attr0 = unsafe { attrs.objectAtIndexedSubscript(0) };
        attr0.setFormat(MTLVertexFormat::Float2);
        unsafe { attr0.setOffset(0) };
        unsafe { attr0.setBufferIndex(0) };
        // uv: float2, offset 8
        let attr1 = unsafe { attrs.objectAtIndexedSubscript(1) };
        attr1.setFormat(MTLVertexFormat::Float2);
        unsafe { attr1.setOffset(8) };
        unsafe { attr1.setBufferIndex(0) };
        // color: uchar4Normalized, offset 16
        let attr2 = unsafe { attrs.objectAtIndexedSubscript(2) };
        attr2.setFormat(MTLVertexFormat::UChar4Normalized);
        unsafe { attr2.setOffset(16) };
        unsafe { attr2.setBufferIndex(0) };
        // layout: stride 20
        let layout0 = unsafe { vertex_desc.layouts().objectAtIndexedSubscript(0) };
        unsafe { layout0.setStride(20) };
        layout0.setStepFunction(MTLVertexStepFunction::PerVertex);

        let egui_pipeline =
            Self::create_pipeline(device, &vertex_fn, &fragment_egui_fn, &vertex_desc);
        let mirror_pipeline =
            Self::create_pipeline(device, &vertex_fn, &fragment_mirror_fn, &vertex_desc);

        let sampler_desc = MTLSamplerDescriptor::new();
        sampler_desc.setMinFilter(MTLSamplerMinMagFilter::Linear);
        sampler_desc.setMagFilter(MTLSamplerMinMagFilter::Linear);
        let sampler = device
            .newSamplerStateWithDescriptor(&sampler_desc)
            .expect("failed to create sampler");

        Self {
            device: device.retain(),
            command_queue,
            egui_pipeline,
            mirror_pipeline,
            sampler,
            egui_textures: HashMap::new(),
        }
    }

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
        color0.setSourceRGBBlendFactor(MTLBlendFactor::One);
        color0.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        color0.setSourceAlphaBlendFactor(MTLBlendFactor::OneMinusDestinationAlpha);
        color0.setDestinationAlphaBlendFactor(MTLBlendFactor::One);

        device
            .newRenderPipelineStateWithDescriptor_error(&desc)
            .expect("failed to create pipeline")
    }

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
                // Partial update
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
                // Full texture creation
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
                let tex = self
                    .device
                    .newTextureWithDescriptor(&desc)
                    .expect("failed to create texture");
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

    fn render(
        &mut self,
        drawable: &ProtocolObject<dyn CAMetalDrawable>,
        meshes: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
        pixels_per_point: f32,
        screen_size_points: [f32; 2],
        mirror_texture: Option<&ProtocolObject<dyn MTLTexture>>,
        mirror_rect: Option<[f32; 4]>,
    ) {
        self.update_textures(textures_delta);

        let Some(cmd_buf) = self.command_queue.commandBuffer() else {
            return;
        };

        let pass_desc = MTLRenderPassDescriptor::renderPassDescriptor();
        let color0 = unsafe { pass_desc.colorAttachments().objectAtIndexedSubscript(0) };
        color0.setTexture(Some(&drawable.texture()));
        color0.setLoadAction(MTLLoadAction::Clear);
        color0.setClearColor(MTLClearColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        });
        color0.setStoreAction(MTLStoreAction::Store);

        let Some(encoder) = cmd_buf.renderCommandEncoderWithDescriptor(&pass_desc) else {
            return;
        };

        let drawable_w = drawable.texture().width() as f64;
        let drawable_h = drawable.texture().height() as f64;

        encoder.setCullMode(MTLCullMode::None);
        encoder.setViewport(MTLViewport {
            originX: 0.0,
            originY: 0.0,
            width: drawable_w,
            height: drawable_h,
            znear: 0.0,
            zfar: 1.0,
        });
        unsafe {
            encoder.setFragmentSamplerState_atIndex(Some(&self.sampler), 0);
            encoder.setVertexBytes_length_atIndex(
                NonNull::new(screen_size_points.as_ptr() as *mut c_void).unwrap(),
                8,
                1,
            );
        }

        // Mirror quad (if present) — drawn first as background
        if let Some(mirror_tex) = mirror_texture {
            let [mx, my, mw, mh] =
                mirror_rect.unwrap_or([0.0, 0.0, screen_size_points[0], screen_size_points[1]]);
            encoder.setRenderPipelineState(&self.mirror_pipeline);
            let verts: [[f32; 5]; 4] = [
                [mx, my, 0.0, 0.0, 0.0],
                [mx + mw, my, 1.0, 0.0, 0.0],
                [mx, my + mh, 0.0, 1.0, 0.0],
                [mx + mw, my + mh, 1.0, 1.0, 0.0],
            ];
            // Pack as egui vertex format: pos(f32x2) + uv(f32x2) + color(u8x4)
            let mut vert_data = Vec::with_capacity(4 * 20);
            for v in &verts {
                vert_data.extend_from_slice(&v[0].to_le_bytes()); // pos.x
                vert_data.extend_from_slice(&v[1].to_le_bytes()); // pos.y
                vert_data.extend_from_slice(&v[2].to_le_bytes()); // uv.x
                vert_data.extend_from_slice(&v[3].to_le_bytes()); // uv.y
                vert_data.extend_from_slice(&[255, 255, 255, 255]); // color
            }
            let indices: [u32; 6] = [0, 1, 2, 2, 1, 3];
            let vbuf = unsafe {
                self.device.newBufferWithBytes_length_options(
                    NonNull::new(vert_data.as_ptr() as *mut c_void).unwrap(),
                    vert_data.len() as NSUInteger,
                    MTLResourceOptions::StorageModeShared,
                )
            }
            .expect("failed to create vertex buffer");
            let ibuf = unsafe {
                self.device.newBufferWithBytes_length_options(
                    NonNull::new(indices.as_ptr() as *mut c_void).unwrap(),
                    (indices.len() * 4) as NSUInteger,
                    MTLResourceOptions::StorageModeShared,
                )
            }
            .expect("failed to create index buffer");

            unsafe {
                encoder.setVertexBuffer_offset_atIndex(Some(&vbuf), 0, 0);
                encoder.setFragmentTexture_atIndex(Some(mirror_tex), 0);
            }
            encoder.setScissorRect(MTLScissorRect {
                x: 0,
                y: 0,
                width: drawable_w as NSUInteger,
                height: drawable_h as NSUInteger,
            });
            unsafe {
                encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
                    MTLPrimitiveType::Triangle,
                    6,
                    MTLIndexType::UInt32,
                    &ibuf,
                    0,
                );
            }
        }

        // Egui meshes
        encoder.setRenderPipelineState(&self.egui_pipeline);
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

            let ppp = pixels_per_point;
            let sx = (clip_rect.min.x * ppp).round() as NSUInteger;
            let sy = (clip_rect.min.y * ppp).round() as NSUInteger;
            let sw = ((clip_rect.max.x - clip_rect.min.x) * ppp).round() as NSUInteger;
            let sh = ((clip_rect.max.y - clip_rect.min.y) * ppp).round() as NSUInteger;
            let dw = drawable_w as NSUInteger;
            let dh = drawable_h as NSUInteger;
            let sx = sx.min(dw);
            let sy = sy.min(dh);
            let sw = sw.min(dw - sx);
            let sh = sh.min(dh - sy);
            if sw == 0 || sh == 0 {
                continue;
            }

            let vbuf = unsafe {
                self.device.newBufferWithBytes_length_options(
                    NonNull::new(mesh.vertices.as_ptr() as *mut c_void).unwrap(),
                    (mesh.vertices.len() * 20) as NSUInteger,
                    MTLResourceOptions::StorageModeShared,
                )
            }
            .expect("failed to create vertex buffer");
            let ibuf = unsafe {
                self.device.newBufferWithBytes_length_options(
                    NonNull::new(mesh.indices.as_ptr() as *mut c_void).unwrap(),
                    (mesh.indices.len() * 4) as NSUInteger,
                    MTLResourceOptions::StorageModeShared,
                )
            }
            .expect("failed to create index buffer");

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

        encoder.endEncoding();
        unsafe {
            let _: () = objc2::msg_send![&*cmd_buf, presentDrawable: drawable];
        }
        cmd_buf.commit();
        cmd_buf.waitUntilCompleted();
    }
}

/// Per-overlay renderer wrapping Metal + egui context.
pub(super) struct OverlayRenderer {
    metal: MetalEguiRenderer,
    pub(super) layer: Retained<CAMetalLayer>,
    egui_ctx: egui::Context,
    events: Rc<RefCell<Vec<egui::Event>>>,
    mirror_texture: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
    mirror_rect: Option<[f32; 4]>, // [x, y, w, h] in logical points, overlay-local
}

impl OverlayRenderer {
    pub(super) fn new(
        device: &ProtocolObject<dyn MTLDevice>,
        layer: Retained<CAMetalLayer>,
    ) -> Self {
        Self {
            metal: MetalEguiRenderer::new(device),
            layer,
            egui_ctx: egui::Context::default(),
            events: Rc::new(RefCell::new(Vec::new())),
            mirror_texture: None,
            mirror_rect: None,
        }
    }

    pub(super) fn set_mirror_surface(
        &mut self,
        surface: &IOSurface,
        width: NSUInteger,
        height: NSUInteger,
    ) {
        let desc = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                MTLPixelFormat::BGRA8Unorm,
                width,
                height,
                false,
            )
        };
        desc.setUsage(MTLTextureUsage::ShaderRead);
        // IOSurface (ObjC) and IOSurfaceRef (CF) are toll-free bridged
        let surface_ref: &objc2_io_surface::IOSurfaceRef =
            unsafe { &*(surface as *const IOSurface as *const objc2_io_surface::IOSurfaceRef) };
        self.mirror_texture =
            self.metal
                .device
                .newTextureWithDescriptor_iosurface_plane(&desc, surface_ref, 0);
    }

    pub(super) fn clear_mirror(&mut self) {
        self.mirror_texture = None;
        self.mirror_rect = None;
    }

    pub(super) fn set_mirror_rect(&mut self, rect: [f32; 4]) {
        self.mirror_rect = Some(rect);
    }

    pub(super) fn events(&self) -> Rc<RefCell<Vec<egui::Event>>> {
        self.events.clone()
    }

    /// Run egui and render. Returns the value produced by `ui_fn`.
    pub(super) fn render<R>(
        &mut self,
        pixels_per_point: f32,
        ui_fn: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
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

        let mut ui_fn = Some(ui_fn);
        let mut result = None;
        let output = self.egui_ctx.run(raw_input, |ctx| {
            egui::Area::new(egui::Id::new("overlay"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    result = Some(ui_fn.take().unwrap()(ui));
                });
        });
        let meshes = self
            .egui_ctx
            .tessellate(output.shapes, output.pixels_per_point);

        if let Some(drawable) = self.layer.nextDrawable() {
            self.metal.render(
                &drawable,
                &meshes,
                &output.textures_delta,
                pixels_per_point,
                [w_pts, h_pts],
                self.mirror_texture.as_deref(),
                self.mirror_rect,
            );
        }
        result.unwrap()
    }
}

pub(super) fn create_metal_layer(
    device: &ProtocolObject<dyn MTLDevice>,
    scale: CGFloat,
) -> Retained<CAMetalLayer> {
    let layer = CAMetalLayer::layer();
    layer.setDevice(Some(device));
    layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    layer.setOpaque(false);
    layer.setContentsScale(scale);
    layer
}

pub(super) struct MetalOverlayViewIvars {
    layer: Retained<CAMetalLayer>,
    events: Rc<RefCell<Vec<egui::Event>>>,
    hub_sender: Option<Sender<HubEvent>>,
    pub(super) cg_id: std::cell::Cell<u32>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = MetalOverlayViewIvars]
    pub(super) struct MetalOverlayView;

    unsafe impl NSObjectProtocol for MetalOverlayView {}

    impl MetalOverlayView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(wantsLayer))]
        fn wants_layer(&self) -> bool {
            true
        }

        #[unsafe(method(makeBackingLayer))]
        fn make_backing_layer(&self) -> *mut objc2_quartz_core::CALayer {
            let layer: Retained<objc2_quartz_core::CALayer> =
                unsafe { Retained::cast_unchecked(self.ivars().layer.clone()) };
            Retained::into_raw(layer)
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if self.ivars().hub_sender.is_some() {
                let sender = self.ivars().hub_sender.as_ref().unwrap();
                sender
                    .send(HubEvent::MirrorClicked(self.ivars().cg_id.get()))
                    .ok();
            } else {
                let pos = self.event_pos(event);
                self.ivars().events.borrow_mut().push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            if self.ivars().hub_sender.is_none() {
                let pos = self.event_pos(event);
                self.ivars().events.borrow_mut().push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            if self.ivars().hub_sender.is_none() {
                let pos = self.event_pos(event);
                self.ivars()
                    .events
                    .borrow_mut()
                    .push(egui::Event::PointerMoved(pos));
            }
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            if self.ivars().hub_sender.is_none() {
                let pos = self.event_pos(event);
                self.ivars()
                    .events
                    .borrow_mut()
                    .push(egui::Event::PointerMoved(pos));
            }
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl MetalOverlayView {
    pub(super) fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        layer: Retained<CAMetalLayer>,
        events: Rc<RefCell<Vec<egui::Event>>>,
        hub_sender: Option<Sender<HubEvent>>,
    ) -> Retained<Self> {
        let ivars = MetalOverlayViewIvars {
            layer,
            events,
            hub_sender,
            cg_id: std::cell::Cell::new(0),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn event_pos(&self, event: &NSEvent) -> egui::Pos2 {
        let loc = event.locationInWindow();
        let view_loc = self.convertPoint_fromView(loc, None);
        egui::pos2(view_loc.x as f32, view_loc.y as f32)
    }
}

fn create_overlay_window(mtm: MainThreadMarker, frame: NSRect, level: isize) -> Retained<NSWindow> {
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setOpaque(false);
    window.setLevel(level);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary,
    );
    unsafe { window.setReleasedWhenClosed(false) };
    window
}

pub(super) struct ContainerOverlayData {
    pub(super) placement: ContainerPlacement,
    pub(super) tab_titles: Vec<String>,
    pub(super) cocoa_frame: NSRect,
}

struct ContainerOverlayEntry {
    window: Retained<NSWindow>,
    renderer: OverlayRenderer,
}

pub(super) struct OverlayManager {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    containers: HashMap<ContainerId, ContainerOverlayEntry>,
}

impl OverlayManager {
    pub(super) fn new(device: Retained<ProtocolObject<dyn MTLDevice>>) -> Self {
        Self {
            device,
            containers: HashMap::new(),
        }
    }

    pub(super) fn process(
        &mut self,
        mtm: MainThreadMarker,
        overlays: Vec<ContainerOverlayData>,
        config: &Config,
        hub_sender: &Sender<HubEvent>,
    ) {
        let new_ids: std::collections::HashSet<_> =
            overlays.iter().map(|o| o.placement.id).collect();
        self.containers.retain(|k, entry| {
            let keep = new_ids.contains(k);
            if !keep {
                entry.window.close();
            }
            keep
        });

        for data in overlays {
            let id = data.placement.id;
            let scale = self.backing_scale(mtm);

            if let Some(entry) = self.containers.get_mut(&id) {
                entry.window.setFrame_display(data.cocoa_frame, true);
                let layer = &entry.renderer.layer;
                let size = data.cocoa_frame.size;
                layer.setDrawableSize(objc2_core_foundation::CGSize {
                    width: size.width * scale,
                    height: size.height * scale,
                });
                layer.setContentsScale(scale);
                let clicked = entry.renderer.render(scale as f32, |ui| {
                    overlay::show_container(ui, &data.placement, &data.tab_titles, config)
                });
                if let Some(tab_idx) = clicked {
                    hub_sender.send(HubEvent::TabClicked(id, tab_idx)).ok();
                }
            } else {
                let window = create_overlay_window(mtm, data.cocoa_frame, NSNormalWindowLevel - 1);
                window.setIgnoresMouseEvents(false);
                window.setAcceptsMouseMovedEvents(true);

                let layer = create_metal_layer(&self.device, scale);
                let size = data.cocoa_frame.size;
                layer.setDrawableSize(objc2_core_foundation::CGSize {
                    width: size.width * scale,
                    height: size.height * scale,
                });

                let mut renderer = OverlayRenderer::new(&self.device, layer.clone());
                let events = renderer.events();
                let view = MetalOverlayView::new(
                    mtm,
                    NSRect::new(NSPoint::new(0.0, 0.0), data.cocoa_frame.size),
                    layer,
                    events,
                    None,
                );
                window.setContentView(Some(&view));
                window.orderFront(None);

                let clicked = renderer.render(scale as f32, |ui| {
                    overlay::show_container(ui, &data.placement, &data.tab_titles, config)
                });
                if let Some(tab_idx) = clicked {
                    hub_sender.send(HubEvent::TabClicked(id, tab_idx)).ok();
                }

                self.containers
                    .insert(id, ContainerOverlayEntry { window, renderer });
            }
        }
    }

    fn backing_scale(&self, mtm: MainThreadMarker) -> CGFloat {
        objc2_app_kit::NSScreen::mainScreen(mtm)
            .map(|s| s.backingScaleFactor())
            .unwrap_or(2.0)
    }
}
