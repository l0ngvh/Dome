use anyhow::{Result, anyhow};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::action::{Action, Actions};
use crate::core::{Length, Logical, PaneDisplay, Pixels, Unit};
use crate::font::{FontConfig, MAX_FONT_SIZE, MIN_FONT_SIZE, default_text_size};
use crate::theme::{Flavor, Theme};
use mlua::LuaSerdeExt;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Config {
    #[serde(skip_deserializing, default)]
    pub(crate) keymaps: ModalKeymaps,
    #[serde(default = "default_border_size")]
    pub(crate) border_size: Pixels<Logical>,
    #[serde(default)]
    pub(crate) theme: Flavor,
    #[serde(default)]
    pub(crate) font: FontConfig,
    #[serde(default)]
    pub(crate) ignore: Vec<WindowMatcher>,
    #[serde(default)]
    pub(crate) log_level: LogLevel,
    #[serde(default)]
    pub(crate) start_at_login: bool,
    #[serde(default = "default_strategy")]
    pub(crate) strategy: Strategy,
    #[serde(default = "default_partition_tree_config")]
    pub(crate) partition_tree: PartitionTreeConfig,
    #[serde(default = "default_master_config")]
    pub(crate) master: MasterConfig,
    #[serde(flatten, default)]
    pub(crate) size_constraints: SizeConstraints,
    #[serde(default)]
    pub(crate) float: Vec<WindowMatcher>,
    #[serde(default)]
    pub(crate) fullscreen: Vec<WindowMatcher>,
}

pub(crate) fn default_border_size() -> Pixels<Logical> {
    Pixels::new(4)
}

fn default_tab_bar_height() -> Pixels<Logical> {
    Pixels::new(24)
}

impl Default for Config {
    fn default() -> Self {
        Config {
            keymaps: ModalKeymaps::default(),
            border_size: default_border_size(),
            theme: Flavor::default(),
            font: FontConfig::default(),
            ignore: default_ignore(),
            log_level: LogLevel::default(),
            start_at_login: false,
            strategy: default_strategy(),
            partition_tree: default_partition_tree_config(),
            master: default_master_config(),
            size_constraints: SizeConstraints::default(),
            float: Vec::new(),
            fullscreen: Vec::new(),
        }
    }
}

