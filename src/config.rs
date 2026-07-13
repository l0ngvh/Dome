use anyhow::{Result, anyhow};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::action::{
    Action, Actions, FocusTarget, MonitorTarget, MoveTarget, TabDirection, ToggleTarget,
};
use crate::core::{Length, Logical, Unit};
use crate::font::{
    FontConfig, MAX_FONT_SIZE, MIN_FONT_SIZE, default_subtext_size, default_text_size,
};
use crate::theme::{Flavor, Theme};

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
                // `cmd` and `win` are platform-flavored aliases for `meta` so users
                // can write keymaps in the vocabulary of their OS without us
                // shipping a platform-conditional config schema.
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

#[derive(Debug, Clone)]
pub(crate) struct ModalKeymaps {
    pub(crate) default: HashMap<Keymap, Actions>,
    pub(crate) modes: HashMap<String, HashMap<Keymap, Actions>>,
}

fn default_keymaps() -> ModalKeymaps {
    let mut keymaps = HashMap::new();
    for i in 0..=9 {
        keymaps.insert(
            Keymap {
                key: i.to_string(),
                modifiers: Modifiers::META,
            },
            Actions::new(vec![Action::Focus(FocusTarget::Workspace {
                name: i.to_string(),
            })]),
        );
        keymaps.insert(
            Keymap {
                key: i.to_string(),
                modifiers: Modifiers::META | Modifiers::SHIFT,
            },
            Actions::new(vec![Action::Move(MoveTarget::Workspace {
                name: i.to_string(),
            })]),
        );
    }
    keymaps.insert(
        Keymap {
            key: "e".into(),
            modifiers: Modifiers::META,
        },
        Actions::new(vec![Action::Toggle(ToggleTarget::Spawn)]),
    );
    keymaps.insert(
        Keymap {
            key: "d".into(),
            modifiers: Modifiers::META,
        },
        Actions::new(vec![Action::Toggle(ToggleTarget::Direction)]),
    );
    keymaps.insert(
        Keymap {
            key: "b".into(),
            modifiers: Modifiers::META,
        },
        Actions::new(vec![Action::Toggle(ToggleTarget::Layout)]),
    );
    keymaps.insert(
        Keymap {
            key: "p".into(),
            modifiers: Modifiers::META,
        },
        Actions::new(vec![Action::Focus(FocusTarget::Parent)]),
    );
    keymaps.insert(
        Keymap {
            key: "h".into(),
            modifiers: Modifiers::META,
        },
        Actions::new(vec![Action::Focus(FocusTarget::Left)]),
    );
    keymaps.insert(
        Keymap {
            key: "j".into(),
            modifiers: Modifiers::META,
        },
        Actions::new(vec![Action::Focus(FocusTarget::Down)]),
    );
    keymaps.insert(
        Keymap {
            key: "k".into(),
            modifiers: Modifiers::META,
        },
        Actions::new(vec![Action::Focus(FocusTarget::Up)]),
    );
    keymaps.insert(
        Keymap {
            key: "l".into(),
            modifiers: Modifiers::META,
        },
        Actions::new(vec![Action::Focus(FocusTarget::Right)]),
    );
    keymaps.insert(
        Keymap {
            key: "[".into(),
            modifiers: Modifiers::META,
        },
        Actions::new(vec![Action::Focus(FocusTarget::Tab {
            direction: TabDirection::Prev,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "]".into(),
            modifiers: Modifiers::META,
        },
        Actions::new(vec![Action::Focus(FocusTarget::Tab {
            direction: TabDirection::Next,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "h".into(),
            modifiers: Modifiers::META | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Move(MoveTarget::Left)]),
    );
    keymaps.insert(
        Keymap {
            key: "j".into(),
            modifiers: Modifiers::META | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Move(MoveTarget::Down)]),
    );
    keymaps.insert(
        Keymap {
            key: "k".into(),
            modifiers: Modifiers::META | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Move(MoveTarget::Up)]),
    );
    keymaps.insert(
        Keymap {
            key: "l".into(),
            modifiers: Modifiers::META | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Move(MoveTarget::Right)]),
    );
    keymaps.insert(
        Keymap {
            key: "f".into(),
            modifiers: Modifiers::META | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Toggle(ToggleTarget::Float)]),
    );
    keymaps.insert(
        Keymap {
            key: "q".into(),
            modifiers: Modifiers::META | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Exit]),
    );
    // Monitor focus: Meta+Alt+hjkl
    for (key, target) in [
        ("h", MonitorTarget::Left),
        ("j", MonitorTarget::Down),
        ("k", MonitorTarget::Up),
        ("l", MonitorTarget::Right),
    ] {
        keymaps.insert(
            Keymap {
                key: key.into(),
                modifiers: Modifiers::META | Modifiers::ALT,
            },
            Actions::new(vec![Action::Focus(FocusTarget::Monitor {
                target: target.clone(),
            })]),
        );
        keymaps.insert(
            Keymap {
                key: key.into(),
                modifiers: Modifiers::META | Modifiers::ALT | Modifiers::SHIFT,
            },
            Actions::new(vec![Action::Move(MoveTarget::Monitor { target })]),
        );
    }
    ModalKeymaps {
        default: keymaps,
        modes: HashMap::new(),
    }
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

pub(crate) trait WalkRecover: Sized {
    fn walk(w: &mut Walker) -> Self;
}

trait WalkRule: serde::de::DeserializeOwned {
    const KNOWN: &'static [&'static str];
}

pub(crate) struct Walker<'a> {
    table: &'a mut toml::Table,
    prefix: String,
}

impl<'a> Walker<'a> {
    fn new(table: &'a mut toml::Table, prefix: impl Into<String>) -> Self {
        Self {
            table,
            prefix: prefix.into(),
        }
    }

