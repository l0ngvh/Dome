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

use crate::config::{Color, Config};
use crate::core::{Child, Dimension, Focus, Hub, WindowId, WorkspaceId};

use super::context::WindowRegistry;

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

fn border_rects(dim: Dimension, border_size: f32, colors: [Color; 4]) -> [OverlayRect; 4] {
    [
        OverlayRect {
            x: dim.x,
            y: dim.y + dim.height - border_size,
            width: dim.width,
            height: border_size,
            color: colors[0],
        },
        OverlayRect {
            x: dim.x,
            y: dim.y,
            width: dim.width,
            height: border_size,
            color: colors[1],
        },
        OverlayRect {
            x: dim.x,
            y: dim.y + border_size,
            width: border_size,
            height: dim.height - 2.0 * border_size,
            color: colors[2],
        },
        OverlayRect {
            x: dim.x + dim.width - border_size,
            y: dim.y + border_size,
            width: border_size,
            height: dim.height - 2.0 * border_size,
            color: colors[3],
        },
    ]
}

pub(super) struct Overlays {
    pub tiling_rects: Vec<OverlayRect>,
    pub tiling_labels: Vec<OverlayLabel>,
    pub float_rects: Vec<OverlayRect>,
}

pub(super) fn collect_overlays(
    hub: &Hub,
    config: &Config,
    workspace_id: WorkspaceId,
    registry: &WindowRegistry,
) -> Overlays {
    let mut tiling_rects = Vec::new();
    let mut tiling_labels = Vec::new();
    let mut float_rects = Vec::new();
    let workspace = hub.get_workspace(workspace_id);
    let focused = workspace.focused();
    let screen = hub.screen();
    let border_size = config.border_size;
    let tab_bar_height = config.tab_bar_height;

    let get_title = |wid: WindowId| -> String {
        registry
            .get_by_tiling_id(wid)
            .map(|w| w.title().to_owned())
            .unwrap_or_else(|| "Unknown".to_owned())
    };

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
                tiling_rects.extend(border_rects(
                    Dimension {
                        x: dim.x,
                        y,
                        width: dim.width,
                        height: dim.height,
                    },
                    border_size,
                    [color; 4],
                ));
            }
            Child::Container(container_id) => {
                let container = hub.get_container(container_id);
                if container.is_tabbed() {
                    if let Some(active) = container.active_tab() {
                        stack.push(active);
                    }
                    let dim = container.dimension();
                    let y = screen.y + screen.height - dim.y - tab_bar_height;
                    let is_focused = focused == Some(Focus::Tiling(Child::Container(container_id)));
                    let tab_border_color = if is_focused {
                        config.focused_color
                    } else {
                        config.border_color
                    };
                    tiling_rects.push(OverlayRect {
                        x: dim.x,
                        y,
                        width: dim.width,
                        height: tab_bar_height,
                        color: config.tab_bar_background_color,
                    });
                    // Tab bar border
                    let tab_dim = Dimension {
                        x: dim.x,
                        y,
                        width: dim.width,
                        height: tab_bar_height,
                    };
                    tiling_rects.extend(border_rects(tab_dim, border_size, [tab_border_color; 4]));

                    let children = container.children();
                    if !children.is_empty() {
                        let tab_width = dim.width / children.len() as f32;
                        let active_tab = container.active_tab_index();
                        // Active tab background
                        let active_x = dim.x + active_tab as f32 * tab_width;
                        tiling_rects.push(OverlayRect {
                            x: active_x,
                            y,
                            width: tab_width,
                            height: tab_bar_height,
                            color: config.active_tab_background_color,
                        });
                        // Tab separators
                        for i in 1..children.len() {
                            let sep_x = dim.x + i as f32 * tab_width - border_size / 2.0;
                            tiling_rects.push(OverlayRect {
                                x: sep_x,
                                y,
                                width: border_size,
                                height: tab_bar_height,
                                color: tab_border_color,
                            });
                        }
                        for (i, c) in children.iter().enumerate() {
                            let label = match c {
                                Child::Window(wid) => get_title(*wid),
                                Child::Container(_) => "Container".to_owned(),
                            };
                            let is_active = i == active_tab;
                            let display = if is_active {
                                format!("[{}]", label)
                            } else {
                                label
                            };
                            let tab_x = dim.x + i as f32 * tab_width + tab_width / 2.0
                                - display.len() as f32 * 3.5;
                            tiling_labels.push(OverlayLabel {
                                x: tab_x,
                                y: y + tab_bar_height / 2.0 - 6.0,
                                text: display,
                                color: Color {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 1.0,
                                },
                                bold: is_active,
                            });
                        }
                    }
                } else {
                    for c in container.children() {
                        stack.push(*c);
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
            let spawn_mode = window.spawn_mode();
            let y = screen.y + screen.height - dim.y - dim.height;
            let top = if spawn_mode.is_tab() {
                spawn_color
            } else {
                color
            };
            let bottom = if spawn_mode.is_vertical() {
                spawn_color
            } else {
                color
            };
            let right = if spawn_mode.is_horizontal() {
                spawn_color
            } else {
                color
            };
            tiling_rects.extend(border_rects(
                Dimension {
                    x: dim.x,
                    y,
                    width: dim.width,
                    height: dim.height,
                },
                border_size,
                [top, bottom, color, right],
            ));
        }
        Some(Focus::Tiling(Child::Container(container_id))) => {
            let color = config.focused_color;
            let spawn_color = config.spawn_indicator_color;
            let container = hub.get_container(container_id);
            let dim = container.dimension();
            let spawn_mode = container.spawn_mode();
            let y = screen.y + screen.height - dim.y - dim.height;
            let top = if spawn_mode.is_tab() {
                spawn_color
            } else {
                color
            };
            let bottom = if spawn_mode.is_vertical() {
                spawn_color
            } else {
                color
            };
            let right = if spawn_mode.is_horizontal() {
                spawn_color
            } else {
                color
            };
            tiling_rects.extend(border_rects(
                Dimension {
                    x: dim.x,
                    y,
                    width: dim.width,
                    height: dim.height,
                },
                border_size,
                [top, bottom, color, right],
            ));
        }
        _ => {}
    }

    for &float_id in workspace.float_windows() {
        let dim = hub.get_float(float_id).dimension();
        let y = screen.y + screen.height - dim.y - dim.height;
        let color = if focused == Some(Focus::Float(float_id)) {
            config.focused_color
        } else {
            config.border_color
        };
        float_rects.extend(border_rects(
            Dimension {
                x: dim.x,
                y,
                width: dim.width,
                height: dim.height,
            },
            border_size,
            [color; 4],
        ));
    }

    Overlays {
        tiling_rects,
        tiling_labels,
        float_rects,
    }
}
