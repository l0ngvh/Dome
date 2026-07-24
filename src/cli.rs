use clap::{Parser, Subcommand};

use crate::action::{
    Action, FocusTarget, MasterTarget, MonitorTarget, MoveTarget, Query, TabDirection,
    ToggleTarget, parse_monitor_target,
};
use crate::core::WindowId;

#[derive(Parser)]
#[command(name = "dome", about = "A cross-platform tiling window manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand)]
enum CliCommand {
    Launch {
        #[arg(short, long)]
        config: Option<String>,
        #[arg(short, long)]
        layout: Option<String>,
    },
    Focus {
        #[command(subcommand)]
        target: CliFocus,
    },
    Move {
        #[command(subcommand)]
        target: CliMove,
    },
    Toggle {
        #[command(subcommand)]
        target: CliToggle,
    },
    Master {
        #[command(subcommand)]
        target: CliMaster,
    },
    Exec {
        command: String,
    },
    Exit,
    Close,
    Mode {
        name: String,
    },
    Export,
    Query {
        #[command(subcommand)]
        query: CliQuery,
    },
    #[command(name = "unminimize-window")]
    UnminimizeWindow {
        id: u64,
    },
}

#[derive(Subcommand)]
enum CliFocus {
    Up,
    Down,
    Left,
    Right,
    Parent,
    Tab {
        #[command(subcommand)]
        direction: CliTab,
    },
    Workspace {
        name: String,
    },
    Monitor {
        #[arg(value_parser = parse_monitor_target)]
        target: MonitorTarget,
    },
}

