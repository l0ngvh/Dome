use std::ffi::c_void;

use objc2::rc::Retained;
use objc2_quartz_core::{CAMetalLayer, CATransaction};

use crate::platform::render::Compositor;

pub(super) struct MacOsCompositor {
    layer: Retained<CAMetalLayer>,
    corner_radius: Option<f64>,
}

impl MacOsCompositor {
    pub(super) fn new(scale: f64, corner_radius: Option<f64>) -> Self {
        let layer: Retained<CAMetalLayer> = CAMetalLayer::new();
        // Set before any drawable exists so first-frame composition is at Retina density.
        layer.setContentsScale(scale);
        Self {
            layer,
            corner_radius,
        }
    }

    pub(super) fn layer(&self) -> Retained<CAMetalLayer> {
        self.layer.clone()
    }
}

impl Compositor for MacOsCompositor {
    fn surface_target(&self) -> wgpu::SurfaceTargetUnsafe {
        wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(Retained::as_ptr(&self.layer) as *mut c_void)
    }

    fn format(&self) -> wgpu::TextureFormat {
        // Non-sRGB keeps egui's premultiplied output byte-identical on the wire.
        wgpu::TextureFormat::Bgra8Unorm
    }

    fn before_configure(&self, scale: f32) {
        self.layer.setContentsScale(scale as f64);
    }

    fn after_configure(&self, surface: &wgpu::Surface<'static>) -> anyhow::Result<()> {
        configure_layer(surface, self.corner_radius);
        Ok(())
    }

    fn begin_present(&self) {
        // Disable Core Animation's implicit crossfade so the surface swap is atomic.
        CATransaction::begin();
        CATransaction::setDisableActions(true);
    }

    fn end_present(&self) {
        CATransaction::commit();
    }
}

pub(super) fn physical_size(logical_w: f64, logical_h: f64, scale: f64) -> (u32, u32) {
    let w = (logical_w * scale).round() as u32;
    let h = (logical_h * scale).round() as u32;
    (w.max(1), h.max(1))
}

fn configure_layer(surface: &wgpu::Surface<'static>, corner_radius: Option<f64>) {
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
        if let Some(r) = corner_radius {
            layer.setCornerRadius(r);
            layer.setMasksToBounds(true);
        }
    }
}
