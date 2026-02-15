use std::sync::mpsc::Sender;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSEvent, NSResponder, NSView};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGWindowID, kCGColorSpaceSRGB};
use objc2_core_media::CMSampleBuffer;
use objc2_foundation::{NSError, NSObject, NSObjectProtocol, NSRect};
use objc2_io_surface::IOSurface;
use objc2_quartz_core::CALayer;
use objc2_screen_capture_kit::{
    SCContentFilter, SCShareableContent, SCStream, SCStreamConfiguration, SCStreamOutput,
    SCStreamOutputType,
};

use super::dome::{HubEvent, HubMessage, MessageSender};
use super::rendering::{clip_to_bounds, to_ns_rect};
use crate::core::Dimension;

pub(super) struct MirrorViewIvars {
    cg_id: CGWindowID,
    layer: Retained<CALayer>,
    hub_tx: Sender<HubEvent>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = MirrorViewIvars]
    pub(super) struct MirrorView;

    unsafe impl NSObjectProtocol for MirrorView {}

    impl MirrorView {
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            self.ivars().hub_tx.send(HubEvent::MirrorClicked(self.ivars().cg_id)).ok();
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl MirrorView {
    pub(super) fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        cg_id: CGWindowID,
        hub_tx: Sender<HubEvent>,
    ) -> Retained<Self> {
        let layer = CALayer::new();
        let this = Self::alloc(mtm).set_ivars(MirrorViewIvars {
            cg_id,
            hub_tx,
            layer: layer.clone(),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        view
    }

    pub(super) fn apply_frame(&self, surface: &IOSurface) {
        unsafe { self.ivars().layer.setContents(Some(surface)) };
    }

    pub(super) fn set_scale(&self, scale: f64) {
        self.ivars().layer.setContentsScale(scale);
    }
}

pub(super) struct WindowCapture {
    stream: Retained<SCStream>,
    handler: Retained<StreamOutputHandler>,
    running: bool,
}

// Safety: SCStream and StreamOutputHandler are thread-safe for the operations we perform
unsafe impl Send for WindowCapture {}

impl WindowCapture {
    /// `scale` is passed separately because the original window may be hidden on a different monitor.
    pub(super) fn start(
        &mut self,
        cg_id: CGWindowID,
        content_dim: Dimension,
        monitor: Dimension,
        scale: f64,
        primary_full_height: f32,
        app_tx: MessageSender,
    ) {
        let Some(clipped) = clip_to_bounds(content_dim, monitor) else {
            self.stop();
            return;
        };
        let frame = to_ns_rect(primary_full_height, clipped);
        let source_rect = compute_source_rect(content_dim, clipped);

        let width = (frame.size.width * scale) as usize;
        let height = (frame.size.height * scale) as usize;

        let config = unsafe { SCStreamConfiguration::new() };
        unsafe {
            config.setWidth(width);
            config.setHeight(height);
            config.setSourceRect(source_rect);
            // calayer expects srgb
            config.setColorSpaceName(kCGColorSpaceSRGB);
            config.setCapturesAudio(false);
            config.setCaptureMicrophone(false);
            config.setExcludesCurrentProcessAudio(false);
        }
        let block = RcBlock::new(move |error: *mut NSError| {
            if !error.is_null() {
                let error = unsafe { &*error };
                tracing::warn!(
                    cg_id,
                    width,
                    height,
                    source_x = source_rect.origin.x,
                    source_y = source_rect.origin.y,
                    source_w = source_rect.size.width,
                    source_h = source_rect.size.height,
                    %error,
                    "capture config update failed"
                );
            }
        });
        unsafe {
            self.stream
                .updateConfiguration_completionHandler(&config, Some(&block))
        };
        if !self.running {
            let block = RcBlock::new(move |error: *mut NSError| {
                if !error.is_null() {
                    let error = unsafe { &*error };
                    tracing::warn!(
                        cg_id,
                        width,
                        height,
                        source_x = source_rect.origin.x,
                        source_y = source_rect.origin.y,
                        source_w = source_rect.size.width,
                        source_h = source_rect.size.height,
                        %error,
                        "capture start failed"
                    );
                    app_tx.send(HubMessage::CaptureFailed { cg_id });
                }
            });
            unsafe { self.stream.startCaptureWithCompletionHandler(Some(&block)) };
            self.running = true;
        }
    }

    pub(super) fn stop(&mut self) {
        if self.running {
            let block = RcBlock::new(|_: *mut NSError| {});
            unsafe { self.stream.stopCaptureWithCompletionHandler(Some(&block)) };
            self.running = false;
        }
    }
}

fn compute_source_rect(original: Dimension, clipped: Dimension) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: (clipped.x - original.x) as f64,
            y: (clipped.y - original.y) as f64,
        },
        size: CGSize {
            width: clipped.width as f64,
            height: clipped.height as f64,
        },
    }
}

