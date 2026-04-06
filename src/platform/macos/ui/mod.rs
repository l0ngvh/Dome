mod mirror;
mod overlay;
mod renderer;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::mpsc;

use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use objc2_core_foundation::{
    CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol};
use objc2_metal::MTLCreateSystemDefaultDevice;

use super::dome::{FrameSender, HubEvent, HubMessage};
use super::listeners::EventListener;
use crate::config::Config;
use crate::core::MonitorId;
use mirror::{WindowCapture, create_captures_async};
use overlay::{FloatOverlay, TilingOverlay};
use renderer::MetalBackend;

#[derive(Clone)]
pub(crate) struct MessageSender {
    tx: mpsc::Sender<HubMessage>,
    source: CFRetained<CFRunLoopSource>,
    run_loop: CFRetained<CFRunLoop>,
}

// Safety: CFRunLoopSource and CFRunLoop are thread-safe for signal/wake_up operations
unsafe impl Send for MessageSender {}

impl MessageSender {
    pub(super) fn send(&self, msg: HubMessage) {
        if self.tx.send(msg).is_ok() {
            self.signal();
        }
    }

    fn signal(&self) {
        self.source.signal();
        self.run_loop.wake_up();
    }
}

impl FrameSender for MessageSender {
    fn send(&self, msg: HubMessage) {
        MessageSender::send(self, msg);
    }
}

pub(super) struct Ui {
    delegate: Retained<AppDelegate>,
    app: Retained<NSApplication>,
    shutdown_tx: calloop::channel::Sender<HubEvent>,
}

impl Ui {
    pub(super) fn new(
        mtm: MainThreadMarker,
        hub_sender: calloop::channel::Sender<HubEvent>,
        event_listener: EventListener,
        config: Config,
    ) -> (Self, MessageSender) {
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        let (frame_tx, frame_rx) = mpsc::channel();

        let device = MTLCreateSystemDefaultDevice().expect("no Metal device");
        let backend = MetalBackend::new(&device);
        let delegate = AppDelegate::new(
            mtm,
            hub_sender.clone(),
            frame_rx,
            event_listener,
            backend,
            config,
        );
        let source = create_frame_source(&delegate);

        let main_run_loop = CFRunLoop::main().unwrap();
        main_run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

        let sender = MessageSender {
            tx: frame_tx,
            source,
            run_loop: main_run_loop,
        };

        (
            Self {
                delegate,
                app,
                shutdown_tx: hub_sender,
            },
            sender,
        )
    }

    pub(super) fn run(self) {
        self.app
            .setDelegate(Some(ProtocolObject::from_ref(&*self.delegate)));

        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.app.run())).is_err() {
            self.shutdown_tx.send(HubEvent::Shutdown).ok();
        }
    }
}

fn create_frame_source(delegate: &Retained<AppDelegate>) -> CFRetained<CFRunLoopSource> {
    let mut context = CFRunLoopSourceContext {
        version: 0,
        info: Retained::as_ptr(delegate) as *mut c_void,
        retain: None,
        release: None,
        copyDescription: None,
        equal: None,
        hash: None,
        schedule: None,
        cancel: None,
        perform: Some(frame_callback),
    };
    unsafe { CFRunLoopSource::new(None, 0, &mut context).unwrap() }
}

struct AppDelegateIvars {
    hub_sender: calloop::channel::Sender<HubEvent>,
    frame_rx: mpsc::Receiver<HubMessage>,
    // Serial background queue for SCStream output handlers — keeps IOSurface extraction
    // off the main thread while preserving frame ordering
    capture_queue: DispatchRetained<DispatchQueue>,
    tiling_overlays: RefCell<HashMap<MonitorId, TilingOverlay>>,
    float_overlays: RefCell<HashMap<CGWindowID, FloatOverlay>>,
    captures: RefCell<HashMap<CGWindowID, WindowCapture>>,
    event_listener: EventListener,
    backend: Rc<MetalBackend>,
    config: RefCell<Config>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppDelegateIvars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            tracing::info!("Application did finish launching");
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            let _ = self.ivars().hub_sender.send(HubEvent::Shutdown);
        }
    }
);

