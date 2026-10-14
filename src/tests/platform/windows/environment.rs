use super::fixtures::{baseline_tiling, default_monitor};
use crate::config::LuaRuntime;
use crate::core::TilingConfig;
use crate::platform::keymap::{KeymapPublisher, KeymapView};
use crate::tests::config_directory::ConfigDirectory;
use crate::tests::platform::windows::test_fixture::{MonitorInfo, WindowsFixture};
use crate::tests::recording_keymap::RecordingKeymap;
use std::ops::{Deref, DerefMut};

pub(super) struct TestEnv {
    fixture: WindowsFixture,
    config_directory: ConfigDirectory,
}

impl Deref for TestEnv {
    type Target = WindowsFixture;
    fn deref(&self) -> &Self::Target {
        &self.fixture
    }
}

impl DerefMut for TestEnv {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fixture
    }
}

impl TestEnv {
    pub(super) fn new() -> Self {
        Self::builder().build()
    }

    pub(super) fn builder() -> TestEnvBuilder {
        TestEnvBuilder {
            tiling: baseline_tiling(),
            config_source: "return {}".to_owned(),
            monitors: vec![default_monitor()],
        }
    }

    pub(super) fn change_config(&mut self, source: &str) {
        self.config_directory.write("config.lua", source);
        self.fixture.reload();
    }
}

pub(super) struct TestEnvBuilder {
    tiling: TilingConfig,
    config_source: String,
    monitors: Vec<MonitorInfo>,
}

impl TestEnvBuilder {
    pub(super) fn monitors(mut self, monitors: Vec<MonitorInfo>) -> Self {
        self.monitors = monitors;
        self
    }

    pub(super) fn tiling(mut self, adjust: impl FnOnce(&mut TilingConfig)) -> Self {
        adjust(&mut self.tiling);
        self
    }

    pub(super) fn config_source(mut self, source: &str) -> Self {
        self.config_source = source.to_owned();
        self
    }

    pub(super) fn build(self) -> TestEnv {
        let directory = ConfigDirectory::create();
        let path = directory.write("config.lua", &self.config_source);
        let (runtime, _) = LuaRuntime::load(
            path.to_str().expect("UTF-8 config path").to_owned(),
            &mut RecordingKeymap::default(),
        )
        .expect("the test Lua VM should build");
        let (sender, _receiver) = std::sync::mpsc::channel();
        let keymap = KeymapPublisher::new(KeymapView::new(), sender);
        TestEnv {
            fixture: WindowsFixture::new(self.tiling, self.monitors, runtime, keymap),
            config_directory: directory,
        }
    }
}
