use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_app_kit::NSApplication;
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGWindowID, kCGColorSpaceSRGB};
use objc2_core_media::CMSampleBuffer;
use objc2_foundation::{MainThreadMarker, NSError, NSObject, NSObjectProtocol};
use objc2_io_surface::IOSurface;
use objc2_screen_capture_kit::{
    SCContentFilter, SCShareableContent, SCStream, SCStreamConfiguration, SCStreamOutput,
    SCStreamOutputType,
};

use super::AppDelegate;
use crate::core::Dimension;

pub(super) struct WindowCapture {
    stream: Retained<SCStream>,
    handler: Retained<StreamOutputHandler>,
    running: bool,
}

// Safety: SCStream and StreamOutputHandler are thread-safe for the operations we perform
unsafe impl Send for WindowCapture {}

impl WindowCapture {
    /// Only used for float windows, where the entire content is visible (no viewport clipping).
    /// `content_dim` is the window content area (frame minus border).
    /// `scale` is passed separately because the original window may be hidden on a different monitor.
    pub(super) fn start(&mut self, cg_id: CGWindowID, content_dim: Dimension, scale: f64) {
        let width = (content_dim.width as f64 * scale) as usize;
        let height = (content_dim.height as f64 * scale) as usize;

        let config = unsafe { SCStreamConfiguration::new() };
        unsafe {
            config.setWidth(width);
            config.setHeight(height);
            // Full content, no sub-rect clipping needed for floats
            config.setSourceRect(CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize {
                    width: content_dim.width as f64,
                    height: content_dim.height as f64,
                },
            });
            config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));
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
                        %error,
                        "capture start failed"
                    );
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
    windows: Vec<CGWindowID>,
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

            for &cg_id in &windows {
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

                let handler = StreamOutputHandler::new(cg_id);

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
                DispatchQueue::main().exec_async(move || {
                    let delegate = app_delegate();
                    if delegate
                        .ivars()
                        .float_overlays
                        .borrow()
                        .contains_key(&cg_id)
                    {
                        delegate
                            .ivars()
                            .captures
                            .borrow_mut()
                            .insert(cg_id, capture);
                    }
                });
            }
        },
    );
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
}

struct StreamOutputHandlerIvars {
    cg_id: CGWindowID,
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
                let cg_id = self.ivars().cg_id;
                DispatchQueue::main().exec_async(move || {
                    let delegate = app_delegate();
                    if let Some(overlay) =
                        delegate.ivars().float_overlays.borrow_mut().get_mut(&cg_id)
                    {
                        overlay.apply_frame(&surface);
                    }
                });
            }
        }
    }
);

impl StreamOutputHandler {
    fn new(cg_id: CGWindowID) -> Retained<Self> {
        let this = Self::alloc().set_ivars(StreamOutputHandlerIvars { cg_id });
        unsafe { msg_send![super(this), init] }
    }
}

fn app_delegate() -> &'static AppDelegate {
    // Safety: we are on the main thread (dispatched via DispatchQueue::main),
    // and the app delegate is always our AppDelegate
    unsafe {
        let mtm = MainThreadMarker::new_unchecked();
        let app = NSApplication::sharedApplication(mtm);
        let delegate = app.delegate().unwrap();
        &*(Retained::as_ptr(&delegate) as *const AppDelegate)
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
