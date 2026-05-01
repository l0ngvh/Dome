use anyhow::{Result, anyhow};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use crate::action::{
    Action, Actions, FocusTarget, HubAction, MonitorTarget, MoveTarget, TabDirection, ToggleTarget,
};
use crate::font::FontConfig;
use crate::theme::{Flavor, Theme};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub(crate) struct Modifiers: u8 {
        const CMD = 1 << 0;
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
                "cmd" => Modifiers::CMD,
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
                modifiers: Modifiers::CMD,
            },
            Actions::new(vec![Action::Hub(HubAction::Focus {
                target: FocusTarget::Workspace {
                    name: i.to_string(),
                },
            })]),
        );
        keymaps.insert(
            Keymap {
                key: i.to_string(),
                modifiers: Modifiers::CMD | Modifiers::SHIFT,
            },
            Actions::new(vec![Action::Hub(HubAction::Move {
                target: MoveTarget::Workspace {
                    name: i.to_string(),
                },
            })]),
        );
    }
    keymaps.insert(
        Keymap {
            key: "e".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Hub(HubAction::Toggle {
            target: ToggleTarget::Spawn,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "d".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Hub(HubAction::Toggle {
            target: ToggleTarget::Direction,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "b".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Hub(HubAction::Toggle {
            target: ToggleTarget::Layout,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "p".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Hub(HubAction::Focus {
            target: FocusTarget::Parent,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "h".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Hub(HubAction::Focus {
            target: FocusTarget::Left,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "j".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Hub(HubAction::Focus {
            target: FocusTarget::Down,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "k".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Hub(HubAction::Focus {
            target: FocusTarget::Up,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "l".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Hub(HubAction::Focus {
            target: FocusTarget::Right,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "[".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Hub(HubAction::Focus {
            target: FocusTarget::Tab {
                direction: TabDirection::Prev,
            },
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "]".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Hub(HubAction::Focus {
            target: FocusTarget::Tab {
                direction: TabDirection::Next,
            },
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "h".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Hub(HubAction::Move {
            target: MoveTarget::Left,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "j".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Hub(HubAction::Move {
            target: MoveTarget::Down,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "k".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Hub(HubAction::Move {
            target: MoveTarget::Up,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "l".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Hub(HubAction::Move {
            target: MoveTarget::Right,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "f".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Hub(HubAction::Toggle {
            target: ToggleTarget::Float,
        })]),
    );
    keymaps.insert(
        Keymap {
            key: "q".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Exit]),
    );
    // Monitor focus: Cmd+Alt+hjkl
    for (key, target) in [
        ("h", MonitorTarget::Left),
        ("j", MonitorTarget::Down),
        ("k", MonitorTarget::Up),
        ("l", MonitorTarget::Right),
    ] {
        keymaps.insert(
            Keymap {
                key: key.into(),
                modifiers: Modifiers::CMD | Modifiers::ALT,
            },
            Actions::new(vec![Action::Hub(HubAction::Focus {
                target: FocusTarget::Monitor {
                    target: target.clone(),
                },
            })]),
        );
        keymaps.insert(
            Keymap {
                key: key.into(),
                modifiers: Modifiers::CMD | Modifiers::ALT | Modifiers::SHIFT,
            },
            Actions::new(vec![Action::Hub(HubAction::Move {
                target: MoveTarget::Monitor { target },
            })]),
        );
    }
    ModalKeymaps {
        default: keymaps,
        modes: HashMap::new(),
    }
}

fn deserialize_modal_keymaps<'de, D>(deserializer: D) -> Result<ModalKeymaps, D::Error>
where
    D: Deserializer<'de>,
{
    // The [keymaps] table mixes key-combo bindings (string -> [actions]) with a
    // special "mode" key (table of named modes). Deserialize as raw TOML values
    // and discriminate on the key name.
    let raw = HashMap::<String, toml::Value>::deserialize(deserializer)?;
    let mut default = HashMap::new();
    let mut modes = HashMap::new();

    for (key_str, value) in raw {
        if key_str == "mode" {
            // value is { mode_name => { key_combo => [action_strings] } }
            let mode_table = mode_table_from_value(value).map_err(serde::de::Error::custom)?;
            for (mode_name, bindings) in mode_table {
                let mut mode_keymaps = HashMap::new();
                for (k, action_strs) in bindings {
                    let keymap = k.parse::<Keymap>().map_err(serde::de::Error::custom)?;
                    let actions = parse_actions(&action_strs).map_err(serde::de::Error::custom)?;
                    mode_keymaps.insert(keymap, actions);
                }
                modes.insert(mode_name, mode_keymaps);
            }
        } else {
            let action_strs: Vec<String> = value.try_into().map_err(serde::de::Error::custom)?;
            let keymap = key_str
                .parse::<Keymap>()
                .map_err(serde::de::Error::custom)?;
            let actions = parse_actions(&action_strs).map_err(serde::de::Error::custom)?;
            default.insert(keymap, actions);
        }
    }

    Ok(ModalKeymaps { default, modes })
}

fn mode_table_from_value(
    value: toml::Value,
) -> Result<HashMap<String, HashMap<String, Vec<String>>>> {
    let toml::Value::Table(table) = value else {
        anyhow::bail!("expected 'mode' to be a table");
    };
    let mut result = HashMap::new();
    for (mode_name, mode_val) in table {
        let toml::Value::Table(bindings_table) = mode_val else {
            anyhow::bail!("expected mode '{mode_name}' to be a table");
        };
        let mut bindings = HashMap::new();
        for (key_combo, actions_val) in bindings_table {
            let toml::Value::Array(arr) = actions_val else {
                anyhow::bail!(
                    "expected actions for key '{key_combo}' in mode '{mode_name}' to be an array"
                );
            };
            let action_strs: Vec<String> = arr
                .into_iter()
                .map(|v| match v {
                    toml::Value::String(s) => Ok(s),
                    other => anyhow::bail!("expected string action, got {other}"),
                })
                .collect::<Result<_>>()?;
            bindings.insert(key_combo, action_strs);
        }
        result.insert(mode_name, bindings);
    }
    Ok(result)
}

fn parse_actions(action_strs: &[String]) -> Result<Actions> {
    let actions: Vec<Action> = action_strs
        .iter()
        .map(|s| s.parse())
        .collect::<Result<_>>()?;
    Ok(Actions::new(actions))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SizeConstraint {
    Pixels(f32),
    Percent(f32),
}

impl Default for SizeConstraint {
    fn default() -> Self {
        SizeConstraint::Pixels(0.0)
    }
}

impl SizeConstraint {
    pub(crate) fn resolve(&self, screen_size: f32) -> f32 {
        match self {
            SizeConstraint::Pixels(px) => *px,
            SizeConstraint::Percent(pct) => screen_size * pct / 100.0,
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
                Ok(SizeConstraint::Pixels(val))
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
        if let Some(pattern) = &self.app
            && !app.is_some_and(|a| pattern_matches(pattern, a))
        {
            return false;
        }
        if let Some(b) = &self.bundle_id
            && bundle_id != Some(b.as_str())
        {
            return false;
        }
        if let Some(pattern) = &self.title
            && !title.is_some_and(|t| pattern_matches(pattern, t))
        {
            return false;
        }
        if app.is_none() && bundle_id.is_none() && title.is_none() {
            return false;
        }
        self.app.is_some() || self.bundle_id.is_some() || self.title.is_some()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct WindowsWindow {
    #[serde(default)]
    pub(crate) process: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
}

#[cfg_attr(not(target_os = "windows"), expect(dead_code))]
impl WindowsWindow {
    pub(crate) fn matches(&self, process: &str, title: Option<&str>) -> bool {
        if let Some(pattern) = &self.process
            && !pattern_matches(pattern, process)
        {
            return false;
        }
        if let Some(pattern) = &self.title
            && !title.is_some_and(|t| pattern_matches(pattern, t))
        {
            return false;
        }
        self.process.is_some() || self.title.is_some()
    }
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

#[cfg_attr(not(target_os = "macos"), expect(dead_code))]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MacosOnOpenRule {
    #[serde(flatten)]
    pub(crate) window: MacosWindow,
    #[serde(default)]
    pub(crate) run: Actions,
}

#[cfg_attr(not(target_os = "windows"), expect(dead_code))]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WindowsOnOpenRule {
    #[serde(flatten)]
    pub(crate) window: WindowsWindow,
    #[serde(default)]
    pub(crate) run: Actions,
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
        },
        WindowsWindow {
            process: Some("SearchHost.exe".into()),
            title: None,
        },
        WindowsWindow {
            process: Some("StartMenuExperienceHost.exe".into()),
            title: None,
        },
        WindowsWindow {
            process: None,
            title: Some("MSCTFIME UI".into()),
        },
        WindowsWindow {
            process: None,
            title: Some("OLEChannelWnd".into()),
        },
    ]
}

fn deserialize_windows_ignore<'de, D>(deserializer: D) -> Result<Vec<WindowsWindow>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut rules: Vec<WindowsWindow> = Vec::deserialize(deserializer)?;
    rules.extend(default_windows_ignore());
    Ok(rules)
}

#[cfg_attr(not(target_os = "windows"), expect(dead_code))]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct WindowsConfig {
    #[serde(
        default = "default_windows_ignore",
        deserialize_with = "deserialize_windows_ignore"
    )]
    pub(crate) ignore: Vec<WindowsWindow>,
    #[serde(default)]
    pub(crate) on_open: Vec<WindowsOnOpenRule>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    #[serde(
        default = "default_keymaps",
        deserialize_with = "deserialize_modal_keymaps"
    )]
    pub(crate) keymaps: ModalKeymaps,
    #[serde(default = "default_border_size")]
    pub(crate) border_size: f32,
    #[serde(default = "default_tab_bar_height")]
    pub(crate) tab_bar_height: f32,
    #[serde(default = "default_automatic_tiling")]
    pub(crate) automatic_tiling: bool,
    #[serde(default = "SizeConstraint::default_min")]
    pub(crate) min_width: SizeConstraint,
    #[serde(default = "SizeConstraint::default_min")]
    pub(crate) min_height: SizeConstraint,
    #[serde(default)]
    pub(crate) max_width: SizeConstraint,
    #[serde(default)]
    pub(crate) max_height: SizeConstraint,
    #[serde(default)]
    pub(crate) theme: Flavor,
    #[serde(default)]
    pub(crate) font: FontConfig,
    #[serde(default)]
    #[cfg_attr(not(target_os = "macos"), expect(dead_code))]
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

fn default_tab_bar_height() -> f32 {
    24.0
}

fn default_automatic_tiling() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Config {
            keymaps: default_keymaps(),
            border_size: default_border_size(),
            tab_bar_height: default_tab_bar_height(),
            automatic_tiling: default_automatic_tiling(),
            min_width: SizeConstraint::default_min(),
            min_height: SizeConstraint::default_min(),
            max_width: SizeConstraint::default(),
            max_height: SizeConstraint::default(),
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
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if let (SizeConstraint::Pixels(min), SizeConstraint::Pixels(max)) =
            (self.min_width, self.max_width)
            && max > 0.0
            && min > max
        {
            anyhow::bail!("min_width ({min}) cannot be greater than max_width ({max})");
        }
        if let (SizeConstraint::Pixels(min), SizeConstraint::Pixels(max)) =
            (self.min_height, self.max_height)
            && max > 0.0
            && min > max
        {
            anyhow::bail!("min_height ({min}) cannot be greater than max_height ({max})");
        }
        self.font.validate()?;
        // "default" is the reserved name for the top-level [keymaps] table.
        if self.keymaps.modes.contains_key("default") {
            anyhow::bail!("mode name 'default' is reserved for the top-level [keymaps] table");
        }
        if self.keymaps.modes.contains_key("") {
            anyhow::bail!("mode name must not be empty");
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
        assert_eq!(config.max_width, SizeConstraint::Pixels(0.0));
        assert_eq!(config.max_height, SizeConstraint::Pixels(0.0));
    }

    #[test]
    fn size_constraint_parses_float_as_pixels() {
        let config: Config = toml::from_str("min_width = 200.0").unwrap();
        assert_eq!(config.min_width, SizeConstraint::Pixels(200.0));
    }

    #[test]
    fn size_constraint_parses_int_as_pixels() {
        let config: Config = toml::from_str("min_width = 200").unwrap();
        assert_eq!(config.min_width, SizeConstraint::Pixels(200.0));
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
        assert_eq!(SizeConstraint::Pixels(200.0).resolve(1000.0), 200.0);
        assert_eq!(SizeConstraint::Percent(10.0).resolve(1000.0), 100.0);
        assert_eq!(SizeConstraint::Percent(5.0).resolve(1920.0), 96.0);
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
        // Configs mentioning any of the five removed color field names must fail
        // at parse time via deny_unknown_fields. This is intentional: the entire
        // per-color config surface was replaced by a single `theme` field.
        assert!(toml::from_str::<Config>(r##"focused_color = "#ff0000""##).is_err());
        assert!(toml::from_str::<Config>(r##"border_color = "#ff0000""##).is_err());
        assert!(toml::from_str::<Config>(r##"spawn_indicator_color = "#ff0000""##).is_err());
        assert!(toml::from_str::<Config>(r##"tab_bar_background_color = "#ff0000""##).is_err());
        assert!(toml::from_str::<Config>(r##"active_tab_background_color = "#ff0000""##).is_err());
    }

    #[test]
    fn removed_border_radius_rejected() {
        // Configs mentioning `border_radius` must fail at parse time via
        // `deny_unknown_fields`. The field was replaced by hardcoded values
        // in `src/overlay.rs` (WINDOW_BORDER_RADIUS and tab_bar_corner_radius).
        assert!(toml::from_str::<Config>("border_radius = 12.0").is_err());
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
        let config: Config = toml::from_str(
            r#"
            [keymaps]
            "cmd+h" = ["focus left"]
            "#,
        )
        .unwrap();
        assert!(config.keymaps.modes.is_empty());
        let keymap = "cmd+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&keymap));
    }

    #[test]
    fn modal_keymaps_with_mode() {
        let config: Config = toml::from_str(
            r#"
            [keymaps]
            "cmd+h" = ["focus left"]

            [keymaps.mode.resize]
            "h" = ["focus left"]
            "escape" = ["mode default"]
            "#,
        )
        .unwrap();
        let cmd_h = "cmd+h".parse::<Keymap>().unwrap();
        assert!(config.keymaps.default.contains_key(&cmd_h));
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
    fn modal_keymaps_rejects_default_mode_name() {
        let config: Config = toml::from_str(
            r#"
            [keymaps]
            "cmd+h" = ["focus left"]

            [keymaps.mode.default]
            "h" = ["focus left"]
            "#,
        )
        .unwrap();
        let err = config.validate().unwrap_err();
        assert!(
            err.to_string().contains("default"),
            "expected error about 'default', got: {err}"
        );
    }

    #[test]
    fn modal_keymaps_rejects_empty_mode_name() {
        let result = toml::from_str::<Config>(
            r#"
            [keymaps]
            "cmd+h" = ["focus left"]

            [keymaps.mode.""]
            "h" = ["focus left"]
            "#,
        );
        // Empty mode name may fail at parse time (TOML key) or at validation.
        // Either way it should not succeed silently.
        match result {
            Ok(config) => {
                let err = config.validate().unwrap_err();
                assert!(
                    err.to_string().contains("empty"),
                    "expected error about empty mode name, got: {err}"
                );
            }
            Err(_) => { /* parse-time rejection is fine */ }
        }
    }

    #[test]
    fn example_config_parses() {
        let path = format!("{}/examples/config.toml", env!("CARGO_MANIFEST_DIR"));
        let content = std::fs::read_to_string(&path).expect("failed to read example config");
        let config: Config = toml::from_str(&content).expect("failed to parse example config");
        config.validate().expect("example config failed validation");
    }

    /// RAII guard that removes a temp file on drop, even if the test panics.
    struct CleanupFile(std::path::PathBuf);
    impl Drop for CleanupFile {
        fn drop(&mut self) {
            // Best-effort cleanup of test temp file; nothing to do if it fails.
            std::fs::remove_file(&self.0).ok();
        }
    }
}
