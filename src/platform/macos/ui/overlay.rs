use std::cell::{Cell, RefCell};
use std::rc::Rc;

use calloop::channel::Sender as CalloopSender;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSFloatingWindowLevel, NSNormalWindowLevel, NSResponder,
    NSView, NSWindow, NSWindowCollectionBehavior, NSWindowLevel, NSWindowStyleMask,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use objc2_io_surface::IOSurface;
use objc2_quartz_core::CAMetalLayer;

use super::super::dome::HubEvent;
use super::renderer::{ContainerRenderer, MetalBackend, WindowRenderer};
use crate::config::Config;
use crate::core::{ContainerId, ContainerPlacement, Dimension, WindowId, WindowPlacement};
use crate::overlay;

const TILING_WINDOW_OVERLAY_LEVEL: NSWindowLevel = NSNormalWindowLevel - 2;
const CONTAINER_OVERLAY_LEVEL: NSWindowLevel = NSNormalWindowLevel - 1;
const FLOAT_WINDOW_OVERLAY_LEVEL: NSWindowLevel = NSFloatingWindowLevel;

pub(super) struct WindowOverlay {
    window: Retained<NSWindow>,
    renderer: WindowRenderer,
    placement: Option<WindowPlacement>,
    visible_content_bounds: Option<[f32; 4]>,
    scale: f64,
    config: Config,
}

impl WindowOverlay {
    pub(super) fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        window_id: WindowId,
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
        let renderer = WindowRenderer::new(backend, scale, frame.size.width, frame.size.height);
        let view = MetalOverlayView::new(
            mtm,
            NSRect::new(NSPoint::new(0.0, 0.0), frame.size),
            renderer.layer(),
            hub_sender,
            window_id,
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

        let level = if placement.is_float {
            FLOAT_WINDOW_OVERLAY_LEVEL
        } else {
            TILING_WINDOW_OVERLAY_LEVEL
        };
        self.window.setLevel(level);

        if !placement.is_focused && placement.is_float {
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
        self.renderer.render(scale as f32, mr, |ui| {
            overlay::paint_window_border(ui.painter(), placement, config);
        });
        self.window.setIsVisible(true);
    }

    pub(super) fn set_config(&mut self, config: Config) {
        self.config = config;
        if let Some(placement) = self.placement {
            let config = &self.config;
            let mr = self.visible_content_bounds;
            self.renderer.render(self.scale as f32, mr, |ui| {
                overlay::paint_window_border(ui.painter(), &placement, config);
            });
        }
    }

    pub(super) fn hide(&self) {
        self.window.setIsVisible(false);
    }

    pub(super) fn apply_frame(&mut self, surface: &IOSurface) {
        self.renderer.set_mirror_surface(surface);
        if let Some(placement) = self.placement {
            let config = &self.config;
            let mr = self.visible_content_bounds;
            self.renderer.render(self.scale as f32, mr, |ui| {
                overlay::paint_window_border(ui.painter(), &placement, config);
            });
        }
    }
}

impl Drop for WindowOverlay {
    fn drop(&mut self) {
        self.window.close();
    }
}

pub(super) struct ContainerOverlay {
    pub(super) window: Retained<NSWindow>,
    pub(super) view: Retained<ContainerOverlayView>,
}

impl ContainerOverlay {
    pub(super) fn new(
        mtm: MainThreadMarker,
        id: ContainerId,
        backend: Rc<MetalBackend>,
        config: Config,
        hub_sender: CalloopSender<HubEvent>,
    ) -> Self {
        let frame = NSRect::ZERO;
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
        window.setLevel(CONTAINER_OVERLAY_LEVEL);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::Auxiliary
                | NSWindowCollectionBehavior::Default
                | NSWindowCollectionBehavior::Transient
                | NSWindowCollectionBehavior::FullScreenNone
                | NSWindowCollectionBehavior::FullScreenDisallowsTiling
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        unsafe { window.setReleasedWhenClosed(false) };
        window.setIgnoresMouseEvents(false);
        window.setAcceptsMouseMovedEvents(true);
        let view = ContainerOverlayView::new(mtm, id, backend, config, hub_sender);
        window.setContentView(Some(&view));
        Self { window, view }
    }

    pub(super) fn show(
        &self,
        placement: ContainerPlacement,
        tab_titles: Vec<String>,
        cocoa_frame: NSRect,
    ) {
        self.window.setFrame_display(cocoa_frame, true);
        self.view.update(placement, tab_titles, cocoa_frame.size);
        self.window.orderFront(None);
    }