impl Config {
    pub(crate) fn theme(&self) -> Theme {
        Theme::from_flavor(self.theme)
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn default_path() -> String {
        let config_dir = std::env::var("APPDATA").unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE").unwrap_or_default();
            format!("{home}\\AppData\\Roaming")
        });
        format!("{config_dir}\\dome\\config.lua")
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn default_path() -> String {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_default();
                format!("{home}/.config")
            });
        format!("{config_dir}/dome/config.lua")
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn log_dir() -> String {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/Library/Logs/dome")
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn log_dir() -> String {
        let config_dir = std::env::var("APPDATA").unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE").unwrap_or_default();
            format!("{home}\\AppData\\Roaming")
        });
        format!("{config_dir}\\dome\\logs")
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn log_dir() -> String {
        let data_dir = std::env::var("XDG_STATE_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_default();
                format!("{home}/.local/state")
            });
        format!("{data_dir}/dome")
    }

    fn validate_layout(&self) -> anyhow::Result<()> {
        if let (SizeConstraint::Pixels(min), SizeConstraint::Pixels(max)) = (
            self.size_constraints.minimum_width,
            self.size_constraints.maximum_width,
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
            self.size_constraints.minimum_height,
            self.size_constraints.maximum_height,
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

    #[cfg(test)]
    pub(crate) fn load(path: &str) -> Result<Self> {
        let src = std::fs::read_to_string(path)?;
        let config = Self::from_lua_src(path, &src).map_err(|e| anyhow!("{e}"))?;
        config.validate_layout()?;
        Ok(config)
    }

    #[cfg(test)]
    fn from_lua_src(path: &str, src: &str) -> mlua::Result<Self> {
        let lua = crate::lua_runtime::build_vm()?;
        let mut registry = Vec::new();
        config_from_lua(&lua, path, src, &mut registry)
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub(crate) struct Modifiers: u8 {
        const META = 1 << 0;
        const SHIFT = 1 << 1;
        const ALT = 1 << 2;
        const CTRL = 1 << 3;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Keymap {
    pub(crate) key: String,
    pub(crate) modifiers: Modifiers,
}

impl FromStr for Keymap {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('+').collect();
        if parts.is_empty() {
            return Err(anyhow!("Empty keymap"));
        }
        let key = parts.last().unwrap().to_string();
        let mut modifiers = Modifiers::empty();
        for m in &parts[..parts.len() - 1] {
            modifiers |= match *m {
                // cmd and win name the meta key on their platforms, so a config need not branch on the OS.
                "meta" | "cmd" | "win" => Modifiers::META,
                "shift" => Modifiers::SHIFT,
                "alt" => Modifiers::ALT,
                "ctrl" => Modifiers::CTRL,
                _ => return Err(anyhow!("Unknown modifier: {}", m)),
            };
        }
        Ok(Keymap { key, modifiers })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CallbackId(pub usize);

/// A resolved keymap value. A static action list resolves on the event-tap
/// thread. A callback is a Lua function held on the `dome-lua` thread and
/// referenced here only by id.
#[derive(Debug, Clone)]
pub(crate) enum Binding {
    Static(Actions),
    Callback(CallbackId),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ModalKeymaps {
    pub(crate) default: HashMap<Keymap, Binding>,
    pub(crate) modes: HashMap<String, HashMap<Keymap, Binding>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Strategy {
    PartitionTree,
    Master,
}

pub(crate) fn default_strategy() -> Strategy {
    Strategy::PartitionTree
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct PartitionTreeConfig {
    #[serde(default = "default_tab_bar_height")]
    pub(crate) tab_bar_height: Pixels<Logical>,
    #[serde(default = "default_automatic_tiling")]
    pub(crate) automatic_tiling: bool,
}

fn default_automatic_tiling() -> bool {
    true
}

pub(crate) fn default_partition_tree_config() -> PartitionTreeConfig {
    PartitionTreeConfig {
        tab_bar_height: default_tab_bar_height(),
        automatic_tiling: default_automatic_tiling(),
    }
}

/// Global `master_ratio` and `master_count` seed new workspaces on their first
/// `attach_window`. They do NOT flow into existing workspaces on hot-reload.
/// Runtime tuning via `master grow/shrink/more/fewer` persists across reloads.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct MasterConfig {
    #[serde(default = "default_master_ratio")]
    pub(crate) master_ratio: f32,
    #[serde(default = "default_master_count")]
    pub(crate) master_count: usize,
}

fn default_master_ratio() -> f32 {
    0.5
}

fn default_master_count() -> usize {
    1
}

pub(crate) fn default_master_config() -> MasterConfig {
    MasterConfig {
        master_ratio: default_master_ratio(),
        master_count: default_master_count(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct SizeConstraints {
    pub(crate) minimum_width: SizeConstraint,
    pub(crate) minimum_height: SizeConstraint,
    pub(crate) maximum_width: SizeConstraint,
    pub(crate) maximum_height: SizeConstraint,
}

impl Default for SizeConstraints {
    fn default() -> Self {
        Self {
            minimum_width: SizeConstraint::default_min(),
            minimum_height: SizeConstraint::default_min(),
            maximum_width: SizeConstraint::default(),
            maximum_height: SizeConstraint::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SizeConstraint {
    Pixels(Pixels<Logical>),
    Percent(f32),
}

impl Default for SizeConstraint {
    fn default() -> Self {
        SizeConstraint::Pixels(Pixels::ZERO)
    }
}

impl SizeConstraint {
    /// `Pixels` is a config-denominated absolute logical length, so it goes
    /// through `to_unit(scale)` to reach the frame unit. `Percent` is a ratio of
    /// `screen_size`, which the caller passes in frame units already, so `scale`
    /// does not apply.
    pub(crate) fn resolve(&self, screen_size: Length<Unit>, scale: f32) -> Length<Unit> {
        match self {
            SizeConstraint::Pixels(px) => Length::from_pixels(*px).to_unit(scale),
            SizeConstraint::Percent(pct) => screen_size * (pct / 100.0),
        }
    }

    pub(crate) fn default_min() -> Self {
        SizeConstraint::Percent(5.0)
    }
}

/// Takes `f64` because narrowing first lands `100000000.5` on an exact `1e8` that then passes
/// as whole.
fn pixels_from_config<E: serde::de::Error>(v: f64) -> Result<Pixels<Logical>, E> {
    if !v.is_finite() || v < 0.0 {
        return Err(E::custom(
            "pixel value must be a finite non-negative number",
        ));
    }
    if v.fract() != 0.0 {
        return Err(E::custom("pixel value must be a whole number"));
    }
    if v > i32::MAX as f64 {
        return Err(E::custom("pixel value is out of range"));
    }
    Ok(Pixels::new(v as i32))
}

impl<'de> Deserialize<'de> for Pixels<Logical> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        pixels_from_config(f64::deserialize(d)?)
    }
}

impl<'de> Deserialize<'de> for SizeConstraint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SizeConstraintVisitor;

        impl<'de> serde::de::Visitor<'de> for SizeConstraintVisitor {
            type Value = SizeConstraint;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter
                    .write_str("a whole number for pixels or a string percentage (e.g., \"10%\")")
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                pixels_from_config(v).map(SizeConstraint::Pixels)
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                self.visit_f64(v as f64)
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                self.visit_f64(v as f64)
            }

            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
                if let Some(pct) = s.strip_suffix('%') {
                    let val: f32 = pct.trim().parse().map_err(E::custom)?;
                    if !(0.0..=100.0).contains(&val) {
                        return Err(E::custom("percentage must be between 0 and 100"));
                    }
                    Ok(SizeConstraint::Percent(val))
                } else {
                    Err(E::custom("string must be a percentage (e.g., \"10%\")"))
                }
            }
        }

        deserializer.deserialize_any(SizeConstraintVisitor)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Deserialize)]
pub(crate) struct WindowMatcher {
    #[serde(default)]
    pub(crate) app: Option<String>,
    #[serde(default)]
    pub(crate) bundle_id: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) process: Option<String>,
    #[serde(default)]
    pub(crate) class: Option<String>,
    #[serde(default)]
    pub(crate) aumid: Option<String>,
}

pub(crate) fn pattern_matches(pattern: &str, text: &str) -> bool {
    if let Some(regex) = pattern.strip_prefix('/').and_then(|p| p.strip_suffix('/')) {
        regex::Regex::new(regex)
            .map(|r| r.is_match(text))
            .unwrap_or(false)
    } else {
        pattern == text
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum WindowMode {
    Tiling,
    Float,
    Fullscreen,
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

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub(crate) struct LayoutConfig {
    #[serde(default)]
    pub(crate) workspace: Vec<LayoutWorkspaceConfig>,
}

fn dedup_preferred_layout_config(
    entries: Vec<LayoutWorkspaceConfig>,
    prefix: &str,
) -> Vec<LayoutWorkspaceConfig> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<LayoutWorkspaceConfig> = Vec::with_capacity(entries.len());
    for entry in entries {
        let ws_name = entry.name().to_string();
        if ws_name.is_empty() {
            tracing::warn!(
                field = %field_path(prefix, "workspace"),
                "Empty workspace name, dropping",
            );
            continue;
        }
        if let Some(&idx) = seen.get(&ws_name) {
            tracing::warn!(
                field = %field_path(prefix, "workspace"),
                name = ws_name,
                "Duplicate workspace, replacing earlier entry",
            );
            out[idx] = entry;
        } else {
            seen.insert(ws_name, out.len());
            out.push(entry);
        }
    }
    out
}

impl LayoutConfig {
    pub(crate) fn load(path: &str) -> anyhow::Result<Self> {
        let src = std::fs::read_to_string(path)?;
        let mut layout = Self::from_jsonc_src(path, &src)?;
        layout.workspace = dedup_preferred_layout_config(layout.workspace, "");
        Ok(layout)
    }

    fn from_jsonc_src(path: &str, src: &str) -> anyhow::Result<Self> {
        let value: serde_json::Value =
            jsonc_parser::parse_to_serde_value(src, &jsonc_parser::ParseOptions::default())
                .map_err(|e| anyhow!("{path}: {e}"))?;
        Ok(serde_json::from_value(value)?)
    }
}

/// One master-strategy pane in `layout.jsonc`. An array of matchers is a tiled
/// pane. An object `{ display, children }` sets the display explicitly, so
/// `{ "display": "tabbed", "children": [...] }` stacks the pane into tabs.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PaneConfig {
    pub(crate) display: PaneDisplay,
    pub(crate) children: Vec<WindowMatcher>,
}

impl PaneConfig {
    #[cfg(test)]
    pub(crate) fn tiled(children: Vec<WindowMatcher>) -> Self {
        Self {
            display: PaneDisplay::Tiled,
            children,
        }
    }
}

#[derive(Deserialize)]
struct PaneContainer {
    #[serde(default)]
    display: PaneDisplay,
    #[serde(default)]
    children: Vec<WindowMatcher>,
}

impl From<PaneContainer> for PaneConfig {
    fn from(c: PaneContainer) -> Self {
        PaneConfig {
            display: c.display,
            children: c.children,
        }
    }
}

impl<'de> Deserialize<'de> for PaneConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = PaneConfig;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an array of window matchers, or a table with display and children")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                seq: A,
            ) -> Result<Self::Value, A::Error> {
                let children = Vec::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))?;
                Ok(PaneConfig {
                    display: PaneDisplay::Tiled,
                    children,
                })
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                map: M,
            ) -> Result<Self::Value, M::Error> {
                PaneContainer::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(Into::into)
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "strategy")]
pub(crate) enum LayoutWorkspaceConfig {
    #[serde(rename = "partition_tree")]
    PartitionTree {
        name: String,
        #[serde(default)]
        tree: Option<TreeLayoutNode>,
        #[serde(default)]
        float: Vec<WindowMatcher>,
        #[serde(default)]
        fullscreen: Vec<WindowMatcher>,
    },
    #[serde(rename = "master")]
    Master {
        name: String,
        #[serde(default)]
        master_ratio: Option<f32>,
        #[serde(default)]
        master_count: Option<usize>,
        #[serde(default)]
        master: PaneConfig,
        #[serde(default)]
        secondary: PaneConfig,
        #[serde(default)]
        float: Vec<WindowMatcher>,
        #[serde(default)]
        fullscreen: Vec<WindowMatcher>,
    },
}

impl LayoutWorkspaceConfig {
    pub(crate) fn name(&self) -> &str {
        match self {
            LayoutWorkspaceConfig::PartitionTree { name, .. }
            | LayoutWorkspaceConfig::Master { name, .. } => name,
        }
    }
}

/// A node in the preferred tree layout for partition-tree workspaces. The
/// custom deserializer accepts three shapes: a leaf window matcher, an array of
/// children, or a `{ split, children }` container.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TreeLayoutNode {
    Leaf(WindowMatcher),
    Container {
        /// `None` leaves the split mode to the runtime, which picks one based
        /// on context when it materializes the tree.
        split: Option<SplitMode>,
        children: Vec<TreeLayoutNode>,
    },
}

#[derive(Deserialize)]
struct TreeContainer {
    #[serde(default)]
    split: Option<SplitMode>,
    children: Vec<TreeLayoutNode>,
}

impl From<TreeContainer> for TreeLayoutNode {
    fn from(c: TreeContainer) -> Self {
        TreeLayoutNode::Container {
            split: c.split,
            children: c.children,
        }
    }
}

impl<'de> Deserialize<'de> for TreeLayoutNode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = TreeLayoutNode;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a window matcher table, an array of children, or a container table with split and children")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                seq: A,
            ) -> Result<Self::Value, A::Error> {
                let children = Vec::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))?;
                Ok(TreeLayoutNode::Container {
                    split: None,
                    children,
                })
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                map: M,
            ) -> Result<Self::Value, M::Error> {
                use serde::de::Error;
                let value: toml::Value =
                    toml::Value::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                        .map_err(|e| M::Error::custom(&e))?;
                if value
                    .as_table()
                    .is_some_and(|t| t.contains_key("split") || t.contains_key("children"))
                {
                    TreeContainer::deserialize(value)
                        .map(Into::into)
                        .map_err(|e| M::Error::custom(&e))
                } else {
                    WindowMatcher::deserialize(value)
                        .map(TreeLayoutNode::Leaf)
                        .map_err(|e| M::Error::custom(&e))
                }
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SplitMode {
    Horizontal,
    Vertical,
    Tabbed,
}

fn parse_actions(action_strs: &[String]) -> Result<Actions> {
    let actions: Vec<Action> = action_strs
        .iter()
        .map(|s| s.parse())
        .collect::<Result<_>>()?;
    Ok(Actions::new(actions))
}

fn field_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

// Clamp out-of-range tuning values per field, keeping the rest. A wrong type or
// a negative or fractional pixel fails deserialization earlier and hits the
// whole-file fallback in load_or_default instead.
fn normalize_config(config: &mut Config) {
    if config.partition_tree.tab_bar_height <= Pixels::ZERO {
        tracing::warn!(
            field = "partition_tree.tab_bar_height",
            value = config.partition_tree.tab_bar_height.value(),
            "Out of range, using default",
        );
        config.partition_tree.tab_bar_height = default_tab_bar_height();
    }
    if !(0.1..=0.9).contains(&config.master.master_ratio) {
        tracing::warn!(
            field = "master.master_ratio",
            value = config.master.master_ratio,
            "Out of range, using default",
        );
        config.master.master_ratio = default_master_ratio();
    }
    if config.master.master_count == 0 {
        tracing::warn!(
            field = "master.master_count",
            value = config.master.master_count,
            "Out of range, using default",
        );
        config.master.master_count = default_master_count();
    }
    if !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&config.font.text_size) {
        tracing::warn!(
            field = "font.text_size",
            value = config.font.text_size,
            "Out of range, using default",
        );
        config.font.text_size = default_text_size();
    }
    if let Some(family) = &config.font.family
        && family.trim().is_empty()
    {
        tracing::warn!(field = "font.family", "Blank font family, using default",);
        config.font.family = None;
    }
}

