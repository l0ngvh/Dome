use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::os::unix::net::UnixListener;
use std::rc::Rc;

use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSFloatingWindowLevel,
    NSNormalWindowLevel, NSScreen, NSWindow,
};
use objc2_application_services::AXIsProcessTrustedWithOptions;
use objc2_core_foundation::{CFMachPort, CFRetained, kCFBooleanTrue};
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::context::{Observers, ThrottleState, WindowRegistry};
use super::handler::render_workspace;
use super::ipc;
use super::listeners::{listen_to_input_devices, setup_app_observers};
use super::overlay::{OverlayView, create_overlay_window};
use crate::config::Config;
use crate::core::{Dimension, Hub};

pub fn run_app(config: Config) {
    use objc2_application_services::kAXTrustedCheckOptionPrompt;
    use objc2_core_foundation::CFDictionary;

    tracing::debug!("Accessibility: {}", unsafe {
        AXIsProcessTrustedWithOptions(Some(
            CFDictionary::from_slices(&[kAXTrustedCheckOptionPrompt], &[kCFBooleanTrue.unwrap()])
                .as_opaque(),
        ))
    });

    let mtm = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let delegate = AppDelegate::new(mtm, config);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    app.run();
}

pub(super) struct AppDelegateIvars {
    pub(super) config: Config,
    pub(super) hub: RefCell<Hub>,
    pub(super) registry: RefCell<WindowRegistry>,
    pub(super) throttle: RefCell<ThrottleState>,
    pub(super) displayed_windows: RefCell<HashSet<CGWindowID>>,
    pub(super) observers: Observers,
    pub(super) tiling_overlay_window: OnceCell<Retained<NSWindow>>,
    pub(super) tiling_overlay: OnceCell<Retained<OverlayView>>,
    pub(super) float_overlay_window: OnceCell<Retained<NSWindow>>,
    pub(super) float_overlay: OnceCell<Retained<OverlayView>>,
    pub(super) event_tap: OnceCell<CFRetained<CFMachPort>>,
    pub(super) listener: OnceCell<UnixListener>,
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

            let screen = delegate.ivars().hub.borrow().screen();
            let frame = NSRect::new(
                NSPoint::new(screen.x as f64, 0.0),
                NSSize::new(screen.width as f64, screen.height as f64),
            );

            let tiling_overlay_window = create_overlay_window(mtm, frame, NSNormalWindowLevel - 1);
            let tiling_overlay = OverlayView::new(mtm, frame);
            tiling_overlay_window.setContentView(Some(&tiling_overlay));
            tiling_overlay_window.makeKeyAndOrderFront(None);

            let float_overlay_window = create_overlay_window(mtm, frame, NSFloatingWindowLevel);
            let float_overlay = OverlayView::new(mtm, frame);
            float_overlay_window.setContentView(Some(&float_overlay));
            float_overlay_window.makeKeyAndOrderFront(None);

            let _ = delegate.ivars().listener.set(listener);
            let _ = delegate
                .ivars()
                .tiling_overlay_window
                .set(tiling_overlay_window);
            let _ = delegate.ivars().tiling_overlay.set(tiling_overlay.clone());
            let _ = delegate
                .ivars()
                .float_overlay_window
                .set(float_overlay_window);
            let _ = delegate.ivars().float_overlay.set(float_overlay.clone());

            if let Err(e) = ipc::register_with_runloop(delegate) {
                tracing::error!("Failed to setup IPC: {e:#}");
            }

            if let Err(e) = listen_to_input_devices(delegate) {
                tracing::error!("Failed to setup keyboard listener: {e:#}");
            }

            setup_app_observers(delegate);

            if let Err(e) = render_workspace(delegate) {
                tracing::warn!("Failed to render workspace after initialization: {e:#}");
            }
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            // Nothing to clean up - ivars are dropped automatically
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker, config: Config) -> Retained<Self> {
        let screen = get_main_screen(mtm);
        let hub = Hub::new(screen, config.tab_bar_height, config.automatic_tiling);
        let ivars = AppDelegateIvars {
            config,
            hub: RefCell::new(hub),
            registry: RefCell::new(WindowRegistry::new()),
            throttle: RefCell::new(ThrottleState::new()),
            displayed_windows: RefCell::new(HashSet::new()),
            observers: Rc::new(RefCell::new(HashMap::new())),
            tiling_overlay_window: OnceCell::new(),
            tiling_overlay: OnceCell::new(),
            float_overlay_window: OnceCell::new(),
            float_overlay: OnceCell::new(),
            event_tap: OnceCell::new(),
            listener: OnceCell::new(),
            is_suspended: Cell::new(false),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
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
