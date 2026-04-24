use anyhow::{Result, anyhow};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcMessage {
    Action(Action),
    Query(Query),
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum Query {
    Workspaces,
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum HubAction {
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
    Master {
        #[command(subcommand)]
        target: MasterTarget,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum Action {
    #[command(flatten)]
    Hub(HubAction),
    Exec {
        command: String,
    },
    Exit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitorTarget {
    Up,
    Down,
    Left,
    Right,
    Name(String),
}

impl fmt::Display for HubAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HubAction::Focus { target } => write!(f, "focus {target}"),
            HubAction::Move { target } => write!(f, "move {target}"),
            HubAction::Toggle { target } => write!(f, "toggle {target}"),
            HubAction::Master { target } => write!(f, "master {target}"),
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Hub(hub) => hub.fmt(f),
            Action::Exec { command } => write!(f, "exec {command}"),
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

impl IntoIterator for Actions {
    type Item = Action;
    type IntoIter = std::vec::IntoIter<Action>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
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

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum FocusTarget {
    Up,
    Down,
    Left,
    Right,
    Parent,
    NextTab,
    PrevTab,
    Workspace {
        name: String,
    },
    Monitor {
        #[arg(value_parser = parse_monitor_target)]
        target: MonitorTarget,
    },
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
            FocusTarget::Workspace { name } => write!(f, "workspace {name}"),
            FocusTarget::Monitor { target } => write!(f, "monitor {target}"),
        }
    }
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum MoveTarget {
    Up,
    Down,
    Left,
    Right,
    Workspace {
        name: String,
    },
    Monitor {
        #[arg(value_parser = parse_monitor_target)]
        target: MonitorTarget,
    },
}

impl fmt::Display for MoveTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoveTarget::Up => write!(f, "up"),
            MoveTarget::Down => write!(f, "down"),
            MoveTarget::Left => write!(f, "left"),
            MoveTarget::Right => write!(f, "right"),
            MoveTarget::Workspace { name } => write!(f, "workspace {name}"),
            MoveTarget::Monitor { target } => write!(f, "monitor {target}"),
        }
    }
}

impl fmt::Display for MonitorTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonitorTarget::Up => write!(f, "up"),
            MonitorTarget::Down => write!(f, "down"),
            MonitorTarget::Left => write!(f, "left"),
            MonitorTarget::Right => write!(f, "right"),
            MonitorTarget::Name(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Subcommand, Serialize, Deserialize)]
pub enum ToggleTarget {
    SpawnDirection,
    Direction,
    Layout,
    Float,
    Fullscreen,
}

impl fmt::Display for ToggleTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToggleTarget::SpawnDirection => write!(f, "spawn direction"),
            ToggleTarget::Direction => write!(f, "direction"),
            ToggleTarget::Layout => write!(f, "layout"),
            ToggleTarget::Float => write!(f, "float"),
            ToggleTarget::Fullscreen => write!(f, "fullscreen"),
        }
    }
}

#[derive(Debug, Clone, Copy, Subcommand, Serialize, Deserialize)]
pub enum MasterTarget {
    IncreaseMasterRatio,
    DecreaseMasterRatio,
    IncrementMasterCount,
    DecrementMasterCount,
}

impl fmt::Display for MasterTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MasterTarget::IncreaseMasterRatio => write!(f, "increase-master-ratio"),
            MasterTarget::DecreaseMasterRatio => write!(f, "decrease-master-ratio"),
            MasterTarget::IncrementMasterCount => write!(f, "increment-master-count"),
            MasterTarget::DecrementMasterCount => write!(f, "decrement-master-count"),
        }
    }
}

impl FromStr for Action {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        // Handle exec specially since command can contain spaces
        if let Some(command) = s.strip_prefix("exec ") {
            return Ok(Action::Exec {
                command: command.to_string(),
            });
        }

