//! Dome's configuration surface.

#[cfg(test)]
use anyhow::Result;
use anyhow::anyhow;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::core::{
    LayoutOptions, Logical, MasterConfig, PartitionTreeConfig, Pixels, SizeConstraint,
    SizeConstraints, Strategy, WindowMatcher,
};
use crate::font::{FontConfig, MAX_FONT_SIZE, MIN_FONT_SIZE, default_font_size};
use crate::keybinding::ModalKeymaps;
use crate::theme::Flavor;

pub(crate) mod bootstrap;
mod layout;
pub(crate) mod lua;
pub(crate) mod paths;
pub(crate) mod watch;

#[cfg(test)]
mod tests;

pub(crate) use layout::PreferredLayouts;

#[cfg(target_os = "macos")]
const BUNDLED_IGNORE: &str = include_str!("../../resources/ignore/macos.lua");

#[cfg(target_os = "windows")]
const BUNDLED_IGNORE: &str = include_str!("../../resources/ignore/windows.lua");

// Parsing the bundled rules costs a whole Lua VM, and every load merges the
// same result, so pay for it once.
static BUNDLED_IGNORE_RULES: LazyLock<Vec<WindowMatcher>> = LazyLock::new(|| {
    let vm = mlua::Lua::new();
    let mut callbacks = Vec::new();
    lua::evaluate(&vm, "bundled ignore", BUNDLED_IGNORE, &mut callbacks)
        .expect("bundled ignore rules must be valid Lua window matchers")
});

#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub(crate) keymaps: ModalKeymaps,
    pub(crate) layout: LayoutOptions,
    pub(crate) appearance: Appearance,
    pub(crate) log_level: LogLevel,
    pub(crate) start_at_login: bool,
    /// Environment variables layered over Dome's own environment for the
    /// commands `actions.execute` spawns. On macOS launchd gives Dome a minimal
    /// environment, so PATH usually must be set here for a spawned command to
    /// find its binary.
    pub(crate) env: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Appearance {
    pub(crate) theme: Flavor,
    pub(crate) font: FontConfig,
}

/// The flat key set `config.lua` accepts. `Config` groups the same values, and
/// grouping them in serde would need a second level of `#[serde(flatten)]`.
/// Nested flatten buffers through serde's `Content` type, which the hand-written
/// `mlua::Value` deserializer under `lua/` does not support.
#[derive(Deserialize)]
struct ConfigFile {
    #[serde(default)]
    keymaps: ModalKeymaps,
    #[serde(default = "LayoutOptions::default_border_size")]
    border_size: Pixels<Logical>,
    #[serde(default)]
    theme: Flavor,
    #[serde(flatten, default)]
    font: FontConfig,
    #[serde(default)]
    ignore: Vec<WindowMatcher>,
    #[serde(default)]
    log_level: LogLevel,
    #[serde(default)]
    start_at_login: bool,
    #[serde(default)]
    strategy: Strategy,
    #[serde(default)]
    partition_tree: PartitionTreeConfig,
    #[serde(default)]
    master: MasterConfig,
    #[serde(flatten, default)]
    size_constraints: SizeConstraints,
    #[serde(default)]
    float: Vec<WindowMatcher>,
    #[serde(default)]
    fullscreen: Vec<WindowMatcher>,
    #[serde(default)]
    env: HashMap<String, String>,
}

impl From<ConfigFile> for Config {
    fn from(file: ConfigFile) -> Self {
        Config {
            keymaps: file.keymaps,
            layout: LayoutOptions {
                strategy: file.strategy,
                border_size: file.border_size,
                partition_tree: file.partition_tree,
                master: file.master,
                size_constraints: file.size_constraints,
                float: file.float,
                fullscreen: file.fullscreen,
                ignore: file.ignore,
            },
            appearance: Appearance {
                theme: file.theme,
                font: file.font,
            },
            log_level: file.log_level,
            start_at_login: file.start_at_login,
            env: file.env,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            keymaps: ModalKeymaps::default(),
            layout: LayoutOptions {
                ignore: Self::default_ignore(),
                ..LayoutOptions::default()
            },
            appearance: Appearance::default(),
            log_level: LogLevel::default(),
            start_at_login: false,
            env: HashMap::new(),
        }
    }
}

