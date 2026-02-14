use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;

use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSColor, NSEvent, NSFloatingWindowLevel, NSNormalWindowLevel, NSResponder, NSScreen, NSView,
    NSWindow, NSWindowCollectionBehavior, NSWindowOrderingMode, NSWindowStyleMask,
};
use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{
    CFDictionary, CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext, kCFBooleanTrue,
    kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGMainDisplayID, CGPreflightScreenCaptureAccess,
    CGRequestScreenCaptureAccess, CGWindowID,
};
use objc2_foundation::{
    NSNotification, NSNumber, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use objc2_io_surface::IOSurface;
use objc2_quartz_core::CALayer;
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

use super::dome::{Dome, HubEvent, HubMessage, MessageSender};
use super::keyboard::KeyboardListener;
use super::listeners::EventListener;
use super::overlay::OverlayManager;
use super::recovery;
use crate::config::{Color, Config, start_config_watcher};
use crate::core::Dimension;
use crate::ipc;
use crate::platform::macos::overlay::draw_rect;

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
    let delegate = AppDelegate::new(mtm, event_tx, frame_rx, event_listener);
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
    ) -> Retained<Self> {
        let ivars = AppDelegateIvars {
            hub_sender: hub_sender.clone(),
            frame_rx,
            overlay_manager: RefCell::new(OverlayManager::new()),
            overlay_windows: RefCell::new(HashMap::new()),
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
    let mtm = delegate.mtm();
    while let Ok(msg) = delegate.ivars().frame_rx.try_recv() {
        match msg {
            HubMessage::Overlays(overlays) => {
                delegate
                    .ivars()
                    .overlay_manager
                    .borrow_mut()
                    .process(mtm, overlays);
            }
            HubMessage::RegisterObservers(apps) => {
                for app in &apps {
                    delegate.ivars().event_listener.register_app(app);
                }
            }
            HubMessage::CaptureFrame { cg_id, surface } => {
                if let Some(overlay) = delegate.ivars().overlay_windows.borrow().get(&cg_id) {
                    overlay.apply_frame(&surface);
                }
            }
            HubMessage::CaptureFailed { cg_id } => {
                // TODO: proper error handling
                tracing::debug!("Failed to screen capture for {cg_id}");
            }
            HubMessage::WindowCreate { cg_id, frame } => {
                let overlay =
                    OverlayWindow::new(mtm, frame, cg_id, delegate.ivars().hub_sender.clone());
                delegate
                    .ivars()
                    .overlay_windows
                    .borrow_mut()
                    .insert(cg_id, overlay);
            }
            HubMessage::WindowShow {
                cg_id,
                frame,
                is_float,
                is_focus,
                edges,
                scale,
                border,
            } => {
                let ui_window = UIWindow {
                    frame,
                    is_float,
                    is_focus,
                    edges,
                    scale,
                    border,
                };
                if let Some(overlay) = delegate.ivars().overlay_windows.borrow().get(&cg_id) {
                    overlay.render(ui_window);
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
            HubMessage::Shutdown => {
                NSApplication::sharedApplication(mtm).terminate(None);
                return;
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct ScreenInfo {
    pub(super) display_id: CGDirectDisplayID,
    pub(super) name: String,
    pub(super) dimension: Dimension,
    pub(super) full_height: f32,
    pub(super) is_primary: bool,
    pub(super) scale: f64,
}

impl std::fmt::Display for ScreenInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (id={}, dim={:?}, scale={})",
            self.name, self.display_id, self.dimension, self.scale
        )
    }
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

            let top_inset =
                (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
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
                scale: screen.backingScaleFactor(),
            }
        })
        .collect()
}

struct OverlayWindow {
    window: Retained<NSWindow>,
    border_view: Retained<BorderView>,
    mirror_view: Retained<MirrorView>,
}

impl OverlayWindow {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        source_cg_id: CGWindowID,
        hub_sender: Sender<HubEvent>,
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
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setOpaque(false);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::Auxiliary
                | NSWindowCollectionBehavior::Default
                | NSWindowCollectionBehavior::Transient
                | NSWindowCollectionBehavior::FullScreenNone
                | NSWindowCollectionBehavior::FullScreenDisallowsTiling
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        unsafe { window.setReleasedWhenClosed(false) };
        let content_view = window.contentView().unwrap();

        let border_view = BorderView::new(mtm, frame);
        content_view.addSubview(&border_view);
        let mirror_view = MirrorView::new(mtm, frame, source_cg_id, hub_sender);
        content_view.addSubview_positioned_relativeTo(
            &mirror_view,
            NSWindowOrderingMode::Above,
            Some(&border_view),
        );
        Self {
            window,
            border_view,
            mirror_view,
        }
    }

    fn render(&self, content_window: UIWindow) {
        let frame = content_window.frame();
        self.window.setFrame_display(frame, true);
        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), frame.size);
        self.border_view.setFrame(bounds);
        self.border_view.set_edges(content_window.edges());
        let b = content_window.border;
        let inner = NSRect::new(
            NSPoint::new(b, b),
            NSSize::new(frame.size.width - 2.0 * b, frame.size.height - 2.0 * b),
        );
        self.mirror_view.setFrame(inner);
        self.mirror_view.set_scale(content_window.scale());
        let level = if content_window.is_float() {
            NSFloatingWindowLevel
        } else {
            NSNormalWindowLevel - 1
        };
        self.window.setLevel(level);
        if !content_window.is_focus() && content_window.is_float() {
            self.mirror_view.setHidden(false);
            self.window.setIgnoresMouseEvents(false);
        } else {
            self.mirror_view.setHidden(true);
            self.window.setIgnoresMouseEvents(true);
        }
        self.window.setIsVisible(true);
    }

    fn hide(&self) {
        self.window.setIsVisible(false);
    }

    fn apply_frame(&self, surface: &IOSurface) {
        self.mirror_view.apply_frame(surface);
    }
}

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        self.window.close();
    }
}