        let parts: Vec<&str> = s.split_whitespace().collect();
        match parts.as_slice() {
            ["focus", "up"] => Ok(Action::Hub(HubAction::Focus {
                target: FocusTarget::Up,
            })),
            ["focus", "down"] => Ok(Action::Hub(HubAction::Focus {
                target: FocusTarget::Down,
            })),
            ["focus", "left"] => Ok(Action::Hub(HubAction::Focus {
                target: FocusTarget::Left,
            })),
            ["focus", "right"] => Ok(Action::Hub(HubAction::Focus {
                target: FocusTarget::Right,
            })),
            ["focus", "parent"] => Ok(Action::Hub(HubAction::Focus {
                target: FocusTarget::Parent,
            })),
            ["focus", "workspace", n] => Ok(Action::Hub(HubAction::Focus {
                target: FocusTarget::Workspace {
                    name: n.to_string(),
                },
            })),
            ["focus", "next_tab"] => Ok(Action::Hub(HubAction::Focus {
                target: FocusTarget::NextTab,
            })),
            ["focus", "prev_tab"] => Ok(Action::Hub(HubAction::Focus {
                target: FocusTarget::PrevTab,
            })),
            ["focus", "monitor", target] => Ok(Action::Hub(HubAction::Focus {
                target: FocusTarget::Monitor {
                    target: parse_monitor_target(target)?,
                },
            })),
            ["move", "up"] => Ok(Action::Hub(HubAction::Move {
                target: MoveTarget::Up,
            })),
            ["move", "down"] => Ok(Action::Hub(HubAction::Move {
                target: MoveTarget::Down,
            })),
            ["move", "left"] => Ok(Action::Hub(HubAction::Move {
                target: MoveTarget::Left,
            })),
            ["move", "right"] => Ok(Action::Hub(HubAction::Move {
                target: MoveTarget::Right,
            })),
            ["move", "workspace", n] => Ok(Action::Hub(HubAction::Move {
                target: MoveTarget::Workspace {
                    name: n.to_string(),
                },
            })),
            ["move", "monitor", target] => Ok(Action::Hub(HubAction::Move {
                target: MoveTarget::Monitor {
                    target: parse_monitor_target(target)?,
                },
            })),
            ["toggle", "spawn_direction"] => Ok(Action::Hub(HubAction::Toggle {
                target: ToggleTarget::SpawnDirection,
            })),
            ["toggle", "direction"] => Ok(Action::Hub(HubAction::Toggle {
                target: ToggleTarget::Direction,
            })),
            ["toggle", "layout"] => Ok(Action::Hub(HubAction::Toggle {
                target: ToggleTarget::Layout,
            })),
            ["toggle", "float"] => Ok(Action::Hub(HubAction::Toggle {
                target: ToggleTarget::Float,
            })),
            ["toggle", "fullscreen"] => Ok(Action::Hub(HubAction::Toggle {
                target: ToggleTarget::Fullscreen,
            })),
            ["master", "increase-master-ratio"] => Ok(Action::Hub(HubAction::Master {
                target: MasterTarget::IncreaseMasterRatio,
            })),
            ["master", "decrease-master-ratio"] => Ok(Action::Hub(HubAction::Master {
                target: MasterTarget::DecreaseMasterRatio,
            })),
            ["master", "increment-master-count"] => Ok(Action::Hub(HubAction::Master {
                target: MasterTarget::IncrementMasterCount,
            })),
            ["master", "decrement-master-count"] => Ok(Action::Hub(HubAction::Master {
                target: MasterTarget::DecrementMasterCount,
            })),
            ["exit"] => Ok(Action::Exit),
            _ => Err(anyhow!("Unknown action: {}", s)),
        }
    }
}

// MonitorTarget is parsed here instead of using clap's Subcommand derive.
// Deriving Subcommand would require nested subcommands (e.g., `dome focus monitor up`
// becoming `dome focus monitor up` with `up` as its own subcommand), which is overly
// complex. Since actions are primarily parsed from config files and IPC strings anyway,
// manual parsing is simpler and more flexible.
fn parse_monitor_target(s: &str) -> Result<MonitorTarget> {
    match s {
        "up" => Ok(MonitorTarget::Up),
        "down" => Ok(MonitorTarget::Down),
        "left" => Ok(MonitorTarget::Left),
        "right" => Ok(MonitorTarget::Right),
        name => Ok(MonitorTarget::Name(name.to_string())),
    }
}