// The bundled default config. `dome.defaults()` re-evaluates this source to
// return a fresh table, and the R8 fallback deserializes it. It must not call
// `dome.defaults()`, which would re-enter its own evaluation and not terminate.
pub(crate) const DEFAULT_LUA: &str = include_str!("../resources/default.lua");

#[cfg(target_os = "macos")]
const BUNDLED_IGNORE: &str = include_str!("../resources/ignore/macos.lua");

#[cfg(target_os = "windows")]
const BUNDLED_IGNORE: &str = include_str!("../resources/ignore/windows.lua");

// The rules ship as bundled Lua data, so a parse or type failure here is a
// build defect in the bundled file rather than a user error.
fn default_ignore() -> Vec<WindowMatcher> {
    let lua = mlua::Lua::new();
    let value: mlua::Value = lua
        .load(BUNDLED_IGNORE)
        .set_name("bundled ignore")
        .eval()
        .expect("bundled ignore rules must be valid Lua");
    lua.from_value(value)
        .expect("bundled ignore rules must deserialize to window matchers")
}

fn walk_lua_keymaps(
    table: &mlua::Table,
    registry: &mut Vec<mlua::Function>,
) -> mlua::Result<ModalKeymaps> {
    let keymaps_table = match table.get::<mlua::Value>("keymaps")? {
        mlua::Value::Table(t) => t,
        mlua::Value::Nil => return Ok(ModalKeymaps::default()),
        other => {
            tracing::warn!(
                field = "keymaps",
                error = %format!("expected table, got {}", other.type_name()),
                "Invalid value, using none",
            );
            return Ok(ModalKeymaps::default());
        }
    };

    // `mode` is pulled aside so it does not parse as a top-level binding.
    let mode_value = keymaps_table.get::<mlua::Value>("mode")?;
    keymaps_table.set("mode", mlua::Value::Nil)?;

    let default = walk_lua_bindings(&keymaps_table, "keymaps", registry)?;

    let mut modes = HashMap::new();
    match mode_value {
        mlua::Value::Table(mode_map) => {
            for pair in mode_map.pairs::<String, mlua::Value>() {
                let (mode_name, mode_val) = pair?;
                if mode_name == "default" {
                    tracing::warn!(
                        field = %format!("keymaps.mode.{mode_name}"),
                        "Reserved mode name, dropping",
                    );
                    continue;
                }
                if mode_name.is_empty() {
                    tracing::warn!(field = "keymaps.mode.", "Empty mode name, dropping",);
                    continue;
                }
                let mlua::Value::Table(bindings) = mode_val else {
                    tracing::warn!(
                        field = %format!("keymaps.mode.{mode_name}"),
                        "Expected table for mode, dropping",
                    );
                    continue;
                };
                let prefix = format!("keymaps.mode.{mode_name}");
                let mode_bindings = walk_lua_bindings(&bindings, &prefix, registry)?;
                modes.insert(mode_name, mode_bindings);
            }
        }
        mlua::Value::Nil => {}
        _ => tracing::warn!(field = "keymaps.mode", "Expected table, ignoring",),
    }

    Ok(ModalKeymaps { default, modes })
}

