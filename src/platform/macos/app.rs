use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;

use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{
    CFDictionary, CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext, kCFBooleanTrue,
    kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess, CGWindowID,
};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol};
use objc2_metal::MTLCreateSystemDefaultDevice;
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

use super::dome::{Dome, HubEvent, HubMessage, MessageSender};
use super::keyboard::KeyboardListener;
use super::listeners::EventListener;
use super::monitor::get_all_screens;
use super::overlay::{OverlayManager, OverlayWindow};
use super::recovery;
use super::renderer::MetalBackend;
use crate::config::{Config, start_config_watcher};
use crate::ipc;

pub fn run_app(config_path: Option<String>) -> anyhow::Result<()> {
    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config from {config_path}: {e}, using defaults");
        Config::default()
    });

    recovery::install_handlers();
    init_tracing(&config);
    tracing::info!(%config_path, "Loaded config");

    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));

    tracing::debug!("Accessibility: {}", unsafe {
        AXIsProcessTrustedWithOptions(Some(
            CFDictionary::from_slices(&[kAXTrustedCheckOptionPrompt], &[kCFBooleanTrue.unwrap()])
                .as_opaque(),
        ))
    });

    if !CGPreflightScreenCaptureAccess() {
        tracing::info!("Screen recording permission not granted, requesting...");
        if !CGRequestScreenCaptureAccess() {
            return Err(anyhow::anyhow!(
                "Screen recording permission required. Please grant permission in System Settings > Privacy & Security > Screen Recording, then restart Dome."
            ));
        }
    }

    let mtm = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let (event_tx, event_rx) = mpsc::channel();
    let (frame_tx, frame_rx) = mpsc::channel();

    let hub_config = config.clone();
    let keymaps = Arc::new(RwLock::new(config.keymaps.clone()));

    let _config_watcher = start_config_watcher(&config_path, {
        let keymaps = keymaps.clone();
        let tx = event_tx.clone();
        move |cfg| {
            *keymaps.write().unwrap() = cfg.keymaps.clone();
            tx.send(HubEvent::ConfigChanged(cfg)).ok();
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    ipc::start_server({
        let tx = event_tx.clone();
        move |actions| {
            tx.send(HubEvent::Action(actions))
                .or(Err(anyhow::anyhow!("channel closed")))
        }
    })?;

    let screens = get_all_screens(mtm);
    if screens.is_empty() {
        return Err(anyhow::anyhow!("No monitors detected"));
    }

    let is_suspended = Rc::new(Cell::new(false));

    let event_listener = EventListener::new(event_tx.clone(), is_suspended.clone());

    let _keyboard_listener = KeyboardListener::new(keymaps, is_suspended, event_tx.clone())?;

    let hub_tx = event_tx.clone();
    let device = MTLCreateSystemDefaultDevice().expect("no Metal device");
    let backend = MetalBackend::new(&device);
    let delegate = AppDelegate::new(
        mtm,
        event_tx,
        frame_rx,
        event_listener,
        backend,
        config.clone(),
    );
    let source = create_frame_source(&delegate);

    let main_run_loop = CFRunLoop::main().unwrap();
    main_run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    let sender = MessageSender {
        tx: frame_tx,
        source,
        run_loop: main_run_loop,
    };

    let hub_thread = thread::spawn(move || {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Dome::new(hub_config, screens, hub_tx, sender).run(event_rx);
        }))
        .is_err()
        {
            recovery::restore_all();
        }
    });

    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.run())).is_err() {
        recovery::restore_all();
    }

    hub_thread.join().ok();
    Ok(())
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
}

struct AppDelegateIvars {
    hub_sender: Sender<HubEvent>,
    frame_rx: Receiver<HubMessage>,
    overlay_manager: RefCell<OverlayManager>,
    overlay_windows: RefCell<HashMap<CGWindowID, OverlayWindow>>,
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
        hub_sender: Sender<HubEvent>,
        frame_rx: Receiver<HubMessage>,
        event_listener: EventListener,
        backend: Rc<MetalBackend>,
        config: Config,
    ) -> Retained<Self> {
        let ivars = AppDelegateIvars {
            hub_sender: hub_sender.clone(),
            frame_rx,
            overlay_manager: RefCell::new(OverlayManager::new(backend.clone(), config.clone())),
            overlay_windows: RefCell::new(HashMap::new()),
            event_listener,
            backend,
            config: RefCell::new(config),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
}

pub(super) fn send_hub_event(hub_sender: &Sender<HubEvent>, event: HubEvent) {
    if hub_sender.send(event).is_err() {
        tracing::error!("Hub thread died, shutting down");
        let mtm = MainThreadMarker::new().unwrap();
        NSApplication::sharedApplication(mtm).terminate(None);
    }
}

unsafe extern "C-unwind" fn frame_callback(info: *mut c_void) {
    let delegate: &AppDelegate = unsafe { &*(info as *const AppDelegate) };
    let mtm = delegate.mtm();
    while let Ok(msg) = delegate.ivars().frame_rx.try_recv() {
        match msg {
            HubMessage::Overlays(overlays) => {
                delegate.ivars().overlay_manager.borrow_mut().process(
                    mtm,
                    overlays,
                    &delegate.ivars().hub_sender,
                );
            }
            HubMessage::RegisterObservers(apps) => {
                for app in &apps {
                    delegate.ivars().event_listener.register_app(app);
                }
            }
            HubMessage::CaptureFrame { cg_id, surface } => {
                if let Some(overlay) = delegate
                    .ivars()
                    .overlay_windows
                    .borrow_mut()
                    .get_mut(&cg_id)
                {
                    overlay.apply_frame(&surface);
                }
            }
            HubMessage::CaptureFailed { cg_id } => {
                tracing::debug!("Failed to screen capture for {cg_id}");
            }
            HubMessage::WindowCreate { cg_id, frame } => {
                let config = delegate.ivars().config.borrow().clone();
                let overlay = OverlayWindow::new(
                    mtm,
                    frame,
                    cg_id,
                    delegate.ivars().hub_sender.clone(),
                    delegate.ivars().backend.clone(),
                    config,
                );
                delegate
                    .ivars()
                    .overlay_windows
                    .borrow_mut()
                    .insert(cg_id, overlay);
            }
            HubMessage::WindowShow {
                cg_id,
                placement,
                cocoa_frame,
                scale,
            } => {
                if let Some(overlay) = delegate
                    .ivars()
                    .overlay_windows
                    .borrow_mut()
                    .get_mut(&cg_id)
                {
                    overlay.render(&placement, cocoa_frame, scale);
                }
            }
            HubMessage::WindowHide { cg_id } => {
                if let Some(overlay) = delegate.ivars().overlay_windows.borrow().get(&cg_id) {
                    overlay.hide();
                }
            }
            HubMessage::WindowDelete { cg_id } => {
                delegate.ivars().overlay_windows.borrow_mut().remove(&cg_id);
            }
            HubMessage::ConfigChanged(new_config) => {
                *delegate.ivars().config.borrow_mut() = new_config.clone();
                for overlay in delegate.ivars().overlay_windows.borrow_mut().values_mut() {
                    overlay.set_config(new_config.clone());
                }
                delegate
                    .ivars()
                    .overlay_manager
                    .borrow_mut()
                    .set_config(new_config);
            }
            HubMessage::Shutdown => {
                NSApplication::sharedApplication(mtm).terminate(None);
                return;
            }
        }
    }
}