#[derive(Subcommand)]
enum CliMove {
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

#[derive(Subcommand)]
enum CliToggle {
    Spawn,
    Direction,
    Layout,
    Float,
    Fullscreen,
    Minimized,
}

#[derive(Subcommand)]
enum CliMaster {
    Grow,
    Shrink,
    More,
    Fewer,
}

#[derive(Subcommand)]
enum CliTab {
    Next,
    Prev,
}

#[derive(Subcommand)]
enum CliQuery {
    Workspaces,
    #[command(name = "minimized")]
    MinimizedWindows,
}

#[derive(Debug)]
enum Dispatch {
    Launch {
        config: Option<String>,
        layout: Option<String>,
    },
    Action(Action),
    Query(Query),
    Export,
}

impl From<CliFocus> for FocusTarget {
    fn from(cf: CliFocus) -> Self {
        match cf {
            CliFocus::Up => FocusTarget::Up,
            CliFocus::Down => FocusTarget::Down,
            CliFocus::Left => FocusTarget::Left,
            CliFocus::Right => FocusTarget::Right,
            CliFocus::Parent => FocusTarget::Parent,
            CliFocus::Tab { direction } => FocusTarget::Tab {
                direction: direction.into(),
            },
            CliFocus::Workspace { name } => FocusTarget::Workspace { name },
            CliFocus::Monitor { target } => FocusTarget::Monitor { target },
        }
    }
}

impl From<CliMove> for MoveTarget {
    fn from(cm: CliMove) -> Self {
        match cm {
            CliMove::Up => MoveTarget::Up,
            CliMove::Down => MoveTarget::Down,
            CliMove::Left => MoveTarget::Left,
            CliMove::Right => MoveTarget::Right,
            CliMove::Workspace { name } => MoveTarget::Workspace { name },
            CliMove::Monitor { target } => MoveTarget::Monitor { target },
        }
    }
}

impl From<CliMaster> for MasterTarget {
    fn from(cm: CliMaster) -> Self {
        match cm {
            CliMaster::Grow => MasterTarget::Grow,
            CliMaster::Shrink => MasterTarget::Shrink,
            CliMaster::More => MasterTarget::More,
            CliMaster::Fewer => MasterTarget::Fewer,
        }
    }
}

impl From<CliTab> for TabDirection {
    fn from(ct: CliTab) -> Self {
        match ct {
            CliTab::Next => TabDirection::Next,
            CliTab::Prev => TabDirection::Prev,
        }
    }
}

impl From<CliQuery> for Query {
    fn from(cq: CliQuery) -> Self {
        match cq {
            CliQuery::Workspaces => Query::Workspaces,
            CliQuery::MinimizedWindows => Query::MinimizedWindows,
        }
    }
}

fn cli_toggle_to_action(t: CliToggle) -> Action {
    match t {
        CliToggle::Minimized => Action::ToggleMinimized,
        CliToggle::Spawn => Action::Toggle(ToggleTarget::Spawn),
        CliToggle::Direction => Action::Toggle(ToggleTarget::Direction),
        CliToggle::Layout => Action::Toggle(ToggleTarget::Layout),
        CliToggle::Float => Action::Toggle(ToggleTarget::Float),
        CliToggle::Fullscreen => Action::Toggle(ToggleTarget::Fullscreen),
    }
}

impl From<CliCommand> for Dispatch {
    fn from(cmd: CliCommand) -> Self {
        match cmd {
            CliCommand::Launch { config, layout } => Dispatch::Launch { config, layout },
            CliCommand::Focus { target } => Dispatch::Action(Action::Focus(target.into())),
            CliCommand::Move { target } => Dispatch::Action(Action::Move(target.into())),
            CliCommand::Toggle { target } => Dispatch::Action(cli_toggle_to_action(target)),
            CliCommand::Master { target } => Dispatch::Action(Action::Master(target.into())),
            CliCommand::Exec { command } => Dispatch::Action(Action::Exec { command }),
            CliCommand::Exit => Dispatch::Action(Action::Exit),
            CliCommand::Close => Dispatch::Action(Action::Close),
            CliCommand::Mode { name } => Dispatch::Action(Action::Mode { name }),
            CliCommand::Export => Dispatch::Export,
            CliCommand::Query { query } => Dispatch::Query(query.into()),
            CliCommand::UnminimizeWindow { id } => {
                // WindowId's tuple-struct constructor is pub(crate) in core, so round-trip
                // through serde instead. Its Deserialize impl accepts a bare integer, and
                // every u64 fits in usize on the 64-bit targets Dome supports.
                let window_id: WindowId = serde_json::from_value(serde_json::json!(id))
                    .expect("WindowId round-trips from a bare integer");
                Dispatch::Action(Action::UnminimizeWindow(window_id))
            }
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let dispatch = match cli.command {
        None => Dispatch::Launch {
            config: None,
            layout: None,
        },
        Some(cmd) => Dispatch::from(cmd),
    };

    match dispatch {
        Dispatch::Launch { config, layout } => crate::run_app(config, layout)?,
        Dispatch::Action(action) => {
            crate::DomeClient.send_action(&action)?;
        }
        Dispatch::Query(query) => {
            let response = crate::DomeClient.send_query(&query)?;
            println!("{response}");
        }
        Dispatch::Export => {
            crate::DomeClient.send_export_layout()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispatch_from_argv(argv: &[&str]) -> Dispatch {
        let cli = Cli::try_parse_from(argv).expect("parse");
        match cli.command {
            None => Dispatch::Launch {
                config: None,
                layout: None,
            },
            Some(cmd) => Dispatch::from(cmd),
        }
    }

    fn assert_action(argv: &[&str], expected: &str) {
        match dispatch_from_argv(argv) {
            Dispatch::Action(a) => assert_eq!(a.to_string(), expected, "{argv:?}"),
            other => panic!("{argv:?} produced {other:?}, expected Action({expected:?})"),
        }
    }

    #[test]
    fn cli_focus_subcommands() {
        assert_action(&["dome", "focus", "up"], "focus up");
        assert_action(&["dome", "focus", "down"], "focus down");
        assert_action(&["dome", "focus", "left"], "focus left");
        assert_action(&["dome", "focus", "right"], "focus right");
        assert_action(&["dome", "focus", "parent"], "focus parent");
        assert_action(&["dome", "focus", "tab", "next"], "focus tab next");
        assert_action(&["dome", "focus", "tab", "prev"], "focus tab prev");
        assert_action(&["dome", "focus", "workspace", "3"], "focus workspace 3");
        assert_action(&["dome", "focus", "monitor", "left"], "focus monitor left");
        assert_action(&["dome", "focus", "monitor", "foo"], "focus monitor foo");
    }

    #[test]
    fn cli_move_subcommands() {
        assert_action(&["dome", "move", "up"], "move up");
        assert_action(&["dome", "move", "down"], "move down");
        assert_action(&["dome", "move", "left"], "move left");
        assert_action(&["dome", "move", "right"], "move right");
        assert_action(&["dome", "move", "workspace", "3"], "move workspace 3");
        assert_action(&["dome", "move", "monitor", "left"], "move monitor left");
    }

    #[test]
    fn cli_toggle_subcommands() {
        assert_action(&["dome", "toggle", "spawn"], "toggle spawn");
        assert_action(&["dome", "toggle", "direction"], "toggle direction");
        assert_action(&["dome", "toggle", "layout"], "toggle layout");
        assert_action(&["dome", "toggle", "float"], "toggle float");
        assert_action(&["dome", "toggle", "fullscreen"], "toggle fullscreen");
        assert_action(&["dome", "toggle", "minimized"], "toggle minimized");
        // Verify `toggle minimized` maps to the dedicated ToggleMinimized variant
        let d = dispatch_from_argv(&["dome", "toggle", "minimized"]);
        match d {
            Dispatch::Action(Action::ToggleMinimized) => {}
            other => panic!("expected Action(ToggleMinimized), got {other:?}"),
        }
    }

    #[test]
    fn cli_master_subcommands() {
        assert_action(&["dome", "master", "grow"], "master grow");
        assert_action(&["dome", "master", "shrink"], "master shrink");
        assert_action(&["dome", "master", "more"], "master more");
        assert_action(&["dome", "master", "fewer"], "master fewer");
    }

    #[test]
    fn cli_exec_passthrough() {
        assert_action(
            &["dome", "exec", "open -a Terminal"],
            "exec open -a Terminal",
        );
    }

    #[test]
    fn cli_mode() {
        assert_action(&["dome", "mode", "resize"], "mode resize");
    }

    #[test]
    fn cli_exit() {
        assert_action(&["dome", "exit"], "exit");
    }

    #[test]
    fn cli_close() {
        assert_action(&["dome", "close"], "close");
    }

    #[test]
    fn cli_query_workspaces() {
        let d = dispatch_from_argv(&["dome", "query", "workspaces"]);
        match d {
            Dispatch::Query(Query::Workspaces) => {}
            other => panic!("expected Query(Workspaces), got {other:?}"),
        }
    }

    #[test]
    fn cli_query_minimized() {
        let d = dispatch_from_argv(&["dome", "query", "minimized"]);
        match d {
            Dispatch::Query(Query::MinimizedWindows) => {}
            other => panic!("expected Query(MinimizedWindows), got {other:?}"),
        }
    }

    #[test]
    fn cli_unminimize_window() {
        let expected: WindowId = serde_json::from_value(serde_json::json!(7)).unwrap();
        let d = dispatch_from_argv(&["dome", "unminimize-window", "7"]);
        match d {
            Dispatch::Action(Action::UnminimizeWindow(id)) if id == expected => {}
            other => panic!("expected Action(UnminimizeWindow(7)), got {other:?}"),
        }
    }

    #[test]
    fn cli_launch_default() {
        let d = dispatch_from_argv(&["dome"]);
        match d {
            Dispatch::Launch {
                config: None,
                layout: None,
            } => {}
            other => panic!("expected Launch {{ None, None }}, got {other:?}"),
        }
    }

    #[test]
    fn cli_launch_with_config() {
        let d = dispatch_from_argv(&["dome", "launch", "--config", "/tmp/c"]);
        match d {
            Dispatch::Launch {
                config: Some(ref s),
                layout: None,
            } if s == "/tmp/c" => {}
            other => panic!("expected Launch {{ Some(\"/tmp/c\"), None }}, got {other:?}"),
        }
    }

    #[test]
    fn cli_launch_with_layout() {
        let d = dispatch_from_argv(&["dome", "launch", "--layout", "/tmp/l"]);
        match d {
            Dispatch::Launch {
                config: None,
                layout: Some(ref s),
            } if s == "/tmp/l" => {}
            other => panic!("expected Launch {{ None, Some(\"/tmp/l\") }}, got {other:?}"),
        }
    }

    #[test]
    fn cli_launch_with_config_and_layout() {
        let d = dispatch_from_argv(&["dome", "launch", "--config", "/tmp/c", "--layout", "/tmp/l"]);
        match d {
            Dispatch::Launch {
                config: Some(ref c),
                layout: Some(ref l),
            } if c == "/tmp/c" && l == "/tmp/l" => {}
            other => {
                panic!("expected Launch {{ Some(\"/tmp/c\"), Some(\"/tmp/l\") }}, got {other:?}")
            }
        }
    }
}
