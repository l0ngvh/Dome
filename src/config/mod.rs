//! Dome's configuration surface.

use std::collections::HashMap;

#[cfg(test)]
use anyhow::Result;
use anyhow::anyhow;

use crate::core::{TilingConfig, WindowMatcher};
use crate::font::FontConfig;
use crate::theme::Flavor;
use lua::deserializer::{LoadContext, string_enum};
use overrides::ConfigOverrides;

pub(crate) mod bootstrap;
pub(crate) mod defaults;
pub(crate) mod keybinding;
pub(crate) mod lua;
mod overrides;
pub(crate) mod paths;
mod preferred_layout;
pub(crate) mod watch;

#[cfg(test)]
pub(crate) mod tests;

pub(crate) use crate::core::PreferredLayouts;
pub(crate) use keybinding::{BASE_MODE, Keystroke, ModalKeymaps, Modifiers};
pub(crate) use lua::{KeymapEffects, KeymapRuntime, LuaRuntime, PlatformEffects};

#[cfg(target_os = "macos")]
const BUNDLED_IGNORE: &str = include_str!("../../resources/ignore/macos.lua");

#[cfg(target_os = "windows")]
const BUNDLED_IGNORE: &str = include_str!("../../resources/ignore/windows.lua");

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const BUNDLED_IGNORE: &str = "return {}";

#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub(crate) keymaps: ModalKeymaps,
    pub(crate) tiling: TilingConfig,
    pub(crate) appearance: Appearance,
    pub(crate) log_level: LogLevel,
    pub(crate) start_at_login: bool,
    /// Variables the `execute` action adds to its child process
    pub(crate) env: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Appearance {
    pub(crate) theme: Flavor,
    pub(crate) font: FontConfig,
}

impl Config {
    pub(crate) fn load(lua: &mlua::Lua, path: &str) -> anyhow::Result<Config> {
        let src = std::fs::read_to_string(path)?;
        let config = Self::from_lua(lua, path, &src).map_err(|e| anyhow!("{e}"))?;
        Ok(config)
    }

    pub(crate) fn load_default(lua: &mlua::Lua) -> anyhow::Result<Config> {
        let config = Self::from_lua(lua, "default.lua", defaults::BUNDLED_SOURCE)
            .map_err(|e| anyhow!("{e}"))?;
        Ok(config)
    }

    #[cfg(test)]
    fn load_from_path(path: &str) -> Result<Self> {
        let src = std::fs::read_to_string(path)?;
        let config = Self::from_lua_src(path, &src).map_err(|e| anyhow!("{e}"))?;
        Ok(config)
    }

    fn from_lua(lua: &mlua::Lua, path: &str, src: &str) -> mlua::Result<Config> {
        let mut cx = LoadContext::new();
        let overrides: ConfigOverrides = lua::evaluate_with(lua, path, src, &mut cx)?;
        let default_keymaps = match overrides.needs_default_keymaps() {
            true => Some(defaults::bundled_keymaps(lua, &mut cx)?),
            false => None,
        };
        let mut config = overrides.merge_over(&defaults::bundled()?, default_keymaps, &mut cx);
        let floor = Self::default_ignore();
        tracing::info!(count = floor.len(), "Applying built-in window-ignore floor");
        tracing::debug!(rules = ?floor, "Built-in window-ignore floor");
        config.tiling.ignore.extend(floor);
        Ok(config)
    }

    #[cfg(test)]
    fn from_lua_src(path: &str, src: &str) -> mlua::Result<Self> {
        let lua = lua::new_vm()?;
        Self::from_lua(&lua, path, src)
    }

    pub(crate) fn default_ignore() -> Vec<WindowMatcher> {
        let vm = lua::new_vm().expect("the bundled Lua VM must build");
        lua::evaluate(&vm, "bundled ignore", BUNDLED_IGNORE)
            .expect("bundled ignore rules must be valid Lua window matchers")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum LogLevel {
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
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}
