use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSFloatingWindowLevel, NSNormalWindowLevel, NSResponder,
    NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::CGFloat;
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect};
use objc2_io_surface::IOSurface;
use objc2_quartz_core::CAMetalLayer;

use super::dome::HubEvent;
use super::renderer::{MetalBackend, OverlayRenderer};
use crate::config::Config;
use crate::core::{ContainerId, ContainerPlacement, WindowPlacement};
use crate::overlay;

pub(super) struct OverlayWindow {
    window: Retained<NSWindow>,
    renderer: OverlayRenderer,
    placement: Option<WindowPlacement>,
    scale: f64,
    config: Config,
}

impl OverlayWindow {
    pub(super) fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        source_cg_id: CGWindowID,
        hub_sender: Sender<HubEvent>,
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
        let renderer = OverlayRenderer::new(backend, scale, frame.size.width, frame.size.height);
        let events = renderer.events();
        let view = MetalOverlayView::new(
            mtm,
            NSRect::new(NSPoint::new(0.0, 0.0), frame.size),
            renderer.layer(),
            events,
            Some(hub_sender),
        );
        view.ivars().cg_id.set(source_cg_id);
        window.setContentView(Some(&view));

        Self {
            window,
            renderer,
            placement: None,
            scale: 1.0,
            config,
        }
    }

    pub(super) fn render(&mut self, placement: &WindowPlacement, cocoa_frame: NSRect, scale: f64) {
        self.placement = Some(*placement);
        self.scale = scale;

        self.window.setFrame_display(cocoa_frame, true);
        self.renderer
            .resize(cocoa_frame.size.width, cocoa_frame.size.height, scale);

        let level = if placement.is_float {
            NSFloatingWindowLevel
        } else {
            NSNormalWindowLevel - 1
        };
        self.window.setLevel(level);

        if !placement.is_focused && placement.is_float {
            self.window.setIgnoresMouseEvents(false);
        } else {
            self.window.setIgnoresMouseEvents(true);
            self.renderer.clear_mirror();
        }

        let b = self.config.border_size;
        let ox = placement.frame.x - placement.visible_frame.x;
        let oy = placement.frame.y - placement.visible_frame.y;
        self.renderer.set_mirror_rect([
            ox + b,
            oy + b,
            placement.frame.width - 2.0 * b,
            placement.frame.height - 2.0 * b,
        ]);

        let config = &self.config;
        self.renderer.render(scale as f32, |ui| {
            overlay::paint_window_border(ui.painter(), placement, config);
        });
        self.window.setIsVisible(true);
    }

    pub(super) fn set_config(&mut self, config: Config) {
        self.config = config;
        if let Some(placement) = self.placement {
            let config = &self.config;
            self.renderer.render(self.scale as f32, |ui| {
                overlay::paint_window_border(ui.painter(), &placement, config);
            });
        }
    }

    pub(super) fn hide(&self) {
        self.window.setIsVisible(false);
    }

    pub(super) fn apply_frame(&mut self, surface: &IOSurface) {
        let w = surface.width();
        let h = surface.height();
        self.renderer
            .set_mirror_surface(surface, w as usize, h as usize);
        if let Some(placement) = self.placement {
            let config = &self.config;
            self.renderer.render(self.scale as f32, |ui| {
                overlay::paint_window_border(ui.painter(), &placement, config);
            });
        }
    }
}

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        self.window.close();
    }
}

pub(super) struct OverlayManager {
    backend: Rc<MetalBackend>,
    config: Config,
    containers: HashMap<ContainerId, ContainerOverlayEntry>,
}

impl OverlayManager {
    pub(super) fn new(backend: Rc<MetalBackend>, config: Config) -> Self {
        Self {
            backend,
            config,
            containers: HashMap::new(),
        }
    }

    pub(super) fn set_config(&mut self, config: Config) {
        self.config = config;
        for entry in self.containers.values_mut() {
            let config = &self.config;
            entry.renderer.render(entry.scale as f32, |ui| {
                overlay::show_container(ui, &entry.placement, &entry.tab_titles, config)
            });
        }
    }