    pub(super) fn hide(&self) {
        self.view.clear();
        self.window.orderOut(None);
    }
}

pub(super) struct MetalOverlayViewIvars {
    layer: Retained<CAMetalLayer>,
    hub_sender: CalloopSender<HubEvent>,
    pub(super) window_id: Cell<WindowId>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = MetalOverlayViewIvars]
    pub(super) struct MetalOverlayView;

    unsafe impl NSObjectProtocol for MetalOverlayView {}

    impl MetalOverlayView {
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
                .send(HubEvent::MirrorClicked(self.ivars().window_id.get()))
                .ok();
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl MetalOverlayView {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        layer: Retained<CAMetalLayer>,
        hub_sender: CalloopSender<HubEvent>,
        window_id: WindowId,
    ) -> Retained<Self> {
        let ivars = MetalOverlayViewIvars {
            layer,
            hub_sender,
            window_id: Cell::new(window_id),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub(super) struct ContainerOverlayViewIvars {
    #[expect(dead_code, reason = "retains CAMetalLayer to prevent deallocation")]
    layer: Retained<CAMetalLayer>,
    events: RefCell<Vec<egui::Event>>,
    renderer: RefCell<ContainerRenderer>,
    placement: Cell<Option<ContainerPlacement>>,
    tab_titles: RefCell<Vec<String>>,
    config: RefCell<Config>,
    scale: Cell<f64>,
    container_id: Cell<ContainerId>,
    hub_sender: CalloopSender<HubEvent>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ContainerOverlayViewIvars]
    pub(super) struct ContainerOverlayView;

    unsafe impl NSObjectProtocol for ContainerOverlayView {}

    impl ContainerOverlayView {
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

impl ContainerOverlayView {
    fn new(
        mtm: MainThreadMarker,
        id: ContainerId,
        backend: Rc<MetalBackend>,
        config: Config,
        hub_sender: CalloopSender<HubEvent>,
    ) -> Retained<Self> {
        // TODO: Change scale when moved between monirors
        let scale = objc2_app_kit::NSScreen::mainScreen(mtm)
            .map(|s| s.backingScaleFactor())
            .unwrap_or(2.0);
        let frame = NSRect::ZERO;
        let renderer = ContainerRenderer::new(backend, scale, 0.0, 0.0);
        let layer = renderer.layer();
        let ivars = ContainerOverlayViewIvars {
            layer: layer.clone(),
            events: RefCell::new(Vec::new()),
            renderer: RefCell::new(renderer),
            placement: Cell::new(None),
            tab_titles: RefCell::new(Vec::new()),
            config: RefCell::new(config),
            scale: Cell::new(scale),
            container_id: Cell::new(id),
            hub_sender,
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        view
    }

    pub(super) fn update(
        &self,
        placement: ContainerPlacement,
        tab_titles: Vec<String>,
        size: NSSize,
    ) {
        let ivars = self.ivars();
        ivars.placement.set(Some(placement));
        ivars.container_id.set(placement.id);
        *ivars.tab_titles.borrow_mut() = tab_titles;
        let scale = ivars.scale.get();
        ivars
            .renderer
            .borrow()
            .resize(size.width, size.height, scale);
        self.render_now();
    }

    pub(super) fn clear(&self) {
        let ivars = self.ivars();
        ivars.placement.set(None);
        ivars.tab_titles.borrow_mut().clear();
    }

    pub(super) fn set_config(&self, config: Config) {
        *self.ivars().config.borrow_mut() = config;
        self.render_now();
    }

    fn render_now(&self) {
        let ivars = self.ivars();
        let Some(placement) = ivars.placement.get() else {
            return;
        };
        let scale = ivars.scale.get();
        let tab_titles = ivars.tab_titles.borrow();
        let config = ivars.config.borrow();
        let events = std::mem::take(&mut *ivars.events.borrow_mut());
        let clicked = ivars
            .renderer
            .borrow_mut()
            .render(scale as f32, events, |ui| {
                overlay::show_container(ui, &placement, &tab_titles, &config)
            });
        if let Some(tab_idx) = clicked {
            ivars
                .hub_sender
                .send(HubEvent::TabClicked(ivars.container_id.get(), tab_idx))
                .ok();
        }
    }

    fn event_pos(&self, event: &NSEvent) -> egui::Pos2 {
        let loc = event.locationInWindow();
        let view_loc = self.convertPoint_fromView(loc, None);
        egui::pos2(view_loc.x as f32, view_loc.y as f32)
    }
}
