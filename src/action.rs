use anyhow::{Result, anyhow};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum MoveTarget {
    Up,
    Down,
    Left,
    Right,
    Workspace { index: usize },
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum ToggleTarget {
    SpawnDirection,
    Direction,
    Layout,
    Float,
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
