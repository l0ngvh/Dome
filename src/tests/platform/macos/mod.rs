mod lifecycle;
mod placement;
mod transitions;
mod uncooperative;
mod window_rules;

use crate::config::{Config, LuaRuntime};
use crate::core::{Dimension, Length, PixelRect, TilingConfig};
use crate::platform::keymap::{KeymapPublisher, KeymapView};
use crate::platform::macos::test_fixture::*;
use crate::tests::config_directory::ConfigDirectory;
use crate::tests::recording_keymap::RecordingKeymap;
use objc2_core_graphics::CGWindowID;
use objc2_foundation::NSRect;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

/// A work area whose every edge sits on the side of the half-point boundary where
/// rounding to nearest would grow the rectangle past what AppKit reported, so
/// containment fails under any policy other than inward.
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

struct MacOS {
    fixture: MacOSFixture,
    config_directory: Option<ConfigDirectory>,
}

impl Deref for MacOS {
    type Target = MacOSFixture;
    fn deref(&self) -> &Self::Target {
        &self.fixture
    }
}

impl DerefMut for MacOS {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fixture
    }
}

impl MacOS {
    fn new() -> Self {
        let tiling = TilingConfig {
            ignore: Config::default_ignore(),
            ..crate::tests::configuration::tiling_config()
        };
        Self {
            fixture: MacOSFixture::new(tiling),
            config_directory: None,
        }
    }

    fn setup_dome(&mut self) -> MacOSDome {
        self.dome_builder().build()
    }

    fn dome_builder(&mut self) -> DomeBuilder<'_> {
        let tiling = self.fixture.tiling();
        DomeBuilder {
            env: self,
            tiling,
            config_source: "return {}".to_owned(),
        }
    }

    fn change_config(&mut self, dome: &mut MacOSDome, source: &str) {
        self.config_directory
            .as_ref()
            .expect("a built dome owns its config directory")
            .write("config.lua", source);
        self.fixture.reload(dome);
    }
}

struct DomeBuilder<'env> {
    env: &'env mut MacOS,
    tiling: TilingConfig,
    config_source: String,
}

impl DomeBuilder<'_> {
    fn tiling(mut self, adjust: impl FnOnce(&mut TilingConfig)) -> Self {
        adjust(&mut self.tiling);
        self
    }

    fn config_source(mut self, source: &str) -> Self {
        self.config_source = source.to_owned();
        self
    }

    fn build(self) -> MacOSDome {
        let directory = ConfigDirectory::create();
        let path = directory.write("config.lua", &self.config_source);
        let (runtime, _) = LuaRuntime::load(
            path.to_str().expect("UTF-8 config path").to_owned(),
            &mut RecordingKeymap::default(),
        )
        .expect("the test Lua VM should build");
        let (sender, _receiver) = std::sync::mpsc::channel();
        let keymap = KeymapPublisher::new(KeymapView::new(), sender);
        let dome = self.env.fixture.build_dome(self.tiling, runtime, keymap);
        self.env.config_directory = Some(directory);
        dome
    }
}
