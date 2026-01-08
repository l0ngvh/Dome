use anyhow::{Result, anyhow};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::str::FromStr;

use crate::action::{Action, Actions, FocusTarget, MoveTarget, ToggleTarget};

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
                target: FocusTarget::Workspace { index: i },
            }]),
        );
        keymaps.insert(
            Keymap {
                key: i.to_string(),
                modifiers: Modifiers::CMD | Modifiers::SHIFT,
            },
            Actions::new(vec![Action::Move {
                target: MoveTarget::Workspace { index: i },
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

#[cfg_attr(not(target_os = "macos"), expect(dead_code))]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct MacosWindowRule {
    #[serde(default)]
    pub(crate) app: Option<String>,
    #[serde(default)]
    pub(crate) bundle_id: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default = "default_manage_window")]
    pub(crate) manage: bool,
    #[serde(default)]
    pub(crate) run: Actions,
}

#[cfg_attr(not(target_os = "windows"), expect(dead_code))]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct WindowsWindowRule {
    /// Process name (e.g., "notepad.exe", "chrome.exe")
    #[serde(default)]
    pub(crate) process: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default = "default_manage_window")]
    pub(crate) manage: bool,
    #[serde(default)]
    pub(crate) run: Actions,
}

fn default_manage_window() -> bool {
    true
}

#[cfg_attr(not(target_os = "macos"), expect(dead_code))]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct MacosConfig {
    #[serde(default)]
    pub(crate) window_rules: Vec<MacosWindowRule>,
}

#[cfg_attr(not(target_os = "windows"), expect(dead_code))]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct WindowsConfig {
    #[serde(default)]
    pub(crate) window_rules: Vec<WindowsWindowRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Config {
    #[serde(default = "default_keymaps", deserialize_with = "deserialize_keymaps")]
    keymaps: HashMap<Keymap, Actions>,
    #[serde(default = "default_border_size")]
    pub(crate) border_size: f32,
    #[serde(default = "default_tab_bar_height")]
    pub(crate) tab_bar_height: f32,
    #[serde(default = "default_automatic_tiling")]
    pub(crate) automatic_tiling: bool,
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
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    pub(crate) fn get_actions(&self, keymap: &Keymap) -> Actions {
        self.keymaps.get(keymap).cloned().unwrap_or_default()
    }
}
