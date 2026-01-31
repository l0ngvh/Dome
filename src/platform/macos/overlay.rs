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
use crate::core::{ContainerId, WindowId};

fn clip_to_bounds(rect: NSRect, bounds: NSRect) -> Option<NSRect> {
    if rect.origin.x >= bounds.origin.x + bounds.size.width
        || rect.origin.y >= bounds.origin.y + bounds.size.height
        || rect.origin.x + rect.size.width <= bounds.origin.x
        || rect.origin.y + rect.size.height <= bounds.origin.y
    {
        return None;
    }
    let x = rect.origin.x.max(bounds.origin.x);
    let y = rect.origin.y.max(bounds.origin.y);
    let right = (rect.origin.x + rect.size.width).min(bounds.origin.x + bounds.size.width);
    let top = (rect.origin.y + rect.size.height).min(bounds.origin.y + bounds.size.height);
    Some(NSRect::new(
        NSPoint::new(x, y),
        NSSize::new(right - x, top - y),
    ))
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
    window.setIgnoresMouseEvents(true);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary,
    );
    unsafe { window.setReleasedWhenClosed(false) };
    window
}

fn compute_border_edges(
    frame: NSRect,
    bounds: NSRect,
    colors: [Color; 4],
    b: f64,
) -> Vec<(NSRect, Color)> {
    let w = frame.size.width;
    let h = frame.size.height;

    // Monitor bounds in local coordinates
    let local_bounds = NSRect::new(
        NSPoint::new(
            bounds.origin.x - frame.origin.x,
            bounds.origin.y - frame.origin.y,
        ),
        bounds.size,
    );

    let mut edges = Vec::new();

    // top
    let top = NSRect::new(NSPoint::new(0.0, h - b), NSSize::new(w, b));
    if let Some(r) = clip_to_bounds(top, local_bounds) {
        edges.push((r, colors[0]));
    }

    // bottom
    let bottom = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, b));
    if let Some(r) = clip_to_bounds(bottom, local_bounds) {
        edges.push((r, colors[2]));
    }

    // right (exclude corners)
    let right = NSRect::new(NSPoint::new(w - b, b), NSSize::new(b, h - 2.0 * b));
    if let Some(r) = clip_to_bounds(right, local_bounds) {
        edges.push((r, colors[1]));
    }

    // left (exclude corners)
    let left = NSRect::new(NSPoint::new(0.0, b), NSSize::new(b, h - 2.0 * b));
    if let Some(r) = clip_to_bounds(left, local_bounds) {
        edges.push((r, colors[3]));
    }

    edges
}

pub(super) struct TilingBorder {
    pub(super) key: WindowId,
    pub(super) frame: NSRect,
    pub(super) bounds: NSRect,
    pub(super) colors: [Color; 4],
}

pub(super) struct FloatBorder {
    pub(super) key: WindowId,
    pub(super) frame: NSRect,
    pub(super) bounds: NSRect,
    pub(super) colors: [Color; 4],
}

pub(super) struct ContainerBorder {
    pub(super) key: ContainerId,
    pub(super) frame: NSRect,
    pub(super) bounds: NSRect,
    pub(super) colors: [Color; 4],
}

pub(super) struct TabInfo {
    pub(super) title: String,
    pub(super) x: f32,
    pub(super) width: f32,
    pub(super) is_active: bool,
}

pub(super) struct TabBarOverlay {
    pub(super) key: ContainerId,
    pub(super) frame: NSRect,
    pub(super) bounds: NSRect,
    pub(super) tabs: Vec<TabInfo>,
    pub(super) background_color: Color,
    pub(super) active_background_color: Color,
}

pub(super) struct Overlays {
    pub(super) tiling_borders: Vec<TilingBorder>,
    pub(super) float_borders: Vec<FloatBorder>,
    pub(super) container_borders: Vec<ContainerBorder>,
    pub(super) tab_bars: Vec<TabBarOverlay>,
    pub(super) border_size: f32,
}