struct UIWindow {
    frame: NSRect,
    is_float: bool,
    is_focus: bool,
    edges: Vec<(NSRect, Color)>,
    scale: f64,
    border: f64,
}

impl UIWindow {
    fn frame(&self) -> NSRect {
        self.frame
    }
    fn is_float(&self) -> bool {
        self.is_float
    }
    fn is_focus(&self) -> bool {
        self.is_focus
    }
    fn edges(&self) -> Vec<(NSRect, Color)> {
        self.edges.clone()
    }
    fn scale(&self) -> f64 {
        self.scale
    }
}

struct BorderViewIvars {
    edges: RefCell<Vec<(NSRect, Color)>>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = BorderViewIvars]
    struct BorderView;

    unsafe impl NSObjectProtocol for BorderView {}

    impl BorderView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            for (rect, color) in self.ivars().edges.borrow().iter() {
                draw_rect(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height, *color);
            }
        }
    }
);

impl BorderView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let ivars = BorderViewIvars {
            edges: RefCell::new(Vec::new()),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn set_edges(&self, edges: Vec<(NSRect, Color)>) {
        *self.ivars().edges.borrow_mut() = edges;
        self.setNeedsDisplay(true);
    }
}

struct MirrorViewIvars {
    cg_id: CGWindowID,
    layer: Retained<CALayer>,
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
        let layer = CALayer::new();
        let this = Self::alloc(mtm).set_ivars(MirrorViewIvars {
            cg_id,
            hub_tx,
            layer: layer.clone(),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        view
    }

    fn apply_frame(&self, surface: &IOSurface) {
        unsafe { self.ivars().layer.setContents(Some(surface)) };
    }

    fn set_scale(&self, scale: f64) {
        self.ivars().layer.setContentsScale(scale);
    }
}
