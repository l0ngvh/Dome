use anyhow::{Result, anyhow};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::path::Path;
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

trait WalkRecover: Sized {
    fn walk(w: &mut Walker) -> Self;
}

trait WalkRule: serde::de::DeserializeOwned {
    const KNOWN: &'static [&'static str];
}

struct Walker<'a> {
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
            min_width: w.field("min_width", SizeConstraint::default_min()),
            min_height: w.field("min_height", SizeConstraint::default_min()),
            max_width: w.field("max_width", SizeConstraint::default()),
            max_height: w.field("max_height", SizeConstraint::default()),
            layout: w.nested_or("layout", default_layout()),
            theme: w.field("theme", Flavor::default()),
            font: w.nested_or("font", FontConfig::default()),
            macos: w.nested_or("macos", default_macos()),
            windows: w.nested_or("windows", default_windows()),
            log_level: w.field("log_level", LogLevel::default()),
            start_at_login: w.field("start_at_login", false),
        }
    }
}

fn default_macos_ignore() -> Vec<MacosWindow> {
    vec![
        MacosWindow {
            app: None,
            bundle_id: Some("com.apple.dock".into()),
            title: None,
        },
        MacosWindow {
            app: None,
            bundle_id: Some("com.apple.controlcenter".into()),
            title: None,
        },
        MacosWindow {
            app: None,
            bundle_id: Some("com.apple.notificationcenterui".into()),
            title: None,
        },
        MacosWindow {
            app: None,
            bundle_id: Some("com.apple.loginwindow".into()),
            title: None,
        },
    ]
}

fn default_macos() -> MacosConfig {
    let mut config = MacosConfig::default();
    config.ignore.extend(default_macos_ignore());
    config
}

fn default_windows() -> WindowsConfig {
    let mut config = WindowsConfig::default();
    config.ignore.extend(default_windows_ignore());
    config
}

