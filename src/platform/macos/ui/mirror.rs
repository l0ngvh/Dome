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
use crate::core::{Dimension, WindowId};

pub(super) struct WindowCapture {
    stream: Retained<SCStream>,
    handler: Retained<StreamOutputHandler>,
    running: bool,
}

// Safety: SCStream and StreamOutputHandler are thread-safe for the operations we perform
unsafe impl Send for WindowCapture {}

impl WindowCapture {
    /// `content_dim` is the unclipped dimension of the captured window without the border, used to
    /// calculate where in the window to start capturing
    /// `visible_content` is the only visible section of the captured window
    /// `scale` is passed separately because the original window may be hidden on a different monitor.
    pub(super) fn start(
        &mut self,
        window_id: WindowId,
        content_dim: Dimension,
        visible_content: Dimension,
        scale: f64,
    ) {
        let source_rect = compute_source_rect(content_dim, visible_content);
        let width = (visible_content.width as f64 * scale) as usize;
        let height = (visible_content.height as f64 * scale) as usize;

        let config = unsafe { SCStreamConfiguration::new() };
        unsafe {
            config.setWidth(width);
            config.setHeight(height);
            config.setSourceRect(source_rect);
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
                    %window_id,
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
                        %window_id,
                        width,
                        height,
                        source_x = source_rect.origin.x,
                        source_y = source_rect.origin.y,
                        source_w = source_rect.size.width,
                        source_h = source_rect.size.height,
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
    windows: Vec<(CGWindowID, WindowId)>,
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

            for &(cg_id, window_id) in &windows {
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

                let handler = StreamOutputHandler::new(window_id);

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
                        .overlay_windows
                        .borrow()
                        .contains_key(&window_id)
                    {
                        delegate
                            .ivars()
                            .captures
                            .borrow_mut()
                            .insert(window_id, capture);
                    }
                });
            }
        },
    );
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
}

struct StreamOutputHandlerIvars {
    window_id: WindowId,
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
                let window_id = self.ivars().window_id;
                DispatchQueue::main().exec_async(move || {
                    let delegate = app_delegate();
                    if let Some(overlay) = delegate
                        .ivars()
                        .overlay_windows
                        .borrow_mut()
                        .get_mut(&window_id)
                    {
                        overlay.apply_frame(&surface);
                    }
                });
            }
        }
    }
);

impl StreamOutputHandler {
    fn new(window_id: WindowId) -> Retained<Self> {
        let this = Self::alloc().set_ivars(StreamOutputHandlerIvars { window_id });
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

fn extract_io_surface(buffer: &CMSampleBuffer) -> Option<Retained<IOSurface>> {
    unsafe {
        let image_buffer = buffer.image_buffer()?;
        let surface = objc2_core_video::CVPixelBufferGetIOSurface(Some(&image_buffer))?;
        let ptr = CFRetained::into_raw(surface).as_ptr() as *mut IOSurface;
        Some(Retained::from_raw(ptr).unwrap())
    }
}
