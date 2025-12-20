use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use serde::{Deserialize, Deserializer};
use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub enum Action {
    Focus(Target),
    Move(Target),
    Toggle(ToggleTarget),
}

#[derive(Debug, Clone)]
pub enum Target {
    Up,
    Down,
    Left,
    Right,
    Parent,
    Workspace(usize),
}

#[derive(Debug, Clone)]
pub enum ToggleTarget {
    Direction,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Modifier {
    Cmd,
    Shift,
    Alt,
    Ctrl,
}

#[derive(Debug, Clone)]
pub struct Keymap {
    pub key: String,
    pub modifiers: HashSet<Modifier>,
}

impl PartialEq for Keymap {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.modifiers == other.modifiers
    }
}

impl Eq for Keymap {}

impl Hash for Keymap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        for m in &self.modifiers {
            m.hash(state);
        }
    }
}

impl FromStr for Action {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        match parts.as_slice() {
            ["focus", "up"] => Ok(Action::Focus(Target::Up)),
            ["focus", "down"] => Ok(Action::Focus(Target::Down)),
            ["focus", "left"] => Ok(Action::Focus(Target::Left)),
            ["focus", "right"] => Ok(Action::Focus(Target::Right)),
            ["focus", "parent"] => Ok(Action::Focus(Target::Parent)),
            ["focus", "workspace", n] => Ok(Action::Focus(Target::Workspace(n.parse()?))),
            ["move", "workspace", n] => Ok(Action::Move(Target::Workspace(n.parse()?))),
            ["toggle", "direction"] => Ok(Action::Toggle(ToggleTarget::Direction)),
            _ => Err(anyhow!("Unknown action: {}", s)),
        }
    }
}

impl FromStr for Modifier {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "cmd" => Ok(Modifier::Cmd),
            "shift" => Ok(Modifier::Shift),
            "alt" => Ok(Modifier::Alt),
            "ctrl" => Ok(Modifier::Ctrl),
            _ => Err(anyhow!("Unknown modifier: {}", s)),
        }
    }
}

impl FromStr for Keymap {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('+').collect();
        if parts.is_empty() {
            return Err(anyhow!("Empty keymap"));
        }
        let key = parts.last().unwrap().to_string();
        let modifiers = parts[..parts.len()-1]
            .iter()
            .map(|m| m.parse())
            .collect::<Result<HashSet<_>>>()?;
        Ok(Keymap { key, modifiers })
    }
}

fn default_keymaps() -> HashMap<Keymap, Vec<Action>> {
    let mut keymaps = HashMap::new();
    for i in 0..=9 {
        keymaps.insert(Keymap { key: i.to_string(), modifiers: HashSet::from([Modifier::Cmd]) }, vec![Action::Focus(Target::Workspace(i))]);
        keymaps.insert(Keymap { key: i.to_string(), modifiers: HashSet::from([Modifier::Cmd, Modifier::Shift]) }, vec![Action::Move(Target::Workspace(i))]);
    }
    keymaps.insert(Keymap { key: "e".into(), modifiers: HashSet::from([Modifier::Cmd]) }, vec![Action::Toggle(ToggleTarget::Direction)]);
    keymaps.insert(Keymap { key: "p".into(), modifiers: HashSet::from([Modifier::Cmd]) }, vec![Action::Focus(Target::Parent)]);
    keymaps.insert(Keymap { key: "h".into(), modifiers: HashSet::from([Modifier::Cmd]) }, vec![Action::Focus(Target::Left)]);
    keymaps.insert(Keymap { key: "j".into(), modifiers: HashSet::from([Modifier::Cmd]) }, vec![Action::Focus(Target::Down)]);
    keymaps.insert(Keymap { key: "k".into(), modifiers: HashSet::from([Modifier::Cmd]) }, vec![Action::Focus(Target::Up)]);
    keymaps.insert(Keymap { key: "l".into(), modifiers: HashSet::from([Modifier::Cmd]) }, vec![Action::Focus(Target::Right)]);
    keymaps
}

fn deserialize_keymaps<'de, D>(deserializer: D) -> Result<HashMap<Keymap, Vec<Action>>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = HashMap::<String, Vec<String>>::deserialize(deserializer)?;
    let mut keymaps = HashMap::new();
    for (key_str, action_strs) in raw {
        let keymap = key_str.parse::<Keymap>().map_err(serde::de::Error::custom)?;
        let actions: Vec<Action> = action_strs
            .iter()
            .map(|s| s.parse())
            .collect::<Result<Vec<_>>>()
            .map_err(serde::de::Error::custom)?;
        keymaps.insert(keymap, actions);
    }
    Ok(keymaps)
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_keymaps", deserialize_with = "deserialize_keymaps")]
    keymaps: HashMap<Keymap, Vec<Action>>,
    #[serde(default = "default_border_size")]
    pub border_size: f32,
}

fn default_border_size() -> f32 {
    2.0
}

impl Config {
    pub fn load() -> Self {
        match std::fs::read_to_string("config.toml") {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => config,
                Err(e) => {
                    tracing::warn!("Failed to parse config: {e}, using defaults");
                    Config { 
                        keymaps: default_keymaps(),
                        border_size: default_border_size(),
                    }
                }
            },
            Err(e) => {
                tracing::warn!("Failed to load config: {e}, using defaults");
                Config { 
                    keymaps: default_keymaps(),
                    border_size: default_border_size(),
                }
            }
        }
    }

    pub fn get_actions(&self, keymap: &Keymap) -> Vec<Action> {
        self.keymaps.get(keymap).cloned().unwrap_or_default()
    }
}
