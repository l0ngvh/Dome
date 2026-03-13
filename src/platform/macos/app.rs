use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, RwLock};
use std::thread;

use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSNormalWindowLevel,
};
use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{
    CFDictionary, CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext, kCFBooleanTrue,
    kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect};
use objc2_metal::MTLCreateSystemDefaultDevice;

use super::dome::{Dome, HubEvent, HubMessage, MessageSender};
use super::keyboard::KeyboardListener;
use super::listeners::EventListener;
use super::monitor::get_all_screens;
use super::overlay::{
    ContainerOverlayEntry, ContainerOverlayView, OverlayWindow, create_overlay_window,
};
use super::recovery;
use super::renderer::MetalBackend;
use crate::config::{Config, start_config_watcher};
use crate::core::{ContainerId, WindowId};
use crate::ipc;
use crate::logging::Logger;

pub fn run_app(config_path: Option<String>) -> anyhow::Result<()> {
    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config from {config_path}: {e}, using defaults");
        Config::default()
    });

    recovery::install_handlers();
    let logger = Logger::init(&config);
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

    let (event_tx, event_rx) = calloop::channel::channel();
    let (frame_tx, frame_rx) = mpsc::channel();

    let hub_config = config.clone();
    let keymaps = Arc::new(RwLock::new(config.keymaps.clone()));

    let _config_watcher = start_config_watcher(&config_path, {
        let keymaps = keymaps.clone();
        let tx = event_tx.clone();
        move |cfg| {
            logger.set_level(cfg.log_level);
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
            let event_loop = calloop::EventLoop::try_new().expect("Failed to create event loop");
            let signal = event_loop.get_signal();
            Dome::new(hub_config, screens, hub_tx, sender, signal).run(event_rx, event_loop);
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

struct AppDelegateIvars {
    hub_sender: calloop::channel::Sender<HubEvent>,
    frame_rx: Receiver<HubMessage>,
    overlay_windows: RefCell<HashMap<WindowId, OverlayWindow>>,
    container_overlays: RefCell<HashMap<ContainerId, ContainerOverlayEntry>>,
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
        frame_rx: Receiver<HubMessage>,
        event_listener: EventListener,
        backend: Rc<MetalBackend>,
        config: Config,
    ) -> Retained<Self> {
        let ivars = AppDelegateIvars {
            hub_sender: hub_sender.clone(),
            frame_rx,
            overlay_windows: RefCell::new(HashMap::new()),
            container_overlays: RefCell::new(HashMap::new()),
            event_listener,
            backend,
            config: RefCell::new(config),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
}

pub(super) fn send_hub_event(hub_sender: &calloop::channel::Sender<HubEvent>, event: HubEvent) {
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
            HubMessage::Frame(frame) => {
                let mut overlays = delegate.ivars().overlay_windows.borrow_mut();
                let mut containers = delegate.ivars().container_overlays.borrow_mut();

                for wid in frame.deletes {
                    overlays.remove(&wid);
                }
                for id in frame.deleted_containers {
                    if let Some(entry) = containers.remove(&id) {
                        entry.window.close();
                    }
                }

                let config = delegate.ivars().config.borrow().clone();
                let scale = objc2_app_kit::NSScreen::mainScreen(mtm)
                    .map(|s| s.backingScaleFactor())
                    .unwrap_or(2.0);

                for create in frame.creates {
                    let overlay = OverlayWindow::new(
                        mtm,
                        create.frame,
                        create.window_id,
                        delegate.ivars().hub_sender.clone(),
                        delegate.ivars().backend.clone(),
                        config.clone(),
                    );
                    overlays.insert(create.window_id, overlay);
                }

                for data in frame.container_creates {
                    let id = data.placement.id;
                    let window =
                        create_overlay_window(mtm, data.cocoa_frame, NSNormalWindowLevel - 1);
                    window.setIgnoresMouseEvents(false);
                    window.setAcceptsMouseMovedEvents(true);
                    let size = data.cocoa_frame.size;
                    let view = ContainerOverlayView::new(
                        mtm,
                        NSRect::new(NSPoint::new(0.0, 0.0), size),
                        delegate.ivars().backend.clone(),
                        scale,
                        size,
                        data.placement,
                        data.tab_titles,
                        config.clone(),
                        delegate.ivars().hub_sender.clone(),
                    );
                    window.setContentView(Some(&view));
                    window.orderFront(None);
                    containers.insert(id, ContainerOverlayEntry { window, view });
                }

                let shown: HashSet<WindowId> = frame.shows.iter().map(|s| s.window_id).collect();
                for show in frame.shows {
                    if let Some(overlay) = overlays.get_mut(&show.window_id) {
                        overlay.render(
                            &show.placement,
                            show.cocoa_frame,
                            show.scale,
                            show.visible_content,
                        );
                    }
                }
                for (wid, overlay) in overlays.iter() {
                    if !shown.contains(wid) {
                        overlay.hide();
                    }
                }

                for data in frame.containers {
                    if let Some(entry) = containers.get(&data.placement.id) {
                        entry.window.setFrame_display(data.cocoa_frame, true);
                        entry.view.update(
                            data.placement,
                            data.tab_titles,
                            scale,
                            data.cocoa_frame.size,
                        );
                        entry.window.orderFront(None);
                    }
                }
            }
            HubMessage::RegisterObservers(apps) => {
                for app in &apps {
                    delegate.ivars().event_listener.register_app(app);
                }
            }
            HubMessage::CaptureFrame { window_id, surface } => {
                if let Some(overlay) = delegate
                    .ivars()
                    .overlay_windows
                    .borrow_mut()
                    .get_mut(&window_id)
                {
                    overlay.apply_frame(&surface);
                }
            }
            HubMessage::CaptureFailed { window_id } => {
                tracing::debug!("Failed to screen capture for {window_id}");
            }
            HubMessage::ConfigChanged(new_config) => {
                *delegate.ivars().config.borrow_mut() = new_config.clone();
                for overlay in delegate.ivars().overlay_windows.borrow_mut().values_mut() {
                    overlay.set_config(new_config.clone());
                }
                for entry in delegate.ivars().container_overlays.borrow().values() {
                    entry.view.set_config(new_config.clone());
                }
            }
            HubMessage::Shutdown => {
                NSApplication::sharedApplication(mtm).terminate(None);
                return;
            }
        }
    }
}
