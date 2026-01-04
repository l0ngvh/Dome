use anyhow::{Result, anyhow};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Subcommand, Serialize, Deserialize)]
pub enum Action {
    Focus {
        #[command(subcommand)]
        target: FocusTarget,
    },
    Move {
        #[command(subcommand)]
        target: MoveTarget,
    },
    Toggle {
        #[command(subcommand)]
        target: ToggleTarget,
    },
    Exit,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Focus { target } => write!(f, "focus {target}"),
            Action::Move { target } => write!(f, "move {target}"),
            Action::Toggle { target } => write!(f, "toggle {target}"),
            Action::Exit => write!(f, "exit"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Actions(Vec<Action>);

impl Actions {
    pub fn new(actions: Vec<Action>) -> Self {
        Self(actions)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Actions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: Vec<_> = self.0.iter().map(|a| a.to_string()).collect();
        write!(f, "[{}]", s.join(", "))
    }
}

impl<'a> IntoIterator for &'a Actions {
    type Item = &'a Action;
    type IntoIter = std::slice::Iter<'a, Action>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'de> serde::Deserialize<'de> for Actions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let strs = Vec::<String>::deserialize(deserializer)?;
        let actions: Vec<Action> = strs
            .iter()
            .map(|s| s.parse())
            .collect::<anyhow::Result<_>>()
            .map_err(serde::de::Error::custom)?;
        Ok(Actions(actions))
    }
}

#[derive(Debug, Clone, Copy, Subcommand, Serialize, Deserialize)]
pub enum FocusTarget {
    Up,
    Down,
    Left,
    Right,
    Parent,
    NextTab,
    PrevTab,
    Workspace { index: usize },
}

impl fmt::Display for FocusTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FocusTarget::Up => write!(f, "up"),
            FocusTarget::Down => write!(f, "down"),
            FocusTarget::Left => write!(f, "left"),
            FocusTarget::Right => write!(f, "right"),
            FocusTarget::Parent => write!(f, "parent"),
            FocusTarget::NextTab => write!(f, "next_tab"),
            FocusTarget::PrevTab => write!(f, "prev_tab"),
            FocusTarget::Workspace { index } => write!(f, "workspace {index}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Subcommand, Serialize, Deserialize)]
pub enum MoveTarget {
    Up,
    Down,
    Left,
    Right,
    Workspace { index: usize },
}

impl fmt::Display for MoveTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoveTarget::Up => write!(f, "up"),
            MoveTarget::Down => write!(f, "down"),
            MoveTarget::Left => write!(f, "left"),
            MoveTarget::Right => write!(f, "right"),
            MoveTarget::Workspace { index } => write!(f, "workspace {index}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Subcommand, Serialize, Deserialize)]
pub enum ToggleTarget {
    SpawnDirection,
    Direction,
    Layout,
    Float,
}

impl fmt::Display for ToggleTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToggleTarget::SpawnDirection => write!(f, "spawn direction"),
            ToggleTarget::Direction => write!(f, "direction"),
            ToggleTarget::Layout => write!(f, "layout"),
            ToggleTarget::Float => write!(f, "float"),
        }
    }
}

impl FromStr for Action {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        match parts.as_slice() {
            ["focus", "up"] => Ok(Action::Focus {
                target: FocusTarget::Up,
            }),
            ["focus", "down"] => Ok(Action::Focus {
                target: FocusTarget::Down,
            }),
            ["focus", "left"] => Ok(Action::Focus {
                target: FocusTarget::Left,
            }),
            ["focus", "right"] => Ok(Action::Focus {
                target: FocusTarget::Right,
            }),
            ["focus", "parent"] => Ok(Action::Focus {
                target: FocusTarget::Parent,
            }),
            ["focus", "workspace", n] => Ok(Action::Focus {
                target: FocusTarget::Workspace { index: n.parse()? },
            }),
            ["focus", "next_tab"] => Ok(Action::Focus {
                target: FocusTarget::NextTab,
            }),
            ["focus", "prev_tab"] => Ok(Action::Focus {
                target: FocusTarget::PrevTab,
            }),
            ["move", "up"] => Ok(Action::Move {
                target: MoveTarget::Up,
            }),
            ["move", "down"] => Ok(Action::Move {
                target: MoveTarget::Down,
            }),
            ["move", "left"] => Ok(Action::Move {
                target: MoveTarget::Left,
            }),
            ["move", "right"] => Ok(Action::Move {
                target: MoveTarget::Right,
            }),
            ["move", "workspace", n] => Ok(Action::Move {
                target: MoveTarget::Workspace { index: n.parse()? },
            }),
            ["toggle", "spawn_direction"] => Ok(Action::Toggle {
                target: ToggleTarget::SpawnDirection,
            }),
            ["toggle", "direction"] => Ok(Action::Toggle {
                target: ToggleTarget::Direction,
            }),
            ["toggle", "layout"] => Ok(Action::Toggle {
                target: ToggleTarget::Layout,
            }),
            ["toggle", "float"] => Ok(Action::Toggle {
                target: ToggleTarget::Float,
            }),
            ["exit"] => Ok(Action::Exit),
            _ => Err(anyhow!("Unknown action: {}", s)),
        }
    }
}