    // Default-on-error policy: this is the single site where Walker substitutes
    // a typed default for a user-supplied field. Reaching the default branch
    // always follows a tracing::warn! that explains what failed. The alternative
    // is wiping the user's whole config. This is the explicit AGENTS.md exception
    // for Default::default() and unwrap_or_default() inside the walker.
    fn field<T: serde::de::DeserializeOwned>(&mut self, name: &str, default: T) -> T {
        let Some(value) = self.table.remove(name) else {
            return default;
        };
        match value.try_into() {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    field = %field_path(&self.prefix, name),
                    error = %e,
                    "Invalid value, using default",
                );
                default
            }
        }
    }

    fn nested<T: WalkRecover>(&mut self, name: &str) -> T {
        let sub_prefix = field_path(&self.prefix, name);
        let mut inner = match self.table.remove(name) {
            Some(toml::Value::Table(t)) => t,
            Some(other) => {
                tracing::warn!(
                    field = %sub_prefix,
                    error = %format!("expected table, got {}", other.type_str()),
                    "Invalid value, using default",
                );
                toml::Table::new()
            }
            None => toml::Table::new(),
        };
        let mut sub = Walker::new(&mut inner, sub_prefix);
        T::walk(&mut sub)
    }

    fn nested_or<T: WalkRecover>(&mut self, name: &str, default: T) -> T {
        let sub_prefix = field_path(&self.prefix, name);
        match self.table.remove(name) {
            Some(toml::Value::Table(mut inner)) => {
                let mut sub = Walker::new(&mut inner, sub_prefix);
                T::walk(&mut sub)
            }
            Some(other) => {
                tracing::warn!(
                    field = %sub_prefix,
                    error = %format!("expected table, got {}", other.type_str()),
                    "Invalid value, using default",
                );
                default
            }
            None => default,
        }
    }

    fn drain_table(&mut self, name: &str) -> Option<toml::Table> {
        match self.table.remove(name) {
            Some(toml::Value::Table(t)) => Some(t),
            Some(other) => {
                tracing::warn!(
                    field = %field_path(&self.prefix, name),
                    error = %format!("expected table, got {}", other.type_str()),
                    "Invalid value, ignoring",
                );
                None
            }
            None => None,
        }
    }

    fn rule_vec<T: WalkRule>(&mut self, name: &str) -> Vec<T> {
        let arr = match self.table.remove(name) {
            Some(toml::Value::Array(a)) => a,
            Some(other) => {
                tracing::warn!(
                    field = %field_path(&self.prefix, name),
                    error = %format!("expected array, got {}", other.type_str()),
                    "Invalid value, using default",
                );
                return Vec::new();
            }
            None => return Vec::new(),
        };
        let mut result = Vec::new();
        for (i, elem) in arr.into_iter().enumerate() {
            let toml::Value::Table(mut elem_table) = elem else {
                tracing::warn!(
                    field = %format!("{}[{}]", field_path(&self.prefix, name), i),
                    "Expected table element, dropping",
                );
                continue;
            };
            let elem_path = format!("{}[{}]", field_path(&self.prefix, name), i);
            elem_table.retain(|key, _| {
                // key.as_str() resolves to unstable str::as_str here, not String::as_str.
                let k: &str = key;
                if T::KNOWN.contains(&k) {
                    true
                } else {
                    tracing::warn!(
                        field = %field_path(&elem_path, key),
                        "Unknown config field, ignoring",
                    );
                    false
                }
            });
            match toml::Value::Table(elem_table).try_into::<T>() {
                Ok(v) => result.push(v),
                Err(e) => {
                    tracing::warn!(
                        field = %elem_path,
                        error = %e,
                        "Invalid rule, dropping",
                    );
                }
            }
        }
        result
    }
}

impl Drop for Walker<'_> {
    fn drop(&mut self) {
        for key in self.table.keys() {
            tracing::warn!(
                field = %field_path(&self.prefix, key),
                "Unknown config field, ignoring",
            );
        }
    }
}

struct RawConfig;

impl RawConfig {
    fn into_config(mut table: toml::Table) -> Config {
        let mut w = Walker::new(&mut table, "");
        Config {
            keymaps: walk_keymaps(&mut w),
            border_size: w.field("border_size", default_border_size()),
            theme: w.field("theme", Flavor::default()),
            font: w.nested_or("font", FontConfig::default()),
            ignore: {
                let mut ignore = w.rule_vec::<WindowMatcher>("ignore");
                ignore.extend(default_ignore());
                ignore
            },
            log_level: w.field("log_level", LogLevel::default()),
            start_at_login: w.field("start_at_login", false),
            strategy: w.field("strategy", default_strategy()),
            partition_tree: w.nested::<PartitionTreeConfig>("partition_tree"),
            master: w.nested::<MasterConfig>("master"),
            min_width: w.field("min_width", SizeConstraint::default_min()),
            min_height: w.field("min_height", SizeConstraint::default_min()),
            max_width: w.field("max_width", SizeConstraint::default()),
            max_height: w.field("max_height", SizeConstraint::default()),
            float: w.rule_vec::<WindowMatcher>("float"),
            fullscreen: w.rule_vec::<WindowMatcher>("fullscreen"),
        }
    }
}

#[cfg(target_os = "macos")]
fn default_ignore() -> Vec<WindowMatcher> {
    vec![
        WindowMatcher {
            bundle_id: Some("com.apple.dock".into()),
            ..Default::default()
        },
        WindowMatcher {
            bundle_id: Some("com.apple.controlcenter".into()),
            ..Default::default()
        },
        WindowMatcher {
            bundle_id: Some("com.apple.notificationcenterui".into()),
            ..Default::default()
        },
        WindowMatcher {
            bundle_id: Some("com.apple.loginwindow".into()),
            ..Default::default()
        },
    ]
}

