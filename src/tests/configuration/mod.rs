mod loading;
mod reserved_area;
mod values;

use crate::config::Config;
use crate::config::LuaRuntime;
use crate::core::TilingConfig;
use crate::tests::config_directory::ConfigDirectory;
use crate::tests::recording_keymap::RecordingKeymap;
use std::path::Path;

/// The `ignore` list stays empty, so a fixture manages every window it inserts.
pub(crate) fn tiling_config() -> TilingConfig {
    TilingConfig {
        ignore: Vec::new(),
        ..load_missing_config().config.tiling
    }
}

pub(crate) struct LoadedConfig {
    pub(crate) config: Config,
    pub(crate) keymaps: Vec<String>,
}

pub(crate) fn load_config(source: &str) -> LoadedConfig {
    let directory = ConfigDirectory::create();
    load_path(&directory.write("config.lua", source))
}

pub(crate) fn load_missing_config() -> LoadedConfig {
    let directory = ConfigDirectory::create();
    load_path(&directory.path("config.lua"))
}

fn load_path(path: &Path) -> LoadedConfig {
    let mut keymap = RecordingKeymap::default();
    let path = path
        .to_str()
        .expect("the test config path should be UTF-8")
        .to_owned();
    let (_, config) = LuaRuntime::load(path, &mut keymap).expect("the test Lua VM should build");
    LoadedConfig {
        config,
        keymaps: keymap.published,
    }
}