fn walk_lua_bindings(
    table: &mlua::Table,
    prefix: &str,
    registry: &mut Vec<mlua::Function>,
) -> mlua::Result<HashMap<Keymap, Binding>> {
    let mut result = HashMap::new();
    for pair in table.pairs::<String, mlua::Value>() {
        let (key_str, value) = pair?;
        let field = field_path(prefix, &key_str);
        let keymap = match key_str.parse::<Keymap>() {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!(field = %field, error = %e, "Invalid key binding, dropping");
                continue;
            }
        };
        let binding = match value {
            mlua::Value::String(s) => match parse_actions(&[s.to_str()?.to_string()]) {
                Ok(actions) => Binding::Static(actions),
                Err(e) => {
                    tracing::warn!(field = %field, error = %e, "Invalid action, dropping binding");
                    continue;
                }
            },
            mlua::Value::Table(list) => {
                let action_strs = match list
                    .sequence_values::<String>()
                    .collect::<mlua::Result<Vec<_>>>()
                {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(field = %field, error = %e, "Invalid actions value, dropping");
                        continue;
                    }
                };
                match parse_actions(&action_strs) {
                    Ok(actions) => Binding::Static(actions),
                    Err(e) => {
                        tracing::warn!(field = %field, error = %e, "Invalid action, dropping binding");
                        continue;
                    }
                }
            }
            mlua::Value::Function(f) => {
                let id = CallbackId(registry.len());
                registry.push(f);
                Binding::Callback(id)
            }
            other => {
                tracing::warn!(
                    field = %field,
                    error = %format!("expected string, list, or function, got {}", other.type_name()),
                    "Invalid actions value, dropping",
                );
                continue;
            }
        };
        result.insert(keymap, binding);
    }
    Ok(result)
}

fn config_from_lua(
    lua: &mlua::Lua,
    path: &str,
    src: &str,
    registry: &mut Vec<mlua::Function>,
) -> mlua::Result<Config> {
    let value: mlua::Value = lua.load(src).set_name(path).eval()?;
    let table = value
        .as_table()
        .ok_or_else(|| mlua::Error::runtime("config must return a table"))?
        .clone();
    let keymaps = walk_lua_keymaps(&table, registry)?;
    // Drop keymaps before serde. A binding value may be a function, which does
    // not deserialize. Keymaps are walked by hand above instead.
    table.set("keymaps", mlua::Value::Nil)?;
    let mut config: Config = lua.from_value(mlua::Value::Table(table))?;
    config.keymaps = keymaps;
    let floor = default_ignore();
    // R19: the floor applies to every config, so surface it at load.
    tracing::info!(count = floor.len(), "Applying built-in window-ignore floor");
    tracing::debug!(rules = ?floor, "Built-in window-ignore floor");
    config.ignore.extend(floor);
    normalize_config(&mut config);
    Ok(config)
}

pub(crate) fn load_config_into(
    lua: &mlua::Lua,
    path: &str,
    registry: &mut Vec<mlua::Function>,
) -> anyhow::Result<Config> {
    let src = std::fs::read_to_string(path)?;
    let config = config_from_lua(lua, path, &src, registry).map_err(|e| anyhow!("{e}"))?;
    config.validate_layout()?;
    Ok(config)
}

pub(crate) fn load_default_config_into(
    lua: &mlua::Lua,
    registry: &mut Vec<mlua::Function>,
) -> anyhow::Result<Config> {
    let config =
        config_from_lua(lua, "default.lua", DEFAULT_LUA, registry).map_err(|e| anyhow!("{e}"))?;
    config.validate_layout()?;
    Ok(config)
}

pub(crate) fn load_or_default<T: Default>(
    path: &str,
    load: impl Fn(&str) -> anyhow::Result<T>,
) -> T {
    match load(path) {
        Ok(v) => v,
        Err(e)
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
        {
            tracing::info!(%path, "File not found, using defaults");
            T::default()
        }
        Err(e) => {
            tracing::warn!(%path, error = %format!("{e:#}"), "Failed to load, using defaults");
            T::default()
        }
    }
}

pub(crate) fn layout_default_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .expect("config path must have a parent directory")
        .join("layout.jsonc")
}

pub(crate) fn start_file_watcher(
    path: &str,
    on_event: impl Fn() + Send + 'static,
) -> anyhow::Result<RecommendedWatcher> {
    let path_buf = Path::new(path).canonicalize()?;
    let watch_dir = path_buf
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent dir"))?
        .to_owned();
    let target = path_buf.clone();
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res
            && matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_))
            && event.paths.iter().any(|p| p == &target)
        {
            on_event();
        }
    })?;
    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;
    tracing::info!(%path, "File watcher started");
    Ok(watcher)
}

