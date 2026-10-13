use objc2_core_graphics::CGWindowID;
use objc2_foundation::NSRect;

use super::CaptureMessage;
use crate::config::Appearance;
use crate::core::{
    ContainerPlacement, Dimension, FloatWindowPlacement, Length, Logical, MonitorId,
    TilingWindowPlacement,
};
use crate::platform::macos::dome::events::MonitorScene;

pub(in crate::platform::macos) trait InternalWindows {
    fn set_monitors(&mut self, monitors: &[MonitorScene]);
    fn show_tiling(&mut self, tiling: &MonitorTilingData);
    fn show_floats(&mut self, floats: &[FloatShow], focused: Option<CGWindowID>);
    fn show_tab_bars(&mut self, containers: &[ContainerShow]);
    fn apply_capture_messages(&mut self, messages: Vec<CaptureMessage>);
    fn focus_tiling_overlay(&mut self, monitor: MonitorId);
    fn set_appearance(&mut self, appearance: &Appearance);
}

pub(in crate::platform::macos) struct MonitorTilingData {
    pub monitor_id: MonitorId,
    pub monitor_dim: Dimension<Logical>,
    pub cocoa_frame: NSRect,
    pub scale: f64,
    pub border_thickness: Length<Logical>,
    pub windows: Vec<TilingWindowShow>,
    pub containers: Vec<ContainerShow>,
}

#[derive(Clone, Copy)]
pub(in crate::platform::macos) struct TilingWindowShow {
    pub placement: TilingWindowPlacement,
    pub corner_radius: Length<Logical>,
}

#[derive(Clone)]
pub(in crate::platform::macos) struct ContainerShow {
    pub placement: ContainerPlacement,
    pub tab_bar_dim: Dimension<Logical>,
    pub tab_bar_cocoa_frame: NSRect,
    pub scale: f64,
    pub border_thickness: Length<Logical>,
}

pub(in crate::platform::macos) struct FloatShow {
    pub cg_id: CGWindowID,
    pub placement: FloatWindowPlacement,
    pub cocoa_frame: NSRect,
    pub scale: f64,
    pub border_thickness: Length<Logical>,
    pub content_dim: Dimension<Logical>,
    pub corner_radius: Length<Logical>,
}