impl AppDelegate {
    fn new(
        mtm: MainThreadMarker,
        hub_sender: calloop::channel::Sender<HubEvent>,
        frame_rx: mpsc::Receiver<HubMessage>,
        event_listener: EventListener,
        backend: Rc<MetalBackend>,
        config: Config,
    ) -> Retained<Self> {
        let ivars = AppDelegateIvars {
            hub_sender: hub_sender.clone(),
            frame_rx,
            capture_queue: DispatchQueue::new("dome.capture", None),
            tiling_overlays: RefCell::new(HashMap::new()),
            float_overlays: RefCell::new(HashMap::new()),
            captures: RefCell::new(HashMap::new()),
            event_listener,
            backend,
            config: RefCell::new(config),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
}

unsafe extern "C-unwind" fn frame_callback(info: *mut c_void) {
    let delegate: &AppDelegate = unsafe { &*(info as *const AppDelegate) };
    let mtm = delegate.mtm();
    while let Ok(msg) = delegate.ivars().frame_rx.try_recv() {
        match msg {
            HubMessage::Frame(frame) => {
                let mut tiling_overlays = delegate.ivars().tiling_overlays.borrow_mut();
                let mut float_overlays = delegate.ivars().float_overlays.borrow_mut();
                let mut captures = delegate.ivars().captures.borrow_mut();

                let config = delegate.ivars().config.borrow().clone();
                let backend = delegate.ivars().backend.clone();
                let hub_sender = delegate.ivars().hub_sender.clone();

                // Tiling overlays: one per monitor
                let active_monitors: Vec<_> = frame.tiling.iter().map(|t| t.monitor_id).collect();
                for data in &frame.tiling {
                    let overlay = tiling_overlays.entry(data.monitor_id).or_insert_with(|| {
                        TilingOverlay::new(
                            mtm,
                            backend.clone(),
                            config.clone(),
                            data.cocoa_frame,
                            data.scale,
                            hub_sender.clone(),
                        )
                    });
                    if data.windows.is_empty() && data.containers.is_empty() {
                        overlay.hide();
                    } else {
                        overlay.render(
                            data.cocoa_frame,
                            data.scale,
                            data.monitor_dim,
                            &data.windows,
                            &data.containers,
                        );
                    }
                }
                tiling_overlays.retain(|id, _| active_monitors.contains(id));

                // Float overlays: create on first show
                let mut capture_pairs = Vec::new();
                for show in &frame.float_shows {
                    let is_new = !float_overlays.contains_key(&show.cg_id);
                    let overlay = float_overlays.entry(show.cg_id).or_insert_with(|| {
                        FloatOverlay::new(
                            mtm,
                            show.cocoa_frame,
                            show.cg_id,
                            hub_sender.clone(),
                            backend.clone(),
                            config.clone(),
                        )
                    });
                    overlay.render(
                        &show.placement,
                        show.cocoa_frame,
                        show.scale,
                        show.visible_content,
                    );

                    if is_new {
                        capture_pairs.push(show.cg_id);
                    }

                    if let Some(capture) = captures.get_mut(&show.cg_id) {
                        if !show.placement.is_focused
                            && let Some(visible_content) = show.visible_content
                        {
                            capture.start(
                                show.cg_id,
                                show.content_dim,
                                visible_content,
                                show.scale,
                            );
                        } else {
                            capture.stop();
                        }
                    }
                }

                if !capture_pairs.is_empty() {
                    create_captures_async(capture_pairs, delegate.ivars().capture_queue.clone());
                }

                // Float windows are rare, so we can afford recreating overlays
                // and captures each time the workspace changes rather than
                // tracking which windows transitioned from float to tiling.
                let active_floats: HashSet<CGWindowID> =
                    frame.float_shows.iter().map(|s| s.cg_id).collect();
                float_overlays.retain(|cg_id, _| active_floats.contains(cg_id));
                captures.retain(|cg_id, _| active_floats.contains(cg_id));
            }
            HubMessage::RegisterObservers(apps) => {
                for app in &apps {
                    delegate.ivars().event_listener.register_app(app);
                }
            }
            HubMessage::ConfigChanged(new_config) => {
                *delegate.ivars().config.borrow_mut() = new_config.clone();
                for overlay in delegate.ivars().float_overlays.borrow_mut().values_mut() {
                    overlay.set_config(new_config.clone());
                }
                for overlay in delegate.ivars().tiling_overlays.borrow().values() {
                    overlay.set_config(new_config.clone());
                }
            }
            HubMessage::Shutdown => {
                NSApplication::sharedApplication(mtm).terminate(None);
                return;
            }
        }
    }
}