impl WalkRecover for LayoutConfig {
    fn walk(w: &mut Walker) -> Self {
        let strategy = w.field("strategy", default_strategy());
        let partition_tree = w.nested::<PartitionTreeConfig>("partition_tree");
        let master = w.nested::<MasterConfig>("master");
        LayoutConfig {
            strategy,
            partition_tree,
            master,
        }
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

impl WalkRecover for MacosConfig {
    fn walk(w: &mut Walker) -> Self {
        let mut ignore = w.rule_vec::<MacosWindow>("ignore");
        ignore.extend(default_macos_ignore());
        MacosConfig {
            ignore,
            on_open: w.rule_vec::<MacosOnOpenRule>("on_open"),
        }
    }
}

impl WalkRecover for WindowsConfig {
    fn walk(w: &mut Walker) -> Self {
        let mut ignore = w.rule_vec::<WindowsWindow>("ignore");
        ignore.extend(default_windows_ignore());
        WindowsConfig {
            ignore,
            on_open: w.rule_vec::<WindowsOnOpenRule>("on_open"),
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
/// No field binds to the strategy instance, so a config change triggers a
/// relayout but never a strategy rebuild. A future field that needs to bind
/// to the strategy instance must override `apply_config` to copy it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct PartitionTreeConfig {
    #[serde(default = "default_tab_bar_height")]
    pub(crate) tab_bar_height: Length<Logical>,
    #[serde(default = "default_automatic_tiling")]
    pub(crate) automatic_tiling: bool,
}

/// Per-strategy config for the master-stack strategy.
///
/// All fields flow into the running `MasterStrategy` via `apply_config`
/// on hot-reload, overwriting any runtime-tuned values (e.g. from
/// `master grow/shrink/more/fewer` commands). The TOML file is the source of
/// truth; runtime commands are a transient override until the next reload.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct MasterConfig {
    #[serde(default = "default_master_ratio")]
    pub(crate) master_ratio: f32,
    #[serde(default = "default_master_count")]
    pub(crate) master_count: usize,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct LayoutConfig {
    #[serde(default = "default_strategy")]
    pub(crate) strategy: Strategy,
    #[serde(default = "default_partition_tree_config")]
    pub(crate) partition_tree: PartitionTreeConfig,
    #[serde(default = "default_master_config")]
    pub(crate) master: MasterConfig,
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
fn default_layout() -> LayoutConfig {
    LayoutConfig {
        strategy: default_strategy(),
        partition_tree: default_partition_tree_config(),
        master: default_master_config(),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct MacosWindow {
    #[serde(default)]
    pub(crate) app: Option<String>,
    #[serde(default)]
    pub(crate) bundle_id: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
}

#[cfg_attr(not(target_os = "macos"), expect(dead_code))]
impl MacosWindow {
    pub(crate) fn matches(
        &self,
        app: Option<&str>,
        bundle_id: Option<&str>,
        title: Option<&str>,
    ) -> bool {
        macos_predicate_matches(
            self.app.as_deref(),
            self.bundle_id.as_deref(),
            self.title.as_deref(),
            app,
            bundle_id,
            title,
        )
    }
}

impl WalkRule for MacosWindow {
    const KNOWN: &'static [&'static str] = &["app", "bundle_id", "title"];
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct WindowsWindow {
    #[serde(default)]
    pub(crate) process: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) class: Option<String>,
    #[serde(default)]
    pub(crate) aumid: Option<String>,
}

#[cfg_attr(not(target_os = "windows"), expect(dead_code))]
impl WindowsWindow {
    pub(crate) fn matches(
        &self,
        process: &str,
        title: Option<&str>,
        class: Option<&str>,
        aumid: Option<&str>,
    ) -> bool {
        windows_predicate_matches(
            self.process.as_deref(),
            self.title.as_deref(),
            self.class.as_deref(),
            self.aumid.as_deref(),
            process,
            title,
            class,
            aumid,
        )
    }
}

impl WalkRule for WindowsWindow {
    const KNOWN: &'static [&'static str] = &["process", "title", "class", "aumid"];
}

fn pattern_matches(pattern: &str, text: &str) -> bool {
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

#[cfg_attr(
    not(target_os = "macos"),
    expect(dead_code, reason = "macOS-only schema")
)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MacosOnOpenRule {
    #[serde(default)]
    pub(crate) app: Option<String>,
    #[serde(default)]
    pub(crate) bundle_id: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) mode: Option<WindowMode>,
    #[serde(default)]
    pub(crate) workspace: Option<String>,
}

#[cfg_attr(
    not(target_os = "macos"),
    expect(dead_code, reason = "macOS-only schema")
)]
impl MacosOnOpenRule {
    pub(crate) fn matches(
        &self,
        app: Option<&str>,
        bundle_id: Option<&str>,
        title: Option<&str>,
    ) -> bool {
        macos_predicate_matches(
            self.app.as_deref(),
            self.bundle_id.as_deref(),
            self.title.as_deref(),
            app,
            bundle_id,
            title,
        )
    }
}

impl WalkRule for MacosOnOpenRule {
    const KNOWN: &'static [&'static str] = &["app", "bundle_id", "title", "mode", "workspace"];
}

#[cfg_attr(
    not(target_os = "windows"),
    expect(dead_code, reason = "Windows-only schema")
)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WindowsOnOpenRule {
    #[serde(default)]
    pub(crate) process: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) class: Option<String>,
    #[serde(default)]
    pub(crate) aumid: Option<String>,
    #[serde(default)]
    pub(crate) mode: Option<WindowMode>,
    #[serde(default)]
    pub(crate) workspace: Option<String>,
}

#[cfg_attr(
    not(target_os = "windows"),
    expect(dead_code, reason = "Windows-only schema")
)]
impl WindowsOnOpenRule {
    pub(crate) fn matches(
        &self,
        process: &str,
        title: Option<&str>,
        class: Option<&str>,
        aumid: Option<&str>,
    ) -> bool {
        windows_predicate_matches(
            self.process.as_deref(),
            self.title.as_deref(),
            self.class.as_deref(),
            self.aumid.as_deref(),
            process,
            title,
            class,
            aumid,
        )
    }
}