#[cfg(target_os = "windows")]
fn default_ignore() -> Vec<WindowMatcher> {
    vec![
        WindowMatcher {
            process: Some("LockApp.exe".into()),
            ..Default::default()
        },
        WindowMatcher {
            process: Some("SearchHost.exe".into()),
            ..Default::default()
        },
        WindowMatcher {
            process: Some("StartMenuExperienceHost.exe".into()),
            ..Default::default()
        },
        WindowMatcher {
            title: Some("MSCTFIME UI".into()),
            ..Default::default()
        },
        WindowMatcher {
            title: Some("OLEChannelWnd".into()),
            ..Default::default()
        },
        WindowMatcher {
            class: Some("Shell_TrayWnd".into()),
            ..Default::default()
        },
        WindowMatcher {
            class: Some("Shell_SecondaryTrayWnd".into()),
            ..Default::default()
        },
        WindowMatcher {
            class: Some("Progman".into()),
            ..Default::default()
        },
        WindowMatcher {
            class: Some("WorkerW".into()),
            ..Default::default()
        },
        WindowMatcher {
            class: Some("TaskListThumbnailWnd".into()),
            ..Default::default()
        },
        WindowMatcher {
            class: Some("MultitaskingViewFrame".into()),
            ..Default::default()
        },
        WindowMatcher {
            class: Some("Xaml_WindowedPopupClass".into()),
            ..Default::default()
        },
        WindowMatcher {
            class: Some("TaskManagerWindow".into()),
            ..Default::default()
        },
        WindowMatcher {
            class: Some("Windows.UI.Core.CoreWindow".into()),
            ..Default::default()
        },
    ]
}

impl WalkRecover for LayoutConfig {
    fn walk(w: &mut Walker) -> Self {
        let raw = w.rule_vec::<LayoutWorkspaceConfig>("workspace");
        let workspace = dedup_preferred_layout_config(raw, &w.prefix);
        LayoutConfig { workspace }
    }
}

impl WalkRecover for PartitionTreeConfig {
    fn walk(w: &mut Walker) -> Self {
        PartitionTreeConfig {
            tab_bar_height: w.field("tab_bar_height", default_tab_bar_height()),
            automatic_tiling: w.field("automatic_tiling", default_automatic_tiling()),
        }
    }
}

impl WalkRecover for MasterConfig {
    fn walk(w: &mut Walker) -> Self {
        let master_ratio = w.field("master_ratio", default_master_ratio());
        let master_ratio = if (0.1..=0.9).contains(&master_ratio) {
            master_ratio
        } else {
            tracing::warn!(
                field = %field_path(&w.prefix, "master_ratio"),
                value = master_ratio,
                "Out of range, using default",
            );
            default_master_ratio()
        };
        let master_count = w.field("master_count", default_master_count());
        let master_count = if master_count >= 1 {
            master_count
        } else {
            tracing::warn!(
                field = %field_path(&w.prefix, "master_count"),
                value = master_count,
                "Out of range, using default",
            );
            default_master_count()
        };
        MasterConfig {
            master_ratio,
            master_count,
        }
    }
}

impl WalkRecover for FontConfig {
    fn walk(w: &mut Walker) -> Self {
        let text_size = w.field("text_size", default_text_size());
        let text_size = if (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&text_size) {
            text_size
        } else {
            tracing::warn!(
                field = %field_path(&w.prefix, "text_size"),
                value = text_size,
                "Out of range, using default",
            );
            default_text_size()
        };
        let subtext_size = w.field("subtext_size", default_subtext_size());
        let subtext_size = if (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&subtext_size) {
            subtext_size
        } else {
            tracing::warn!(
                field = %field_path(&w.prefix, "subtext_size"),
                value = subtext_size,
                "Out of range, using default",
            );
            default_subtext_size()
        };
        let family: Option<String> = w.field("family", None);
        let family = match family {
            Some(s) if s.trim().is_empty() => {
                tracing::warn!(
                    field = %field_path(&w.prefix, "family"),
                    "Blank font family, using default",
                );
                None
            }
            other => other,
        };
        FontConfig {
            text_size,
            subtext_size,
            family,
        }
    }
}

fn walk_keymaps(w: &mut Walker) -> ModalKeymaps {
    let Some(mut keymaps_table) = w.drain_table("keymaps") else {
        return default_keymaps();
    };

    let mode_table = keymaps_table.remove("mode");

    let default = walk_bindings_table(keymaps_table, "keymaps");

    let mut modes = HashMap::new();
    if let Some(toml::Value::Table(mode_map)) = mode_table {
        for (mode_name, mode_val) in mode_map {
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
            let toml::Value::Table(bindings) = mode_val else {
                tracing::warn!(
                    field = %format!("keymaps.mode.{mode_name}"),
                    "Expected table for mode, dropping",
                );
                continue;
            };
            let prefix = format!("keymaps.mode.{mode_name}");
            let mode_bindings = walk_bindings_table(bindings, &prefix);
            modes.insert(mode_name, mode_bindings);
        }
    } else if let Some(_non_table) = mode_table {
        tracing::warn!(field = "keymaps.mode", "Expected table, ignoring",);
    }

    ModalKeymaps { default, modes }
}

fn walk_bindings_table(table: toml::Table, prefix: &str) -> HashMap<Keymap, Actions> {
    let mut result = HashMap::new();
    for (key_str, value) in table {
        let field = field_path(prefix, &key_str);
        let keymap = match key_str.parse::<Keymap>() {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!(
                    field = %field,
                    error = %e,
                    "Invalid key binding, dropping",
                );
                continue;
            }
        };
        let action_strs: Vec<String> = match value.try_into() {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    field = %field,
                    error = %e,
                    "Invalid actions value, dropping",
                );
                continue;
            }
        };
        match parse_actions(&action_strs) {
            Ok(actions) => {
                result.insert(keymap, actions);
            }
            Err(e) => {
                tracing::warn!(
                    field = %field,
                    error = %e,
                    "Invalid action, dropping binding",
                );
            }
        }
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SizeConstraint {
    Pixels(Length<Logical>),
    Percent(f32),
}

impl Default for SizeConstraint {
    fn default() -> Self {
        SizeConstraint::Pixels(Length::new(0.0))
    }
}

impl SizeConstraint {
    /// Resolves to a frame-unit length.
    ///
    /// `Pixels` is a config-denominated absolute length (logical), so it goes
    /// through `to_unit(scale)` to reach the frame unit. `Percent` is a ratio
    /// of `screen_size` (already in frame units), so `scale` does not apply --
    /// the result is wrapped directly as `Length<Unit>`.
    pub(crate) fn resolve(&self, screen_size: Length<Unit>, scale: f32) -> Length<Unit> {
        match self {
            SizeConstraint::Pixels(px) => px.to_unit(scale),
            // screen_size is already in Unit space (monitor frame dimension),
            // so the result is directly Length<Unit> — no logical-to-unit conversion needed.
            SizeConstraint::Percent(pct) => screen_size * (pct / 100.0),
        }
    }

