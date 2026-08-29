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
    Generate {
        #[command(subcommand)]
        bar: CliGenerate,
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
        #[arg(long)]
        monitor: Option<String>,
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
        #[arg(long)]
        monitor: Option<String>,
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
    Monitors,
}

#[derive(Subcommand, Debug)]
enum CliGenerate {
    Yasb {
        /// YASB config.yaml to edit. Defaults to the YASB config location.
        #[arg(long)]
        config: Option<String>,
    },
    Sketchybar,
    Zebar {
        /// Directory to scaffold the widget pack into. Defaults to the Zebar
        /// pack location.
        #[arg(long)]
        out: Option<String>,
    },
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
    Generate(CliGenerate),
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
            CliFocus::Workspace { name, monitor } => FocusTarget::Workspace { name, monitor },
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
            CliMove::Workspace { name, monitor } => MoveTarget::Workspace { name, monitor },
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
            CliQuery::Monitors => Query::Monitors,
        }
    }
}

fn cli_toggle_to_action(t: CliToggle) -> Action {
    match t {
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
            CliCommand::Generate { bar } => Dispatch::Generate(bar),
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
            crate::DomeClient.action(&action)?;
        }
        Dispatch::Query(query) => {
            let response = crate::DomeClient.query(&query)?;
            println!("{response}");
        }
        Dispatch::Export => {
            crate::DomeClient.export()?;
        }
        Dispatch::Generate(CliGenerate::Yasb { config }) => {
            crate::integrations::yasb::generate(config.as_deref())?;
        }
        Dispatch::Generate(CliGenerate::Sketchybar) => {
            crate::integrations::sketchybar::generate()?;
        }
        Dispatch::Generate(CliGenerate::Zebar { out }) => {
            crate::integrations::zebar::generate(out.as_deref())?;
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

    fn focus_target(argv: &[&str]) -> FocusTarget {
        match dispatch_from_argv(argv) {
            Dispatch::Action(Action::Focus(t)) => t,
            other => panic!("{argv:?} produced {other:?}, expected Focus"),
        }
    }

    fn move_target(argv: &[&str]) -> MoveTarget {
        match dispatch_from_argv(argv) {
            Dispatch::Action(Action::Move(t)) => t,
            other => panic!("{argv:?} produced {other:?}, expected Move"),
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
    fn cli_focus_workspace_without_monitor() {
        match focus_target(&["dome", "focus", "workspace", "3"]) {
            FocusTarget::Workspace { name, monitor } => {
                assert_eq!(name, "3");
                assert_eq!(monitor, None);
            }
            other => panic!("expected Workspace, got {other:?}"),
        }
    }

    #[test]
    fn cli_focus_workspace_with_monitor() {
        match focus_target(&[
            "dome",
            "focus",
            "workspace",
            "3",
            "--monitor",
            "DELL U2720Q #1",
        ]) {
            FocusTarget::Workspace { name, monitor } => {
                assert_eq!(name, "3");
                assert_eq!(monitor.as_deref(), Some("DELL U2720Q #1"));
            }
            other => panic!("expected Workspace, got {other:?}"),
        }
    }

    #[test]
    fn cli_move_workspace_without_monitor() {
        match move_target(&["dome", "move", "workspace", "3"]) {
            MoveTarget::Workspace { name, monitor } => {
                assert_eq!(name, "3");
                assert_eq!(monitor, None);
            }
            other => panic!("expected Workspace, got {other:?}"),
        }
    }

    #[test]
    fn cli_move_workspace_with_monitor() {
        match move_target(&["dome", "move", "workspace", "2", "--monitor", "B"]) {
            MoveTarget::Workspace { name, monitor } => {
                assert_eq!(name, "2");
                assert_eq!(monitor.as_deref(), Some("B"));
            }
            other => panic!("expected Workspace, got {other:?}"),
        }
    }

    #[test]
    fn cli_toggle_subcommands() {
        assert_action(&["dome", "toggle", "spawn"], "toggle spawn");
        assert_action(&["dome", "toggle", "direction"], "toggle direction");
        assert_action(&["dome", "toggle", "layout"], "toggle layout");
        assert_action(&["dome", "toggle", "float"], "toggle float");
        assert_action(&["dome", "toggle", "fullscreen"], "toggle fullscreen");
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
    fn cli_query_monitors() {
        let d = dispatch_from_argv(&["dome", "query", "monitors"]);
        match d {
            Dispatch::Query(Query::Monitors) => {}
            other => panic!("expected Query(Monitors), got {other:?}"),
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
    fn cli_generate_yasb() {
        let d = dispatch_from_argv(&["dome", "generate", "yasb"]);
        match d {
            Dispatch::Generate(CliGenerate::Yasb { config: None }) => {}
            other => panic!("expected Generate(Yasb) with no config, got {other:?}"),
        }
    }

    #[test]
    fn cli_generate_sketchybar() {
        let d = dispatch_from_argv(&["dome", "generate", "sketchybar"]);
        match d {
            Dispatch::Generate(CliGenerate::Sketchybar) => {}
            other => panic!("expected Generate(Sketchybar), got {other:?}"),
        }
    }

    #[test]
    fn cli_generate_zebar() {
        let d = dispatch_from_argv(&["dome", "generate", "zebar"]);
        match d {
            Dispatch::Generate(CliGenerate::Zebar { out: None }) => {}
            other => panic!("expected Generate(Zebar) with no out, got {other:?}"),
        }
    }

    #[test]
    fn cli_generate_zebar_with_out() {
        let d = dispatch_from_argv(&["dome", "generate", "zebar", "--out", "/tmp/pack"]);
        match d {
            Dispatch::Generate(CliGenerate::Zebar { out: Some(ref p) }) if p == "/tmp/pack" => {}
            other => panic!("expected Generate(Zebar) with out, got {other:?}"),
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
