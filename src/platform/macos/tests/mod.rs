mod env;
mod focus;
mod lifecycle;
mod matcher;
mod placement;
mod transitions;
mod uncooperative;

use env::TestEnv;

use objc2_core_graphics::CGWindowID;

use crate::config::Config;
use crate::core::{Dimension, Length, PixelRect, TilingConfig};
use crate::platform::macos::MonitorInfo;

const SCREEN_WIDTH: Length = Length::new(1920.0);
const SCREEN_HEIGHT: Length = Length::new(1080.0);

/// A work area whose every edge sits on the side of the half-point boundary where
/// rounding to nearest would grow the rectangle past what AppKit reported, so
/// containment fails under any policy other than inward. Its consumers name that
/// policy themselves, since `MonitorInfo` now holds an already-snapped rect.
const FRACTIONAL_WORK_AREA: Dimension = Dimension::new(
    Length::new(10.4),
    Length::new(20.4),
    Length::new(1899.2),
    Length::new(1039.2),
);

fn assert_inside_work_area(frame: (i32, i32, i32, i32), reported: Dimension) {
    let (x, y, width, height) = frame;
    assert!(
        x as f32 >= reported.x.value() && y as f32 >= reported.y.value(),
        "origin ({x}, {y}) escaped the reported ({}, {})",
        reported.x.value(),
        reported.y.value()
    );
    assert!(
        (x + width) as f32 <= (reported.x + reported.width).value()
            && (y + height) as f32 <= (reported.y + reported.height).value(),
        "far edge ({}, {}) escaped the reported ({}, {})",
        x + width,
        y + height,
        (reported.x + reported.width).value(),
        (reported.y + reported.height).value()
    );
}

fn baseline_tiling() -> TilingConfig {
    TilingConfig {
        ignore: Config::default_ignore(),
        ..crate::config::tests::tiling_config()
    }
}

fn default_monitor() -> MonitorInfo {
    MonitorInfo {
        display_id: 1,
        name: "Test".to_string(),
        work_area: PixelRect::from_dimension_inward(Dimension::new(
            Length::ZERO,
            Length::ZERO,
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
        )),
        bounds: Dimension::new(Length::ZERO, Length::ZERO, SCREEN_WIDTH, SCREEN_HEIGHT),
        full_height: SCREEN_HEIGHT.value(),
        is_primary: true,
        backing_scale: 2.0,
    }
}

fn second_monitor() -> MonitorInfo {
    MonitorInfo {
        display_id: 2,
        name: "External".to_string(),
        work_area: PixelRect::new(1920, 0, 2560, 1440),
        bounds: Dimension::new(
            Length::new(1920.0),
            Length::ZERO,
            Length::new(2560.0),
            Length::new(1440.0),
        ),
        full_height: 1440.0,
        is_primary: false,
        backing_scale: 2.0,
    }
}