impl WalkRule for WindowsOnOpenRule {
    const KNOWN: &'static [&'static str] =
        &["process", "title", "class", "aumid", "mode", "workspace"];
}

#[cfg_attr(not(target_os = "macos"), expect(dead_code))]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct MacosConfig {
    #[serde(default)]
    pub(crate) ignore: Vec<MacosWindow>,
    #[serde(default)]
    pub(crate) on_open: Vec<MacosOnOpenRule>,
}

fn default_windows_ignore() -> Vec<WindowsWindow> {
    vec![
        WindowsWindow {
            process: Some("LockApp.exe".into()),
            title: None,
            class: None,
            aumid: None,
        },
        WindowsWindow {
            process: Some("SearchHost.exe".into()),
            title: None,
            class: None,
            aumid: None,
        },
        WindowsWindow {
            process: Some("StartMenuExperienceHost.exe".into()),
            title: None,
            class: None,
            aumid: None,
        },
        WindowsWindow {
            process: None,
            title: Some("MSCTFIME UI".into()),
            class: None,
            aumid: None,
        },
        WindowsWindow {
            process: None,
            title: Some("OLEChannelWnd".into()),
            class: None,
            aumid: None,
        },
        WindowsWindow {
            process: None,
            title: None,
            class: Some("Shell_TrayWnd".into()),
            aumid: None,
        },
        WindowsWindow {
            process: None,
            title: None,
            class: Some("Shell_SecondaryTrayWnd".into()),
            aumid: None,
        },
        WindowsWindow {
            process: None,
            title: None,
            class: Some("Progman".into()),
            aumid: None,
        },
        WindowsWindow {
            process: None,
            title: None,
            class: Some("WorkerW".into()),
            aumid: None,
        },
        WindowsWindow {
            process: None,
            title: None,
            class: Some("TaskListThumbnailWnd".into()),
            aumid: None,
        },
        WindowsWindow {
            process: None,
            title: None,
            class: Some("MultitaskingViewFrame".into()),
            aumid: None,
        },
        WindowsWindow {
            process: None,
            title: None,
            class: Some("Xaml_WindowedPopupClass".into()),
            aumid: None,
        },
        WindowsWindow {
            process: None,
            title: None,
            class: Some("TaskManagerWindow".into()),
            aumid: None,
        },
        // Top-level CoreWindow HWNDs are exclusively shell surfaces on modern
        // Windows (lock screen, sign-in UI, Start, Search/Cortana flyout,
        // Action Center). UWP apps surface as ApplicationFrameWindow.
        WindowsWindow {
            process: None,
            title: None,
            class: Some("Windows.UI.Core.CoreWindow".into()),
            aumid: None,
        },
    ]
}

#[cfg_attr(not(target_os = "windows"), expect(dead_code))]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct WindowsConfig {
    #[serde(default)]
    pub(crate) ignore: Vec<WindowsWindow>,
    #[serde(default)]
    pub(crate) on_open: Vec<WindowsOnOpenRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Config {
    #[serde(skip_deserializing, default = "default_keymaps")]
    pub(crate) keymaps: ModalKeymaps,
    #[serde(default = "default_border_size")]
    pub(crate) border_size: f32,
    #[serde(default = "SizeConstraint::default_min")]
    pub(crate) min_width: SizeConstraint,
    #[serde(default = "SizeConstraint::default_min")]
    pub(crate) min_height: SizeConstraint,
    #[serde(default)]
    pub(crate) max_width: SizeConstraint,
    #[serde(default)]
    pub(crate) max_height: SizeConstraint,
    #[serde(default = "default_layout")]
    pub(crate) layout: LayoutConfig,
    #[serde(default)]
    pub(crate) theme: Flavor,
    #[serde(default)]
    pub(crate) font: FontConfig,
    #[serde(default)]
    #[cfg_attr(
        not(target_os = "macos"),
        expect(dead_code, reason = "only read by macOS platform code")
    )]
    pub(crate) macos: MacosConfig,
    #[serde(default)]
    #[cfg_attr(not(target_os = "windows"), expect(dead_code))]
    pub(crate) windows: WindowsConfig,
    #[serde(default)]
    pub(crate) log_level: LogLevel,
    #[serde(default)]
    pub(crate) start_at_login: bool,
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
            min_width: SizeConstraint::default_min(),
            min_height: SizeConstraint::default_min(),
            max_width: SizeConstraint::default(),
            max_height: SizeConstraint::default(),
            layout: default_layout(),
            // Mocha is the darkest flavour and matches Dome's pre-theme default palette.
            theme: Flavor::default(),
            font: FontConfig::default(),
            macos: MacosConfig::default(),
            windows: WindowsConfig::default(),
            log_level: LogLevel::default(),
            start_at_login: false,
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

    /// Loads config from `path`, falling back to defaults on any error.
    /// The error is logged via `tracing::warn!` so it reaches `dome.log` and
    /// stdout (see docs/configuration.md "Log File").
    pub(crate) fn load_or_default(path: &str) -> Config {
        match Self::load(path) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(%path, error = %e, "Failed to load config, using defaults");
                Config::default()
            }
        }
    }

    pub(crate) fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let table: toml::Table = toml::from_str(&content)?;
        let config = RawConfig::into_config(table);
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        // Validation compares config values in logical space directly -- no scale
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
}

