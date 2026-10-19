use super::capture_stream::{CaptureSettings, CaptureStream, CaptureSurface, CreateCapture};
use super::{MirrorPlacement, frame_attrs};
use crate::platform::macos::ui::window::Window;
use crate::platform::macos::ui::window::WindowAttributes;
use crate::platform::macos::ui::window::WindowInput;
use dome_core::WindowId;
use objc2::rc::Retained;
use objc2_io_surface::IOSurface;
use objc2_quartz_core::{CALayer, CATransaction, kCAGravityResize};
use std::rc::Rc;

/// A captured window image, drawn independently of its border.
pub(in crate::platform::macos) struct MirrorOverlay {
    capture: CaptureStream,
    window: Window,
    surface: Rc<dyn CaptureSurface>,
}

impl MirrorOverlay {
    pub(super) fn new(
        window_id: WindowId,
        show: &MirrorPlacement,
        captures: Rc<dyn CreateCapture>,
    ) -> Self {
        let (position, size) = frame_attrs(show.cocoa_frame);
        let window = Window::new(
            &WindowAttributes {
                position,
                size,
                click_through: false,
                focusable: false,
            },
            Some(WindowInput::Mirror(window_id)),
        )
        .expect("auxiliary window on main thread");
        window.set_level(show.level);
        let layer = CALayer::layer();
        unsafe { layer.setContentsGravity(kCAGravityResize) };
        window.set_content_layer(&layer);
        let surface: Rc<dyn CaptureSurface> = Rc::new(Layer(layer));
        surface.set_scale(show.scale);
        window.set_visible(true);
        let capture = CaptureStream::new(
            window_id,
            surface.clone(),
            CaptureSettings {
                source: show.source,
                scale: show.scale,
            },
            captures,
        );
        Self {
            capture,
            window,
            surface,
        }
    }

    pub(super) fn update(&mut self, show: &MirrorPlacement) {
        let (position, size) = frame_attrs(show.cocoa_frame);
        self.window.set_frame(position, size);
        self.surface.set_scale(show.scale);
        self.window.set_level(show.level);
        self.window.set_visible(true);
        self.capture.update(CaptureSettings {
            source: show.source,
            scale: show.scale,
        });
    }
}

struct Layer(Retained<CALayer>);

impl CaptureSurface for Layer {
    fn set_contents(&self, surface: &IOSurface) {
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        unsafe { self.0.setContents(Some(surface)) };
        CATransaction::commit();
    }

    fn set_scale(&self, scale: f64) {
        self.0.setContentsScale(scale);
    }
}
