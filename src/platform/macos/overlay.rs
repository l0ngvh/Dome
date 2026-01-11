use std::cell::RefCell;

use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSBackingStoreType, NSBezierPath, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSResponder, NSStringDrawing, NSView, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::CGFloat;
use objc2_foundation::{
    NSDictionary, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};

use crate::config::Color;

pub(super) fn create_overlay_window(
    mtm: MainThreadMarker,
    frame: NSRect,
    level: isize,
) -> Retained<NSWindow> {
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

#[derive(Default)]
pub(super) struct OverlayViewIvars {
    rects: RefCell<Vec<OverlayRect>>,
    labels: RefCell<Vec<OverlayLabel>>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = OverlayViewIvars]
    pub(super) struct OverlayView;

    unsafe impl NSObjectProtocol for OverlayView {}

    impl OverlayView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            for rect in self.ivars().rects.borrow().iter() {
                let color = NSColor::colorWithSRGBRed_green_blue_alpha(
                    rect.color.r as CGFloat, rect.color.g as CGFloat, rect.color.b as CGFloat, rect.color.a as CGFloat,
                );
                color.setFill();
                NSBezierPath::fillRect(NSRect::new(
                    NSPoint::new(rect.x as CGFloat, rect.y as CGFloat),
                    NSSize::new(rect.width as CGFloat, rect.height as CGFloat),
                ));
            }
            for label in self.ivars().labels.borrow().iter() {
                let color = NSColor::colorWithSRGBRed_green_blue_alpha(
                    label.color.r as CGFloat, label.color.g as CGFloat, label.color.b as CGFloat, label.color.a as CGFloat,
                );
                let ns_string = NSString::from_str(&label.text);
                let font = if label.bold {
                    NSFont::boldSystemFontOfSize(12.0)
                } else {
                    NSFont::systemFontOfSize(12.0)
                };
                let attrs = unsafe {
                    NSDictionary::from_slices(
                        &[NSForegroundColorAttributeName, NSFontAttributeName],
                        &[
                            &*Retained::into_super(Retained::into_super(color)),
                            &*Retained::into_super(Retained::into_super(font)),
                        ],
                    )
                };
                unsafe {
                    ns_string.drawAtPoint_withAttributes(
                        NSPoint::new(label.x as CGFloat, label.y as CGFloat),
                        Some(&attrs),
                    );
                }
            }
        }
    }
);

impl OverlayView {
    pub(super) fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(OverlayViewIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    pub(super) fn set_rects(&self, rects: Vec<OverlayRect>, labels: Vec<OverlayLabel>) {
        *self.ivars().rects.borrow_mut() = rects;
        *self.ivars().labels.borrow_mut() = labels;
        self.setNeedsDisplay(true);
    }
}

#[derive(Clone)]
pub(super) struct OverlayRect {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) color: Color,
}

#[derive(Clone)]
pub(super) struct OverlayLabel {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) text: String,
    pub(super) color: Color,
    pub(super) bold: bool,
}

pub(super) struct Overlays {
    pub(super) tiling_rects: Vec<OverlayRect>,
    pub(super) tiling_labels: Vec<OverlayLabel>,
    pub(super) float_rects: Vec<OverlayRect>,
}