pub(crate) fn start_config_watcher(
    config_path: &str,
    on_change: impl Fn(Config) + Send + 'static,
) -> anyhow::Result<RecommendedWatcher> {
    let path = Path::new(config_path).canonicalize()?;
    let Some(watch_dir) = path.parent().map(|p| p.to_owned()) else {
        anyhow::bail!("no parent dir");
    };

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res
            && matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_))
            && event.paths.iter().any(|p| p == &path)
        {
            match Config::load(path.to_str().unwrap()) {
                Ok(new_config) => {
                    tracing::info!("Config reloaded");
                    on_change(new_config);
                }
                Err(e) => tracing::warn!("Failed to reload config: {e}"),
            }
        }
    })?;

    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;
    tracing::info!(path = config_path, "Config watcher started");
    Ok(watcher)
}

/// Evaluates macOS on-open and ignore-rule predicates. Returns false when:
/// - any specified predicate does not match its corresponding window field,
/// - all window metadata fields are None (empty-predicate guard), or
/// - no predicate field is set on the rule at all.
fn macos_predicate_matches(
    rule_app: Option<&str>,
    rule_bundle_id: Option<&str>,
    rule_title: Option<&str>,
    app: Option<&str>,
    bundle_id: Option<&str>,
    title: Option<&str>,
) -> bool {
    if let Some(p) = rule_app
        && !app.is_some_and(|a| pattern_matches(p, a))
    {
        return false;
    }
    // bundle_id matches by exact equality, not pattern_matches.
    if let Some(b) = rule_bundle_id
        && bundle_id != Some(b)
    {
        return false;
    }
    if let Some(p) = rule_title
        && !title.is_some_and(|t| pattern_matches(p, t))
    {
        return false;
    }
    // Reject when all window metadata is absent. Without this a rule with
    // e.g. only `app` set would spuriously match windows whose AX query
    // returned no metadata at all.
    if app.is_none() && bundle_id.is_none() && title.is_none() {
        return false;
    }
    rule_app.is_some() || rule_bundle_id.is_some() || rule_title.is_some()
}

