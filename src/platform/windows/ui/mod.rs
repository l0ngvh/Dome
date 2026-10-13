mod auxiliary;
mod focus;
pub(in crate::platform::windows) mod overlay;
mod recovery;
mod registry;
mod state;
pub(in crate::platform::windows) mod window;

pub(super) use auxiliary::AuxiliaryWindows;
pub(super) use state::UiState;

use super::dome::events::{MonitorScene, TilingWindowShow};
use super::external::{HwndId, ZOrder};
use crate::config::Appearance;
use crate::core::{
    ContainerPlacement, FloatWindowPlacement, Length, Logical, MonitorId, Physical, Pixels,
};

pub(super) enum InternalWindow {
    TilingOverlay(MonitorId),
    FloatOverlay(HwndId),
}

pub(super) struct FloatShow {
    pub window: HwndId,
    pub placement: FloatWindowPlacement,
    pub scale: f32,
    pub border_thickness: Pixels<Physical>,
    pub corner_radius: Length<Logical>,
}

pub(super) trait InternalWindows {
    fn set_monitors(&mut self, monitors: &[MonitorScene]);
    fn show_tiling(
        &mut self,
        monitor: MonitorId,
        windows: &[TilingWindowShow],
        containers: &[ContainerPlacement],
    );
    fn show_floats(&mut self, floats: &[FloatShow]);
    fn show_tab_bars(&mut self, monitors: &[MonitorScene]);
    fn stack(&mut self, window: InternalWindow, order: ZOrder);
    fn focus_tiling_overlay(&mut self, monitor: MonitorId);
    fn set_appearance(&mut self, appearance: &Appearance);
}
