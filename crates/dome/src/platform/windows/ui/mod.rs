pub(crate) mod borders;
pub(crate) mod drawing;
mod overlays;
pub(crate) mod tab_bar;
pub(crate) mod thumbnail_overlays;
pub(crate) mod window;
pub(super) mod wnd_proc;

pub(crate) use borders::TilingWindowShow;
pub(in crate::platform::windows) use overlays::NativeOverlays;

use super::window_server::WindowOrder;
use crate::config::Appearance;
use dome_core::{
    ContainerId, ContainerPlacement, FloatWindowPlacement, FractionalLength, Length, Logical,
    MonitorId, Physical, PixelRect, WindowId,
};
use std::collections::HashMap;

pub(in crate::platform::windows) trait Overlays {
    fn update(&mut self, placements: OverlayPlacements);
}

pub(in crate::platform::windows) struct OverlayPlacements {
    pub tiling: HashMap<MonitorId, TilingOverlayPlacement>,
    pub tabs: HashMap<ContainerId, (MonitorId, ContainerPlacement)>,
    pub floats: HashMap<WindowId, FloatBorderRequest>,
    pub thumbnails: HashMap<WindowId, ThumbnailPlacement>,
    pub appearance: Appearance,
    pub focus_tiling: Option<MonitorId>,
}

pub(in crate::platform::windows) struct TilingOverlayPlacement {
    pub monitor: PixelRect,
    pub windows: Vec<TilingWindowShow>,
    pub containers: Vec<ContainerPlacement>,
    pub scale: f64,
    pub border_thickness: Length<Physical>,
    pub order: WindowOrder,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::platform::windows) struct FloatBorderPlacement {
    pub placement: FloatWindowPlacement,
    pub scale: f64,
    pub border_thickness: Length<Physical>,
    pub corner_radius: FractionalLength<Logical>,
    pub order: WindowOrder,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::platform::windows) enum FloatBorderState {
    Visible(FloatBorderPlacement),
    Hidden(FloatBorderPlacement),
}

#[derive(Clone, Copy, Debug)]
pub(in crate::platform::windows) enum FloatBorderRequest {
    Present(FloatBorderState),
    PreserveExisting,
}

pub(in crate::platform::windows) struct ThumbnailPlacement {
    pub frame: PixelRect,
    pub source_rect: PixelRect,
}