pub(crate) fn start_config_watcher<T: Send + 'static>(
    path: &str,
    load_fn: impl Fn(&str) -> anyhow::Result<T> + Send + 'static,
    on_change: impl Fn(T) + Send + 'static,
) -> anyhow::Result<RecommendedWatcher> {
    let target = Path::new(path)
        .canonicalize()?
        .to_string_lossy()
        .into_owned();
    start_file_watcher(path, move || match load_fn(&target) {
        Ok(v) => {
            tracing::info!(path = %target, "File reloaded");
            on_change(v);
        }
        Err(e) => tracing::warn!(path = %target, error = %e, "Failed to reload"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_from(src: &str) -> Config {
        Config::from_lua_src("test config", src).expect("config should load")
    }

    fn config_and_registry_from(src: &str) -> (Config, Vec<mlua::Function>) {
        let lua = crate::lua_runtime::build_vm().expect("vm should build");
        let mut registry = Vec::new();
        let config =
            config_from_lua(&lua, "test config", src, &mut registry).expect("config should load");
        (config, registry)
    }

    fn try_config(src: &str) -> Result<Config> {
        Config::from_lua_src("test config", src).map_err(|e| anyhow!("{e}"))
    }

    fn layout_from(src: &str) -> LayoutConfig {
        let mut layout =
            LayoutConfig::from_jsonc_src("test layout", src).expect("layout should load");
        layout.workspace = dedup_preferred_layout_config(layout.workspace, "");
        layout
    }

    fn workspace_from(src: &str) -> LayoutWorkspaceConfig {
        try_workspace(src).expect("workspace should deserialize")
    }

    fn try_workspace(src: &str) -> anyhow::Result<LayoutWorkspaceConfig> {
        let value: serde_json::Value =
            jsonc_parser::parse_to_serde_value(src, &jsonc_parser::ParseOptions::default())?;
        Ok(serde_json::from_value(value)?)
    }

    struct CleanupFile(std::path::PathBuf);
    impl Drop for CleanupFile {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).ok();
        }
    }

    fn temp_lua_path(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("dome_{tag}_{nanos}.lua"))
    }

    fn temp_jsonc_path(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("dome_{tag}_{nanos}.jsonc"))
    }

    #[test]
    fn min_size_default() {
        let config = config_from("return {}");
        assert_eq!(
            config.size_constraints.minimum_width,
            SizeConstraint::Percent(5.0)
        );
        assert_eq!(
            config.size_constraints.minimum_height,
            SizeConstraint::Percent(5.0)
        );
    }

    #[test]
    fn max_size_default() {
        let config = config_from("return {}");
        assert_eq!(
            config.size_constraints.maximum_width,
            SizeConstraint::Pixels(Pixels::new(0))
        );
        assert_eq!(
            config.size_constraints.maximum_height,
            SizeConstraint::Pixels(Pixels::new(0))
        );
    }

    #[test]
    fn size_constraint_parses_float_as_pixels() {
        let config = config_from("return { minimum_width = 200.0 }");
        assert_eq!(
            config.size_constraints.minimum_width,
            SizeConstraint::Pixels(Pixels::new(200))
        );
    }

    #[test]
    fn size_constraint_parses_int_as_pixels() {
        let config = config_from("return { minimum_width = 200 }");
        assert_eq!(
            config.size_constraints.minimum_width,
            SizeConstraint::Pixels(Pixels::new(200))
        );
    }

    #[test]
    fn size_constraint_parses_string_percent() {
        let config = config_from(r#"return { minimum_width = "10%" }"#);
        assert_eq!(
            config.size_constraints.minimum_width,
            SizeConstraint::Percent(10.0)
        );
    }

    #[test]
    fn size_constraint_rejects_invalid_percent() {
        assert!(try_config(r#"return { minimum_width = "101%" }"#).is_err());
        assert!(try_config(r#"return { minimum_width = "-5%" }"#).is_err());
    }

    #[test]
    fn size_constraint_rejects_negative_pixels() {
        assert!(try_config("return { minimum_width = -100 }").is_err());
    }

    #[test]
    fn size_constraint_rejects_fractional_pixels() {
        assert!(try_config("return { minimum_width = 100.5 }").is_err());
    }

    #[test]
    fn size_constraint_rejects_non_finite_pixels() {
        for expr in ["0/0", "math.huge", "-math.huge"] {
            assert!(
                try_config(&format!("return {{ minimum_width = {expr} }}")).is_err(),
                "{expr} should be rejected"
            );
        }
    }

    #[test]
    fn size_constraint_rejects_string_without_percent() {
        assert!(try_config(r#"return { minimum_width = "200" }"#).is_err());
    }

    #[test]
    fn size_constraint_resolve() {
        assert_eq!(
            SizeConstraint::Pixels(Pixels::new(200))
                .resolve(Length::new(1000.0), 1.0)
                .value(),
            200.0
        );
        // On macOS (Unit = Logical), to_unit is identity so scale does not affect
        // Pixels. On Windows (Unit = Physical), scale multiplies through.
        #[cfg(target_os = "windows")]
        assert_eq!(
            SizeConstraint::Pixels(Pixels::new(200))
                .resolve(Length::new(1000.0), 1.5)
                .value(),
            300.0
        );
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            SizeConstraint::Pixels(Pixels::new(200))
                .resolve(Length::new(1000.0), 1.5)
                .value(),
            200.0
        );
        assert_eq!(
            SizeConstraint::Percent(10.0)
                .resolve(Length::new(1000.0), 1.0)
                .value(),
            100.0
        );
        assert_eq!(
            SizeConstraint::Percent(10.0)
                .resolve(Length::new(1000.0), 2.0)
                .value(),
            100.0
        );
        assert_eq!(
            SizeConstraint::Percent(5.0)
                .resolve(Length::new(1920.0), 1.0)
                .value(),
            96.0
        );
    }

    #[test]
    fn layout_validates_min_le_max() {
        assert!(
            config_from("return { minimum_width = 200, maximum_width = 100 }")
                .validate_layout()
                .is_err()
        );
        assert!(
            config_from("return { minimum_height = 200, maximum_height = 100 }")
                .validate_layout()
                .is_err()
        );
        assert!(
            config_from("return { minimum_width = 200, maximum_width = 0 }")
                .validate_layout()
                .is_ok()
        );
    }

    #[test]
    fn start_at_login_defaults_to_false() {
        assert!(!config_from("return {}").start_at_login);
    }

    #[test]
    fn start_at_login_parses_true() {
        assert!(config_from("return { start_at_login = true }").start_at_login);
    }

    #[test]
    fn theme_deserializes() {
        assert_eq!(
            config_from(r#"return { theme = "latte" }"#).theme,
            Flavor::Latte
        );
    }

    #[test]
    fn font_missing_is_default() {
        assert_eq!(
            config_from("return {}").font,
            crate::font::FontConfig::default()
        );
    }

    #[test]
    fn font_deserializes_via_config() {
        let config = config_from("return { font = { text_size = 18.0 } }");
        assert_eq!(config.font.text_size, 18.0);
    }

    #[test]
    fn config_theme_method_returns_correct_theme() {
        use crate::theme::Theme;
        let config = Config {
            theme: Flavor::Latte,
            ..Config::default()
        };
        assert_eq!(
            config.theme().focused_border,
            Theme::from_flavor(Flavor::Latte).focused_border
        );
    }

    #[test]
    fn dome_os_is_available_for_branching() {
        let (present, absent) = if cfg!(target_os = "macos") {
            ("meta+h", "meta+l")
        } else {
            ("meta+l", "meta+h")
        };
        let config = config_from(
            r#"local key = dome.os == "macos" and "meta+h" or "meta+l"
return { keymaps = { [key] = "focus left" } }"#,
        );
        assert!(
            config
                .keymaps
                .default
                .contains_key(&present.parse::<Keymap>().unwrap())
        );
        assert!(
            !config
                .keymaps
                .default
                .contains_key(&absent.parse::<Keymap>().unwrap())
        );
    }

    #[test]
    fn config_load_errors_on_invalid_value_then_falls_back() {
        let path = temp_lua_path("bad_value");
        std::fs::write(&path, "return { border_size = 9.5 }\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        assert!(Config::load(path.to_str().unwrap()).is_err());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.border_size, default_border_size());
    }

    #[test]
    fn zero_tab_bar_height_falls_back_to_default() {
        let config = config_from("return { partition_tree = { tab_bar_height = 0 } }");
        assert_eq!(
            config.partition_tree.tab_bar_height,
            default_tab_bar_height()
        );
    }

    #[test]
    fn master_ratio_out_of_range_falls_back_to_default() {
        let config = config_from("return { master = { master_ratio = 1.5, master_count = 3 } }");
        assert_eq!(config.master.master_ratio, default_master_ratio());
        assert_eq!(config.master.master_count, 3);
    }

    #[test]
    fn font_family_blank_falls_back_to_default() {
        let config = config_from(r#"return { font = { family = "   ", text_size = 18.0 } }"#);
        assert_eq!(config.font.family, None);
        assert_eq!(config.font.text_size, 18.0);
    }

    #[test]
    fn load_or_default_returns_defaults_when_path_missing() {
        let path = temp_lua_path("does_not_exist");
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.log_level.as_str(), "info");
        assert!(!config.start_at_login);
    }

    #[test]
    fn load_or_default_returns_parsed_config_on_valid_lua() {
        let path = temp_lua_path("valid");
        std::fs::write(
            &path,
            r#"return { log_level = "debug", start_at_login = true }"#,
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.log_level.as_str(), "debug");
        assert!(config.start_at_login);
    }

    #[test]
    fn load_or_default_returns_defaults_on_malformed_lua() {
        let path = temp_lua_path("malformed");
        std::fs::write(&path, "this is = = not valid lua\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.log_level.as_str(), "info");
    }

    #[test]
    fn config_must_return_a_table() {
        assert!(try_config("return 42").is_err());
        assert!(try_config("local x = 1").is_err());
    }

    #[test]
    fn modal_keymaps_empty_modes() {
        let config = config_from(r#"return { keymaps = { ["meta+h"] = "focus left" } }"#);
        assert!(config.keymaps.modes.is_empty());
        let keymap = "meta+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&keymap));
    }

    #[test]
    fn keymap_list_value_parses() {
        let config = config_from(r#"return { keymaps = { ["meta+h"] = { "focus left" } } }"#);
        let keymap = "meta+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&keymap));
    }

    #[test]
    fn keymap_function_value_becomes_callback() {
        let (config, registry) =
            config_and_registry_from(r#"return { keymaps = { ["meta+h"] = function() end } }"#);
        let keymap = "meta+h".parse::<Keymap>().unwrap();
        assert!(matches!(
            config.keymaps.default.get(&keymap),
            Some(Binding::Callback(_))
        ));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn keymap_string_and_list_values_are_static() {
        let (config, registry) = config_and_registry_from(
            r#"return { keymaps = { ["meta+h"] = "focus left", ["meta+j"] = { "focus down" } } }"#,
        );
        let h = "meta+h".parse::<Keymap>().unwrap();
        let j = "meta+j".parse::<Keymap>().unwrap();
        assert!(matches!(
            config.keymaps.default.get(&h),
            Some(Binding::Static(_))
        ));
        assert!(matches!(
            config.keymaps.default.get(&j),
            Some(Binding::Static(_))
        ));
        assert!(registry.is_empty());
    }

    #[test]
    fn dome_defaults_returns_the_default_keymaps() {
        let config = config_from("return dome.defaults()");
        assert_eq!(config.keymaps.default.len(), 44);
        let meta_h = "meta+h".parse::<Keymap>().unwrap();
        assert!(matches!(
            config.keymaps.default.get(&meta_h),
            Some(Binding::Static(_))
        ));
    }

    #[test]
    fn dome_defaults_override_keeps_defaults_and_adds_a_binding() {
        let config = config_from(
            r#"local c = dome.defaults()
c.keymaps["meta+x"] = "close"
return c"#,
        );
        let meta_h = "meta+h".parse::<Keymap>().unwrap();
        let meta_x = "meta+x".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&meta_h));
        assert!(config.keymaps.default.contains_key(&meta_x));
    }

    #[test]
    fn keymap_parses_meta_modifier() {
        let key: Keymap = "meta+t".parse().unwrap();
        assert_eq!(key.modifiers, Modifiers::META);
    }

    #[test]
    fn keymap_accepts_cmd_and_win_aliases() {
        let cmd: Keymap = "cmd+t".parse().unwrap();
        assert_eq!(cmd.modifiers, Modifiers::META);
        let win: Keymap = "win+t".parse().unwrap();
        assert_eq!(win.modifiers, Modifiers::META);
        let mixed: Keymap = "cmd+shift+t".parse().unwrap();
        assert_eq!(mixed.modifiers, Modifiers::META | Modifiers::SHIFT);
    }

    #[test]
    fn modal_keymaps_with_mode() {
        let config = config_from(
            r#"return {
  keymaps = {
    ["meta+h"] = "focus left",
    mode = {
      resize = {
        ["h"] = "focus left",
        ["escape"] = "mode default",
      },
    },
  },
}"#,
        );
        let meta_h = "meta+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&meta_h));
        let resize = config
            .keymaps
            .modes
            .get("resize")
            .expect("resize mode missing");
        let h = "h".parse::<Keymap>().unwrap();
        assert!(resize.contains_key(&h));
        let esc = "escape".parse::<Keymap>().unwrap();
        assert!(resize.contains_key(&esc));
    }

    #[test]
    fn modal_keymaps_drops_default_mode_name() {
        let config = config_from(
            r#"return {
  keymaps = {
    ["meta+h"] = "focus left",
    mode = { default = { ["h"] = "focus left" } },
  },
}"#,
        );
        let meta_h = "meta+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&meta_h));
        assert!(!config.keymaps.modes.contains_key("default"));
    }

    #[test]
    fn modal_keymaps_drops_empty_mode_name() {
        let config = config_from(
            r#"return {
  keymaps = {
    ["meta+h"] = "focus left",
    mode = { [""] = { ["h"] = "focus left" } },
  },
}"#,
        );
        let meta_h = "meta+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&meta_h));
        assert!(!config.keymaps.modes.contains_key(""));
    }

    #[test]
    fn load_drops_single_bad_keymap_binding() {
        let config = config_from(
            r#"return { keymaps = { ["meta+a"] = "focus left", ["unkmod+h"] = "focus left" } }"#,
        );
        let good = "meta+a".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&good));
        assert_eq!(config.keymaps.default.len(), 1);
    }

    #[test]
    fn load_drops_single_bad_action_in_binding() {
        let config = config_from(
            r#"return { keymaps = { ["meta+a"] = "fly to mars", ["meta+b"] = "focus left" } }"#,
        );
        let b = "meta+b".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&b));
        let a = "meta+a".parse::<Keymap>().unwrap();
        assert!(!config.keymaps.default.contains_key(&a));
    }

    #[test]
    fn example_config_parses() {
        let path = format!("{}/examples/config.lua", env!("CARGO_MANIFEST_DIR"));
        let config = Config::load(&path).expect("example config failed to load");
        assert_eq!(
            config.size_constraints.minimum_width,
            SizeConstraint::Pixels(Pixels::new(200))
        );
    }

    #[test]
    fn example_layout_parses() {
        let path = format!("{}/examples/layout.jsonc", env!("CARGO_MANIFEST_DIR"));
        let layout = LayoutConfig::load(&path).expect("example layout failed to load");
        assert_eq!(layout.workspace.len(), 2);
    }

    #[test]
    fn partition_tree_config_parses_fields() {
        let config = config_from(
            "return { partition_tree = { tab_bar_height = 30.0, automatic_tiling = false } }",
        );
        assert_eq!(config.partition_tree.tab_bar_height.value(), 30);
        assert!(!config.partition_tree.automatic_tiling);
    }

    #[test]
    fn partition_tree_config_defaults() {
        let config = config_from("return {}");
        assert_eq!(config.partition_tree.tab_bar_height.value(), 24);
        assert!(config.partition_tree.automatic_tiling);
    }

    #[test]
    fn layout_defaults_to_partition_tree() {
        let config = config_from("return {}");
        assert_eq!(config.strategy, Strategy::PartitionTree);
        assert_eq!(config.master.master_ratio, 0.5);
        assert_eq!(config.master.master_count, 1);
    }

    #[test]
    fn layout_parses_master_strategy() {
        let config = config_from(r#"return { strategy = "master" }"#);
        assert_eq!(config.strategy, Strategy::Master);
        assert_eq!(config.partition_tree.tab_bar_height.value(), 24);
        assert_eq!(config.master.master_ratio, 0.5);
    }

    #[test]
    fn layout_parses_master_params() {
        let config = config_from("return { master = { master_ratio = 0.3, master_count = 2 } }");
        assert_eq!(config.master.master_ratio, 0.3);
        assert_eq!(config.master.master_count, 2);
    }

    #[test]
    fn config_rejects_unknown_strategy() {
        assert!(try_config(r#"return { strategy = "floating" }"#).is_err());
    }

    #[test]
    fn config_load_parses_root_schema() {
        let config = config_from(
            r#"return {
  strategy = "master",
  partition_tree = { tab_bar_height = 32.0 },
  master = { master_ratio = 0.6, master_count = 2 },
}"#,
        );
        assert_eq!(config.strategy, Strategy::Master);
        assert_eq!(config.partition_tree.tab_bar_height.value(), 32);
        assert_eq!(config.master.master_ratio, 0.6);
        assert_eq!(config.master.master_count, 2);
    }

    #[test]
    fn config_parses_size_constraints() {
        let config = config_from(
            r#"return { minimum_width = 200, maximum_width = "50%", minimum_height = 100, maximum_height = 0 }"#,
        );
        assert_eq!(
            config.size_constraints.minimum_width,
            SizeConstraint::Pixels(Pixels::new(200))
        );
        assert_eq!(
            config.size_constraints.maximum_width,
            SizeConstraint::Percent(50.0)
        );
        assert_eq!(
            config.size_constraints.minimum_height,
            SizeConstraint::Pixels(Pixels::new(100))
        );
        assert_eq!(
            config.size_constraints.maximum_height,
            SizeConstraint::Pixels(Pixels::new(0))
        );
    }

    #[test]
    fn config_load_falls_back_when_validate_fails() {
        let path = temp_lua_path("validate_fail");
        std::fs::write(
            &path,
            "return { minimum_width = 100, maximum_width = 50 }\n",
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        assert!(Config::load(path.to_str().unwrap()).is_err());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.strategy, Config::default().strategy);
        assert_eq!(config.partition_tree, Config::default().partition_tree);
        assert_eq!(config.master, Config::default().master);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn macos_ignore_defaults() {
        let rules = default_ignore();
        assert_eq!(rules.len(), 4);
        assert!(
            rules
                .iter()
                .any(|r| r.bundle_id.as_deref() == Some("com.apple.dock"))
        );
        let config = config_from("return {}");
        assert!(
            config
                .ignore
                .iter()
                .any(|r| r.bundle_id.as_deref() == Some("com.apple.dock"))
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_ignore_defaults() {
        let rules = default_ignore();
        assert_eq!(rules.len(), 15);
        assert!(
            rules
                .iter()
                .any(|r| r.class.as_deref() == Some("Shell_TrayWnd"))
        );
        let config = config_from("return {}");
        assert!(
            config
                .ignore
                .iter()
                .any(|r| r.class.as_deref() == Some("Shell_TrayWnd"))
        );
        let core_window = config
            .ignore
            .iter()
            .find(|r| r.class.as_deref() == Some("Windows.UI.Core.CoreWindow"))
            .expect("CoreWindow ignore rule present in merged config");
        assert!(core_window.title.is_none());
        assert!(core_window.aumid.is_none());
    }

    #[test]
    fn layout_load_or_default_returns_defaults_when_missing() {
        let path = temp_lua_path("layout_missing");
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.strategy, default_strategy());
        assert_eq!(config.partition_tree, default_partition_tree_config());
        assert_eq!(config.master, default_master_config());
    }

    #[test]
    fn layout_load_or_default_returns_defaults_on_malformed() {
        let path = temp_jsonc_path("layout_malformed");
        std::fs::write(&path, "this is not valid json {{{\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), LayoutConfig::load);
        assert!(layout.workspace.is_empty());
    }

    #[test]
    fn preferred_layout_default_empty() {
        assert!(layout_from("{}").workspace.is_empty());
    }

    #[test]
    fn preferred_layout_parse_single_entry() {
        let layout = layout_from(r#"{ "workspace": [ { "name": "1", "strategy": "master" } ] }"#);
        assert_eq!(layout.workspace.len(), 1);
        assert_eq!(layout.workspace[0].name(), "1");
        assert!(matches!(
            layout.workspace[0],
            LayoutWorkspaceConfig::Master { .. }
        ));
    }

    #[test]
    fn preferred_layout_parse_multiple_distinct() {
        let layout = layout_from(
            r#"{ "workspace": [
  { "name": "1", "strategy": "master" },
  { "name": "scratch", "strategy": "partition_tree" }
] }"#,
        );
        assert_eq!(layout.workspace.len(), 2);
        assert_eq!(layout.workspace[0].name(), "1");
        assert!(matches!(
            layout.workspace[0],
            LayoutWorkspaceConfig::Master { .. }
        ));
        assert_eq!(layout.workspace[1].name(), "scratch");
        assert!(matches!(
            layout.workspace[1],
            LayoutWorkspaceConfig::PartitionTree { .. }
        ));
    }

    #[test]
    fn preferred_layout_rejects_unknown_strategy() {
        assert!(
            LayoutConfig::from_jsonc_src(
                "test",
                r#"{ "workspace": [ { "name": "bad", "strategy": "floating" } ] }"#,
            )
            .is_err()
        );
    }

    #[test]
    fn preferred_layout_drop_empty_name() {
        let layout = layout_from(
            r#"{ "workspace": [
  { "name": "", "strategy": "master" },
  { "name": "valid", "strategy": "partition_tree" }
] }"#,
        );
        assert_eq!(layout.workspace.len(), 1);
        assert_eq!(layout.workspace[0].name(), "valid");
    }

    #[test]
    fn preferred_layout_dedup_last_wins() {
        let layout = layout_from(
            r#"{ "workspace": [
  { "name": "1", "strategy": "partition_tree" },
  { "name": "1", "strategy": "master" }
] }"#,
        );
        assert_eq!(layout.workspace.len(), 1);
        assert_eq!(layout.workspace[0].name(), "1");
        assert!(matches!(
            layout.workspace[0],
            LayoutWorkspaceConfig::Master { .. }
        ));
    }

    #[test]
    fn tree_leaf_parses() {
        let ws = workspace_from(
            r#"{ "name": "dev", "strategy": "partition_tree", "tree": { "process": "editor.exe" } }"#,
        );
        assert_eq!(ws.name(), "dev");
        match ws {
            LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
                assert!(matches!(tree, Some(TreeLayoutNode::Leaf(..))));
            }
            _ => panic!("expected PartitionTree variant"),
        }
    }

    #[test]
    fn tree_array_container_parses() {
        let ws = workspace_from(
            r#"{ "name": "dev", "strategy": "partition_tree", "tree": [
  { "process": "editor.exe" },
  { "process": "terminal.exe" }
] }"#,
        );
        match ws {
            LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
                let Some(TreeLayoutNode::Container { split, children }) = tree else {
                    panic!("expected Container");
                };
                assert!(split.is_none());
                assert_eq!(children.len(), 2);
                assert!(matches!(children[0], TreeLayoutNode::Leaf(..)));
                assert!(matches!(children[1], TreeLayoutNode::Leaf(..)));
            }
            _ => panic!("expected PartitionTree variant"),
        }
    }

    #[test]
    fn tree_split_container_parses() {
        let ws = workspace_from(
            r#"{ "name": "dev", "strategy": "partition_tree", "tree": {
  "split": "horizontal",
  "children": [ { "process": "a.exe" }, { "process": "b.exe" } ]
} }"#,
        );
        match ws {
            LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
                let Some(TreeLayoutNode::Container { split, children }) = tree else {
                    panic!("expected Container");
                };
                assert_eq!(split, Some(SplitMode::Horizontal));
                assert_eq!(children.len(), 2);
            }
            _ => panic!("expected PartitionTree variant"),
        }
    }

    #[test]
    fn tree_tabbed_parses() {
        let ws = workspace_from(
            r#"{ "name": "dev", "strategy": "partition_tree", "tree": {
  "split": "tabbed",
  "children": [ { "process": "browser.exe" }, { "process": "editor.exe" } ]
} }"#,
        );
        match ws {
            LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
                let Some(TreeLayoutNode::Container { split, children }) = tree else {
                    panic!("expected Container");
                };
                assert_eq!(split, Some(SplitMode::Tabbed));
                assert_eq!(children.len(), 2);
            }
            _ => panic!("expected PartitionTree variant"),
        }
    }

    #[test]
    fn tree_nested_parses() {
        let ws = workspace_from(
            r#"{ "name": "dev", "strategy": "partition_tree", "tree": {
  "split": "horizontal",
  "children": [
    { "process": "editor.exe" },
    { "split": "vertical", "children": [
      { "process": "terminal.exe" },
      { "process": "logs.exe" }
    ] }
  ]
} }"#,
        );
        match ws {
            LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
                let Some(TreeLayoutNode::Container { split, children }) = tree else {
                    panic!("expected outer Container");
                };
                assert_eq!(split, Some(SplitMode::Horizontal));
                assert_eq!(children.len(), 2);
                assert!(matches!(children[0], TreeLayoutNode::Leaf(..)));
                assert!(matches!(
                    children[1],
                    TreeLayoutNode::Container {
                        split: Some(SplitMode::Vertical),
                        ..
                    }
                ));
            }
            _ => panic!("expected PartitionTree variant"),
        }
    }

    #[test]
    fn tree_default_none() {
        let ws = workspace_from(r#"{ "name": "dev", "strategy": "partition_tree" }"#);
        match ws {
            LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
                assert!(tree.is_none());
            }
            _ => panic!("expected PartitionTree variant"),
        }
    }

    #[test]
    fn tree_invalid_split_rejected() {
        assert!(
            try_workspace(
                r#"{ "name": "dev", "strategy": "partition_tree", "tree": { "split": "diagonal", "children": [] } }"#,
            )
            .is_err()
        );
    }
}
