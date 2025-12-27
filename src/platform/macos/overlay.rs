use std::cell::RefCell;

use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSBackingStoreType, NSBezierPath, NSColor, NSEvent, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSNormalWindowLevel, NSResponder, NSStringDrawing, NSView,
    NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::CGFloat;
use objc2_foundation::{NSDictionary, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::config::{Color, Config};
use crate::core::{Child, Dimension, Direction, Focus, Hub, WorkspaceId};

pub(super) fn create_overlay_window(mtm: MainThreadMarker, frame: NSRect) -> Retained<NSWindow> {
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
    window.setLevel(NSNormalWindowLevel - 1);
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

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let location = event.locationInWindow();
            tracing::debug!("Overlay clicked at: ({}, {})", location.x, location.y);
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

fn border_rects(dim: Dimension, border_size: f32, inset: bool, colors: [Color; 4]) -> [OverlayRect; 4] {
    if inset {
        [
            OverlayRect { x: dim.x, y: dim.y + dim.height - border_size, width: dim.width, height: border_size, color: colors[0] },
            OverlayRect { x: dim.x, y: dim.y, width: dim.width, height: border_size, color: colors[1] },
            OverlayRect { x: dim.x, y: dim.y + border_size, width: border_size, height: dim.height - 2.0 * border_size, color: colors[2] },
            OverlayRect { x: dim.x + dim.width - border_size, y: dim.y + border_size, width: border_size, height: dim.height - 2.0 * border_size, color: colors[3] },
        ]
    } else {
        [
            OverlayRect { x: dim.x - border_size, y: dim.y + dim.height, width: dim.width + border_size * 2.0, height: border_size, color: colors[0] },
            OverlayRect { x: dim.x - border_size, y: dim.y - border_size, width: dim.width + border_size * 2.0, height: border_size, color: colors[1] },
            OverlayRect { x: dim.x - border_size, y: dim.y, width: border_size, height: dim.height, color: colors[2] },
            OverlayRect { x: dim.x + dim.width, y: dim.y, width: border_size, height: dim.height, color: colors[3] },
        ]
    }
}

pub(super) fn collect_overlays(hub: &Hub, config: &Config, workspace_id: WorkspaceId) -> (Vec<OverlayRect>, Vec<OverlayLabel>) {
    let mut rects = Vec::new();
    let mut labels = Vec::new();
    let workspace = hub.get_workspace(workspace_id);
    let focused = workspace.focused();
    let screen = hub.screen();
    let border_size = config.border_size;
    let tab_bar_height = config.tab_bar_height;

    let mut stack: Vec<Child> = workspace.root().into_iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Window(window_id) => {
                if focused == Some(Focus::Tiling(child)) {
                    continue;
                }
                let dim = hub.get_window(window_id).dimension();
                let y = screen.y + screen.height - dim.y - dim.height;
                let color = config.border_color;
                rects.extend(border_rects(Dimension { x: dim.x, y, width: dim.width, height: dim.height }, border_size, false, [color, color, color, color]));
            }
            Child::Container(container_id) => {
                let container = hub.get_container(container_id);
                for c in container.children() {
                    stack.push(*c);
                }

                if container.is_tabbed() {
                    let dim = container.dimension();
                    let y = screen.y + screen.height - dim.y - tab_bar_height;
                    let is_focused = focused == Some(Focus::Tiling(Child::Container(container_id)));
                    let tab_border_color = if is_focused { config.focused_color } else { config.border_color };
                    rects.push(OverlayRect { x: dim.x, y, width: dim.width, height: tab_bar_height, color: config.tab_bar_background_color });
                    // Tab bar border (inset)
                    let tab_dim = Dimension { x: dim.x, y, width: dim.width, height: tab_bar_height };
                    rects.extend(border_rects(tab_dim, border_size, true, [tab_border_color; 4]));

                    let children = container.children();
                    if !children.is_empty() {
                        let tab_width = dim.width / children.len() as f32;
                        let active_tab = container.active_tab();
                        // Active tab background
                        let active_x = dim.x + active_tab as f32 * tab_width;
                        rects.push(OverlayRect { x: active_x, y, width: tab_width, height: tab_bar_height, color: config.active_tab_background_color });
                        // Tab separators
                        for i in 1..children.len() {
                            let sep_x = dim.x + i as f32 * tab_width - border_size / 2.0;
                            rects.push(OverlayRect { x: sep_x, y, width: border_size, height: tab_bar_height, color: tab_border_color });
                        }
                        for (i, c) in children.iter().enumerate() {
                            let label = match c {
                                Child::Window(wid) => hub.get_window(*wid).title().to_string(),
                                Child::Container(_) => "Container".to_string(),
                            };
                            let is_active = i == active_tab;
                            let display = if is_active { format!("[{}]", label) } else { label };
                            let tab_x = dim.x + i as f32 * tab_width + tab_width / 2.0 - display.len() as f32 * 3.5;
                            labels.push(OverlayLabel { x: tab_x, y: y + tab_bar_height / 2.0 - 6.0, text: display, color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }, bold: is_active });
                        }
                    }
                }
            }
        }
    }

    match focused {
        Some(Focus::Tiling(Child::Window(window_id))) => {
            let color = config.focused_color;
            let spawn_color = config.spawn_indicator_color;
            let window = hub.get_window(window_id);
            let dim = window.dimension();
            let direction = window.new_window_direction();
            let y = screen.y + screen.height - dim.y - dim.height;
            let bottom = if direction == Direction::Vertical { spawn_color } else { color };
            let right = if direction == Direction::Horizontal { spawn_color } else { color };
            rects.extend(border_rects(Dimension { x: dim.x, y, width: dim.width, height: dim.height }, border_size, false, [color, bottom, color, right]));
        }
        Some(Focus::Tiling(Child::Container(container_id))) => {
            let color = config.focused_color;
            let spawn_color = config.spawn_indicator_color;
            let container = hub.get_container(container_id);
            let dim = container.dimension();
            let direction = container.new_window_direction();
            let y = screen.y + screen.height - dim.y - dim.height;
            let bottom = if direction == Direction::Vertical { spawn_color } else { color };
            let right = if direction == Direction::Horizontal { spawn_color } else { color };
            rects.extend(border_rects(Dimension { x: dim.x, y, width: dim.width, height: dim.height }, border_size, true, [color, bottom, color, right]));
        }
        _ => {}
    }

    for &float_id in workspace.float_windows() {
        let dim = hub.get_float(float_id).dimension();
        let y = screen.y + screen.height - dim.y - dim.height;
        let color = if focused == Some(Focus::Float(float_id)) { config.focused_color } else { config.border_color };
        rects.extend(border_rects(Dimension { x: dim.x, y, width: dim.width, height: dim.height }, border_size, false, [color; 4]));
    }

    (rects, labels)
}
