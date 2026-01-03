use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSFloatingWindowLevel,
    NSNormalWindowLevel, NSScreen,
};
use objc2_application_services::AXIsProcessTrustedWithOptions;
use objc2_core_foundation::kCFBooleanTrue;
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::context::{Observers, WindowContext};
use super::handler::render_workspace;
use super::ipc;
use super::listeners::{listen_to_input_devices, setup_app_observers};
use super::overlay::{OverlayView, create_overlay_window};
use crate::config::Config;
use crate::core::Dimension;

pub fn run_app(config_path: Option<String>) {
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

    let delegate = AppDelegate::new(mtm, config_path);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    app.run();
}

#[derive(Default)]
struct AppDelegateIvars {
    config_path: Option<String>,
    context: std::cell::OnceCell<*mut WindowContext>,
    observers: std::cell::OnceCell<Observers>,
    tiling_overlay_window: std::cell::OnceCell<Retained<objc2_app_kit::NSWindow>>,
    float_overlay_window: std::cell::OnceCell<Retained<objc2_app_kit::NSWindow>>,
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
            let mtm = self.mtm();

            let listener = match ipc::try_bind() {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("{e}");
                    NSApplication::sharedApplication(mtm).terminate(None);
                    return;
                }
            };

            let config = Config::load(self.ivars().config_path.as_deref());
            let screen = get_main_screen();
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

            let context_ptr = Box::into_raw(Box::new(WindowContext::new(
                tiling_overlay,
                float_overlay,
                screen,
                config,
                listener,
            )));

            if let Err(e) = ipc::register_with_runloop(context_ptr) {
                tracing::error!("Failed to setup IPC: {e:#}");
            }

            if let Err(e) = listen_to_input_devices(context_ptr) {
                tracing::error!("Failed to setup keyboard listener: {e:#}");
            }

            let apps = setup_app_observers(context_ptr);

            let context = unsafe { &mut *context_ptr };
            if let Err(e) = render_workspace(context) {
                tracing::warn!("Failed to render workspace after initialization: {e:#}");
            }

            self.ivars().context.set(context_ptr).unwrap();
            self.ivars().observers.set(apps).unwrap();
            self.ivars()
                .tiling_overlay_window
                .set(tiling_overlay_window)
                .unwrap();
            self.ivars()
                .float_overlay_window
                .set(float_overlay_window)
                .unwrap();
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            if let Some(&context_ptr) = self.ivars().context.get() {
                let _ = unsafe { Box::from_raw(context_ptr) };
            }
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker, config_path: Option<String>) -> Retained<Self> {
        let ivars = AppDelegateIvars {
            config_path,
            ..Default::default()
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
}

fn get_main_screen() -> Dimension {
    let mtm = MainThreadMarker::new().unwrap();
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
