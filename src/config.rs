use anyhow::{Result, anyhow};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use crate::action::{Action, Actions, FocusTarget, MonitorTarget, MoveTarget, ToggleTarget};

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

fn default_keymaps() -> HashMap<Keymap, Actions> {
    let mut keymaps = HashMap::new();
    for i in 0..=9 {
        keymaps.insert(
            Keymap {
                key: i.to_string(),
                modifiers: Modifiers::CMD,
            },
            Actions::new(vec![Action::Focus {
                target: FocusTarget::Workspace {
                    name: i.to_string(),
                },
            }]),
        );
        keymaps.insert(
            Keymap {
                key: i.to_string(),
                modifiers: Modifiers::CMD | Modifiers::SHIFT,
            },
            Actions::new(vec![Action::Move {
                target: MoveTarget::Workspace {
                    name: i.to_string(),
                },
            }]),
        );
    }
    keymaps.insert(
        Keymap {
            key: "e".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Toggle {
            target: ToggleTarget::SpawnDirection,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "d".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Toggle {
            target: ToggleTarget::Direction,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "b".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Toggle {
            target: ToggleTarget::Layout,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "p".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Focus {
            target: FocusTarget::Parent,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "h".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Focus {
            target: FocusTarget::Left,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "j".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Focus {
            target: FocusTarget::Down,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "k".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Focus {
            target: FocusTarget::Up,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "l".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Focus {
            target: FocusTarget::Right,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "[".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Focus {
            target: FocusTarget::PrevTab,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "]".into(),
            modifiers: Modifiers::CMD,
        },
        Actions::new(vec![Action::Focus {
            target: FocusTarget::NextTab,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "h".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Move {
            target: MoveTarget::Left,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "j".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Move {
            target: MoveTarget::Down,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "k".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Move {
            target: MoveTarget::Up,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "l".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Move {
            target: MoveTarget::Right,
        }]),
    );
    keymaps.insert(
        Keymap {
            key: "f".into(),
            modifiers: Modifiers::CMD | Modifiers::SHIFT,
        },
        Actions::new(vec![Action::Toggle {
            target: ToggleTarget::Float,
        }]),
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
            Actions::new(vec![Action::Focus {
                target: FocusTarget::Monitor {
                    target: target.clone(),
                },
            }]),
        );
        keymaps.insert(
            Keymap {
                key: key.into(),
                modifiers: Modifiers::CMD | Modifiers::ALT | Modifiers::SHIFT,
            },
            Actions::new(vec![Action::Move {
                target: MoveTarget::Monitor { target },
            }]),
        );
    }
    keymaps
}

fn deserialize_keymaps<'de, D>(deserializer: D) -> Result<HashMap<Keymap, Actions>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = HashMap::<String, Vec<String>>::deserialize(deserializer)?;
    let mut keymaps = HashMap::new();
    for (key_str, action_strs) in raw {
        let keymap = key_str
            .parse::<Keymap>()
            .map_err(serde::de::Error::custom)?;
        let actions = parse_actions(&action_strs).map_err(serde::de::Error::custom)?;
        keymaps.insert(keymap, actions);
    }
    Ok(keymaps)
}

fn parse_actions(action_strs: &[String]) -> Result<Actions> {
    let actions: Vec<Action> = action_strs
        .iter()
        .map(|s| s.parse())
        .collect::<Result<_>>()?;
    Ok(Actions::new(actions))
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Color {
    pub(crate) r: f32,
    pub(crate) g: f32,
    pub(crate) b: f32,
    pub(crate) a: f32,
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let c = s
            .parse::<csscolorparser::Color>()
            .map_err(serde::de::Error::custom)?;
        Ok(Color {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        })
    }
}

fn default_focused_color() -> Color {
    Color {
        r: 0.4,
        g: 0.6,
        b: 1.0,
        a: 1.0,
    }
}

fn default_spawn_indicator_color() -> Color {
    Color {
        r: 1.0,
        g: 0.6,
        b: 0.2,
        a: 1.0,
    }
}

fn default_border_color() -> Color {
    Color {
        r: 0.3,
        g: 0.3,
        b: 0.3,
        a: 1.0,
    }
}

fn default_tab_bar_background_color() -> Color {
    Color {
        r: 0.15,
        g: 0.15,
        b: 0.2,
        a: 1.0,
    }
}

fn default_active_tab_background_color() -> Color {
    Color {
        r: 0.3,
        g: 0.3,
        b: 0.4,
        a: 1.0,
    }
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
    pub(crate) fn matches(&self, app: &str, bundle_id: Option<&str>, title: Option<&str>) -> bool {
        if let Some(pattern) = &self.app
            && !pattern_matches(pattern, app)
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

#[cfg_attr(not(target_os = "windows"), expect(dead_code))]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct WindowsConfig {
    #[serde(default)]
    pub(crate) ignore: Vec<WindowsWindow>,
    #[serde(default)]
    pub(crate) on_open: Vec<WindowsOnOpenRule>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    #[serde(default = "default_keymaps", deserialize_with = "deserialize_keymaps")]
    pub(crate) keymaps: HashMap<Keymap, Actions>,
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
    #[serde(default = "default_focused_color")]
    pub(crate) focused_color: Color,
    #[serde(default = "default_spawn_indicator_color")]
    pub(crate) spawn_indicator_color: Color,
    #[serde(default = "default_border_color")]
    pub(crate) border_color: Color,
    #[serde(default = "default_tab_bar_background_color")]
    pub(crate) tab_bar_background_color: Color,
    #[serde(default = "default_active_tab_background_color")]
    pub(crate) active_tab_background_color: Color,
    #[serde(default)]
    #[cfg_attr(not(target_os = "macos"), expect(dead_code))]
    pub(crate) macos: MacosConfig,
    #[serde(default)]
    #[cfg_attr(not(target_os = "windows"), expect(dead_code))]
    pub(crate) windows: WindowsConfig,
    #[serde(default)]
    pub(crate) log_level: Option<String>,
}

fn default_border_size() -> f32 {
    2.0
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
            focused_color: default_focused_color(),
            spawn_indicator_color: default_spawn_indicator_color(),
            border_color: default_border_color(),
            tab_bar_background_color: default_tab_bar_background_color(),
            active_tab_background_color: default_active_tab_background_color(),
            macos: MacosConfig::default(),
            windows: WindowsConfig::default(),
            log_level: None,
        }
    }
}

impl Config {
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
}
