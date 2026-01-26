use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSBackingStoreType, NSBezierPath, NSColor, NSFloatingWindowLevel, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSNormalWindowLevel, NSResponder, NSStringDrawing, NSView,
    NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::CGFloat;
use objc2_foundation::{
    NSDictionary, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};

use crate::config::Color;
use crate::core::{ContainerId, FloatWindowId, WindowId};

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
    window.setIgnoresMouseEvents(true);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary,
    );
    unsafe { window.setReleasedWhenClosed(false) };
    window
}

pub(super) struct TilingBorder {
    pub key: WindowId,
    pub frame: NSRect,
    pub colors: [Color; 4],
}

pub(super) struct FloatBorder {
    pub key: FloatWindowId,
    pub frame: NSRect,
    pub colors: [Color; 4],
}

pub(super) struct ContainerBorder {
    pub key: ContainerId,
    pub frame: NSRect,
    pub colors: [Color; 4],
}

pub(super) struct TabInfo {
    pub title: String,
    pub x: f32,
    pub width: f32,
    pub is_active: bool,
}

pub(super) struct TabBarOverlay {
    pub key: ContainerId,
    pub frame: NSRect,
    pub tabs: Vec<TabInfo>,
    pub background_color: Color,
    pub active_background_color: Color,
}

pub(super) struct Overlays {
    pub tiling_borders: Vec<TilingBorder>,
    pub float_borders: Vec<FloatBorder>,
    pub container_borders: Vec<ContainerBorder>,
    pub tab_bars: Vec<TabBarOverlay>,
    pub border_size: f32,
}

// BorderView
struct BorderViewIvars {
    colors: RefCell<[Color; 4]>,
    border_size: Cell<f32>,
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
            let frame = self.bounds();
            let colors = self.ivars().colors.borrow();
            let b = self.ivars().border_size.get() as f64;
            let w = frame.size.width;
            let h = frame.size.height;

            draw_rect(0.0, h - b, w, b, colors[0]); // top
            draw_rect(0.0, 0.0, w, b, colors[2]);   // bottom
            // left/right exclude corners to avoid overlap with different spawn indicator colors
            draw_rect(w - b, b, b, h - 2.0 * b, colors[1]); // right
            draw_rect(0.0, b, b, h - 2.0 * b, colors[3]);   // left
        }
    }
);

impl BorderView {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        colors: [Color; 4],
        border_size: f32,
    ) -> Retained<Self> {
        let ivars = BorderViewIvars {
            colors: RefCell::new(colors),
            border_size: Cell::new(border_size),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn set_data(&self, colors: [Color; 4], border_size: f32) {
        *self.ivars().colors.borrow_mut() = colors;
        self.ivars().border_size.set(border_size);
        self.setNeedsDisplay(true);
    }
}

// TabBarView
struct TabBarViewIvars {
    tabs: RefCell<Vec<TabInfo>>,
    background_color: Cell<Color>,
    active_background_color: Cell<Color>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TabBarViewIvars]
    struct TabBarView;

    unsafe impl NSObjectProtocol for TabBarView {}

    impl TabBarView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            let frame = self.bounds();
            let tabs = self.ivars().tabs.borrow();
            let bg = self.ivars().background_color.get();
            let active_bg = self.ivars().active_background_color.get();

            draw_rect(0.0, 0.0, frame.size.width, frame.size.height, bg);

            for tab in tabs.iter() {
                if tab.is_active {
                    draw_rect(tab.x as f64, 0.0, tab.width as f64, frame.size.height, active_bg);
                }
                let text_color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
                draw_label(
                    &tab.title,
                    tab.x + 8.0,
                    frame.size.height as f32 / 2.0 - 6.0,
                    text_color,
                    tab.is_active,
                );
            }
        }
    }
);

