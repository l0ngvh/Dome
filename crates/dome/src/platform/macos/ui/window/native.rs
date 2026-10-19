use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSFloatingWindowLevel, NSNormalWindowLevel, NSResponder,
    NSView, NSWindow, NSWindowCollectionBehavior, NSWindowLevel, NSWindowStyleMask,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use objc2_quartz_core::CALayer;

use crate::platform::macos::ui::window::MouseButton;
use crate::platform::macos::ui::window::Point;
use crate::platform::macos::ui::window::Size;
use crate::platform::macos::ui::window::WindowAttributes;
use crate::platform::macos::ui::window::WindowInput;
use dome_core::Logical;

pub(super) struct NativeWindow {
    window: Retained<AuxiliaryNSWindow>,
    view: Retained<AuxiliaryView>,
}

impl NativeWindow {
    pub(super) fn new(
        attributes: &WindowAttributes,
        input: Option<WindowInput>,
    ) -> anyhow::Result<Self> {
        let mtm = MainThreadMarker::new().expect("NativeWindow::new must run on the main thread");
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

        let view = AuxiliaryView::new(mtm, NSRect::new(NSPoint::new(0.0, 0.0), frame.size), input);
        window.setContentView(Some(&view));

        Ok(Self { window, view })
    }

    pub(super) fn set_frame(&self, position: Point<Logical>, size: Size<Logical>) {
        self.window
            .setFrame_display(to_nsrect(position, size), true);
    }

    pub(super) fn set_visible(&self, visible: bool) {
        self.window.setIsVisible(visible);
    }

    pub(super) fn set_content_layer(&self, layer: &CALayer) {
        self.view.setLayer(Some(layer));
        self.view.setWantsLayer(true);
    }

    pub(super) fn set_level(&self, level: crate::platform::macos::ui::window::WindowLevel) {
        self.window.setLevel(level.to_ns());
    }

    pub(super) fn focus(&self) {
        self.window.makeKeyAndOrderFront(None);
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        self.window.close();
    }
}

impl crate::platform::macos::ui::window::WindowLevel {
    fn to_ns(self) -> NSWindowLevel {
        match self {
            Self::Floating => NSFloatingWindowLevel,
            Self::Bottom => NSNormalWindowLevel - 1,
        }
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
    input: RefCell<Option<WindowInput>>,
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
            if let Some(input) = self.ivars().input.borrow_mut().as_mut() {
                input.mouse_down(at, MouseButton::Primary);
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let at = event_pos(self, event);
            if let Some(input) = self.ivars().input.borrow_mut().as_mut() {
                input.mouse_up(at, MouseButton::Primary);
            }
        }
    }
);

impl AuxiliaryView {
    fn new(mtm: MainThreadMarker, frame: NSRect, input: Option<WindowInput>) -> Retained<Self> {
        let ivars = AuxiliaryViewIvars {
            flipped: true,
            accepts_first_mouse: true,
            input: RefCell::new(input),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

fn to_nsrect(position: Point<Logical>, size: Size<Logical>) -> NSRect {
    NSRect::new(
        NSPoint::new(position.x().value() as f64, position.y().value() as f64),
        NSSize::new(size.width().value() as f64, size.height().value() as f64),
    )
}

fn event_pos(view: &NSView, event: &NSEvent) -> Point<Logical> {
    let loc = event.locationInWindow();
    let view_loc = view.convertPoint_fromView(loc, None);
    Point::new(view_loc.x as i32, view_loc.y as i32)
}