#[derive(Serialize, Deserialize)]
enum FlatAction {
    Focus { target: FocusTarget },
    Move { target: MoveTarget },
    Toggle { target: ToggleTarget },
    Master { target: MasterTarget },
    Exec { command: String },
    Exit,
}

impl From<Action> for FlatAction {
    fn from(a: Action) -> Self {
        match a {
            Action::Hub(HubAction::Focus { target }) => FlatAction::Focus { target },
            Action::Hub(HubAction::Move { target }) => FlatAction::Move { target },
            Action::Hub(HubAction::Toggle { target }) => FlatAction::Toggle { target },
            Action::Hub(HubAction::Master { target }) => FlatAction::Master { target },
            Action::Exec { command } => FlatAction::Exec { command },
            Action::Exit => FlatAction::Exit,
        }
    }
}

impl From<FlatAction> for Action {
    fn from(a: FlatAction) -> Self {
        match a {
            FlatAction::Focus { target } => Action::Hub(HubAction::Focus { target }),
            FlatAction::Move { target } => Action::Hub(HubAction::Move { target }),
            FlatAction::Toggle { target } => Action::Hub(HubAction::Toggle { target }),
            FlatAction::Master { target } => Action::Hub(HubAction::Master { target }),
            FlatAction::Exec { command } => Action::Exec { command },
            FlatAction::Exit => Action::Exit,
        }
    }
}

impl Serialize for Action {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        FlatAction::from(self.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        FlatAction::deserialize(deserializer).map(Action::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_wire_format() {
        let cases = vec![
            (
                Action::Hub(HubAction::Focus {
                    target: FocusTarget::Up,
                }),
                r#"{"Focus":{"target":"Up"}}"#,
            ),
            (
                Action::Hub(HubAction::Move {
                    target: MoveTarget::Workspace { name: "1".into() },
                }),
                r#"{"Move":{"target":{"Workspace":{"name":"1"}}}}"#,
            ),
            (
                Action::Hub(HubAction::Toggle {
                    target: ToggleTarget::Float,
                }),
                r#"{"Toggle":{"target":"Float"}}"#,
            ),
            (
                Action::Hub(HubAction::Master {
                    target: MasterTarget::IncreaseMasterRatio,
                }),
                r#"{"Master":{"target":"IncreaseMasterRatio"}}"#,
            ),
            (
                Action::Exec {
                    command: "open -a Terminal".into(),
                },
                r#"{"Exec":{"command":"open -a Terminal"}}"#,
            ),
            (Action::Exit, r#""Exit""#),
        ];
        for (action, expected) in &cases {
            let json = serde_json::to_string(action).unwrap();
            assert_eq!(&json, expected, "serialize {action}");
            let round_trip: Action = serde_json::from_str(expected).unwrap();
            assert_eq!(
                &serde_json::to_string(&round_trip).unwrap(),
                expected,
                "round-trip {action}"
            );
        }
    }

    #[test]
    fn ipc_message_serde() {
        let cases = vec![
            (IpcMessage::Action(Action::Exit), r#"{"Action":"Exit"}"#),
            (
                IpcMessage::Action(Action::Hub(HubAction::Focus {
                    target: FocusTarget::Up,
                })),
                r#"{"Action":{"Focus":{"target":"Up"}}}"#,
            ),
            (
                IpcMessage::Query(Query::Workspaces),
                r#"{"Query":"Workspaces"}"#,
            ),
        ];
        for (msg, expected) in &cases {
            let json = serde_json::to_string(msg).unwrap();
            assert_eq!(&json, expected, "serialize {msg:?}");
            let round_trip: IpcMessage = serde_json::from_str(expected).unwrap();
            assert_eq!(
                &serde_json::to_string(&round_trip).unwrap(),
                expected,
                "round-trip {msg:?}"
            );
        }
    }
}
