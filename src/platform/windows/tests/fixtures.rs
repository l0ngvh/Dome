use super::env::TestEnv;
use crate::config::Config;
use crate::core::{
    Dimension, Length, Physical, PixelRect, Pixels, ScrollingConfig, SizeConstraint, Strategy,
    TilingConfig, TilingWindowPlacement,
};
use crate::platform::windows::dome::MonitorInfo;
use crate::platform::windows::external::HwndId;

pub(super) const SCREEN_WIDTH: Length = Length::new(1920.0);
pub(super) const SCREEN_HEIGHT: Length = Length::new(1080.0);
pub(super) const OFFSCREEN_POS: Length = Length::new(-32000.0);

/// Initial rect a freshly-spawned mock reports until layout overwrites it.
pub(super) const SPAWN_DIM: Dimension<Physical> = Dimension::new(
    Length::ZERO,
    Length::ZERO,
    Length::new(800.0),
    Length::new(600.0),
);

pub(super) fn baseline_tiling() -> TilingConfig {
    TilingConfig {
        ignore: Config::default_ignore(),
        ..crate::config::tests::tiling_config()
    }
}

/// Four borderless scrolling columns, each 40% of the screen wide, opened in turn and settled.
/// The newest has focus, which leaves the right half of the second on screen and the first
/// wholly off it.
pub(super) fn four_scrolling_columns() -> (TestEnv, [HwndId; 4]) {
    let mut env = TestEnv::builder()
        .tiling(|tiling| {
            tiling.border_size = Pixels::ZERO;
            tiling.layout = Strategy::Scrolling;
            tiling.scrolling = ScrollingConfig {
                column_width: SizeConstraint::Percent(40.0),
            };
        })
        .build();
    let windows = env.open_many(4);
    env.settle(10);
    let windows = windows.try_into().expect("four windows opened");
    (env, windows)
}

pub(super) fn dim(x: i32, y: i32, w: i32, h: i32) -> Dimension<Physical> {
    Dimension::new(
        Length::new(x as f32),
        Length::new(y as f32),
        Length::new(w as f32),
        Length::new(h as f32),
    )
}

pub(super) fn fullscreen_dim() -> Dimension {
    Dimension::new(Length::ZERO, Length::ZERO, SCREEN_WIDTH, SCREEN_HEIGHT)
}

pub(super) fn default_monitor() -> MonitorInfo {
    MonitorInfo {
        handle: 1,
        name: "Test".to_string(),
        gdi_device: "\\\\.\\DISPLAY1".to_string(),
        work_area: PixelRect::from_dimension(Dimension::new(
            Length::ZERO,
            Length::ZERO,
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
        )),
        is_primary: true,
        scale: 1.0,
    }
}

pub(super) fn second_monitor() -> MonitorInfo {
    MonitorInfo {
        handle: 2,
        name: "External".to_string(),
        gdi_device: "\\\\.\\DISPLAY2".to_string(),
        work_area: PixelRect::from_dimension(Dimension::new(
            SCREEN_WIDTH,
            Length::ZERO,
            Length::new(2560.0),
            Length::new(1440.0),
        )),
        is_primary: false,
        scale: 1.0,
    }
}

pub(super) fn scaled_monitor(scale: f32) -> MonitorInfo {
    MonitorInfo {
        handle: 1,
        name: "Test".to_string(),
        gdi_device: "\\\\.\\DISPLAY1".to_string(),
        work_area: PixelRect::from_dimension(Dimension::new(
            Length::ZERO,
            Length::ZERO,
            SCREEN_WIDTH * scale,
            SCREEN_HEIGHT * scale,
        )),
        is_primary: true,
        scale,
    }
}

pub(super) fn assert_content_box_centered_in_border_box(placement: &TilingWindowPlacement) {
    let inset = placement.content_box.x() - placement.border_box.x();
    assert_eq!(placement.content_box.y() - placement.border_box.y(), inset);
    assert_eq!(
        placement.content_box.width(),
        placement.border_box.width() - inset * 2
    );
    assert_eq!(
        placement.content_box.height(),
        placement.border_box.height() - inset * 2
    );
}
