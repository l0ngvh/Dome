use clap::{Parser, Subcommand};

use crate::action::{
    Action, FocusTarget, MasterTarget, MonitorTarget, MoveTarget, Query, TabDirection,
    ToggleTarget, parse_monitor_target,
};

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
    Mode {
        name: String,
    },
    Query {
        #[command(subcommand)]
        query: CliQuery,
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
}

#[derive(Debug)]
enum Dispatch {
    Launch(Option<String>),
    Action(Action),
    Query(Query),
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
            CliCommand::Launch { config } => Dispatch::Launch(config),
            CliCommand::Focus { target } => Dispatch::Action(Action::Focus(target.into())),
            CliCommand::Move { target } => Dispatch::Action(Action::Move(target.into())),
            CliCommand::Toggle { target } => Dispatch::Action(cli_toggle_to_action(target)),
            CliCommand::Master { target } => Dispatch::Action(Action::Master(target.into())),
            CliCommand::Exec { command } => Dispatch::Action(Action::Exec { command }),
            CliCommand::Exit => Dispatch::Action(Action::Exit),
            CliCommand::Mode { name } => Dispatch::Action(Action::Mode { name }),
            CliCommand::Query { query } => Dispatch::Query(query.into()),
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let dispatch = match cli.command {
        None => Dispatch::Launch(None),
        Some(cmd) => Dispatch::from(cmd),
    };

    match dispatch {
        Dispatch::Launch(config) => crate::run_app(config)?,
        Dispatch::Action(action) => {
            crate::DomeClient.send_action(&action)?;
        }
        Dispatch::Query(query) => {
            let response = crate::DomeClient.send_query(&query)?;
            println!("{response}");
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
            None => Dispatch::Launch(None),
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
    fn cli_query_workspaces() {
        let d = dispatch_from_argv(&["dome", "query", "workspaces"]);
        match d {
            Dispatch::Query(Query::Workspaces) => {}
            other => panic!("expected Query(Workspaces), got {other:?}"),
        }
    }

    #[test]
    fn cli_launch_default() {
        let d = dispatch_from_argv(&["dome"]);
        match d {
            Dispatch::Launch(None) => {}
            other => panic!("expected Launch(None), got {other:?}"),
        }
    }

    #[test]
    fn cli_launch_with_config() {
        let d = dispatch_from_argv(&["dome", "launch", "--config", "/tmp/c"]);
        match d {
            Dispatch::Launch(Some(ref s)) if s == "/tmp/c" => {}
            other => panic!("expected Launch(Some(\"/tmp/c\")), got {other:?}"),
        }
    }
}
