use std::collections::HashMap;

use anyhow::anyhow;

use crate::core::{TilingConfig, WindowMatcher};
use crate::font::FontConfig;
use crate::theme::Flavor;
use lua::deserializer::{LoadContext, string_enum};

mod defaults;
mod keybinding;
pub(crate) mod lua;
mod overrides;
mod preferred_layout;

pub use crate::core::PreferredLayouts;
pub use keybinding::{BASE_MODE, Keystroke, ModalKeymaps, Modifiers};
pub use lua::{KeymapEffects, LuaRuntime, PlatformEffects};

#[cfg(target_os = "macos")]
const BUNDLED_IGNORE: &str = include_str!("../../../../resources/ignore/macos.lua");

#[cfg(target_os = "windows")]
const BUNDLED_IGNORE: &str = include_str!("../../../../resources/ignore/windows.lua");

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const BUNDLED_IGNORE: &str = "return {}";

#[derive(Debug, Clone)]
pub struct Config {
    pub keymaps: ModalKeymaps,
    pub reserved_area: mlua::Function,
    pub tiling: TilingConfig,
    pub appearance: Appearance,
    pub log_level: LogLevel,
    pub start_at_login: bool,
    /// Variables the `execute` action adds to its child process
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Appearance {
    pub theme: Flavor,
    pub font: FontConfig,
}

impl Config {
    /// Invalid user fields recover through the field readers and bundled defaults.
    /// File or Lua evaluation errors, a non-table result, or a missing required
    /// bundled default fail the whole load instead.
    pub fn load(
        lua: &mlua::Lua,
        path: &str,
        source: anyhow::Result<String>,
    ) -> anyhow::Result<Config> {
        let src = source?;
        let config = Self::from_lua(lua, path, &src).map_err(|e| anyhow!("{e}"))?;
        Ok(config)
    }

    pub fn load_default(lua: &mlua::Lua) -> anyhow::Result<Config> {
        let config = Self::from_lua(lua, "default.lua", defaults::BUNDLED_SOURCE)
            .map_err(|e| anyhow!("{e}"))?;
        Ok(config)
    }

    pub fn default_ignore() -> Vec<WindowMatcher> {
        let vm = lua::new_vm().expect("the bundled Lua VM must build");
        lua::evaluate(&vm, "bundled ignore", BUNDLED_IGNORE)
            .expect("bundled ignore rules must be valid Lua window matchers")
    }

    fn from_lua(lua: &mlua::Lua, path: &str, src: &str) -> mlua::Result<Config> {
        let user: mlua::Value = lua.load(src).set_name(path).eval()?;
        let defaults: mlua::Value = lua
            .load(defaults::BUNDLED_SOURCE)
            .set_name("default.lua")
            .eval()?;
        let mut config = overrides::read_config(&user, &defaults, &mut LoadContext::new())?;
        let floor = Self::default_ignore();
        tracing::info!(count = floor.len(), "Applying built-in window-ignore floor");
        tracing::debug!(rules = ?floor, "Built-in window-ignore floor");
        config.tiling.ignore.extend(floor);
        Ok(config)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

string_enum!(
    LogLevel,
    "a log level",
    "trace" => LogLevel::Trace,
    "debug" => LogLevel::Debug,
    "info" => LogLevel::Info,
    "warn" => LogLevel::Warn,
    "error" => LogLevel::Error,
);

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}
