use clap::{Parser, Subcommand};

use crate::action::{
    Action, FocusTarget, MasterTarget, MonitorTarget, MoveTarget, Query, TabDirection, ToggleTarget,
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
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    FocusParent,
    FocusTabNext,
    FocusTabPrev,
    FocusWorkspace {
        name: String,
        #[arg(long)]
        monitor: Option<String>,
    },
    FocusMonitorLeft,
    FocusMonitorRight,
    FocusMonitorUp,
    FocusMonitorDown,
    FocusMonitor {
        // Never matched against the four direction words, which have their own
        // commands, so a monitor named `left` stays reachable.
        name: String,
    },
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveToWorkspace {
        name: String,
        #[arg(long)]
        monitor: Option<String>,
    },
    MoveToMonitorLeft,
    MoveToMonitorRight,
    MoveToMonitorUp,
    MoveToMonitorDown,
    MoveToMonitor {
        name: String,
    },
    ToggleSplit,
    Rotate,
    ToggleTabbed,
    ToggleFloat,
    ToggleFullscreen,
    IncreaseMasterRatio,
    DecreaseMasterRatio,
    IncreaseMasterCount,
    DecreaseMasterCount,
    Execute {
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
enum CliQuery {
    Workspaces,
    #[command(name = "minimized")]
    MinimizedWindows,
    Monitors,
}

#[derive(Subcommand, Debug)]
enum CliGenerate {
    Yasb,
    Sketchybar,
    Zebar,
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

impl From<CliQuery> for Query {
    fn from(cq: CliQuery) -> Self {
        match cq {
            CliQuery::Workspaces => Query::Workspaces,
            CliQuery::MinimizedWindows => Query::MinimizedWindows,
            CliQuery::Monitors => Query::Monitors,
        }
    }
}

impl From<CliCommand> for Dispatch {
    fn from(cmd: CliCommand) -> Self {
        match cmd {
            CliCommand::Launch { config, layout } => Dispatch::Launch { config, layout },
            CliCommand::FocusLeft => focus(FocusTarget::Left),
            CliCommand::FocusRight => focus(FocusTarget::Right),
            CliCommand::FocusUp => focus(FocusTarget::Up),
            CliCommand::FocusDown => focus(FocusTarget::Down),
            CliCommand::FocusParent => focus(FocusTarget::Parent),
            CliCommand::FocusTabNext => focus(FocusTarget::Tab {
                direction: TabDirection::Next,
            }),
            CliCommand::FocusTabPrev => focus(FocusTarget::Tab {
                direction: TabDirection::Prev,
            }),
            CliCommand::FocusWorkspace { name, monitor } => {
                focus(FocusTarget::Workspace { name, monitor })
            }
            CliCommand::FocusMonitorLeft => focus_monitor(MonitorTarget::Left),
            CliCommand::FocusMonitorRight => focus_monitor(MonitorTarget::Right),
            CliCommand::FocusMonitorUp => focus_monitor(MonitorTarget::Up),
            CliCommand::FocusMonitorDown => focus_monitor(MonitorTarget::Down),
            CliCommand::FocusMonitor { name } => focus_monitor(MonitorTarget::Name(name)),
            CliCommand::MoveLeft => move_window(MoveTarget::Left),
            CliCommand::MoveRight => move_window(MoveTarget::Right),
            CliCommand::MoveUp => move_window(MoveTarget::Up),
            CliCommand::MoveDown => move_window(MoveTarget::Down),
            CliCommand::MoveToWorkspace { name, monitor } => {
                move_window(MoveTarget::Workspace { name, monitor })
            }
            CliCommand::MoveToMonitorLeft => move_to_monitor(MonitorTarget::Left),
            CliCommand::MoveToMonitorRight => move_to_monitor(MonitorTarget::Right),
            CliCommand::MoveToMonitorUp => move_to_monitor(MonitorTarget::Up),
            CliCommand::MoveToMonitorDown => move_to_monitor(MonitorTarget::Down),
            CliCommand::MoveToMonitor { name } => move_to_monitor(MonitorTarget::Name(name)),
            CliCommand::ToggleSplit => toggle(ToggleTarget::Spawn),
            CliCommand::Rotate => toggle(ToggleTarget::Direction),
            CliCommand::ToggleTabbed => toggle(ToggleTarget::Layout),
            CliCommand::ToggleFloat => toggle(ToggleTarget::Float),
            CliCommand::ToggleFullscreen => toggle(ToggleTarget::Fullscreen),
            CliCommand::IncreaseMasterRatio => master(MasterTarget::Grow),
            CliCommand::DecreaseMasterRatio => master(MasterTarget::Shrink),
            CliCommand::IncreaseMasterCount => master(MasterTarget::More),
            CliCommand::DecreaseMasterCount => master(MasterTarget::Fewer),
            CliCommand::Execute { command } => Dispatch::Action(Action::Execute { command }),
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
                Dispatch::Action(Action::UnminimizeWindow { id: window_id })
            }
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let dispatch = dispatch_from(cli.command);

    match dispatch {
        Dispatch::Launch { config, layout } => {
            if crate::DomeClient.ping() {
                anyhow::bail!("dome is already running");
            }
            crate::run_app(config, layout)?
        }
        Dispatch::Action(action) => {
            crate::DomeClient.action(&action)?;
        }
        Dispatch::Query(query) => {
            let json = match query {
                Query::Workspaces => serde_json::to_string(&crate::DomeClient.workspaces()?)?,
                Query::MinimizedWindows => {
                    serde_json::to_string(&crate::DomeClient.minimized_windows()?)?
                }
                Query::Monitors => serde_json::to_string(&crate::DomeClient.monitors()?)?,
            };
            println!("{json}");
        }
        Dispatch::Export => {
            crate::DomeClient.export_layout()?;
        }
        Dispatch::Generate(CliGenerate::Yasb) => {
            crate::integrations::yasb::generate()?;
        }
        Dispatch::Generate(CliGenerate::Sketchybar) => {
            crate::integrations::sketchybar::generate()?;
        }
        Dispatch::Generate(CliGenerate::Zebar) => {
            crate::integrations::zebar::generate()?;
        }
    }
    Ok(())
}

fn dispatch_from(command: Option<CliCommand>) -> Dispatch {
    match command {
        None => Dispatch::Launch {
            config: None,
            layout: None,
        },
        Some(cmd) => Dispatch::from(cmd),
    }
}

fn focus(target: FocusTarget) -> Dispatch {
    Dispatch::Action(Action::Focus { target })
}

fn focus_monitor(target: MonitorTarget) -> Dispatch {
    focus(FocusTarget::Monitor { target })
}

fn move_window(target: MoveTarget) -> Dispatch {
    Dispatch::Action(Action::Move { target })
}

fn move_to_monitor(target: MonitorTarget) -> Dispatch {
    move_window(MoveTarget::Monitor { target })
}

fn toggle(target: ToggleTarget) -> Dispatch {
    Dispatch::Action(Action::Toggle { target })
}

fn master(target: MasterTarget) -> Dispatch {
    Dispatch::Action(Action::Master { target })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn dispatch_from_argv(argv: &[&str]) -> Dispatch {
        let cli = Cli::try_parse_from(argv).expect("parse");
        dispatch_from(cli.command)
    }

    fn focus_target(argv: &[&str]) -> FocusTarget {
        match dispatch_from_argv(argv) {
            Dispatch::Action(Action::Focus { target: t }) => t,
            other => panic!("{argv:?} produced {other:?}, expected Focus"),
        }
    }

    fn move_target(argv: &[&str]) -> MoveTarget {
        match dispatch_from_argv(argv) {
            Dispatch::Action(Action::Move { target: t }) => t,
            other => panic!("{argv:?} produced {other:?}, expected Move"),
        }
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn cli_launch_default() {
        match dispatch_from_argv(&["dome"]) {
            Dispatch::Launch {
                config: None,
                layout: None,
            } => {}
            other => panic!("expected Launch {{ None, None }}, got {other:?}"),
        }
    }

    #[test]
    fn cli_focus_monitor_takes_a_name_not_a_direction() {
        match focus_target(&["dome", "focus-monitor", "left"]) {
            FocusTarget::Monitor {
                target: MonitorTarget::Name(name),
            } => assert_eq!(name, "left"),
            other => panic!("expected Monitor {{ Name(\"left\") }}, got {other:?}"),
        }
    }

    #[test]
    fn cli_move_to_monitor_takes_a_name_not_a_direction() {
        match move_target(&["dome", "move-to-monitor", "left"]) {
            MoveTarget::Monitor {
                target: MonitorTarget::Name(name),
            } => assert_eq!(name, "left"),
            other => panic!("expected Monitor {{ Name(\"left\") }}, got {other:?}"),
        }
    }

    #[test]
    fn cli_focus_workspace_without_monitor() {
        match focus_target(&["dome", "focus-workspace", "3"]) {
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
            "focus-workspace",
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
    fn cli_move_to_workspace_without_monitor() {
        match move_target(&["dome", "move-to-workspace", "3"]) {
            MoveTarget::Workspace { name, monitor } => {
                assert_eq!(name, "3");
                assert_eq!(monitor, None);
            }
            other => panic!("expected Workspace, got {other:?}"),
        }
    }

    #[test]
    fn cli_move_to_workspace_with_monitor() {
        match move_target(&["dome", "move-to-workspace", "2", "--monitor", "B"]) {
            MoveTarget::Workspace { name, monitor } => {
                assert_eq!(name, "2");
                assert_eq!(monitor.as_deref(), Some("B"));
            }
            other => panic!("expected Workspace, got {other:?}"),
        }
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
            Dispatch::Action(Action::UnminimizeWindow { id }) if id == expected => {}
            other => panic!("expected Action(UnminimizeWindow(7)), got {other:?}"),
        }
    }

    #[test]
    fn cli_generate_yasb() {
        let d = dispatch_from_argv(&["dome", "generate", "yasb"]);
        match d {
            Dispatch::Generate(CliGenerate::Yasb) => {}
            other => panic!("expected Generate(Yasb), got {other:?}"),
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
            Dispatch::Generate(CliGenerate::Zebar) => {}
            other => panic!("expected Generate(Zebar), got {other:?}"),
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
