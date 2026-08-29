use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSFloatingWindowLevel, NSNormalWindowLevel, NSResponder,
    NSView, NSWindow, NSWindowCollectionBehavior, NSWindowLevel, NSWindowStyleMask,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use objc2_quartz_core::CALayer;

use crate::{
    AuxiliaryWindowHandler, MouseButton, PhysicalPosition, PhysicalSize, WindowAttributes,
};

impl crate::WindowLevel {
    fn to_ns(self) -> NSWindowLevel {
        match self {
            Self::Floating => NSFloatingWindowLevel,
            Self::Bottom => NSNormalWindowLevel - 1,
        }
    }
}

pub trait AuxiliaryWindowExtMacOs {
    fn set_content_layer(&self, layer: &CALayer);
    /// Toggles click-through at runtime. `WindowAttributes::click_through` sets the
    /// initial value.
    fn set_click_through(&self, click_through: bool);
    /// Makes the window key. This is only the `makeKeyAndOrderFront` half. The consumer
    /// forces app-frontmost separately, because that path needs accessibility the crate
    /// must not pull in.
    fn focus(&self);
}

impl AuxiliaryWindowExtMacOs for crate::AuxiliaryWindow {
    fn set_content_layer(&self, layer: &CALayer) {
        self.inner.set_content_layer(layer);
    }

    fn set_click_through(&self, click_through: bool) {
        self.inner.set_click_through(click_through);
    }

    fn focus(&self) {
        self.inner.focus();
    }
}

/// Present on every space, never full-screen, and out of the window cycle.
fn auxiliary_collection_behavior() -> NSWindowCollectionBehavior {
    NSWindowCollectionBehavior::Default
        | NSWindowCollectionBehavior::FullScreenNone
        | NSWindowCollectionBehavior::FullScreenDisallowsTiling
        | NSWindowCollectionBehavior::IgnoresCycle
        | NSWindowCollectionBehavior::Auxiliary
        | NSWindowCollectionBehavior::Transient
}

struct AuxiliaryWindowIvars {
    can_become_key: bool,
}

define_class!(
    // A borderless NSWindow returns false from canBecomeKeyWindow and has no runtime
    // setter, so honoring can_become_key needs this subclass rather than a plain window.
    #[unsafe(super(NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AuxiliaryWindowIvars]
    struct AuxiliaryNSWindow;

    unsafe impl NSObjectProtocol for AuxiliaryNSWindow {}

    impl AuxiliaryNSWindow {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            self.ivars().can_become_key
        }
    }
);

impl AuxiliaryNSWindow {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        style: NSWindowStyleMask,
        can_become_key: bool,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AuxiliaryWindowIvars { can_become_key });
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

struct AuxiliaryViewIvars {
    flipped: bool,
    accepts_first_mouse: bool,
    handler: RefCell<Box<dyn AuxiliaryWindowHandler>>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AuxiliaryViewIvars]
    struct AuxiliaryView;

    unsafe impl NSObjectProtocol for AuxiliaryView {}

    impl AuxiliaryView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            self.ivars().flipped
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            self.ivars().accepts_first_mouse
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let at = event_pos(self, event);
            self.ivars()
                .handler
                .borrow_mut()
                .on_mouse_down(at, MouseButton::Primary);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let at = event_pos(self, event);
            self.ivars()
                .handler
                .borrow_mut()
                .on_mouse_up(at, MouseButton::Primary);
        }
    }
);

impl AuxiliaryView {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        handler: Box<dyn AuxiliaryWindowHandler>,
    ) -> Retained<Self> {
        let ivars = AuxiliaryViewIvars {
            flipped: true,
            accepts_first_mouse: true,
            handler: RefCell::new(handler),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub(crate) struct Window {
    window: Retained<AuxiliaryNSWindow>,
    view: Retained<AuxiliaryView>,
}

impl Window {
    pub(crate) fn new(
        attributes: &WindowAttributes,
        handler: Box<dyn AuxiliaryWindowHandler>,
    ) -> anyhow::Result<Self> {
        let mtm =
            MainThreadMarker::new().expect("AuxiliaryWindow::new must run on the main thread");
        let frame = to_nsrect(attributes.position, attributes.size);
        let window = AuxiliaryNSWindow::new(
            mtm,
            frame,
            NSWindowStyleMask::Borderless,
            attributes.focusable,
        );
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setOpaque(false);
        window.setCollectionBehavior(auxiliary_collection_behavior());
        unsafe { window.setReleasedWhenClosed(false) };
        window.setIgnoresMouseEvents(attributes.click_through);

        let view = AuxiliaryView::new(
            mtm,
            NSRect::new(NSPoint::new(0.0, 0.0), frame.size),
            handler,
        );
        window.setContentView(Some(&view));

        Ok(Self { window, view })
    }

    pub(crate) fn set_frame(&self, position: PhysicalPosition, size: PhysicalSize) {
        self.window
            .setFrame_display(to_nsrect(position, size), true);
    }

    pub(crate) fn set_visible(&self, visible: bool) {
        self.window.setIsVisible(visible);
    }

    pub(crate) fn set_content_layer(&self, layer: &CALayer) {
        self.view.setLayer(Some(layer));
        self.view.setWantsLayer(true);
    }

    pub(crate) fn set_click_through(&self, click_through: bool) {
        self.window.setIgnoresMouseEvents(click_through);
    }

    pub(crate) fn set_level(&self, level: crate::WindowLevel) {
        self.window.setLevel(level.to_ns());
    }

    pub(crate) fn focus(&self) {
        self.window.makeKeyAndOrderFront(None);
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.window.close();
    }
}

fn to_nsrect(position: PhysicalPosition, size: PhysicalSize) -> NSRect {
    NSRect::new(
        NSPoint::new(position.x as f64, position.y as f64),
        NSSize::new(size.width as f64, size.height as f64),
    )
}

fn event_pos(view: &NSView, event: &NSEvent) -> PhysicalPosition {
    let loc = event.locationInWindow();
    let view_loc = view.convertPoint_fromView(loc, None);
    PhysicalPosition {
        x: view_loc.x as i32,
        y: view_loc.y as i32,
    }
}
