use std::cell::{Cell, RefCell};
use std::rc::Rc;

use calloop::channel::Sender as CalloopSender;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSFloatingWindowLevel, NSNormalWindowLevel, NSResponder,
    NSView, NSWindow, NSWindowCollectionBehavior, NSWindowLevel, NSWindowStyleMask,
};
use objc2_application_services::AXUIElement;
use objc2_core_foundation::kCFBooleanTrue;
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use objc2_io_surface::IOSurface;
use objc2_quartz_core::CAMetalLayer;

use super::super::dome::HubEvent;
use super::renderer::{MetalBackend, OverlayRenderer};
use crate::config::Config;
use crate::core::{ContainerPlacement, Dimension, WindowPlacement};
use crate::overlay;

define_class!(
    #[unsafe(super(NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    struct KeyableWindow;

    unsafe impl NSObjectProtocol for KeyableWindow {}

    impl KeyableWindow {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }
    }
);

impl KeyableWindow {
    fn new(mtm: MainThreadMarker, frame: NSRect, style: NSWindowStyleMask) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe {
            msg_send![
                super(this),
                initWithContentRect: frame,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false,
            ]
        }
    }
}

const FLOAT_OVERLAY_LEVEL: NSWindowLevel = NSFloatingWindowLevel;

pub(super) struct FloatOverlay {
    window: Retained<NSWindow>,
    renderer: OverlayRenderer,
    placement: Option<WindowPlacement>,
    visible_content_bounds: Option<[f32; 4]>,
    scale: f64,
    config: Config,
}

impl FloatOverlay {
    pub(super) fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        cg_id: CGWindowID,
        hub_sender: CalloopSender<HubEvent>,
        backend: Rc<MetalBackend>,
        config: Config,
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
        window.setLevel(FLOAT_OVERLAY_LEVEL);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::Auxiliary
                | NSWindowCollectionBehavior::Default
                | NSWindowCollectionBehavior::Transient
                | NSWindowCollectionBehavior::FullScreenNone
                | NSWindowCollectionBehavior::FullScreenDisallowsTiling
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        unsafe { window.setReleasedWhenClosed(false) };

        let scale = window.backingScaleFactor();
        let renderer = OverlayRenderer::new(backend, scale, frame.size.width, frame.size.height);
        let view = FloatOverlayView::new(
            mtm,
            NSRect::new(NSPoint::new(0.0, 0.0), frame.size),
            renderer.layer(),
            hub_sender,
            cg_id,
        );
        window.setContentView(Some(&view));

        Self {
            window,
            renderer,
            placement: None,
            visible_content_bounds: None,
            scale: 1.0,
            config,
        }
    }

    pub(super) fn render(
        &mut self,
        placement: &WindowPlacement,
        cocoa_frame: NSRect,
        scale: f64,
        visible_content_dim: Option<Dimension>,
    ) {
        self.placement = Some(*placement);
        self.scale = scale;

        self.window.setFrame_display(cocoa_frame, true);
        self.renderer
            .resize(cocoa_frame.size.width, cocoa_frame.size.height, scale);

        if !placement.is_focused {
            self.window.setIgnoresMouseEvents(false);
        } else {
            self.window.setIgnoresMouseEvents(true);
            self.renderer.clear_mirror();
        }

        self.visible_content_bounds = visible_content_dim.map(|mr| {
            let v = placement.visible_frame;
            [mr.x - v.x, mr.y - v.y, mr.width, mr.height]
        });

        let config = &self.config;
        let mr = self.visible_content_bounds;
        self.renderer.render(scale as f32, Vec::new(), mr, |ctx| {
            egui::Area::new(egui::Id::new("overlay"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    overlay::paint_window_border(ui.painter(), placement, config, egui::Vec2::ZERO);
                });
        });
        self.window.setIsVisible(true);
    }

    pub(super) fn set_config(&mut self, config: Config) {
        self.config = config;
        if let Some(placement) = self.placement {
            let config = &self.config;
            let mr = self.visible_content_bounds;
            self.renderer
                .render(self.scale as f32, Vec::new(), mr, |ctx| {
                    egui::Area::new(egui::Id::new("overlay"))
                        .fixed_pos(egui::pos2(0.0, 0.0))
                        .show(ctx, |ui| {
                            overlay::paint_window_border(
                                ui.painter(),
                                &placement,
                                config,
                                egui::Vec2::ZERO,
                            );
                        });
                });
        }
    }

    pub(super) fn apply_frame(&mut self, surface: &IOSurface) {
        self.renderer.set_mirror_surface(surface);
        if let Some(placement) = self.placement {
            let config = &self.config;
            let mr = self.visible_content_bounds;
            self.renderer
                .render(self.scale as f32, Vec::new(), mr, |ctx| {
                    egui::Area::new(egui::Id::new("overlay"))
                        .fixed_pos(egui::pos2(0.0, 0.0))
                        .show(ctx, |ui| {
                            overlay::paint_window_border(
                                ui.painter(),
                                &placement,
                                config,
                                egui::Vec2::ZERO,
                            );
                        });
                });
        }
    }
}