impl Config {
    pub(crate) fn load_into(
        lua: &mlua::Lua,
        path: &str,
        registry: &mut Vec<mlua::Function>,
    ) -> anyhow::Result<Config> {
        let src = std::fs::read_to_string(path)?;
        let config = Self::from_lua(lua, path, &src, registry).map_err(|e| anyhow!("{e}"))?;
        config.validate_layout()?;
        Ok(config)
    }

    pub(crate) fn load_default_into(
        lua: &mlua::Lua,
        registry: &mut Vec<mlua::Function>,
    ) -> anyhow::Result<Config> {
        let config = Self::from_lua(lua, "default.lua", "return dome.defaults()", registry)
            .map_err(|e| anyhow!("{e}"))?;
        config.validate_layout()?;
        Ok(config)
    }

    #[cfg(test)]
    fn load(path: &str) -> Result<Self> {
        let src = std::fs::read_to_string(path)?;
        let config = Self::from_lua_src(path, &src).map_err(|e| anyhow!("{e}"))?;
        config.validate_layout()?;
        Ok(config)
    }

    fn validate_layout(&self) -> anyhow::Result<()> {
        if let (SizeConstraint::Pixels(min), SizeConstraint::Pixels(max)) = (
            self.layout.size_constraints.minimum_width,
            self.layout.size_constraints.maximum_width,
        ) && max > Pixels::ZERO
            && min > max
        {
            anyhow::bail!(
                "minimum_width ({}) cannot be greater than maximum_width ({})",
                min.value(),
                max.value()
            );
        }
        if let (SizeConstraint::Pixels(min), SizeConstraint::Pixels(max)) = (
            self.layout.size_constraints.minimum_height,
            self.layout.size_constraints.maximum_height,
        ) && max > Pixels::ZERO
            && min > max
        {
            anyhow::bail!(
                "minimum_height ({}) cannot be greater than maximum_height ({})",
                min.value(),
                max.value()
            );
        }
        Ok(())
    }

    fn from_lua(
        lua: &mlua::Lua,
        path: &str,
        src: &str,
        registry: &mut Vec<mlua::Function>,
    ) -> mlua::Result<Config> {
        let file: ConfigFile = lua::evaluate(lua, path, src, registry)?;
        let mut config = Config::from(file);
        let floor = Self::default_ignore();
        tracing::info!(count = floor.len(), "Applying built-in window-ignore floor");
        tracing::debug!(rules = ?floor, "Built-in window-ignore floor");
        config.layout.ignore.extend(floor);
        config.normalize();
        Ok(config)
    }

    #[cfg(test)]
    fn from_lua_src(path: &str, src: &str) -> mlua::Result<Self> {
        let lua = lua::new_vm()?;
        let mut registry = Vec::new();
        Self::from_lua(&lua, path, src, &mut registry)
    }

    /// Clamp out-of-range scalars back to their defaults, warning on each. Runs
    /// after deserialization, where serde has already applied per-field defaults
    /// for missing keys but cannot range-check a value the user did supply.
    fn normalize(&mut self) {
        if self.layout.partition_tree.tab_bar_height <= Pixels::ZERO {
            tracing::warn!(
                field = "partition_tree.tab_bar_height",
                value = self.layout.partition_tree.tab_bar_height.value(),
                "Out of range, using default",
            );
            self.layout.partition_tree.tab_bar_height =
                PartitionTreeConfig::default().tab_bar_height;
        }
        if !(0.1..=0.9).contains(&self.layout.master.master_ratio) {
            tracing::warn!(
                field = "master.master_ratio",
                value = self.layout.master.master_ratio,
                "Out of range, using default",
            );
            self.layout.master.master_ratio = MasterConfig::default().master_ratio;
        }
        if self.layout.master.master_count == 0 {
            tracing::warn!(
                field = "master.master_count",
                value = self.layout.master.master_count,
                "Out of range, using default",
            );
            self.layout.master.master_count = MasterConfig::default().master_count;
        }
        if !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&self.appearance.font.size) {
            tracing::warn!(
                field = "font_size",
                value = self.appearance.font.size,
                "Out of range, using default",
            );
            self.appearance.font.size = default_font_size();
        }
        if let Some(family) = &self.appearance.font.family
            && family.trim().is_empty()
        {
            tracing::warn!(field = "font_family", "Blank font family, using default",);
            self.appearance.font.family = None;
        }
    }

    fn default_ignore() -> Vec<WindowMatcher> {
        BUNDLED_IGNORE_RULES.clone()
    }
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

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
