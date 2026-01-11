use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::os::unix::net::UnixListener;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};

use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSScreen, NSWindow,
};
use objc2_application_services::{
    AXIsProcessTrustedWithOptions, AXObserver, kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{
    CFDictionary, CFFileDescriptor, CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource,
    CFRunLoopSourceContext, CFRunLoopTimer, kCFBooleanTrue, kCFRunLoopDefaultMode,
};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

use super::config_watcher::setup_config_watcher;
use super::hub::{Frame, HubEvent, HubMessage, HubThread};
use super::ipc;
use super::listeners::{ThrottleState, listen_to_input_devices, setup_app_observers};
use super::overlay::{OverlayView, create_overlay_window};
use crate::config::Config;
use crate::core::Dimension;
use crate::platform::macos::window::AXRegistry;

pub fn run_app(config_path: Option<String>) -> anyhow::Result<()> {
    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config from {config_path}: {e}, using defaults");
        Config::default()
    });

    init_tracing(&config);

    tracing::debug!("Accessibility: {}", unsafe {
        AXIsProcessTrustedWithOptions(Some(
            CFDictionary::from_slices(&[kAXTrustedCheckOptionPrompt], &[kCFBooleanTrue.unwrap()])
                .as_opaque(),
        ))
    });

    let mtm = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let delegate = AppDelegate::new(mtm, config, config_path);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    app.run();
    Ok(())
}

fn init_tracing(config: &Config) {
    let filter = config
        .log_level
        .as_ref()
        .and_then(|l| l.parse().ok())
        .unwrap_or_else(EnvFilter::from_default_env);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(ErrorLayer::default())
        .init();
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));
}

pub(super) struct AppDelegateIvars {
    pub(super) config: RefCell<Config>,
    pub(super) config_path: String,
    pub(super) screen: Dimension,
    pub(super) border_size: Cell<f32>,
    pub(super) ax_registry: RefCell<AXRegistry>,
    pub(super) throttle: RefCell<ThrottleState>,
    /// References to all observers to prevent them from being dropped
    pub(super) observers: Observers,
    pub(super) hub_thread: RefCell<Option<HubThread>>,
    pub(super) hub_sender: OnceCell<Sender<HubEvent>>,
    pub(super) frame_rx: OnceCell<Receiver<HubMessage>>,
    /// Reference to overlay window to prevent it from being dropped
    pub(super) overlay_window: OnceCell<Retained<NSWindow>>,
    pub(super) overlay: OnceCell<Retained<OverlayView>>,
    pub(super) event_tap: OnceCell<CFRetained<CFMachPort>>,
    pub(super) listener: OnceCell<UnixListener>,
    pub(super) config_fd: OnceCell<CFRetained<CFFileDescriptor>>,
    pub(super) sync_timer: OnceCell<CFRetained<CFRunLoopTimer>>,
    /// To suspend on sleep/screen lock to save battery
    /// Not reliable to detect whether screen is locked for other purposes as screen sleep/lock
    /// notification can arrive after screen is locked
    pub(super) is_suspended: Cell<bool>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppDelegateIvars]
    pub(super) struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            tracing::info!("Application did finish launching");
            // Safety: AppDelegate lives until the end of the app
            let delegate: &'static AppDelegate = unsafe { std::mem::transmute(self) };
            let mtm = self.mtm();

            let listener = match ipc::try_bind() {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("{e}");
                    NSApplication::sharedApplication(mtm).terminate(None);
                    return;
                }
            };

            let screen = delegate.ivars().screen;
            let frame = NSRect::new(
                NSPoint::new(screen.x as f64, 0.0),
                NSSize::new(screen.width as f64, screen.height as f64),
            );

            let overlay_window = create_overlay_window(mtm, frame);
            let overlay = OverlayView::new(mtm, frame);
            overlay_window.setContentView(Some(&overlay));
            overlay_window.makeKeyAndOrderFront(None);

            let _ = delegate.ivars().listener.set(listener);
            let _ = delegate.ivars().overlay_window.set(overlay_window);
            let _ = delegate.ivars().overlay.set(overlay);

            let (event_tx, event_rx) = mpsc::channel();
            let (frame_tx, frame_rx) = mpsc::channel();

            let delegate_ptr = delegate as *const AppDelegate as *mut c_void;
            let mut context = CFRunLoopSourceContext {
                version: 0,
                info: delegate_ptr,
                retain: None,
                release: None,
                copyDescription: None,
                equal: None,
                hash: None,
                schedule: None,
                cancel: None,
                perform: Some(frame_callback),
            };

            let source = unsafe { CFRunLoopSource::new(None, 0, &mut context).unwrap() };
            let main_run_loop = CFRunLoop::current().unwrap();
            main_run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

            let config = delegate.ivars().config.borrow().clone();
            let hub_thread =
                HubThread::spawn(config, screen, event_rx, frame_tx, source, main_run_loop);

            let _ = delegate.ivars().hub_thread.borrow_mut().replace(hub_thread);
            let _ = delegate.ivars().hub_sender.set(event_tx);
            let _ = delegate.ivars().frame_rx.set(frame_rx);

            if let Err(e) = ipc::register_with_runloop(delegate) {
                tracing::error!("Failed to setup IPC: {e:#}");
            }

            if let Err(e) = listen_to_input_devices(delegate) {
                tracing::error!("Failed to setup keyboard listener: {e:#}");
            }

            if let Err(e) = setup_config_watcher(delegate) {
                tracing::warn!("Failed to setup config watcher: {e:#}");
            }

            setup_app_observers(delegate);
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            if let Some(timer) = self.ivars().sync_timer.get() {
                CFRunLoopTimer::invalidate(timer);
            }

            if let Some(sender) = self.ivars().hub_sender.get() {
                let _ = sender.send(HubEvent::Shutdown);
            }

            if let Some(handle) = self.ivars().hub_thread.borrow_mut().take() {
                handle.join();
            }
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker, config: Config, config_path: String) -> Retained<Self> {
        let screen = get_main_screen(mtm);
        let border_size = config.border_size;
        let ivars = AppDelegateIvars {
            config: RefCell::new(config),
            config_path,
            screen,
            border_size: Cell::new(border_size),
            ax_registry: RefCell::new(AXRegistry::new()),
            throttle: RefCell::new(ThrottleState::new()),
            observers: Rc::new(RefCell::new(HashMap::new())),
            hub_thread: RefCell::new(None),
            hub_sender: OnceCell::new(),
            frame_rx: OnceCell::new(),
            overlay_window: OnceCell::new(),
            overlay: OnceCell::new(),
            event_tap: OnceCell::new(),
            listener: OnceCell::new(),
            config_fd: OnceCell::new(),
            sync_timer: OnceCell::new(),
            is_suspended: Cell::new(false),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }

    pub(super) fn send_event(&self, event: HubEvent) {
        if let Some(sender) = self.ivars().hub_sender.get()
            && sender.send(event).is_err()
        {
            tracing::error!("Hub thread died, shutting down");
            let mtm = MainThreadMarker::new().unwrap();
            NSApplication::sharedApplication(mtm).terminate(None);
        }
    }
}

