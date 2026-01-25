use std::cell::{Cell, OnceCell};
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;

use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSScreen, NSWindow,
};
use objc2_core_graphics::{CGDirectDisplayID, CGDisplayBounds, CGMainDisplayID};
use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{
    CFDictionary, CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext, kCFBooleanTrue,
    kCFRunLoopDefaultMode,
};
use objc2_foundation::{NSNotification, NSNumber, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

use super::dome::{Dome, HubEvent, HubMessage, MessageSender};
use super::keyboard::KeyboardListener;
use super::listeners::EventListener;
use super::overlay::{OverlayView, create_overlay_window};
use super::recovery;
use crate::config::{Config, start_config_watcher};
use crate::core::Dimension;
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
    let global_bounds = compute_global_bounds(&screens);

    let is_suspended = Rc::new(Cell::new(false));

    let event_listener = EventListener::new(event_tx.clone(), is_suspended.clone());

    let _keyboard_listener = KeyboardListener::new(keymaps, is_suspended, event_tx.clone())
        .inspect_err(|e| {
            tracing::error!("Failed to create keyboard listener: {e:#}");
        });

    let delegate = AppDelegate::new(mtm, global_bounds, event_tx, frame_rx, event_listener);
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
            Dome::new(hub_config, screens, global_bounds, sender).run(event_rx);
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

pub(super) struct AppDelegateIvars {
    pub(super) global_bounds: Dimension,
    pub(super) hub_sender: Sender<HubEvent>,
    pub(super) frame_rx: Receiver<HubMessage>,
    pub(super) overlay_window: OnceCell<Retained<NSWindow>>,
    pub(super) overlay: OnceCell<Retained<OverlayView>>,
    pub(super) event_listener: EventListener,
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
            let mtm = self.mtm();

            let bounds = self.ivars().global_bounds;
            let frame = NSRect::new(
                NSPoint::new(bounds.x as f64, 0.0),
                NSSize::new(bounds.width as f64, bounds.height as f64),
            );

            let overlay_window = create_overlay_window(mtm, frame);
            let overlay = OverlayView::new(mtm, frame);
            overlay_window.setContentView(Some(&overlay));
            overlay_window.makeKeyAndOrderFront(None);

            let _ = self.ivars().overlay_window.set(overlay_window);
            let _ = self.ivars().overlay.set(overlay);
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
        global_bounds: Dimension,
        hub_sender: Sender<HubEvent>,
        frame_rx: Receiver<HubMessage>,
        event_listener: EventListener,
    ) -> Retained<Self> {
        let ivars = AppDelegateIvars {
            global_bounds,
            hub_sender,
            frame_rx,
            overlay_window: OnceCell::new(),
            overlay: OnceCell::new(),
            event_listener,
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

    while let Ok(msg) = delegate.ivars().frame_rx.try_recv() {
        match msg {
            HubMessage::Overlays(overlays) => {
                if let Some(overlay) = delegate.ivars().overlay.get() {
                    overlay.set_rects(overlays);
                }
            }
            HubMessage::RegisterObservers(apps) => {
                for app in &apps {
                    delegate.ivars().event_listener.register_app(app);
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

#[derive(Clone)]
pub(super) struct ScreenInfo {
    pub display_id: CGDirectDisplayID,
    pub name: String,
    pub dimension: Dimension,
    pub full_height: f32,
    pub is_primary: bool,
}

fn get_display_id(screen: &NSScreen) -> CGDirectDisplayID {
    let desc = screen.deviceDescription();
    let key = NSString::from_str("NSScreenNumber");
    desc.objectForKey(&key)
        .and_then(|obj| {
            let num: Option<&NSNumber> = obj.downcast_ref();
            num.map(|n| n.unsignedIntValue())
        })
        .unwrap_or(0)
}

pub(super) fn get_all_screens(mtm: MainThreadMarker) -> Vec<ScreenInfo> {
    let primary_id = CGMainDisplayID();

    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let display_id = get_display_id(&screen);
            let name = screen.localizedName().to_string();
            let bounds = CGDisplayBounds(display_id);
            let frame = screen.frame();
            let visible = screen.visibleFrame();

            let top_inset = (frame.origin.y + frame.size.height)
                - (visible.origin.y + visible.size.height);
            let bottom_inset = visible.origin.y - frame.origin.y;

            ScreenInfo {
                display_id,
                name,
                dimension: Dimension {
                    x: bounds.origin.x as f32,
                    y: (bounds.origin.y + top_inset) as f32,
                    width: bounds.size.width as f32,
                    height: (bounds.size.height - top_inset - bottom_inset) as f32,
                },
                full_height: bounds.size.height as f32,
                is_primary: display_id == primary_id,
            }
        })
        .collect()
}

pub(super) fn compute_global_bounds(screens: &[ScreenInfo]) -> Dimension {
    let min_x = screens.iter().map(|s| s.dimension.x).fold(f32::MAX, f32::min);
    let min_y = screens.iter().map(|s| s.dimension.y).fold(f32::MAX, f32::min);
    let max_x = screens
        .iter()
        .map(|s| s.dimension.x + s.dimension.width)
        .fold(f32::MIN, f32::max);
    let max_y = screens
        .iter()
        .map(|s| s.dimension.y + s.dimension.height)
        .fold(f32::MIN, f32::max);
    Dimension {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}