    pub(crate) fn default_min() -> Self {
        SizeConstraint::Percent(5.0)
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
                formatter.write_str("a float for pixels or a string percentage (e.g., \"10%\")")
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                let val = v as f32;
                if val < 0.0 {
                    return Err(E::custom("pixel value must be non-negative"));
                }
                Ok(SizeConstraint::Pixels(Length::new(val)))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Strategy {
    PartitionTree,
    Master,
}

/// Per-strategy config for the partition-tree strategy.
///
/// All fields are read fresh from `hub.config.layout.partition_tree` by the
/// strategy on every layout pass (see `src/core/partition_tree/layout.rs`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct PartitionTreeConfig {
    #[serde(default = "default_tab_bar_height")]
    pub(crate) tab_bar_height: Length<Logical>,
    #[serde(default = "default_automatic_tiling")]
    pub(crate) automatic_tiling: bool,
}

/// Per-strategy config for the master-stack strategy.
///
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
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

impl WalkRule for WindowMatcher {
    const KNOWN: &'static [&'static str] =
        &["app", "bundle_id", "title", "process", "class", "aumid"];
}

/// Split mode for a `TreeLayoutNode::Container`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SplitMode {
    Horizontal,
    Vertical,
    Tabbed,
}

/// A node in the preferred tree layout for partition-tree workspaces.
/// Either a single window matcher (leaf) or a container with nested children.
///
/// TOML shapes:
///   Leaf:            `{ process = "editor.exe" }`
///   Array container:  `[{ process = "a" }, { process = "b" }]`
///   Split container:  `{ split = "horizontal", children = [...] }`
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TreeLayoutNode {
    Leaf(WindowMatcher),
    Container {
        /// `None` means the split mode is unspecified — the runtime
        /// will pick based on context when materializing the tree.
        split: Option<SplitMode>,
        children: Vec<TreeLayoutNode>,
    },
}

/// Helper struct for deserializing the container variant of `TreeLayoutNode`.
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

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub(crate) struct LayoutConfig {
    #[serde(default)]
    pub(crate) workspace: Vec<LayoutWorkspaceConfig>,
}