    pub(super) fn process(
        &mut self,
        mtm: MainThreadMarker,
        overlays: Vec<ContainerOverlayData>,
        hub_sender: &Sender<HubEvent>,
    ) {
        let new_ids: std::collections::HashSet<_> =
            overlays.iter().map(|o| o.placement.id).collect();
        self.containers.retain(|k, entry| {
            let keep = new_ids.contains(k);
            if !keep {
                entry.window.close();
            }
            keep
        });

        for data in overlays {
            let id = data.placement.id;
            let scale = self.backing_scale(mtm);

            if let Some(entry) = self.containers.get_mut(&id) {
                entry.placement = data.placement;
                entry.tab_titles = data.tab_titles;
                entry.scale = scale;
                entry.window.setFrame_display(data.cocoa_frame, true);
                let size = data.cocoa_frame.size;
                entry.renderer.resize(size.width, size.height, scale);
                let config = &self.config;
                let clicked = entry.renderer.render(scale as f32, |ui| {
                    overlay::show_container(ui, &entry.placement, &entry.tab_titles, config)
                });
                if let Some(tab_idx) = clicked {
                    hub_sender.send(HubEvent::TabClicked(id, tab_idx)).ok();
                }
            } else {
                let window = create_overlay_window(mtm, data.cocoa_frame, NSNormalWindowLevel - 1);
                window.setIgnoresMouseEvents(false);
                window.setAcceptsMouseMovedEvents(true);

                let size = data.cocoa_frame.size;
                let mut renderer =
                    OverlayRenderer::new(self.backend.clone(), scale, size.width, size.height);
                let events = renderer.events();
                let view = MetalOverlayView::new(
                    mtm,
                    NSRect::new(NSPoint::new(0.0, 0.0), data.cocoa_frame.size),
                    renderer.layer(),
                    events,
                    None,
                );
                window.setContentView(Some(&view));
                window.orderFront(None);

                let config = &self.config;
                let clicked = renderer.render(scale as f32, |ui| {
                    overlay::show_container(ui, &data.placement, &data.tab_titles, config)
                });
                if let Some(tab_idx) = clicked {
                    hub_sender.send(HubEvent::TabClicked(id, tab_idx)).ok();
                }

                self.containers.insert(
                    id,
                    ContainerOverlayEntry {
                        window,
                        renderer,
                        placement: data.placement,
                        tab_titles: data.tab_titles,
                        scale,
                    },
                );
            }
        }
    }

    fn backing_scale(&self, mtm: MainThreadMarker) -> CGFloat {
        objc2_app_kit::NSScreen::mainScreen(mtm)
            .map(|s| s.backingScaleFactor())
            .unwrap_or(2.0)
    }
}

pub(super) struct ContainerOverlayData {
    pub(super) placement: ContainerPlacement,
    pub(super) tab_titles: Vec<String>,
    pub(super) cocoa_frame: NSRect,
}

struct ContainerOverlayEntry {
    window: Retained<NSWindow>,
    renderer: OverlayRenderer,
    placement: ContainerPlacement,
    tab_titles: Vec<String>,
    scale: f64,
}

fn create_overlay_window(mtm: MainThreadMarker, frame: NSRect, level: isize) -> Retained<NSWindow> {
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
    window.setLevel(level);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary,
    );
    unsafe { window.setReleasedWhenClosed(false) };
    window
}

pub(super) struct MetalOverlayViewIvars {
    layer: Retained<CAMetalLayer>,
    events: Rc<RefCell<Vec<egui::Event>>>,
    hub_sender: Option<Sender<HubEvent>>,
    pub(super) cg_id: std::cell::Cell<u32>,
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
        fn mouse_down(&self, event: &NSEvent) {
            if let Some(sender) = &self.ivars().hub_sender {
                sender
                    .send(HubEvent::MirrorClicked(self.ivars().cg_id.get()))
                    .ok();
            } else {
                let pos = self.event_pos(event);
                self.ivars().events.borrow_mut().push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            if self.ivars().hub_sender.is_none() {
                let pos = self.event_pos(event);
                self.ivars().events.borrow_mut().push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            if self.ivars().hub_sender.is_none() {
                let pos = self.event_pos(event);
                self.ivars()
                    .events
                    .borrow_mut()
                    .push(egui::Event::PointerMoved(pos));
            }
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            if self.ivars().hub_sender.is_none() {
                let pos = self.event_pos(event);
                self.ivars()
                    .events
                    .borrow_mut()
                    .push(egui::Event::PointerMoved(pos));
            }
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
        events: Rc<RefCell<Vec<egui::Event>>>,
        hub_sender: Option<Sender<HubEvent>>,
    ) -> Retained<Self> {
        let ivars = MetalOverlayViewIvars {
            layer,
            events,
            hub_sender,
            cg_id: std::cell::Cell::new(0),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn event_pos(&self, event: &NSEvent) -> egui::Pos2 {
        let loc = event.locationInWindow();
        let view_loc = self.convertPoint_fromView(loc, None);
        egui::pos2(view_loc.x as f32, view_loc.y as f32)
    }
}
