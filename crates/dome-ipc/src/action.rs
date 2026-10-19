use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WindowId(usize);

impl WindowId {
    pub fn new(raw: usize) -> Self {
        Self(raw)
    }

    pub fn get(self) -> usize {
        self.0
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WindowId({})", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcMessage {
    Action { action: Action },
    Query { query: Query },
    SaveLayout,
    ApplyLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Query {
    Workspaces,
    MinimizedWindows,
    Monitors,
}

/// `bundle_id` is populated on macOS, `executable_path` on Windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimizedWindow {
    pub id: WindowId,
    pub title: String,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    pub executable_path: Option<String>,
}

/// Wire DTO for `Query::Monitors`. Not `MonitorInfo`, which both platform layers
/// already define.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorDetails {
    pub device_name: String,
    pub unique_name: String,
    /// Session-scoped.
    pub cg_display_id: Option<u32>,
    /// Windows can reassign it to another display, so it is restamped every
    /// reconcile.
    pub gdi_device: Option<String>,
    pub work_area: MonitorFrame,
}

/// Core's `PixelRect` is generic over a unit tag, so it does not serialize.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorFrame {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Serializable workspace metadata for IPC queries. External tools (status bars,
/// scripts) consume this as JSON over IPC -- the JSON field names are the
/// stability contract, not this Rust type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceInfo {
    pub name: String,
    /// The owning monitor's stored `unique_name`, the position-ranked name
    /// shown to the user. For a Parked workspace this is the ORIGIN
    /// monitor's stored `unique_name`, frozen at unplug, which is NOT
    /// currently connected.
    pub monitor: String,
    pub state: WorkspaceState,
    pub is_focused: bool,
    pub is_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    Attached,
    Parked,
}

/// The IPC wire format tags each variant by its snake_case name, so renaming a variant
/// breaks the wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Focus {
        target: FocusTarget,
    },
    Move {
        target: MoveTarget,
    },
    Toggle {
        target: ToggleTarget,
    },
    Master {
        target: MasterTarget,
    },
    Grow,
    Shrink,
    /// Lacks `FromStr` and is not bindable in a keymap because a `WindowId` is not stable
    /// across daemon restarts, so a bound id would mean nothing after a reload.
    UnminimizeWindow {
        id: WindowId,
    },
    Execute {
        command: String,
    },
    Exit,
    Close,
    Mode {
        name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorTarget {
    Up,
    Down,
    Left,
    Right,
    Name(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
            Action::Focus { target } => write!(f, "focus {target}"),
            Action::Move { target } => write!(f, "move {target}"),
            Action::Toggle { target } => write!(f, "toggle {target}"),
            Action::Master { target } => write!(f, "master {target}"),
            Action::Grow => write!(f, "grow"),
            Action::Shrink => write!(f, "shrink"),
            Action::UnminimizeWindow { id } => write!(f, "unminimize window {id}"),
            Action::Execute { command } => write!(f, "execute {command}"),
            Action::Exit => write!(f, "exit"),
            Action::Close => write!(f, "close"),
            Action::Mode { name } => write!(f, "mode {name}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusTarget {
    Up,
    Down,
    Left,
    Right,
    Parent,
    Tab {
        direction: TabDirection,
    },
    Workspace {
        name: String,
        monitor: Option<String>,
    },
    Monitor {
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
            FocusTarget::Tab { direction } => write!(f, "tab {direction}"),
            FocusTarget::Workspace { name, monitor } => match monitor {
                Some(m) => write!(f, "workspace {name} --monitor {m}"),
                None => write!(f, "workspace {name}"),
            },
            FocusTarget::Monitor { target } => write!(f, "monitor {target}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveTarget {
    Up,
    Down,
    Left,
    Right,
    Workspace {
        name: String,
        monitor: Option<String>,
    },
    Monitor {
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
            MoveTarget::Workspace { name, monitor } => match monitor {
                Some(m) => write!(f, "workspace {name} --monitor {m}"),
                None => write!(f, "workspace {name}"),
            },
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
#[serde(rename_all = "snake_case")]
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
#[serde(rename_all = "snake_case")]
pub enum MasterTarget {
    More,
    Fewer,
}

impl fmt::Display for MasterTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MasterTarget::More => write!(f, "more"),
            MasterTarget::Fewer => write!(f, "fewer"),
        }
    }
}

impl FromStr for Action {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        // strip_prefix rather than a match arm, because the argument can contain spaces.
        if let Some(command) = s.strip_prefix("execute ") {
            return Ok(Action::Execute {
                command: command.to_string(),
            });
        }

        if let Some(name) = s.strip_prefix("mode ") {
            let name = name.trim();
            if !name.is_empty() {
                return Ok(Action::Mode {
                    name: name.to_string(),
                });
            }
        }

        // Parsed before the slice match because the optional --monitor selector needs
        // the monitor-pinned-last split in parse_workspace_selector, which a
        // split_whitespace match cannot express.
        if let Some(rest) = s.strip_prefix("focus workspace ") {
            let (name, monitor) = parse_workspace_selector(rest);
            return Ok(Action::Focus {
                target: FocusTarget::Workspace { name, monitor },
            });
        }
        if let Some(rest) = s.strip_prefix("move workspace ") {
            let (name, monitor) = parse_workspace_selector(rest);
            return Ok(Action::Move {
                target: MoveTarget::Workspace { name, monitor },
            });
        }

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
            ["focus", "tab", "next"] => Ok(Action::Focus {
                target: FocusTarget::Tab {
                    direction: TabDirection::Next,
                },
            }),
            ["focus", "tab", "prev"] => Ok(Action::Focus {
                target: FocusTarget::Tab {
                    direction: TabDirection::Prev,
                },
            }),
            ["focus", "monitor", target] => Ok(Action::Focus {
                target: FocusTarget::Monitor {
                    target: parse_monitor_target(target)?,
                },
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
            ["move", "monitor", target] => Ok(Action::Move {
                target: MoveTarget::Monitor {
                    target: parse_monitor_target(target)?,
                },
            }),
            ["toggle", "spawn"] => Ok(Action::Toggle {
                target: ToggleTarget::Spawn,
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
            ["toggle", "fullscreen"] => Ok(Action::Toggle {
                target: ToggleTarget::Fullscreen,
            }),
            ["grow"] => Ok(Action::Grow),
            ["shrink"] => Ok(Action::Shrink),
            ["master", "more"] => Ok(Action::Master {
                target: MasterTarget::More,
            }),
            ["master", "fewer"] => Ok(Action::Master {
                target: MasterTarget::Fewer,
            }),
            ["exit"] => Ok(Action::Exit),
            ["close"] => Ok(Action::Close),
            _ => Err(anyhow!("Unknown action: {}", s)),
        }
    }
}

fn parse_monitor_target(s: &str) -> Result<MonitorTarget> {
    match s {
        "up" => Ok(MonitorTarget::Up),
        "down" => Ok(MonitorTarget::Down),
        "left" => Ok(MonitorTarget::Left),
        "right" => Ok(MonitorTarget::Right),
        name => Ok(MonitorTarget::Name(name.to_string())),
    }
}

// Monitor is pinned last: everything after the first " --monitor " is the
// monitor name, everything before is the workspace name. This lets both the
// workspace name and the monitor name contain spaces, which a
// split_whitespace match could not.
fn parse_workspace_selector(rest: &str) -> (String, Option<String>) {
    match rest.split_once(" --monitor ") {
        Some((name, monitor)) => (name.trim().to_string(), Some(monitor.trim().to_string())),
        None => (rest.trim().to_string(), None),
    }
}