fn default_strategy() -> Strategy {
    Strategy::PartitionTree
}
fn default_automatic_tiling() -> bool {
    true
}
fn default_master_ratio() -> f32 {
    0.5
}
fn default_master_count() -> usize {
    1
}
fn default_partition_tree_config() -> PartitionTreeConfig {
    PartitionTreeConfig {
        tab_bar_height: default_tab_bar_height(),
        automatic_tiling: default_automatic_tiling(),
    }
}
fn default_master_config() -> MasterConfig {
    MasterConfig {
        master_ratio: default_master_ratio(),
        master_count: default_master_count(),
    }
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
        let layout = load_toml::<LayoutConfig>(path)?;
        Ok(layout)
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
        master: Vec<WindowMatcher>,
        #[serde(default)]
        secondary: Vec<WindowMatcher>,
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

impl WalkRule for LayoutWorkspaceConfig {
    const KNOWN: &'static [&'static str] = &[
        "name",
        "strategy",
        "master_ratio",
        "master_count",
        "master",
        "secondary",
        "float",
        "fullscreen",
        "tree",
    ];
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

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Config {
    #[serde(skip_deserializing, default = "default_keymaps")]
    pub(crate) keymaps: ModalKeymaps,
    #[serde(default = "default_border_size")]
    pub(crate) border_size: f32,
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
    #[serde(default = "SizeConstraint::default_min")]
    pub(crate) min_width: SizeConstraint,
    #[serde(default = "SizeConstraint::default_min")]
    pub(crate) min_height: SizeConstraint,
    #[serde(default)]
    pub(crate) max_width: SizeConstraint,
    #[serde(default)]
    pub(crate) max_height: SizeConstraint,
    #[serde(default)]
    pub(crate) float: Vec<WindowMatcher>,
    #[serde(default)]
    pub(crate) fullscreen: Vec<WindowMatcher>,
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

fn default_border_size() -> f32 {
    4.0
}

fn default_tab_bar_height() -> Length<Logical> {
    Length::new(24.0)
}

impl Default for Config {
    fn default() -> Self {
        Config {
            keymaps: default_keymaps(),
            border_size: default_border_size(),
            // Mocha is the darkest flavour and matches Dome's pre-theme default palette.
            theme: Flavor::default(),
            font: FontConfig::default(),
            ignore: default_ignore(),
            log_level: LogLevel::default(),
            start_at_login: false,
            strategy: default_strategy(),
            partition_tree: default_partition_tree_config(),
            master: default_master_config(),
            min_width: SizeConstraint::default_min(),
            min_height: SizeConstraint::default_min(),
            max_width: SizeConstraint::default(),
            max_height: SizeConstraint::default(),
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
        format!("{config_dir}\\dome\\config.toml")
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
        format!("{config_dir}/dome/config.toml")
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
        // Validation compares config values in logical space directly. No scale
        // factor exists at validation time, so `.logical()` is the correct escape
        // hatch here (not `to_unit`).
        if let (SizeConstraint::Pixels(min), SizeConstraint::Pixels(max)) =
            (self.min_width, self.max_width)
            && max.logical() > 0.0
            && min > max
        {
            anyhow::bail!("min_width ({min}) cannot be greater than max_width ({max})");
        }
        if let (SizeConstraint::Pixels(min), SizeConstraint::Pixels(max)) =
            (self.min_height, self.max_height)
            && max.logical() > 0.0
            && min > max
        {
            anyhow::bail!("min_height ({min}) cannot be greater than max_height ({max})");
        }
        Ok(())
    }

    pub(crate) fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let table: toml::Table = toml::from_str(&content)?;
        let config = RawConfig::into_config(table);
        config.validate_layout()?;
        Ok(config)
    }
}

fn load_toml<T: WalkRecover>(path: &str) -> anyhow::Result<T> {
    let content = std::fs::read_to_string(path)?;
    let mut table: toml::Table = toml::from_str(&content)?;
    let mut w = Walker::new(&mut table, "");
    Ok(T::walk(&mut w))
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
        .join("layout.toml")
}

pub(crate) fn start_config_watcher<T: Send + 'static>(
    path: &str,
    load_fn: impl Fn(&str) -> anyhow::Result<T> + Send + 'static,
    on_change: impl Fn(T) + Send + 'static,
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
            match load_fn(target.to_str().unwrap()) {
                Ok(v) => {
                    tracing::info!(path = %target.display(), "File reloaded");
                    on_change(v);
                }
                Err(e) => tracing::warn!(path = %target.display(), error = %e, "Failed to reload"),
            }
        }
    })?;
    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;
    tracing::info!(%path, "File watcher started");
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_size_default() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.min_width, SizeConstraint::Percent(5.0));
        assert_eq!(config.min_height, SizeConstraint::Percent(5.0));
    }

    #[test]
    fn max_size_default() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.max_width, SizeConstraint::Pixels(Length::new(0.0)));
        assert_eq!(config.max_height, SizeConstraint::Pixels(Length::new(0.0)));
    }

    #[test]
    fn size_constraint_parses_float_as_pixels() {
        let config: Config = toml::from_str("min_width = 200.0").unwrap();
        assert_eq!(config.min_width, SizeConstraint::Pixels(Length::new(200.0)));
    }

    #[test]
    fn size_constraint_parses_int_as_pixels() {
        let config: Config = toml::from_str("min_width = 200").unwrap();
        assert_eq!(config.min_width, SizeConstraint::Pixels(Length::new(200.0)));
    }

    #[test]
    fn size_constraint_parses_string_percent() {
        let config: Config = toml::from_str(r#"min_width = "10%""#).unwrap();
        assert_eq!(config.min_width, SizeConstraint::Percent(10.0));
    }

    #[test]
    fn size_constraint_rejects_invalid_percent() {
        assert!(toml::from_str::<Config>(r#"min_width = "101%""#).is_err());
        assert!(toml::from_str::<Config>(r#"min_width = "-5%""#).is_err());
    }

    #[test]
    fn size_constraint_rejects_negative_pixels() {
        assert!(toml::from_str::<Config>("min_width = -100").is_err());
    }

    #[test]
    fn size_constraint_rejects_string_without_percent() {
        assert!(toml::from_str::<Config>(r#"min_width = "200""#).is_err());
    }

    #[test]
    fn size_constraint_resolve() {
        assert_eq!(
            SizeConstraint::Pixels(Length::new(200.0))
                .resolve(Length::new(1000.0), 1.0)
                .value(),
            200.0
        );
        // On macOS (Unit = Logical), to_unit is identity so scale doesn't affect Pixels.
        // On Windows (Unit = Physical), Pixels(200) * scale 1.5 = 300.
        #[cfg(target_os = "windows")]
        assert_eq!(
            SizeConstraint::Pixels(Length::new(200.0))
                .resolve(Length::new(1000.0), 1.5)
                .value(),
            300.0
        );
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            SizeConstraint::Pixels(Length::new(200.0))
                .resolve(Length::new(1000.0), 1.5)
                .value(),
            200.0
        );
        // scale must not affect Percent (screen_size is already in Unit space)
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
        let config: Config = toml::from_str("min_width = 200\nmax_width = 100").unwrap();
        assert!(config.validate_layout().is_err());

        let config: Config = toml::from_str("min_height = 200\nmax_height = 100").unwrap();
        assert!(config.validate_layout().is_err());

        let config: Config = toml::from_str("min_width = 200\nmax_width = 0").unwrap();
        assert!(config.validate_layout().is_ok());
    }

    #[test]
    fn start_at_login_defaults_to_false() {
        let config: Config = toml::from_str("").unwrap();
        assert!(!config.start_at_login);
    }

    #[test]
    fn start_at_login_parses_true() {
        let config: Config = toml::from_str("start_at_login = true").unwrap();
        assert!(config.start_at_login);
    }

    #[test]
    fn theme_deserializes() {
        let config: Config = toml::from_str(r#"theme = "latte""#).unwrap();
        assert_eq!(config.theme, Flavor::Latte);
    }

    #[test]
    fn font_missing_is_default() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.font, crate::font::FontConfig::default());
    }

    #[test]
    fn font_deserializes_via_config() {
        let config: Config =
            toml::from_str("[font]\ntext_size = 18.0\nsubtext_size = 15.0").unwrap();
        assert_eq!(config.font.text_size, 18.0);
        assert_eq!(config.font.subtext_size, 15.0);
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
    fn removed_color_field_rejected() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_color_field_{nanos}.toml"));
        std::fs::write(&path, "focused_color = \"#ff0000\"\ntheme = \"latte\"\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.theme, Flavor::Latte);
    }

    #[test]
    fn removed_border_radius_rejected() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_border_radius_{nanos}.toml"));
        std::fs::write(&path, "border_radius = 4\nborder_size = 5.0\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.border_size, 5.0);
    }

    #[test]
    fn removed_top_level_layout_fields_rejected() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_top_layout_{nanos}.toml"));
        std::fs::write(
            &path,
            "tab_bar_height = 30\nautomatic_tiling = true\nborder_size = 5.0\n",
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.border_size, 5.0);
    }

    #[test]
    fn load_or_default_returns_defaults_when_path_missing() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_does_not_exist_{nanos}.toml"));
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.log_level.as_str(), "info");
        assert!(!config.start_at_login);
    }

    #[test]
    fn load_or_default_returns_parsed_config_on_valid_toml() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_valid_{nanos}.toml"));
        std::fs::write(&path, "log_level = \"debug\"\nstart_at_login = true\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.log_level.as_str(), "debug");
        assert!(config.start_at_login);
    }

    #[test]
    fn load_or_default_returns_defaults_on_malformed_toml() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_malformed_{nanos}.toml"));
        std::fs::write(&path, "this is = = not valid toml\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.log_level.as_str(), "info");
    }

    #[test]
    fn modal_keymaps_empty_modes() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_keymaps_empty_{nanos}.toml"));
        std::fs::write(&path, "[keymaps]\n\"meta+h\" = [\"focus left\"]\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert!(config.keymaps.modes.is_empty());
        let keymap = "meta+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&keymap));
    }

    #[test]
    fn keymap_parses_meta_modifier() {
        let key: Keymap = "meta+t".parse().unwrap();
        assert_eq!(key.modifiers, Modifiers::META);
    }

    #[test]
    fn keymap_accepts_cmd_and_win_aliases() {
        // `cmd` (macOS) and `win` (Windows) are aliases for `meta` so users can
        // write keymaps in the vocabulary of their OS.
        let cmd: Keymap = "cmd+t".parse().unwrap();
        assert_eq!(cmd.modifiers, Modifiers::META);
        let win: Keymap = "win+t".parse().unwrap();
        assert_eq!(win.modifiers, Modifiers::META);
        let mixed: Keymap = "cmd+shift+t".parse().unwrap();
        assert_eq!(mixed.modifiers, Modifiers::META | Modifiers::SHIFT);
    }

    #[test]
    fn modal_keymaps_with_mode() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_keymaps_mode_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[keymaps]\n",
                "\"meta+h\" = [\"focus left\"]\n",
                "\n",
                "[keymaps.mode.resize]\n",
                "\"h\" = [\"focus left\"]\n",
                "\"escape\" = [\"mode default\"]\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
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
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_keymaps_default_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[keymaps]\n",
                "\"meta+h\" = [\"focus left\"]\n",
                "\n",
                "[keymaps.mode.default]\n",
                "\"h\" = [\"focus left\"]\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        let meta_h = "meta+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&meta_h));
        assert!(!config.keymaps.modes.contains_key("default"));
    }

    #[test]
    fn modal_keymaps_drops_empty_mode_name() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("dome_config_keymaps_empty_mode_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[keymaps]\n",
                "\"meta+h\" = [\"focus left\"]\n",
                "\n",
                "[keymaps.mode.\"\"]\n",
                "\"h\" = [\"focus left\"]\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        let meta_h = "meta+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&meta_h));
        assert!(!config.keymaps.modes.contains_key(""));
    }

    #[test]
    fn example_config_parses() {
        let path = format!("{}/examples/config.toml", env!("CARGO_MANIFEST_DIR"));
        Config::load(&path).expect("example config failed to load");
    }

    #[test]
    fn example_layout_parses() {
        let path = format!("{}/examples/layout.toml", env!("CARGO_MANIFEST_DIR"));
        LayoutConfig::load(&path).expect("example layout failed to load");
    }

    /// RAII guard that removes a temp file on drop, even if the test panics.
    struct CleanupFile(std::path::PathBuf);
    impl Drop for CleanupFile {
        fn drop(&mut self) {
            // Best-effort cleanup of test temp file; nothing to do if it fails.
            std::fs::remove_file(&self.0).ok();
        }
    }

    #[test]
    fn partition_tree_config_parses_fields() {
        let config: Config =
            toml::from_str("[partition_tree]\ntab_bar_height = 30.0\nautomatic_tiling = false")
                .unwrap();
        assert_eq!(config.partition_tree.tab_bar_height.logical(), 30.0);
        assert!(!config.partition_tree.automatic_tiling);
    }

    #[test]
    fn partition_tree_config_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.partition_tree.tab_bar_height.logical(), 24.0);
        assert!(config.partition_tree.automatic_tiling);
    }

    #[test]
    fn layout_defaults_to_partition_tree() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.strategy, Strategy::PartitionTree);
        assert_eq!(config.master.master_ratio, 0.5);
        assert_eq!(config.master.master_count, 1);
    }

    #[test]
    fn layout_parses_master_strategy() {
        let config: Config = toml::from_str("strategy = \"master\"\n").unwrap();
        assert_eq!(config.strategy, Strategy::Master);
        // Sub-tables still get their defaults
        assert_eq!(config.partition_tree.tab_bar_height.logical(), 24.0);
        assert_eq!(config.master.master_ratio, 0.5);
    }

    #[test]
    fn layout_parses_master_params() {
        let config: Config =
            toml::from_str("[master]\nmaster_ratio = 0.3\nmaster_count = 2").unwrap();
        assert_eq!(config.master.master_ratio, 0.3);
        assert_eq!(config.master.master_count, 2);
    }

    #[test]
    fn layout_rejects_unknown_strategy() {
        assert!(toml::from_str::<Config>("strategy = \"floating\"").is_err());
    }

    #[test]
    fn layout_rejects_unknown_subfield_master() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_unknown_master_{nanos}.toml"));
        std::fs::write(&path, "[master]\nfoo = 1\nmaster_ratio = 0.6\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(layout.master.master_ratio, 0.6);
    }

    #[test]
    fn layout_rejects_unknown_subfield_partition_tree() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_unknown_pt_{nanos}.toml"));
        std::fs::write(&path, "[partition_tree]\nfoo = 1\ntab_bar_height = 30.0\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(layout.partition_tree.tab_bar_height.logical(), 30.0);
    }

    #[test]
    fn load_recovers_when_top_level_scalar_has_wrong_type() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_scalar_wrong_{nanos}.toml"));
        std::fs::write(&path, "border_size = \"abc\"\ntheme = \"latte\"\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.border_size, default_border_size());
        assert_eq!(config.theme, Flavor::Latte);
    }

    #[test]
    fn load_recovers_when_inner_field_of_nested_struct_fails() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_nested_field_{nanos}.toml"));
        std::fs::write(
            &path,
            "[master]\nmaster_ratio = \"abc\"\nmaster_count = 3\n",
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(layout.master.master_ratio, default_master_ratio());
        assert_eq!(layout.master.master_count, 3);
        assert_eq!(layout.strategy, default_strategy());
        assert_eq!(
            layout.partition_tree.tab_bar_height,
            default_tab_bar_height()
        );
    }

    #[test]
    fn load_recovers_when_two_nested_levels_have_failures() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_two_levels_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "strategy = \"banana\"\n",
                "\n",
                "[master]\n",
                "master_ratio = 0.6\n",
                "master_count = \"oops\"\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(layout.strategy, default_strategy());
        assert_eq!(layout.master.master_ratio, 0.6);
        assert_eq!(layout.master.master_count, default_master_count());
    }

    #[test]
    fn load_warns_on_unknown_top_level_key() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_unknown_top_{nanos}.toml"));
        std::fs::write(&path, "unknown_field = 1\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.border_size, default_border_size());
        assert_eq!(config.theme, Flavor::default());
    }

    #[test]
    fn load_warns_on_unknown_field_inside_nested_table() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_unknown_nested_{nanos}.toml"));
        std::fs::write(&path, "[master]\nunknown = 1\nmaster_ratio = 0.7\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(layout.master.master_ratio, 0.7);
    }

    #[test]
    fn layout_load_or_default_falls_back_when_validate_fails() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_layout_validate_fail_{nanos}.toml"));
        std::fs::write(&path, "min_width = 100\nmax_width = 50\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        assert!(Config::load(path.to_str().unwrap()).is_err());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.strategy, Config::default().strategy);
        assert_eq!(config.partition_tree, Config::default().partition_tree);
        assert_eq!(config.master, Config::default().master);
    }

    #[test]
    fn load_recovers_when_master_ratio_out_of_range() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_ratio_range_{nanos}.toml"));
        std::fs::write(&path, "[master]\nmaster_ratio = 1.5\nmaster_count = 3\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(layout.master.master_ratio, default_master_ratio());
        assert_eq!(layout.master.master_count, 3);
    }

    #[test]
    fn load_recovers_when_font_family_is_blank() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_font_blank_{nanos}.toml"));
        std::fs::write(&path, "[font]\nfamily = \"   \"\ntext_size = 18.0\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(config.font.family, None);
        assert_eq!(config.font.text_size, 18.0);
    }

    #[test]
    fn load_drops_single_bad_keymap_binding() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_bad_binding_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[keymaps]\n",
                "\"meta+a\" = [\"focus left\"]\n",
                "\"unkmod+h\" = [\"focus left\"]\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        let good = "meta+a".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&good));
        assert_eq!(config.keymaps.default.len(), 1);
    }

    #[test]
    fn load_drops_single_bad_action_in_binding() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_bad_action_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[keymaps]\n",
                "\"meta+a\" = [\"fly to mars\"]\n",
                "\"meta+b\" = [\"focus left\"]\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        let b = "meta+b".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&b));
        let a = "meta+a".parse::<Keymap>().unwrap();
        assert!(!config.keymaps.default.contains_key(&a));
    }

    #[test]
    #[cfg(windows)]
    fn default_windows_ignore_contains_shell_tray() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_win_defaults_{nanos}.toml"));
        std::fs::write(&path, "[windows]\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert!(
            config
                .ignore
                .iter()
                .any(|r| r.class.as_deref() == Some("Shell_TrayWnd"))
        );
    }

    #[test]
    #[cfg(windows)]
    fn default_windows_ignore_contains_core_window() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_win_core_window_{nanos}.toml"));
        std::fs::write(&path, "[windows]\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        let entry = config
            .ignore
            .iter()
            .find(|r| r.class.as_deref() == Some("Windows.UI.Core.CoreWindow"));
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert!(entry.title.is_none());
        assert!(entry.aumid.is_none());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn default_macos_ignore_contains_dock() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_macos_defaults_{nanos}.toml"));
        std::fs::write(&path, "[macos]\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = load_or_default(path.to_str().unwrap(), Config::load);
        assert!(
            config
                .ignore
                .iter()
                .any(|r| r.bundle_id.as_deref() == Some("com.apple.dock"))
        );
    }

    #[test]
    fn layout_load_or_default_returns_defaults_when_missing() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_layout_does_not_exist_{nanos}.toml"));
        let layout = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(layout.strategy, default_strategy());
        assert_eq!(layout.partition_tree, default_partition_tree_config());
        assert_eq!(layout.master, default_master_config());
    }

    #[test]
    fn layout_load_or_default_returns_defaults_on_malformed() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_layout_malformed_{nanos}.toml"));
        std::fs::write(&path, "this is = = not valid toml\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), Config::load);
        assert_eq!(layout.strategy, default_strategy());
        assert_eq!(layout.partition_tree, default_partition_tree_config());
        assert_eq!(layout.master, default_master_config());
    }

    #[test]
    fn layout_load_parses_root_schema() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_layout_root_schema_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "strategy = \"master\"\n",
                "\n",
                "[partition_tree]\n",
                "tab_bar_height = 32.0\n",
                "\n",
                "[master]\n",
                "master_ratio = 0.6\n",
                "master_count = 2\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = Config::load(path.to_str().unwrap()).unwrap();
        assert_eq!(layout.strategy, Strategy::Master);
        assert_eq!(layout.partition_tree.tab_bar_height.logical(), 32.0);
        assert_eq!(layout.master.master_ratio, 0.6);
        assert_eq!(layout.master.master_count, 2);
    }

    #[test]
    fn layout_load_or_default_parses_size_constraints() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_layout_size_constraints_{nanos}.toml"));
        std::fs::write(
            &path,
            "min_width = 200\nmax_width = \"50%\"\nmin_height = 100\nmax_height = 0\n",
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = Config::load(path.to_str().unwrap()).unwrap();
        assert_eq!(layout.min_width, SizeConstraint::Pixels(Length::new(200.0)));
        assert_eq!(layout.max_width, SizeConstraint::Percent(50.0));
        assert_eq!(
            layout.min_height,
            SizeConstraint::Pixels(Length::new(100.0))
        );
        assert_eq!(layout.max_height, SizeConstraint::Pixels(Length::new(0.0)));
    }

    #[test]
    fn config_load_warns_on_legacy_layout_block() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_legacy_layout_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "border_size = 5.0\n",
                "min_width = 100\n",
                "[layout]\n",
                "strategy = \"master\"\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load(path.to_str().unwrap()).unwrap();
        assert_eq!(config.border_size, 5.0);
    }

    #[test]
    fn preferred_layout_default_empty() {
        let layout: LayoutConfig = toml::from_str("").unwrap();
        assert!(layout.workspace.is_empty());
    }

    #[test]
    fn preferred_layout_parse_single_entry() {
        let layout: LayoutConfig =
            toml::from_str("[[workspace]]\nname = \"1\"\nstrategy = \"master\"\n").unwrap();
        assert_eq!(layout.workspace.len(), 1);
        assert_eq!(layout.workspace[0].name(), "1");
        assert!(matches!(
            layout.workspace[0],
            LayoutWorkspaceConfig::Master { .. }
        ));
    }

    #[test]
    fn preferred_layout_parse_multiple_distinct() {
        let layout: LayoutConfig = toml::from_str(concat!(
            "[[workspace]]\n",
            "name = \"1\"\n",
            "strategy = \"master\"\n",
            "\n",
            "[[workspace]]\n",
            "name = \"scratch\"\n",
            "strategy = \"partition_tree\"\n",
        ))
        .unwrap();
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
    fn preferred_layout_drop_unknown_strategy() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_ws_unknown_strategy_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[[workspace]]\n",
                "name = \"good\"\n",
                "strategy = \"master\"\n",
                "\n",
                "[[workspace]]\n",
                "name = \"bad\"\n",
                "strategy = \"floating\"\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), LayoutConfig::load);
        assert_eq!(layout.workspace.len(), 1);
        assert_eq!(layout.workspace[0].name(), "good");
    }

    #[test]
    fn preferred_layout_drop_missing_name() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_ws_missing_name_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[[workspace]]\n",
                "strategy = \"master\"\n",
                "\n",
                "[[workspace]]\n",
                "name = \"valid\"\n",
                "strategy = \"master\"\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), LayoutConfig::load);
        assert_eq!(layout.workspace.len(), 1);
        assert_eq!(layout.workspace[0].name(), "valid");
    }

    #[test]
    fn preferred_layout_drop_empty_name() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_ws_empty_name_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[[workspace]]\n",
                "name = \"\"\n",
                "strategy = \"master\"\n",
                "\n",
                "[[workspace]]\n",
                "name = \"valid\"\n",
                "strategy = \"partition_tree\"\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), LayoutConfig::load);
        assert_eq!(layout.workspace.len(), 1);
        assert_eq!(layout.workspace[0].name(), "valid");
    }

    #[test]
    fn preferred_layout_drop_unknown_field() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_ws_unknown_field_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[[workspace]]\n",
                "name = \"1\"\n",
                "strategy = \"master\"\n",
                "foo = 1\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), LayoutConfig::load);
        assert_eq!(layout.workspace.len(), 1);
        assert_eq!(layout.workspace[0].name(), "1");
        assert!(matches!(
            layout.workspace[0],
            LayoutWorkspaceConfig::Master { .. }
        ));
    }

    #[test]
    fn preferred_layout_dedup_last_wins() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_ws_dedup_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[[workspace]]\n",
                "name = \"1\"\n",
                "strategy = \"partition_tree\"\n",
                "\n",
                "[[workspace]]\n",
                "name = \"1\"\n",
                "strategy = \"master\"\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), LayoutConfig::load);
        assert_eq!(layout.workspace.len(), 1);
        assert_eq!(layout.workspace[0].name(), "1");
        assert!(matches!(
            layout.workspace[0],
            LayoutWorkspaceConfig::Master { .. }
        ));
    }

    #[test]
    fn preferred_layout_drop_non_table_element() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_ws_non_table_{nanos}.toml"));
        // TOML inline arrays can hold non-table elements. Write raw to test recovery.
        std::fs::write(&path, "workspace = [\"not_a_table\"]\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), LayoutConfig::load);
        assert!(layout.workspace.is_empty());
    }

    #[test]
    fn preferred_layout_survive_other_field_failure() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_ws_cross_recovery_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[master]\n",
                "master_ratio = \"abc\"\n",
                "\n",
                "[[workspace]]\n",
                "name = \"1\"\n",
                "strategy = \"master\"\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let layout = load_or_default(path.to_str().unwrap(), LayoutConfig::load);
        assert_eq!(layout.workspace.len(), 1);
        assert_eq!(layout.workspace[0].name(), "1");
        assert!(matches!(
            layout.workspace[0],
            LayoutWorkspaceConfig::Master { .. }
        ));
    }

    #[test]
    fn tree_leaf_parses() {
        let ws: LayoutWorkspaceConfig = toml::from_str(
            r#"name = "dev"
strategy = "partition_tree"
tree = { process = "editor.exe" }
"#,
        )
        .unwrap();
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
        let ws: LayoutWorkspaceConfig = toml::from_str(
            r#"name = "dev"
strategy = "partition_tree"
tree = [
  { process = "editor.exe" },
  { process = "terminal.exe" },
]
"#,
        )
        .unwrap();
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
        let ws: LayoutWorkspaceConfig = toml::from_str(
            r#"name = "dev"
strategy = "partition_tree"
tree = { split = "horizontal", children = [
  { process = "a.exe" },
  { process = "b.exe" },
]}
"#,
        )
        .unwrap();
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
        let ws: LayoutWorkspaceConfig = toml::from_str(
            r#"name = "dev"
strategy = "partition_tree"
tree = { split = "tabbed", children = [
  { process = "browser.exe" },
  { process = "editor.exe" },
]}
"#,
        )
        .unwrap();
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
        let ws: LayoutWorkspaceConfig = toml::from_str(
            r#"name = "dev"
strategy = "partition_tree"
tree = { split = "horizontal", children = [
  { process = "editor.exe" },
  { split = "vertical", children = [
    { process = "terminal.exe" },
    { process = "logs.exe" },
  ]},
]}
"#,
        )
        .unwrap();
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
        let ws: LayoutWorkspaceConfig = toml::from_str(
            r#"name = "dev"
strategy = "partition_tree"
"#,
        )
        .unwrap();
        match ws {
            LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
                assert!(tree.is_none());
            }
            _ => panic!("expected PartitionTree variant"),
        }
    }

    #[test]
    fn tree_invalid_split_warns() {
        assert!(
            toml::from_str::<LayoutWorkspaceConfig>(
                r#"name = "dev"
strategy = "partition_tree"
tree = { split = "diagonal", children = []}
"#,
            )
            .is_err()
        );
    }
}