#[expect(
    clippy::too_many_arguments,
    reason = "predicate fields are individually named for clarity at call sites"
)]
fn windows_predicate_matches(
    rule_process: Option<&str>,
    rule_title: Option<&str>,
    rule_class: Option<&str>,
    rule_aumid: Option<&str>,
    process: &str,
    title: Option<&str>,
    class: Option<&str>,
    aumid: Option<&str>,
) -> bool {
    if let Some(p) = rule_process
        && !pattern_matches(p, process)
    {
        return false;
    }
    if let Some(p) = rule_title
        && !title.is_some_and(|t| pattern_matches(p, t))
    {
        return false;
    }
    if let Some(p) = rule_class
        && !class.is_some_and(|c| pattern_matches(p, c))
    {
        return false;
    }
    if let Some(p) = rule_aumid
        && !aumid.is_some_and(|a| pattern_matches(p, a))
    {
        return false;
    }
    rule_process.is_some() || rule_title.is_some() || rule_class.is_some() || rule_aumid.is_some()
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
    fn validation_rejects_min_greater_than_max_width() {
        let config: Config = toml::from_str("min_width = 200\nmax_width = 100").unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_rejects_min_greater_than_max_height() {
        let config: Config = toml::from_str("min_height = 200\nmax_height = 100").unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_allows_zero_max() {
        let config: Config = toml::from_str("min_width = 200\nmax_width = 0").unwrap();
        assert!(config.validate().is_ok());
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
            "tab_bar_height = 30\nautomatic_tiling = true\n[layout]\nstrategy = \"master\"\n",
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert_eq!(config.layout.strategy, Strategy::Master);
    }

    #[test]
    fn load_or_default_returns_defaults_when_path_missing() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_does_not_exist_{nanos}.toml"));
        let config = Config::load_or_default(path.to_str().unwrap());
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
        let config = Config::load_or_default(path.to_str().unwrap());
        let meta_h = "meta+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&meta_h));
        assert!(!config.keymaps.modes.contains_key(""));
    }

    #[test]
    fn example_config_parses() {
        let path = format!("{}/examples/config.toml", env!("CARGO_MANIFEST_DIR"));
        Config::load(&path).expect("example config failed to load");
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
        let config: Config = toml::from_str(
            "[layout.partition_tree]\ntab_bar_height = 30.0\nautomatic_tiling = false",
        )
        .unwrap();
        assert_eq!(config.layout.partition_tree.tab_bar_height.logical(), 30.0);
        assert!(!config.layout.partition_tree.automatic_tiling);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn partition_tree_config_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.layout.partition_tree.tab_bar_height.logical(), 24.0);
        assert!(config.layout.partition_tree.automatic_tiling);
    }

    #[test]
    fn layout_defaults_to_partition_tree() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.layout.strategy, Strategy::PartitionTree);
        assert_eq!(config.layout.master.master_ratio, 0.5);
        assert_eq!(config.layout.master.master_count, 1);
    }

    #[test]
    fn layout_parses_master_strategy() {
        let config: Config = toml::from_str("[layout]\nstrategy = \"master\"\n").unwrap();
        assert_eq!(config.layout.strategy, Strategy::Master);
        // Sub-tables still get their defaults
        assert_eq!(config.layout.partition_tree.tab_bar_height.logical(), 24.0);
        assert_eq!(config.layout.master.master_ratio, 0.5);
    }

    #[test]
    fn layout_parses_master_params() {
        let config: Config =
            toml::from_str("[layout.master]\nmaster_ratio = 0.3\nmaster_count = 2").unwrap();
        assert_eq!(config.layout.master.master_ratio, 0.3);
        assert_eq!(config.layout.master.master_count, 2);
    }

    #[test]
    fn layout_rejects_unknown_strategy() {
        assert!(toml::from_str::<Config>("[layout]\nstrategy = \"floating\"").is_err());
    }

    #[test]
    fn layout_rejects_unknown_subfield_master() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_unknown_master_{nanos}.toml"));
        std::fs::write(&path, "[layout.master]\nfoo = 1\nmaster_ratio = 0.6\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert_eq!(config.layout.master.master_ratio, 0.6);
    }

    #[test]
    fn layout_rejects_unknown_subfield_partition_tree() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_unknown_pt_{nanos}.toml"));
        std::fs::write(
            &path,
            "[layout.partition_tree]\nfoo = 1\ntab_bar_height = 30.0\n",
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert_eq!(config.layout.partition_tree.tab_bar_height.logical(), 30.0);
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
            "[layout.master]\nmaster_ratio = \"abc\"\nmaster_count = 3\n",
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert_eq!(config.layout.master.master_ratio, default_master_ratio());
        assert_eq!(config.layout.master.master_count, 3);
        assert_eq!(config.layout.strategy, default_strategy());
        assert_eq!(
            config.layout.partition_tree.tab_bar_height,
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
                "[layout]\n",
                "strategy = \"banana\"\n",
                "\n",
                "[layout.master]\n",
                "master_ratio = 0.6\n",
                "master_count = \"oops\"\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert_eq!(config.layout.strategy, default_strategy());
        assert_eq!(config.layout.master.master_ratio, 0.6);
        assert_eq!(config.layout.master.master_count, default_master_count());
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
        std::fs::write(
            &path,
            "[layout]\nunknown = 1\n\n[layout.master]\nmaster_ratio = 0.7\n",
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert_eq!(config.layout.master.master_ratio, 0.7);
    }

    #[test]
    fn load_falls_back_to_defaults_when_validate_fails() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_validate_fail_{nanos}.toml"));
        std::fs::write(&path, "min_width = 100\nmax_width = 50\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        assert!(Config::load(path.to_str().unwrap()).is_err());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert_eq!(config.border_size, default_border_size());
    }

    #[test]
    fn load_recovers_when_master_ratio_out_of_range() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_ratio_range_{nanos}.toml"));
        std::fs::write(
            &path,
            "[layout.master]\nmaster_ratio = 1.5\nmaster_count = 3\n",
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert_eq!(config.layout.master.master_ratio, default_master_ratio());
        assert_eq!(config.layout.master.master_count, 3);
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
        let config = Config::load_or_default(path.to_str().unwrap());
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
        let config = Config::load_or_default(path.to_str().unwrap());
        let b = "meta+b".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&b));
        let a = "meta+a".parse::<Keymap>().unwrap();
        assert!(!config.keymaps.default.contains_key(&a));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn load_drops_single_bad_macos_on_open_rule() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_bad_on_open_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[[macos.on_open]]\n",
                "app = \"Finder\"\n",
                "mode = \"float\"\n",
                "\n",
                "[[macos.on_open]]\n",
                "app = \"Safari\"\n",
                "mode = \"invalid_not_a_mode\"\n",
                "\n",
                "[[macos.on_open]]\n",
                "bundle_id = \"com.apple.mail\"\n",
                "workspace = \"mail\"\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert_eq!(config.macos.on_open.len(), 2);
        assert_eq!(config.macos.on_open[0].app.as_deref(), Some("Finder"));
        assert_eq!(
            config.macos.on_open[1].bundle_id.as_deref(),
            Some("com.apple.mail")
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn load_warns_on_unknown_field_inside_array_of_table_element() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_unknown_arr_{nanos}.toml"));
        std::fs::write(
            &path,
            concat!(
                "[[macos.on_open]]\n",
                "app = \"Finder\"\n",
                "unknown_field = 42\n",
                "mode = \"float\"\n",
            ),
        )
        .unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert_eq!(config.macos.on_open.len(), 1);
        assert_eq!(config.macos.on_open[0].app.as_deref(), Some("Finder"));
    }

    #[test]
    fn windows_rule_parses_class_and_aumid() {
        let config: Config = toml::from_str(concat!(
            "[[windows.ignore]]\n",
            "class = \"Foo\"\n",
            "aumid = \"Some.App.Id\"\n",
            "\n",
            "[[windows.on_open]]\n",
            "class = \"Bar\"\n",
            "aumid = \"Other.App\"\n",
            "mode = \"float\"\n",
        ))
        .unwrap();
        let ignore = &config.windows.ignore;
        let user_rule = ignore.iter().find(|r| r.class.as_deref() == Some("Foo"));
        assert!(user_rule.is_some());
        assert_eq!(user_rule.unwrap().aumid.as_deref(), Some("Some.App.Id"));

        let on_open = &config.windows.on_open;
        assert_eq!(on_open[0].class.as_deref(), Some("Bar"));
        assert_eq!(on_open[0].aumid.as_deref(), Some("Other.App"));
    }

    #[test]
    fn windows_predicate_matches_class_only() {
        assert!(windows_predicate_matches(
            None,
            None,
            Some("Foo"),
            None,
            "any.exe",
            Some("title"),
            Some("Foo"),
            None,
        ));
        assert!(!windows_predicate_matches(
            None,
            None,
            Some("Foo"),
            None,
            "any.exe",
            Some("title"),
            Some("Bar"),
            None,
        ));
    }

    #[test]
    fn windows_predicate_matches_class_regex() {
        assert!(windows_predicate_matches(
            None,
            None,
            Some("/^Shell_/"),
            None,
            "explorer.exe",
            None,
            Some("Shell_TrayWnd"),
            None,
        ));
        assert!(!windows_predicate_matches(
            None,
            None,
            Some("/^Shell_/"),
            None,
            "explorer.exe",
            None,
            Some("NotShell"),
            None,
        ));
    }

    #[test]
    fn windows_predicate_matches_aumid_literal_and_regex() {
        assert!(windows_predicate_matches(
            None,
            None,
            None,
            Some("MyApp_8wekyb3d8bbwe"),
            "app.exe",
            None,
            None,
            Some("MyApp_8wekyb3d8bbwe"),
        ));
        assert!(windows_predicate_matches(
            None,
            None,
            None,
            Some("/^Microsoft\\./"),
            "app.exe",
            None,
            None,
            Some("Microsoft.WindowsCalculator"),
        ));
        assert!(!windows_predicate_matches(
            None,
            None,
            None,
            Some("/^Microsoft\\./"),
            "app.exe",
            None,
            None,
            Some("NotMicrosoft.App"),
        ));
    }

    #[test]
    fn windows_predicate_matches_combined_process_and_class() {
        assert!(windows_predicate_matches(
            Some("explorer.exe"),
            None,
            Some("Shell_TrayWnd"),
            None,
            "explorer.exe",
            None,
            Some("Shell_TrayWnd"),
            None,
        ));
        assert!(!windows_predicate_matches(
            Some("explorer.exe"),
            None,
            Some("Shell_TrayWnd"),
            None,
            "other.exe",
            None,
            Some("Shell_TrayWnd"),
            None,
        ));
    }

    #[test]
    fn windows_predicate_matches_combined_process_and_aumid() {
        assert!(windows_predicate_matches(
            Some("app.exe"),
            None,
            None,
            Some("MyApp_id"),
            "app.exe",
            None,
            None,
            Some("MyApp_id"),
        ));
        assert!(!windows_predicate_matches(
            Some("app.exe"),
            None,
            None,
            Some("MyApp_id"),
            "app.exe",
            None,
            None,
            Some("WrongApp_id"),
        ));
    }

    #[test]
    fn windows_predicate_fail_open_on_none_class() {
        assert!(!windows_predicate_matches(
            None,
            None,
            Some("Foo"),
            None,
            "any.exe",
            None,
            None,
            None,
        ));
    }

    #[test]
    fn windows_predicate_fail_open_on_none_aumid() {
        assert!(!windows_predicate_matches(
            None,
            None,
            None,
            Some("Some.Id"),
            "any.exe",
            None,
            None,
            None,
        ));
    }

    #[test]
    fn default_windows_ignore_contains_shell_tray() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_win_defaults_{nanos}.toml"));
        std::fs::write(&path, "[windows]\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert!(
            config
                .windows
                .ignore
                .iter()
                .any(|r| r.class.as_deref() == Some("Shell_TrayWnd"))
        );
    }

    #[test]
    fn default_windows_ignore_contains_core_window() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_win_core_window_{nanos}.toml"));
        std::fs::write(&path, "[windows]\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        let entry = config
            .windows
            .ignore
            .iter()
            .find(|r| r.class.as_deref() == Some("Windows.UI.Core.CoreWindow"));
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert!(entry.title.is_none());
        assert!(entry.aumid.is_none());
    }

    #[test]
    fn default_macos_ignore_contains_dock() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_config_macos_defaults_{nanos}.toml"));
        std::fs::write(&path, "[macos]\n").unwrap();
        let _cleanup = CleanupFile(path.clone());
        let config = Config::load_or_default(path.to_str().unwrap());
        assert!(
            config
                .macos
                .ignore
                .iter()
                .any(|r| r.bundle_id.as_deref() == Some("com.apple.dock"))
        );
    }
}