impl Drop for WindowCapture {
    fn drop(&mut self) {
        self.stop();
        unsafe {
            self.stream
                .removeStreamOutput_type_error(
                    ProtocolObject::from_ref(&*self.handler),
                    SCStreamOutputType::Screen,
                )
                .ok();
        }
    }
}

pub(super) fn create_captures_async(
    cg_ids: Vec<CGWindowID>,
    hub_tx: Sender<HubEvent>,
    app_tx: MessageSender,
    queue: DispatchRetained<DispatchQueue>,
) {
    let block = RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            if !error.is_null() || content.is_null() {
                tracing::error!("Failed to get shareable content");
                return;
            }
            let content = unsafe { Retained::retain(content).unwrap() };
            let sc_windows = unsafe { content.windows() };

            for cg_id in &cg_ids {
                let cg_id = *cg_id;
                let Some(sc_window) = sc_windows.iter().find(|w| unsafe { w.windowID() } == cg_id)
                else {
                    continue;
                };

                let filter = unsafe {
                    SCContentFilter::initWithDesktopIndependentWindow(
                        <SCContentFilter as AnyThread>::alloc(),
                        &sc_window,
                    )
                };

                let config = unsafe { SCStreamConfiguration::new() };
                unsafe { config.setQueueDepth(3) };

                let handler = StreamOutputHandler::new(cg_id, app_tx.clone());

                let stream = unsafe {
                    SCStream::initWithFilter_configuration_delegate(
                        <SCStream as AnyThread>::alloc(),
                        &filter,
                        &config,
                        None,
                    )
                };

                if unsafe {
                    stream.addStreamOutput_type_sampleHandlerQueue_error(
                        ProtocolObject::from_ref(&*handler),
                        SCStreamOutputType::Screen,
                        Some(&queue),
                    )
                }
                .is_err()
                {
                    continue;
                }

                let capture = WindowCapture {
                    stream,
                    handler,
                    running: false,
                };
                hub_tx.send(HubEvent::CaptureReady { cg_id, capture }).ok();
            }
        },
    );
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
}

struct StreamOutputHandlerIvars {
    cg_id: CGWindowID,
    app_tx: MessageSender,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = StreamOutputHandlerIvars]
    struct StreamOutputHandler;

    unsafe impl NSObjectProtocol for StreamOutputHandler {}

    unsafe impl SCStreamOutput for StreamOutputHandler {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn did_output_sample_buffer(
            &self,
            _stream: &SCStream,
            buffer: &CMSampleBuffer,
            output_type: SCStreamOutputType,
        ) {
            if output_type == SCStreamOutputType::Screen
                && let Some(surface) = extract_io_surface(buffer)
            {
                self.ivars().app_tx.send(HubMessage::CaptureFrame {
                    cg_id: self.ivars().cg_id,
                    surface,
                });
            }
        }
    }
);

impl StreamOutputHandler {
    fn new(cg_id: CGWindowID, app_tx: MessageSender) -> Retained<Self> {
        let this = Self::alloc().set_ivars(StreamOutputHandlerIvars { cg_id, app_tx });
        unsafe { msg_send![super(this), init] }
    }
}

fn extract_io_surface(buffer: &CMSampleBuffer) -> Option<Retained<IOSurface>> {
    unsafe {
        let image_buffer = buffer.image_buffer()?;
        let surface = objc2_core_video::CVPixelBufferGetIOSurface(Some(&image_buffer))?;
        let ptr = CFRetained::into_raw(surface).as_ptr() as *mut IOSurface;
        Some(Retained::from_raw(ptr).unwrap())
    }
}
