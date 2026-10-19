mod activation;
pub(super) mod borders;
pub(super) mod capture_stream;
pub(crate) mod drawing;
pub(super) mod mirror_overlays;
pub(super) mod tab_bar;

mod overlays;
pub(super) use borders::TilingWindowShow;
pub(super) use overlays::*;

use crate::platform::macos::ui::window::Point;
use crate::platform::macos::ui::window::Size;
use dome_core::{ContainerPlacement, Dimension, Logical, Physical};
use objc2_foundation::NSRect;

#[derive(Clone)]
pub(in crate::platform::macos) struct ContainerShow {
    pub(in crate::platform::macos) placement: ContainerPlacement,
    /// `placement.tab_bar_band` at its configured height, which can run past the bottom of
    /// `placement.border_box`.
    pub(in crate::platform::macos) tab_bar_dim: Dimension<Logical>,
    /// Pre-flipped Cocoa frame for `placement.visible_tab_bar_band`. Always populated, even when
    /// `!placement.is_tabbed`.
    pub(in crate::platform::macos) tab_bar_cocoa_frame: NSRect,
}

fn frame_attrs(frame: NSRect) -> (Point<Logical>, Size<Logical>) {
    (
        Point::new(frame.origin.x.round() as i32, frame.origin.y.round() as i32),
        Size::new(
            frame.size.width.round() as u32,
            frame.size.height.round() as u32,
        ),
    )
}

fn physical_size(logical_w: f64, logical_h: f64, scale: f64) -> Size<Physical> {
    let w = (logical_w * scale).round() as u32;
    let h = (logical_h * scale).round() as u32;
    Size::new(w.max(1), h.max(1))
}

pub(crate) mod window;
