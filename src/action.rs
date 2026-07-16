use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcMessage {
    Action(Action),
    Query(Query),
    ExportLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Query {
    Workspaces,
}

use crate::core::WindowId;

/// Every user-visible action Dome can perform. This is the single source of
/// truth for the action set: CLI (`src/cli.rs`), IPC JSON, and TOML keymap
/// strings all parse into this enum. Adding a new action requires editing only
/// this enum and its `Display`/`FromStr` impls. IPC wire format uses the
/// variant name as its tag; a rename is a wire-format break.
///
/// `ToggleMinimized` is a first-class variant (not a `ToggleTarget` member)
/// because the picker is a UI concern, not a tree mutation. Making it top-level
/// lets the compiler enforce that every platform runner handles it explicitly,
/// instead of relying on `unreachable!()` arms in hub dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Focus(FocusTarget),
    Move(MoveTarget),
    Toggle(ToggleTarget),
    Master(MasterTarget),
    ToggleMinimized,
    /// Restore a specific minimized window. Sent by the picker UI, not
    /// user-configurable (no `FromStr` arm, no CLI subcommand).
    UnminimizeWindow(WindowId),
    Exec {
        command: String,
    },
    Exit,
    Mode {
        name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitorTarget {
    Up,
    Down,
    Left,
    Right,
    Name(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TabDirection {
    Next,
    Prev,
}

impl fmt::Display for TabDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TabDirection::Next => write!(f, "next"),
            TabDirection::Prev => write!(f, "prev"),
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Focus(t) => write!(f, "focus {t}"),
            Action::Move(t) => write!(f, "move {t}"),
            Action::Toggle(t) => write!(f, "toggle {t}"),
            Action::Master(t) => write!(f, "master {t}"),
            Action::ToggleMinimized => write!(f, "toggle minimized"),
            Action::UnminimizeWindow(id) => write!(f, "unminimize window {id}"),
            Action::Exec { command } => write!(f, "exec {command}"),
            Action::Exit => write!(f, "exit"),
            Action::Mode { name } => write!(f, "mode {name}"),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FocusTarget {
    Up,
    Down,
    Left,
    Right,
    Parent,
    Tab { direction: TabDirection },
    Workspace { name: String },
    Monitor { target: MonitorTarget },
}

impl fmt::Display for FocusTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FocusTarget::Up => write!(f, "up"),
            FocusTarget::Down => write!(f, "down"),
            FocusTarget::Left => write!(f, "left"),
            FocusTarget::Right => write!(f, "right"),
            FocusTarget::Parent => write!(f, "parent"),
            FocusTarget::Tab { direction } => write!(f, "tab {direction}"),
            FocusTarget::Workspace { name } => write!(f, "workspace {name}"),
            FocusTarget::Monitor { target } => write!(f, "monitor {target}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveTarget {
    Up,
    Down,
    Left,
    Right,
    Workspace { name: String },
    Monitor { target: MonitorTarget },
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ToggleTarget {
    Spawn,
    Direction,
    Layout,
    Float,
    Fullscreen,
}

impl fmt::Display for ToggleTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToggleTarget::Spawn => write!(f, "spawn"),
            ToggleTarget::Direction => write!(f, "direction"),
            ToggleTarget::Layout => write!(f, "layout"),
            ToggleTarget::Float => write!(f, "float"),
            ToggleTarget::Fullscreen => write!(f, "fullscreen"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MasterTarget {
    Grow,
    Shrink,
    More,
    Fewer,
}

impl fmt::Display for MasterTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MasterTarget::Grow => write!(f, "grow"),
            MasterTarget::Shrink => write!(f, "shrink"),
            MasterTarget::More => write!(f, "more"),
            MasterTarget::Fewer => write!(f, "fewer"),
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

        // Uses strip_prefix (like exec) instead of the match-arm shape so mode
        // names with spaces work and parsing stays consistent across free-form args.
        if let Some(name) = s.strip_prefix("mode ") {
            let name = name.trim();
            if !name.is_empty() {
                return Ok(Action::Mode {
                    name: name.to_string(),
                });
            }
        }

        let parts: Vec<&str> = s.split_whitespace().collect();
        match parts.as_slice() {
            ["focus", "up"] => Ok(Action::Focus(FocusTarget::Up)),
            ["focus", "down"] => Ok(Action::Focus(FocusTarget::Down)),
            ["focus", "left"] => Ok(Action::Focus(FocusTarget::Left)),
            ["focus", "right"] => Ok(Action::Focus(FocusTarget::Right)),
            ["focus", "parent"] => Ok(Action::Focus(FocusTarget::Parent)),
            ["focus", "workspace", n] => Ok(Action::Focus(FocusTarget::Workspace {
                name: n.to_string(),
            })),
            ["focus", "tab", "next"] => Ok(Action::Focus(FocusTarget::Tab {
                direction: TabDirection::Next,
            })),
            ["focus", "tab", "prev"] => Ok(Action::Focus(FocusTarget::Tab {
                direction: TabDirection::Prev,
            })),
            ["focus", "monitor", target] => Ok(Action::Focus(FocusTarget::Monitor {
                target: parse_monitor_target(target)?,
            })),
            ["move", "up"] => Ok(Action::Move(MoveTarget::Up)),
            ["move", "down"] => Ok(Action::Move(MoveTarget::Down)),
            ["move", "left"] => Ok(Action::Move(MoveTarget::Left)),
            ["move", "right"] => Ok(Action::Move(MoveTarget::Right)),
            ["move", "workspace", n] => Ok(Action::Move(MoveTarget::Workspace {
                name: n.to_string(),
            })),
            ["move", "monitor", target] => Ok(Action::Move(MoveTarget::Monitor {
                target: parse_monitor_target(target)?,
            })),
            ["toggle", "spawn"] => Ok(Action::Toggle(ToggleTarget::Spawn)),
            ["toggle", "direction"] => Ok(Action::Toggle(ToggleTarget::Direction)),
            ["toggle", "layout"] => Ok(Action::Toggle(ToggleTarget::Layout)),
            ["toggle", "float"] => Ok(Action::Toggle(ToggleTarget::Float)),
            ["toggle", "fullscreen"] => Ok(Action::Toggle(ToggleTarget::Fullscreen)),
            ["toggle", "minimized"] => Ok(Action::ToggleMinimized),
            ["master", "grow"] => Ok(Action::Master(MasterTarget::Grow)),
            ["master", "shrink"] => Ok(Action::Master(MasterTarget::Shrink)),
            ["master", "more"] => Ok(Action::Master(MasterTarget::More)),
            ["master", "fewer"] => Ok(Action::Master(MasterTarget::Fewer)),
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
pub(crate) fn parse_monitor_target(s: &str) -> Result<MonitorTarget> {
    match s {
        "up" => Ok(MonitorTarget::Up),
        "down" => Ok(MonitorTarget::Down),
        "left" => Ok(MonitorTarget::Left),
        "right" => Ok(MonitorTarget::Right),
        name => Ok(MonitorTarget::Name(name.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_wire_format() {
        let cases = vec![
            (Action::Focus(FocusTarget::Up), r#"{"Focus":"Up"}"#),
            (
                Action::Move(MoveTarget::Workspace { name: "1".into() }),
                r#"{"Move":{"Workspace":{"name":"1"}}}"#,
            ),
            (Action::Toggle(ToggleTarget::Float), r#"{"Toggle":"Float"}"#),
            (Action::Master(MasterTarget::Grow), r#"{"Master":"Grow"}"#),
            (
                Action::Exec {
                    command: "open -a Terminal".into(),
                },
                r#"{"Exec":{"command":"open -a Terminal"}}"#,
            ),
            (Action::Exit, r#""Exit""#),
            (Action::ToggleMinimized, r#""ToggleMinimized""#),
            (
                Action::Mode {
                    name: "resize".into(),
                },
                r#"{"Mode":{"name":"resize"}}"#,
            ),
            (
                Action::Focus(FocusTarget::Tab {
                    direction: TabDirection::Next,
                }),
                r#"{"Focus":{"Tab":{"direction":"Next"}}}"#,
            ),
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
                IpcMessage::Action(Action::Focus(FocusTarget::Up)),
                r#"{"Action":{"Focus":"Up"}}"#,
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

    #[test]
    fn action_from_str_round_trip() {
        // Every action string whose FromStr path takes no free-form argument
        // must survive a parse -> Display -> compare cycle. This locks
        // Display/FromStr symmetry and would have caught the old
        // SpawnDirection round-trip bug.
        let cases = [
            "focus up",
            "focus down",
            "focus left",
            "focus right",
            "focus parent",
            "focus tab next",
            "focus tab prev",
            "focus workspace 3",
            "focus monitor left",
            "focus monitor foo",
            "move up",
            "move down",
            "move left",
            "move right",
            "move workspace 3",
            "move monitor left",
            "toggle spawn",
            "toggle direction",
            "toggle layout",
            "toggle float",
            "toggle fullscreen",
            "toggle minimized",
            "master grow",
            "master shrink",
            "master more",
            "master fewer",
            "exit",
            "mode resize",
            "exec open -a Terminal",
        ];
        for input in cases {
            let action = Action::from_str(input)
                .unwrap_or_else(|e| panic!("FromStr failed for {input:?}: {e}"));
            let formatted = action.to_string();
            assert_eq!(
                formatted, input,
                "round-trip mismatch: from_str({input:?}).to_string() = {formatted:?}"
            );
        }
    }

    #[test]
    fn unminimize_window_display_uses_space() {
        let id: WindowId = serde_json::from_value(serde_json::json!(7)).unwrap();
        let action = Action::UnminimizeWindow(id);
        assert_eq!(action.to_string(), "unminimize window WindowId(7)");
    }
}