// BorderView
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
    fn new(mtm: MainThreadMarker, frame: NSRect, edges: Vec<(NSRect, Color)>) -> Retained<Self> {
        let ivars = BorderViewIvars {
            edges: RefCell::new(edges),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn set_data(&self, edges: Vec<(NSRect, Color)>) {
        *self.ivars().edges.borrow_mut() = edges;
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

    fn set_data(
        &self,
        tabs: Vec<TabInfo>,
        background_color: Color,
        active_background_color: Color,
    ) {
        *self.ivars().tabs.borrow_mut() = tabs;
        self.ivars().background_color.set(background_color);
        self.ivars()
            .active_background_color
            .set(active_background_color);
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
    NSBezierPath::fillRect(NSRect::new(NSPoint::new(x, y), NSSize::new(width, height)));
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
    float: HashMap<WindowId, Retained<NSWindow>>,
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
            if !keep {
                w.close();
            }
            keep
        });
        self.float.retain(|k, w| {
            let keep = new_float.contains(k);
            if !keep {
                w.close();
            }
            keep
        });
        self.container.retain(|k, w| {
            let keep = new_container.contains(k);
            if !keep {
                w.close();
            }
            keep
        });
        self.tab_bars.retain(|k, w| {
            let keep = new_tab_bars.contains(k);
            if !keep {
                w.close();
            }
            keep
        });

        // Tiling borders
        for border in overlays.tiling_borders {
            let edges = compute_border_edges(border.frame, border.bounds, border.colors, b as f64);
            if edges.is_empty() {
                if let Some(w) = self.tiling.remove(&border.key) {
                    w.close();
                }
                continue;
            }
            if let Some(window) = self.tiling.get(&border.key) {
                window.setFrame_display(border.frame, true);
                if let Some(view) = window.contentView() {
                    let v: &BorderView = unsafe { std::mem::transmute(&*view) };
                    v.set_data(edges);
                }
            } else {
                let window = create_overlay_window(mtm, border.frame, NSNormalWindowLevel - 1);
                let view = BorderView::new(mtm, border.frame, edges);
                window.setContentView(Some(&view));
                window.orderFront(None);
                self.tiling.insert(border.key, window);
            }
        }

        // Float borders
        for border in overlays.float_borders {
            let edges = compute_border_edges(border.frame, border.bounds, border.colors, b as f64);
            if edges.is_empty() {
                if let Some(w) = self.float.remove(&border.key) {
                    w.close();
                }
                continue;
            }
            if let Some(window) = self.float.get(&border.key) {
                window.setFrame_display(border.frame, true);
                if let Some(view) = window.contentView() {
                    let v: &BorderView = unsafe { std::mem::transmute(&*view) };
                    v.set_data(edges);
                }
            } else {
                let window = create_overlay_window(mtm, border.frame, NSFloatingWindowLevel);
                let view = BorderView::new(mtm, border.frame, edges);
                window.setContentView(Some(&view));
                window.orderFront(None);
                self.float.insert(border.key, window);
            }
        }

        // Container borders
        for border in overlays.container_borders {
            let edges = compute_border_edges(border.frame, border.bounds, border.colors, b as f64);
            if edges.is_empty() {
                if let Some(w) = self.container.remove(&border.key) {
                    w.close();
                }
                continue;
            }
            if let Some(window) = self.container.get(&border.key) {
                window.setFrame_display(border.frame, true);
                if let Some(view) = window.contentView() {
                    let v: &BorderView = unsafe { std::mem::transmute(&*view) };
                    v.set_data(edges);
                }
            } else {
                let window = create_overlay_window(mtm, border.frame, NSNormalWindowLevel - 1);
                let view = BorderView::new(mtm, border.frame, edges);
                window.setContentView(Some(&view));
                window.orderFront(None);
                self.container.insert(border.key, window);
            }
        }

        // Tab bars
        for tab_bar in overlays.tab_bars {
            let Some(frame) = clip_to_bounds(tab_bar.frame, tab_bar.bounds) else {
                if let Some(w) = self.tab_bars.remove(&tab_bar.key) {
                    w.close();
                }
                continue;
            };
            if let Some(window) = self.tab_bars.get(&tab_bar.key) {
                window.setFrame_display(frame, true);
                if let Some(view) = window.contentView() {
                    let v: &TabBarView = unsafe { std::mem::transmute(&*view) };
                    v.set_data(
                        tab_bar.tabs,
                        tab_bar.background_color,
                        tab_bar.active_background_color,
                    );
                }
            } else {
                let window = create_overlay_window(mtm, frame, NSFloatingWindowLevel);
                let view = TabBarView::new(
                    mtm,
                    frame,
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