unsafe extern "C-unwind" fn frame_callback(info: *mut c_void) {
    let delegate: &'static AppDelegate = unsafe { &*(info as *const AppDelegate) };
    let Some(frame_rx) = delegate.ivars().frame_rx.get() else {
        return;
    };

    while let Ok(msg) = frame_rx.try_recv() {
        match msg {
            HubMessage::Frame(frame) => {
                if let Err(e) = process_frame(delegate, &frame) {
                    tracing::warn!("Failed to process frame: {e:#}");
                }
            }
            HubMessage::Shutdown => {
                let mtm = MainThreadMarker::new().unwrap();
                NSApplication::sharedApplication(mtm).terminate(None);
                return;
            }
        }
    }
}

fn process_frame(delegate: &AppDelegate, frame: &Frame) -> anyhow::Result<()> {
    let ax_registry = delegate.ivars().ax_registry.borrow();
    let border = delegate.ivars().border_size.get();
    let overlay = delegate.ivars().overlay.get().unwrap();

    for &cg_id in frame.hide() {
        if let Some(ax_window) = ax_registry.get(cg_id)
            && let Err(e) = ax_window.hide()
        {
            tracing::trace!("Failed to hide window: {e:#}");
        }
    }

    for &(cg_id, dim) in frame.windows() {
        if let Some(ax_window) = ax_registry.get(cg_id) {
            let inset = Dimension {
                x: dim.x + border,
                y: dim.y + border,
                width: dim.width - 2.0 * border,
                height: dim.height - 2.0 * border,
            };
            if let Err(e) = ax_window.set_dimension(inset) {
                tracing::trace!("Failed to set dimension: {e:#}");
            }
        }
    }

    let overlays = frame.overlays();
    overlay.set_rects(overlays.rects.clone(), overlays.labels.clone());

    if let Some(cg_id) = frame.focus()
        && let Some(ax_window) = ax_registry.get(cg_id)
    {
        ax_window.focus()?;
    }

    Ok(())
}

fn get_main_screen(mtm: MainThreadMarker) -> Dimension {
    let main_screen = NSScreen::mainScreen(mtm).unwrap();
    let frame = main_screen.frame();
    let visible_frame = main_screen.visibleFrame();
    Dimension {
        x: visible_frame.origin.x as f32,
        y: (frame.size.height - visible_frame.size.height) as f32,
        width: visible_frame.size.width as f32,
        height: visible_frame.size.height as f32,
    }
}

type Observers = Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>;
