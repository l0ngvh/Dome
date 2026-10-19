use crate::config::{KeymapEffects, PreferredLayouts};

pub struct Configuration {
    pub directory: ConfigDirectory,
    pub runtime: LuaRuntime,
    pub config: Config,
    pub layouts: PreferredLayouts,
}

impl Configuration {
    pub fn new(source: &str, layout: Option<&str>, keymap: &mut dyn KeymapEffects) -> Self {
        super::logging::initialize();
        let directory = ConfigDirectory::create();
        let path = directory.write("config.lua", &config_source(source));
        let path = path.to_str().expect("UTF-8 path").to_owned();
        let (runtime, config) = LuaRuntime::load(
            path.clone(),
            std::fs::read_to_string(&path).map_err(Into::into),
            keymap,
        )
        .expect("the test Lua VM should build");
        let layouts = if let Some(layout) = layout {
            let path = directory.write("layout.lua", layout);
            PreferredLayouts::load_or_default(
                path.to_str().expect("UTF-8 path"),
                std::fs::read_to_string(&path).map_err(Into::into),
            )
        } else {
            PreferredLayouts::default()
        };
        Self {
            directory,
            runtime,
            config,
            layouts,
        }
    }
}

use crate::config::Config;
use crate::config::LuaRuntime;
use crate::tests::fixtures::directory::ConfigDirectory;
use crate::tests::fixtures::keymap::RecordingKeymap;
use std::path::Path;

pub struct LoadedConfig {
    pub config: Config,
    pub keymaps: Vec<String>,
}

pub fn load_config(source: &str) -> LoadedConfig {
    let directory = ConfigDirectory::create();
    load_path(&directory.write("config.lua", source))
}

pub fn load_missing_config() -> LoadedConfig {
    let directory = ConfigDirectory::create();
    load_path(&directory.path("config.lua"))
}

fn load_path(path: &Path) -> LoadedConfig {
    let mut keymap = RecordingKeymap::default();
    let path = path
        .to_str()
        .expect("the test config path should be UTF-8")
        .to_owned();
    let source = std::fs::read_to_string(&path).map_err(Into::into);
    let (_, config) =
        LuaRuntime::load(path, source, &mut keymap).expect("the test Lua VM should build");
    LoadedConfig {
        config,
        keymaps: keymap.published,
    }
}

const BASE_SETTINGS: &str = "{
    border_size = 1,
    minimum_width = 1,
    minimum_height = 1,
    partition_tree = { tab_bar_height = 2, automatic_tiling = false },
}";

pub fn config_source(chunk: &str) -> String {
    format!(
        "local test = (function() {chunk}
end)()
local config = {BASE_SETTINGS}
for key, value in pairs(test) do
    if type(value) == 'table' and type(config[key]) == 'table' then
        for inner_key, inner_value in pairs(value) do
            config[key][inner_key] = inner_value
        end
    else
        config[key] = value
    end
end
return config"
    )
}