impl Drop for FloatOverlay {
    fn drop(&mut self) {
        self.window.close();
    }
}

pub(super) struct TilingOverlay {
    window: Retained<KeyableWindow>,
    view: Retained<TilingOverlayView>,
}

impl TilingOverlay {
    pub(super) fn new(
        mtm: MainThreadMarker,
        backend: Rc<MetalBackend>,
        config: Config,
        cocoa_frame: NSRect,
        scale: f64,
        hub_sender: CalloopSender<HubEvent>,
    ) -> Self {
        let window = KeyableWindow::new(mtm, cocoa_frame, NSWindowStyleMask::Borderless);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setOpaque(false);
        window.setLevel(NSNormalWindowLevel - 1);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::Default
                | NSWindowCollectionBehavior::FullScreenNone
                | NSWindowCollectionBehavior::FullScreenDisallowsTiling
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        unsafe { window.setReleasedWhenClosed(false) };
        window.setIgnoresMouseEvents(false);
        window.setAcceptsMouseMovedEvents(true);

        let view = TilingOverlayView::new(mtm, backend, config, scale, hub_sender);
        window.setContentView(Some(&view));
        window.setFrame_display(cocoa_frame, false);

        Self { window, view }
    }

    pub(super) fn render(
        &self,
        cocoa_frame: NSRect,
        scale: f64,
        monitor: Dimension,
        windows: &[WindowPlacement],
        containers: &[(ContainerPlacement, Vec<String>)],
    ) {
        self.window.setFrame_display(cocoa_frame, false);
        self.view.update(monitor, windows, containers, scale);
        self.window.orderFront(None);
    }

    pub(super) fn clear(&self) {
        self.view.clear();
        self.view.render_now();
        self.window.orderFront(None);
    }

    // macOS 14+ "cooperative activation" silently ignores NSApplication.activate() for
    // self-activation. The AX API bypasses this via the privileged accessibility subsystem.
    pub(super) fn focus(&self, _mtm: MainThreadMarker) {
        let pid = std::process::id() as i32;
        let ax_app = unsafe { AXUIElement::new_application(pid) };
        crate::platform::macos::objc2_wrapper::set_attribute_value(
            &ax_app,
            &crate::platform::macos::objc2_wrapper::kAXFrontmostAttribute(),
            unsafe { kCFBooleanTrue.unwrap() },
        )
        .ok();
        self.window.makeKeyAndOrderFront(None);
    }

    pub(super) fn set_config(&self, config: Config) {
        self.view.set_config(config);
    }
}

impl Drop for TilingOverlay {
    fn drop(&mut self) {
        self.window.close();
    }
}

