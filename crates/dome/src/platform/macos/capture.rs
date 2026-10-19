use super::objc2_wrapper::dimension_to_cg_rect;
use block2::{Block, RcBlock};
use dispatch2::DispatchQueue;
use dome_core::Dimension;
use futures::channel::{mpsc, oneshot};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, available, define_class, msg_send};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{CGWindowID, kCGColorSpaceSRGB};
use objc2_core_media::CMSampleBuffer;
use objc2_foundation::{NSError, NSObject, NSObjectProtocol};
use objc2_io_surface::IOSurface;
use objc2_screen_capture_kit::{
    SCCaptureResolutionType, SCContentFilter, SCShareableContent, SCStream, SCStreamConfiguration,
    SCStreamOutput, SCStreamOutputType,
};
use std::sync::Mutex;

pub(in crate::platform::macos) struct WindowCapture {
    stream: Retained<SCStream>,
    handler: Retained<StreamOutputHandler>,
    running: bool,
}

pub(in crate::platform::macos) struct CaptureSession {
    pub capture: WindowCapture,
    pub frames: mpsc::UnboundedReceiver<Retained<IOSurface>>,
}

impl WindowCapture {
    pub(super) fn is_running(&self) -> bool {
        self.running
    }

    /// `source` is the captured part of the window, relative to the window's top-left corner.
    /// `scale` is the destination display's backing scale. The captured window may be parked on another display.
    pub(super) async fn update_configuration(
        &mut self,
        source: Dimension,
        scale: f64,
    ) -> anyhow::Result<()> {
        let width = (source.width.value() * scale) as usize;
        let height = (source.height.value() * scale) as usize;

        let config = unsafe { SCStreamConfiguration::new() };
        unsafe {
            config.setWidth(width);
            config.setHeight(height);
            config.setSourceRect(dimension_to_cg_rect(source));
            config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));
            config.setColorSpaceName(kCGColorSpaceSRGB);
            config.setCapturesAudio(false);
            config.setCaptureMicrophone(false);
            config.setExcludesCurrentProcessAudio(false);
            config.setShowsCursor(false);
            if available!(macos = 14.0) {
                // With the shadow, the output also holds the shadow margin. The window's
                // transparent rounded corners then fill with shadow pixels.
                config.setIgnoreShadowsSingleWindow(true);
                // A parked window lies mostly off the screen. The global clip would crop that part.
                config.setIgnoreGlobalClipSingleWindow(true);
                config.setCaptureResolution(SCCaptureResolutionType::Best);
            }
        }
        await_completion(|completion| unsafe {
            self.stream
                .updateConfiguration_completionHandler(&config, Some(completion));
        })
        .await
    }

    pub(super) async fn start(&mut self) -> anyhow::Result<()> {
        if !self.running {
            self.running = true;
            if let Err(error) = await_completion(|completion| unsafe {
                self.stream
                    .startCaptureWithCompletionHandler(Some(completion));
            })
            .await
            {
                self.running = false;
                return Err(error);
            }
        }
        Ok(())
    }

    pub(super) async fn stop(&mut self) -> anyhow::Result<()> {
        if self.running {
            await_completion(|completion| unsafe {
                self.stream
                    .stopCaptureWithCompletionHandler(Some(completion));
            })
            .await?;
            self.running = false;
        }
        Ok(())
    }
}

impl Drop for WindowCapture {
    fn drop(&mut self) {
        // Capture can still need stopping if its start or stop future was dropped before native completion.
        if self.running {
            unsafe { self.stream.stopCaptureWithCompletionHandler(None) };
        }
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

pub(in crate::platform::macos) async fn create_capture(
    native_window_id: Option<CGWindowID>,
    queue: &DispatchQueue,
) -> anyhow::Result<Option<CaptureSession>> {
    let content = shareable_content().await?;
    let Some(cg_id) = native_window_id else {
        return Ok(None);
    };
    let available_windows = unsafe { content.windows() };
    let Some(window) = available_windows
        .iter()
        .find(|window| unsafe { window.windowID() } == cg_id)
    else {
        return Ok(None);
    };
    let filter = unsafe {
        SCContentFilter::initWithDesktopIndependentWindow(
            <SCContentFilter as AnyThread>::alloc(),
            &window,
        )
    };
    let config = unsafe { SCStreamConfiguration::new() };
    unsafe {
        config.setQueueDepth(3);
    }
    let (sender, frames) = mpsc::unbounded();
    let handler = StreamOutputHandler::new(sender);
    let stream = unsafe {
        SCStream::initWithFilter_configuration_delegate(
            <SCStream as AnyThread>::alloc(),
            &filter,
            &config,
            None,
        )
    };
    if let Err(error) = unsafe {
        stream.addStreamOutput_type_sampleHandlerQueue_error(
            ProtocolObject::from_ref(&*handler),
            SCStreamOutputType::Screen,
            Some(queue),
        )
    } {
        tracing::warn!(cg_id, %error, "capture output registration failed");
        return Ok(None);
    }
    Ok(Some(CaptureSession {
        capture: WindowCapture {
            stream,
            handler,
            running: false,
        },
        frames,
    }))
}

async fn await_completion(
    operation: impl FnOnce(&Block<dyn Fn(*mut NSError)>),
) -> anyhow::Result<()> {
    let (sender, completion) = oneshot::channel();
    let sender = Mutex::new(Some(sender));
    let block = RcBlock::new(move |error: *mut NSError| {
        let result = if error.is_null() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("{}", unsafe { &*error }))
        };
        if let Some(sender) = sender.lock().unwrap().take() {
            let _ = sender.send(result);
        }
    });
    operation(&block);
    completion.await?
}

async fn shareable_content() -> anyhow::Result<Retained<SCShareableContent>> {
    let (sender, completion) = oneshot::channel();
    let sender = Mutex::new(Some(sender));
    let block = RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            let result = if !error.is_null() {
                Err(anyhow::anyhow!("{}", unsafe { &*error }))
            } else {
                unsafe { Retained::retain(content) }.ok_or_else(|| {
                    anyhow::anyhow!("ScreenCaptureKit returned no shareable content")
                })
            };
            if let Some(sender) = sender.lock().unwrap().take() {
                let _ = sender.send(result);
            }
        },
    );
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
    completion.await?
}

struct StreamOutputHandlerIvars {
    frames: mpsc::UnboundedSender<Retained<IOSurface>>,
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
                let _ = self.ivars().frames.unbounded_send(surface);
            }
        }
    }
);

impl StreamOutputHandler {
    fn new(frames: mpsc::UnboundedSender<Retained<IOSurface>>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(StreamOutputHandlerIvars { frames });
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
