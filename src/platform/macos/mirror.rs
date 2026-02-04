use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Sender;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::runtime::ProtocolObject;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{
    NSBackingStoreType, NSEvent, NSResponder, NSView, NSWindow, NSWindowCollectionBehavior,
    NSWindowStyleMask,
};
use objc2_core_foundation::{CFRetained, CGRect};
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
use super::overlay::MirrorUpdate;

pub(super) struct WindowCapture {
    stream: Retained<SCStream>,
    handler: Retained<StreamOutputHandler>,
    running: bool,
}

// Safety: SCStream and StreamOutputHandler are thread-safe for the operations we perform
unsafe impl Send for WindowCapture {}

impl WindowCapture {
    pub(super) fn start(
        &mut self,
        cg_id: CGWindowID,
        source_rect: CGRect,
        width: u32,
        height: u32,
        scale: f64,
        app_tx: MessageSender,
    ) {
        let config = unsafe { SCStreamConfiguration::new() };
        unsafe {
            config.setWidth((width as f64 * scale) as usize);
            config.setHeight((height as f64 * scale) as usize);
            config.setSourceRect(source_rect);
            // calayer expects srgb
            config.setColorSpaceName(kCGColorSpaceSRGB);
        }
        let block = RcBlock::new(|_: *mut NSError| {});
        unsafe {
            self.stream
                .updateConfiguration_completionHandler(&config, Some(&block))
        };
        if !self.running {
            let block = RcBlock::new(move |error: *mut NSError| {
                if !error.is_null() {
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
            if output_type == SCStreamOutputType::Screen {
                if let Some(surface) = extract_io_surface(buffer) {
                    self.ivars().app_tx.send(HubMessage::CaptureFrame {
                        cg_id: self.ivars().cg_id,
                        surface,
                    });
                }
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

struct MirrorViewIvars {
    cg_id: CGWindowID,
    hub_tx: Sender<HubEvent>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = MirrorViewIvars]
    struct MirrorView;

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
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        cg_id: CGWindowID,
        hub_tx: Sender<HubEvent>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MirrorViewIvars { cg_id, hub_tx });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setWantsLayer(true);
        view
    }
}

struct MirrorWindow {
    window: Retained<NSWindow>,
    layer: Retained<CALayer>,
    visible: bool,
}

impl MirrorWindow {
    fn new(
        mtm: MainThreadMarker,
        cg_id: CGWindowID,
        frame: NSRect,
        level: isize,
        scale: f64,
        hub_tx: Sender<HubEvent>,
    ) -> Self {
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                frame,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };

        window.setOpaque(true);
        window.setLevel(level);
        window.setIgnoresMouseEvents(false);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary,
        );
        unsafe { window.setReleasedWhenClosed(false) };

        let view = MirrorView::new(mtm, frame, cg_id, hub_tx);
        let layer = CALayer::new();
        layer.setContentsScale(scale);
        view.setLayer(Some(&layer));
        window.setContentView(Some(&view));

        Self {
            window,
            layer,
            visible: false,
        }
    }

    fn update(&self, frame: NSRect) {
        self.window.setFrame_display(frame, true);
    }

    fn show(&mut self) {
        if !self.visible {
            self.window.orderFront(None);
            self.visible = true;
        }
    }

    fn apply_frame(&mut self, surface: &IOSurface) {
        unsafe { self.layer.setContents(Some(surface)) };
        self.show();
    }
}

impl Drop for MirrorWindow {
    fn drop(&mut self) {
        self.window.close();
    }
}

pub(super) struct MirrorManager {
    mtm: MainThreadMarker,
    mirrors: HashMap<CGWindowID, MirrorWindow>,
    hub_tx: Sender<HubEvent>,
}

impl MirrorManager {
    pub(super) fn new(mtm: MainThreadMarker, hub_tx: Sender<HubEvent>) -> Self {
        Self {
            mtm,
            mirrors: HashMap::new(),
            hub_tx,
        }
    }

    pub(super) fn process_mirrors(&mut self, updates: Vec<MirrorUpdate>) {
        let desired: HashSet<_> = updates.iter().map(|m| m.cg_id).collect();

        self.mirrors.retain(|cg_id, _| desired.contains(cg_id));

        for m in updates {
            if let Some(mirror) = self.mirrors.get(&m.cg_id) {
                mirror.update(m.frame);
            } else {
                let mirror =
                    MirrorWindow::new(self.mtm, m.cg_id, m.frame, m.level, m.scale, self.hub_tx.clone());
                self.mirrors.insert(m.cg_id, mirror);
            }
        }
    }

    pub(super) fn apply_frame(&mut self, cg_id: CGWindowID, surface: &IOSurface) {
        if let Some(mirror) = self.mirrors.get_mut(&cg_id) {
            mirror.apply_frame(surface);
        }
    }

    pub(super) fn show_error(&self, cg_id: CGWindowID) {
        tracing::warn!(cg_id, "Capture failed");
    }
}