impl TabBarView {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        tabs: Vec<TabInfo>,
        background_color: Color,
        active_background_color: Color,
    ) -> Retained<Self> {
        let ivars = TabBarViewIvars {
            tabs: RefCell::new(tabs),
            background_color: Cell::new(background_color),
            active_background_color: Cell::new(active_background_color),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn set_data(&self, tabs: Vec<TabInfo>, background_color: Color, active_background_color: Color) {
        *self.ivars().tabs.borrow_mut() = tabs;
        self.ivars().background_color.set(background_color);
        self.ivars().active_background_color.set(active_background_color);
        self.setNeedsDisplay(true);
    }
}

fn draw_rect(x: f64, y: f64, width: f64, height: f64, color: Color) {
    let ns_color = NSColor::colorWithSRGBRed_green_blue_alpha(
        color.r as CGFloat,
        color.g as CGFloat,
        color.b as CGFloat,
        color.a as CGFloat,
    );
    ns_color.setFill();
    NSBezierPath::fillRect(NSRect::new(
        NSPoint::new(x, y),
        NSSize::new(width, height),
    ));
}

fn draw_label(text: &str, x: f32, y: f32, color: Color, bold: bool) {
    let ns_color = NSColor::colorWithSRGBRed_green_blue_alpha(
        color.r as CGFloat,
        color.g as CGFloat,
        color.b as CGFloat,
        color.a as CGFloat,
    );
    let ns_string = NSString::from_str(text);
    let font = if bold {
        NSFont::boldSystemFontOfSize(12.0)
    } else {
        NSFont::systemFontOfSize(12.0)
    };
    let attrs = unsafe {
        NSDictionary::from_slices(
            &[NSForegroundColorAttributeName, NSFontAttributeName],
            &[
                &*Retained::into_super(Retained::into_super(ns_color)),
                &*Retained::into_super(Retained::into_super(font)),
            ],
        )
    };
    unsafe {
        ns_string.drawAtPoint_withAttributes(NSPoint::new(x as f64, y as f64), Some(&attrs));
    }
}

pub(super) struct OverlayManager {
    tiling: HashMap<WindowId, Retained<NSWindow>>,
    float: HashMap<FloatWindowId, Retained<NSWindow>>,
    container: HashMap<ContainerId, Retained<NSWindow>>,
    tab_bars: HashMap<ContainerId, Retained<NSWindow>>,
}

impl OverlayManager {
    pub(super) fn new() -> Self {
        Self {
            tiling: HashMap::new(),
            float: HashMap::new(),
            container: HashMap::new(),
            tab_bars: HashMap::new(),
        }
    }

    pub(super) fn process(&mut self, mtm: MainThreadMarker, overlays: Overlays) {
        let b = overlays.border_size;

        // Collect new keys
        let new_tiling: std::collections::HashSet<_> =
            overlays.tiling_borders.iter().map(|x| x.key).collect();
        let new_float: std::collections::HashSet<_> =
            overlays.float_borders.iter().map(|x| x.key).collect();
        let new_container: std::collections::HashSet<_> =
            overlays.container_borders.iter().map(|x| x.key).collect();
        let new_tab_bars: std::collections::HashSet<_> =
            overlays.tab_bars.iter().map(|x| x.key).collect();

        // Remove stale
        self.tiling.retain(|k, w| {
            let keep = new_tiling.contains(k);
            if !keep { w.close(); }
            keep
        });
        self.float.retain(|k, w| {
            let keep = new_float.contains(k);
            if !keep { w.close(); }
            keep
        });
        self.container.retain(|k, w| {
            let keep = new_container.contains(k);
            if !keep { w.close(); }
            keep
        });
        self.tab_bars.retain(|k, w| {
            let keep = new_tab_bars.contains(k);
            if !keep { w.close(); }
            keep
        });

        // Tiling borders
        for border in overlays.tiling_borders {
            if let Some(window) = self.tiling.get(&border.key) {
                window.setFrame_display(border.frame, true);
                if let Some(view) = window.contentView() {
                    let v: &BorderView = unsafe { std::mem::transmute(&*view) };
                    v.set_data(border.colors, b);
                }
            } else {
                let window = create_overlay_window(mtm, border.frame, NSNormalWindowLevel - 1);
                let view = BorderView::new(mtm, border.frame, border.colors, b);
                window.setContentView(Some(&view));
                window.orderFront(None);
                self.tiling.insert(border.key, window);
            }
        }

        // Float borders
        for border in overlays.float_borders {
            if let Some(window) = self.float.get(&border.key) {
                window.setFrame_display(border.frame, true);
                if let Some(view) = window.contentView() {
                    let v: &BorderView = unsafe { std::mem::transmute(&*view) };
                    v.set_data(border.colors, b);
                }
            } else {
                let window = create_overlay_window(mtm, border.frame, NSFloatingWindowLevel);
                let view = BorderView::new(mtm, border.frame, border.colors, b);
                window.setContentView(Some(&view));
                window.orderFront(None);
                self.float.insert(border.key, window);
            }
        }

        // Container borders
        for border in overlays.container_borders {
            if let Some(window) = self.container.get(&border.key) {
                window.setFrame_display(border.frame, true);
                if let Some(view) = window.contentView() {
                    let v: &BorderView = unsafe { std::mem::transmute(&*view) };
                    v.set_data(border.colors, b);
                }
            } else {
                let window = create_overlay_window(mtm, border.frame, NSNormalWindowLevel - 1);
                let view = BorderView::new(mtm, border.frame, border.colors, b);
                window.setContentView(Some(&view));
                window.orderFront(None);
                self.container.insert(border.key, window);
            }
        }

        // Tab bars
        for tab_bar in overlays.tab_bars {
            if let Some(window) = self.tab_bars.get(&tab_bar.key) {
                window.setFrame_display(tab_bar.frame, true);
                if let Some(view) = window.contentView() {
                    let v: &TabBarView = unsafe { std::mem::transmute(&*view) };
                    v.set_data(tab_bar.tabs, tab_bar.background_color, tab_bar.active_background_color);
                }
            } else {
                let window = create_overlay_window(mtm, tab_bar.frame, NSFloatingWindowLevel);
                let view = TabBarView::new(
                    mtm,
                    tab_bar.frame,
                    tab_bar.tabs,
                    tab_bar.background_color,
                    tab_bar.active_background_color,
                );
                window.setContentView(Some(&view));
                window.orderFront(None);
                self.tab_bars.insert(tab_bar.key, window);
            }
        }
    }
}
