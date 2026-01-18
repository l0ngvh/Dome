use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashSet;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};

use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSScreen, NSWindow,
};
use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{
    CFDictionary, CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext, kCFBooleanTrue,
    kCFRunLoopDefaultMode,
};
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

use super::hub::{Frame, HubEvent, HubMessage, HubThread};
use super::keyboard::KeyboardListener;
use super::listeners::EventListener;
use super::overlay::{OverlayView, create_overlay_window};
use super::recovery;
use crate::config::{Config, start_config_watcher};
use crate::core::Dimension;
use crate::ipc;
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
    let screen = get_main_screen(mtm);

    let ax_registry = Rc::new(RefCell::new(AXRegistry::new()));
    let is_suspended = Rc::new(Cell::new(false));

    let _event_listener = EventListener::new(
        screen,
        ax_registry.clone(),
        event_tx.clone(),
        is_suspended.clone(),
    );

    let _keyboard_listener =
        KeyboardListener::new(keymaps, is_suspended, event_tx.clone()).inspect_err(|e| {
            tracing::error!("Failed to create keyboard listener: {e:#}");
        });

    let delegate = AppDelegate::new(mtm, screen, event_tx, frame_rx, ax_registry);
    let source = create_frame_source(&delegate);

    let main_run_loop = CFRunLoop::main().unwrap();
    main_run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    let hub_thread = HubThread::spawn(
        hub_config,
        screen,
        event_rx,
        frame_tx,
        source,
        main_run_loop,
    );

    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();

    hub_thread.join();
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
    std::panic::set_hook(Box::new(|panic_info| {
        recovery::restore_all();
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));
    recovery::install_handlers();
}

pub(super) struct AppDelegateIvars {
    pub(super) screen: Dimension,
    pub(super) ax_registry: Rc<RefCell<AXRegistry>>,
    pub(super) hub_sender: Sender<HubEvent>,
    pub(super) frame_rx: Receiver<HubMessage>,
    pub(super) overlay_window: OnceCell<Retained<NSWindow>>,
    pub(super) overlay: OnceCell<Retained<OverlayView>>,
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
            // AppDelegate lives for the entire duration of the app
            let delegate: &'static AppDelegate = unsafe { std::mem::transmute(self) };
            let mtm = self.mtm();

            let screen = delegate.ivars().screen;
            let frame = NSRect::new(
                NSPoint::new(screen.x as f64, 0.0),
                NSSize::new(screen.width as f64, screen.height as f64),
            );

            let overlay_window = create_overlay_window(mtm, frame);
            let overlay = OverlayView::new(mtm, frame);
            overlay_window.setContentView(Some(&overlay));
            overlay_window.makeKeyAndOrderFront(None);

            let _ = delegate.ivars().overlay_window.set(overlay_window);
            let _ = delegate.ivars().overlay.set(overlay);
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            recovery::restore_all();
            let _ = self.ivars().hub_sender.send(HubEvent::Shutdown);
        }
    }
);

impl AppDelegate {
    fn new(
        mtm: MainThreadMarker,
        screen: Dimension,
        hub_sender: Sender<HubEvent>,
        frame_rx: Receiver<HubMessage>,
        ax_registry: Rc<RefCell<AXRegistry>>,
    ) -> Retained<Self> {
        let ivars = AppDelegateIvars {
            screen,
            ax_registry,
            hub_sender,
            frame_rx,
            overlay_window: OnceCell::new(),
            overlay: OnceCell::new(),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }

    pub(super) fn send_event(&self, event: HubEvent) {
        send_hub_event(&self.ivars().hub_sender, event);
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
    let delegate: &'static AppDelegate = unsafe { &*(info as *const AppDelegate) };

    while let Ok(msg) = delegate.ivars().frame_rx.try_recv() {
        match msg {
            HubMessage::Frame(frame) => {
                if let Err(e) = process_frame(delegate, &frame) {
                    tracing::warn!("Failed to process frame: {e:#}");
                }
            }
            HubMessage::SyncResponse {
                managed,
                current_workspace,
            } => {
                process_sync(delegate, &managed, &current_workspace);
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
    let overlay = delegate.ivars().overlay.get().unwrap();
    let screen = delegate.ivars().screen;

    for &cg_id in frame.hide() {
        if let Some(ax_window) = ax_registry.get(cg_id)
            && let Err(e) = ax_window.hide()
        {
            tracing::trace!("Failed to hide window: {e:#}");
        }
    }

    for &(cg_id, dim) in frame.windows() {
        if let Some(ax_window) = ax_registry.get(cg_id) {
            // macOS doesn't allow windows completely offscreen - use hide position instead
            if is_completely_offscreen(dim, screen) {
                if let Err(e) = ax_window.hide() {
                    tracing::trace!("Failed to hide offscreen window: {e:#}");
                }
                continue;
            }

            if let Err(e) = ax_window.set_dimension(dim) {
                tracing::trace!("Failed to set dimension: {e:#}");
                continue;
            }

            // Min size discovery: check if window resized itself larger
            if let Ok((actual_w, actual_h)) = ax_window.get_size() {
                const EPSILON: f32 = 1.0;
                let discovered_w = if actual_w > dim.width + EPSILON {
                    actual_w
                } else {
                    0.0
                };
                let discovered_h = if actual_h > dim.height + EPSILON {
                    actual_h
                } else {
                    0.0
                };

                if discovered_w > 0.0 || discovered_h > 0.0 {
                    delegate.send_event(HubEvent::SetMinSize {
                        cg_id,
                        width: discovered_w,
                        height: discovered_h,
                    });
                }
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

fn is_completely_offscreen(dim: Dimension, screen: Dimension) -> bool {
    dim.x + dim.width <= screen.x
        || dim.x >= screen.x + screen.width
        || dim.y + dim.height <= screen.y
        || dim.y >= screen.y + screen.height
}

fn process_sync(
    delegate: &AppDelegate,
    managed: &HashSet<CGWindowID>,
    current_workspace: &HashSet<CGWindowID>,
) {
    let mut ax_registry = delegate.ivars().ax_registry.borrow_mut();
    let to_remove: Vec<_> = ax_registry
        .iter()
        .filter_map(|(cg_id, ax_window)| {
            if !managed.contains(&cg_id) {
                Some(cg_id)
            } else if !current_workspace.contains(&cg_id) {
                if let Err(e) = ax_window.hide() {
                    tracing::trace!("Failed to hide window: {e:#}");
                }
                None
            } else {
                None
            }
        })
        .collect();
    for cg_id in to_remove {
        ax_registry.remove(cg_id);
    }
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
