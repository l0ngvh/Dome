mod env;
mod mock;

use crate::core::{Dimension, Length, Logical, Physical, PixelRect};
pub(crate) use crate::platform::windows::HwndId;
pub(crate) use crate::platform::windows::MAX_DRIFT_RETRIES;
use crate::platform::windows::MonitorInfo as PlatformMonitorInfo;
pub(crate) use env::{FocusTarget, PaintedThumbnail, WindowsFixture};

pub(crate) fn round_radius() -> Length<Logical> {
    Length::new(8.0)
}
pub(crate) fn small_radius() -> Length<Logical> {
    Length::new(4.0)
}

pub(crate) const OFFSCREEN_POS: Length = Length::new(-32000.0);
pub(crate) const SPAWN_DIM: Dimension<Physical> = Dimension::new(
    Length::ZERO,
    Length::ZERO,
    Length::new(800.0),
    Length::new(600.0),
);

pub(crate) fn dim(x: i32, y: i32, width: i32, height: i32) -> Dimension<Physical> {
    Dimension::new(
        Length::new(x as f32),
        Length::new(y as f32),
        Length::new(width as f32),
        Length::new(height as f32),
    )
}

#[derive(Clone)]
pub(crate) struct MonitorInfo {
    pub(crate) handle: isize,
    pub(crate) name: String,
    pub(crate) gdi_device: String,
    pub(crate) work_area: PixelRect,
    pub(crate) is_primary: bool,
    pub(crate) scale: f32,
}

impl MonitorInfo {
    fn into_platform(self) -> PlatformMonitorInfo {
        PlatformMonitorInfo {
            handle: self.handle,
            name: self.name,
            gdi_device: self.gdi_device,
            work_area: self.work_area,
            is_primary: self.is_primary,
            scale: self.scale,
        }
    }
}