pub(super) struct FloatOverlayViewIvars {
    layer: Retained<CAMetalLayer>,
    hub_sender: CalloopSender<HubEvent>,
    cg_id: Cell<CGWindowID>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = FloatOverlayViewIvars]
    pub(super) struct FloatOverlayView;

    unsafe impl NSObjectProtocol for FloatOverlayView {}

    impl FloatOverlayView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(wantsLayer))]
        fn wants_layer(&self) -> bool {
            true
        }

        #[unsafe(method(makeBackingLayer))]
        fn make_backing_layer(&self) -> *mut objc2_quartz_core::CALayer {
            let layer: Retained<objc2_quartz_core::CALayer> =
                unsafe { Retained::cast_unchecked(self.ivars().layer.clone()) };
            Retained::into_raw(layer)
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            self.ivars()
                .hub_sender
                .send(HubEvent::MirrorClicked(self.ivars().cg_id.get()))
                .ok();
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl FloatOverlayView {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        layer: Retained<CAMetalLayer>,
        hub_sender: CalloopSender<HubEvent>,
        cg_id: CGWindowID,
    ) -> Retained<Self> {
        let ivars = FloatOverlayViewIvars {
            layer,
            hub_sender,
            cg_id: Cell::new(cg_id),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub(super) struct TilingOverlayViewIvars {
    #[expect(dead_code, reason = "retains CAMetalLayer to prevent deallocation")]
    layer: Retained<CAMetalLayer>,
    events: RefCell<Vec<egui::Event>>,
    renderer: RefCell<OverlayRenderer>,
    monitor: Cell<Dimension>,
    windows: RefCell<Vec<WindowPlacement>>,
    containers: RefCell<Vec<(ContainerPlacement, Vec<String>)>>,
    config: RefCell<Config>,
    scale: Cell<f64>,
    hub_sender: CalloopSender<HubEvent>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TilingOverlayViewIvars]
    pub(super) struct TilingOverlayView;

    unsafe impl NSObjectProtocol for TilingOverlayView {}

    impl TilingOverlayView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let pos = self.event_pos(event);
            self.ivars().events.borrow_mut().push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            self.render_now();
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let pos = self.event_pos(event);
            self.ivars().events.borrow_mut().push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            self.render_now();
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            let pos = self.event_pos(event);
            self.ivars()
                .events
                .borrow_mut()
                .push(egui::Event::PointerMoved(pos));
            self.render_now();
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let pos = self.event_pos(event);
            self.ivars()
                .events
                .borrow_mut()
                .push(egui::Event::PointerMoved(pos));
            self.render_now();
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl TilingOverlayView {
    fn new(
        mtm: MainThreadMarker,
        backend: Rc<MetalBackend>,
        config: Config,
        scale: f64,
        hub_sender: CalloopSender<HubEvent>,
    ) -> Retained<Self> {
        let renderer = OverlayRenderer::new(backend, scale, 0.0, 0.0);
        let layer = renderer.layer();
        let ivars = TilingOverlayViewIvars {
            layer: layer.clone(),
            events: RefCell::new(Vec::new()),
            renderer: RefCell::new(renderer),
            monitor: Cell::new(Dimension::default()),
            windows: RefCell::new(Vec::new()),
            containers: RefCell::new(Vec::new()),
            config: RefCell::new(config),
            scale: Cell::new(scale),
            hub_sender,
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        view
    }

    fn update(
        &self,
        monitor: Dimension,
        windows: &[WindowPlacement],
        containers: &[(ContainerPlacement, Vec<String>)],
        scale: f64,
    ) {
        let ivars = self.ivars();
        ivars.monitor.set(monitor);
        ivars.scale.set(scale);
        *ivars.windows.borrow_mut() = windows.to_vec();
        *ivars.containers.borrow_mut() = containers.to_vec();
        ivars
            .renderer
            .borrow()
            .resize(monitor.width as f64, monitor.height as f64, scale);
        self.render_now();
    }

    fn clear(&self) {
        let ivars = self.ivars();
        ivars.windows.borrow_mut().clear();
        ivars.containers.borrow_mut().clear();
    }

    fn set_config(&self, config: Config) {
        *self.ivars().config.borrow_mut() = config;
        self.render_now();
    }

    fn render_now(&self) {
        let ivars = self.ivars();
        let windows = ivars.windows.borrow();
        let containers = ivars.containers.borrow();
        let monitor = ivars.monitor.get();
        let config = ivars.config.borrow();
        let events = std::mem::take(&mut *ivars.events.borrow_mut());
        let scale = ivars.scale.get();
        let clicked_tabs = ivars
            .renderer
            .borrow_mut()
            .render(scale as f32, events, None, |ctx| {
                overlay::paint_tiling_overlay(ctx, monitor, &windows, &containers, &config)
            });
        for (container_id, tab_idx) in clicked_tabs {
            ivars
                .hub_sender
                .send(HubEvent::TabClicked(container_id, tab_idx))
                .ok();
        }
    }

    fn event_pos(&self, event: &NSEvent) -> egui::Pos2 {
        let loc = event.locationInWindow();
        let view_loc = self.convertPoint_fromView(loc, None);
        egui::pos2(view_loc.x as f32, view_loc.y as f32)
    }
}
