use std::io::Write;

use clap::{Parser, Subcommand};
use serde::de::DeserializeOwned;

use crate::action::{
    Action, FocusTarget, MasterTarget, MinimizedWindow, MonitorDetails, MonitorTarget, MoveTarget,
    Query, TabDirection, ToggleTarget, WorkspaceInfo, WorkspaceState,
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

#[derive(Subcommand, Debug)]
enum CliQuery {
    Workspaces {
        #[arg(long)]
        monitor: Option<String>,
    },
    #[command(name = "minimized")]
    MinimizedWindows,
    Monitors,
}

#[derive(Subcommand, Debug)]
enum CliGenerate {
    Sketchybar,
}

#[derive(Debug)]
enum Dispatch {
    Launch {
        config: Option<String>,
        layout: Option<String>,
    },
    Action(Action),
    Query(CliQuery),
    Export,
    Generate(CliGenerate),
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
            CliCommand::Query { query } => Dispatch::Query(query),
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

/// The running dome that a command talks to.
trait Client {
    fn ping(&self) -> bool;
    fn action(&self, action: &Action) -> anyhow::Result<()>;
    fn export_layout(&self) -> anyhow::Result<()>;
    fn query<T: DeserializeOwned>(&self, query: &Query) -> anyhow::Result<T>;
}

impl Client for crate::DomeClient {
    fn ping(&self) -> bool {
        crate::DomeClient::ping(self)
    }

    fn action(&self, action: &Action) -> anyhow::Result<()> {
        crate::DomeClient::action(self, action)
    }

    fn export_layout(&self) -> anyhow::Result<()> {
        crate::DomeClient::export_layout(self)
    }

    fn query<T: DeserializeOwned>(&self, query: &Query) -> anyhow::Result<T> {
        crate::DomeClient::query(self, query)
    }
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    execute(
        dispatch_from(cli.command),
        &crate::DomeClient,
        &mut std::io::stdout(),
    )
}

fn execute(dispatch: Dispatch, client: &impl Client, out: &mut impl Write) -> anyhow::Result<()> {
    match dispatch {
        Dispatch::Launch { config, layout } => {
            if client.ping() {
                anyhow::bail!("dome is already running");
            }
            crate::run_app(config, layout)?
        }
        Dispatch::Action(action) => {
            client.action(&action)?;
        }
        Dispatch::Query(query) => {
            let json = match query {
                CliQuery::Workspaces { monitor } => {
                    let mut rows: Vec<WorkspaceInfo> = client.query(&Query::Workspaces)?;
                    let monitors: Vec<MonitorDetails> = client.query(&Query::Monitors)?;
                    match monitor {
                        Some(wanted) => {
                            let Some(found) = monitors.iter().find(|m| {
                                m.unique_name == wanted
                                    || m.gdi_device.as_deref() == Some(wanted.as_str())
                            }) else {
                                anyhow::bail!(
                                    "no connected monitor matches {wanted:?}. Run `dome query \
                                     monitors` to list each unique_name and gdi_device."
                                );
                            };
                            rows.retain(|ws| {
                                ws.state == WorkspaceState::Attached
                                    && ws.monitor == found.unique_name
                            });
                            rows.sort_by(|a, b| name_key(&a.name).cmp(&name_key(&b.name)));
                        }
                        None => {
                            rows.sort_by(|a, b| {
                                (group_key(a, &monitors), name_key(&a.name))
                                    .cmp(&(group_key(b, &monitors), name_key(&b.name)))
                            });
                        }
                    }
                    serde_json::to_string(&rows)?
                }
                CliQuery::MinimizedWindows => {
                    serde_json::to_string(
                        &client.query::<Vec<MinimizedWindow>>(&Query::MinimizedWindows)?,
                    )?
                }
                CliQuery::Monitors => {
                    serde_json::to_string(&client.query::<Vec<MonitorDetails>>(&Query::Monitors)?)?
                }
            };
            writeln!(out, "{json}")?;
        }
        Dispatch::Export => {
            client.export_layout()?;
        }
        Dispatch::Generate(CliGenerate::Sketchybar) => {
            crate::integrations::sketchybar::generate()?;
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

// Reordering these variants changes the sort order.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum GroupKey<'a> {
    /// Position of the row's monitor in the monitor list.
    Known(usize),
    /// Monitor name of an attached row whose monitor the monitor list lacks.
    Unknown(&'a str),
    /// Origin monitor name of a parked row.
    Parked(&'a str),
}

fn group_key<'a>(ws: &'a WorkspaceInfo, monitors: &[MonitorDetails]) -> GroupKey<'a> {
    if ws.state == WorkspaceState::Parked {
        return GroupKey::Parked(&ws.monitor);
    }
    match monitors.iter().position(|m| m.unique_name == ws.monitor) {
        Some(index) => GroupKey::Known(index),
        None => GroupKey::Unknown(&ws.monitor),
    }
}

// Numeric names sort before text names. Swapping these variants reverses that.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum NameKey<'a> {
    /// The text breaks a tie between equal values such as `1` and `01`.
    Numeric(i128, &'a str),
    Text(&'a str),
}

// `i128::from_str` accepts exactly `[+-]?[0-9]+`. A name past the `i128` range fails to
// parse and sorts as text.
fn name_key(name: &str) -> NameKey<'_> {
    match name.parse::<i128>() {
        Ok(value) => NameKey::Numeric(value, name),
        Err(_) => NameKey::Text(name),
    }
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

    struct MockClient {
        workspaces: Vec<WorkspaceInfo>,
        monitors: Vec<MonitorDetails>,
    }

    impl Client for MockClient {
        fn ping(&self) -> bool {
            unreachable!("no test launches dome")
        }

        fn action(&self, _: &Action) -> anyhow::Result<()> {
            unreachable!("no test sends an action")
        }

        fn export_layout(&self) -> anyhow::Result<()> {
            unreachable!("no test exports a layout")
        }

        fn query<T: DeserializeOwned>(&self, query: &Query) -> anyhow::Result<T> {
            let data = match query {
                Query::Workspaces => serde_json::to_value(&self.workspaces)?,
                Query::Monitors => serde_json::to_value(&self.monitors)?,
                Query::MinimizedWindows => unreachable!("no test queries minimized windows"),
            };
            Ok(serde_json::from_value(data)?)
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
    fn cli_query_monitors() {
        let d = dispatch_from_argv(&["dome", "query", "monitors"]);
        match d {
            Dispatch::Query(CliQuery::Monitors) => {}
            other => panic!("expected Query(Monitors), got {other:?}"),
        }
    }

    #[test]
    fn cli_query_minimized() {
        let d = dispatch_from_argv(&["dome", "query", "minimized"]);
        match d {
            Dispatch::Query(CliQuery::MinimizedWindows) => {}
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
    fn cli_generate_sketchybar() {
        let d = dispatch_from_argv(&["dome", "generate", "sketchybar"]);
        match d {
            Dispatch::Generate(CliGenerate::Sketchybar) => {}
            other => panic!("expected Generate(Sketchybar), got {other:?}"),
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

    fn monitor(unique_name: &str) -> MonitorDetails {
        MonitorDetails {
            device_name: unique_name.to_string(),
            unique_name: unique_name.to_string(),
            cg_display_id: None,
            gdi_device: None,
            work_area: crate::action::MonitorFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
        }
    }

    fn workspace(name: &str, monitor: &str, state: WorkspaceState) -> WorkspaceInfo {
        WorkspaceInfo {
            name: name.to_string(),
            monitor: monitor.to_string(),
            state,
            is_focused: false,
            is_visible: false,
        }
    }

    fn query_workspaces(client: &MockClient, flags: &[&str]) -> anyhow::Result<Vec<WorkspaceInfo>> {
        let argv = [&["dome", "query", "workspaces"], flags].concat();
        let mut out = Vec::new();
        execute(dispatch_from_argv(&argv), client, &mut out)?;
        Ok(serde_json::from_slice(&out)?)
    }

    fn rows(printed: &[WorkspaceInfo]) -> Vec<(&str, &str, WorkspaceState)> {
        printed
            .iter()
            .map(|ws| (ws.monitor.as_str(), ws.name.as_str(), ws.state.clone()))
            .collect()
    }

    #[test]
    fn cli_query_workspaces_order() {
        use WorkspaceState::{Attached, Parked};

        let past_i128 = "9".repeat(40);
        let workspaces = vec![
            workspace("1", "Old", Parked),
            workspace("web", "Left", Attached),
            workspace("10", "Left", Attached),
            workspace("2", "Gone", Parked),
            workspace("Mail", "Left", Attached),
            workspace("2", "Left", Attached),
            workspace("+3", "Left", Attached),
            workspace("-1", "Left", Attached),
            workspace(&past_i128, "Left", Attached),
            workspace("1", "Left", Attached),
            workspace("01", "Left", Attached),
            workspace("1", "Unlisted", Attached),
            workspace("5", "Right", Attached),
            workspace("4", "Right", Attached),
            workspace("1", "Gone", Parked),
        ];
        let client = MockClient {
            workspaces,
            monitors: vec![monitor("Right"), monitor("Left")],
        };

        let printed = query_workspaces(&client, &[]).unwrap();
        assert_eq!(
            rows(&printed),
            [
                ("Right", "4", Attached),
                ("Right", "5", Attached),
                ("Left", "-1", Attached),
                ("Left", "01", Attached),
                ("Left", "1", Attached),
                ("Left", "2", Attached),
                ("Left", "+3", Attached),
                ("Left", "10", Attached),
                ("Left", past_i128.as_str(), Attached),
                ("Left", "Mail", Attached),
                ("Left", "web", Attached),
                ("Unlisted", "1", Attached),
                ("Gone", "1", Parked),
                ("Gone", "2", Parked),
                ("Old", "1", Parked),
            ]
        );
    }

    #[test]
    fn cli_query_workspaces_on_one_monitor_lists_its_attached_workspaces_in_name_order() {
        use WorkspaceState::{Attached, Parked};

        let client = MockClient {
            workspaces: vec![
                workspace("b", "Left", Attached),
                workspace("2", "Right", Attached),
                workspace("1", "Left", Parked),
                workspace("a", "Left", Attached),
                workspace("10", "Left", Attached),
                workspace("9", "Left", Attached),
            ],
            monitors: vec![monitor("Right"), monitor("Left")],
        };

        let printed = query_workspaces(&client, &["--monitor", "Left"]).unwrap();
        assert_eq!(
            rows(&printed),
            [
                ("Left", "9", Attached),
                ("Left", "10", Attached),
                ("Left", "a", Attached),
                ("Left", "b", Attached),
            ]
        );
    }

    #[test]
    fn cli_query_workspaces_on_one_monitor_matches_its_gdi_device() {
        use WorkspaceState::Attached;

        let client = MockClient {
            workspaces: vec![
                workspace("1", "Left", Attached),
                workspace("2", "Right", Attached),
            ],
            monitors: vec![
                monitor("Right"),
                MonitorDetails {
                    gdi_device: Some(r"\\.\DISPLAY2".to_string()),
                    ..monitor("Left")
                },
            ],
        };

        let printed = query_workspaces(&client, &["--monitor", r"\\.\DISPLAY2"]).unwrap();
        assert_eq!(rows(&printed), [("Left", "1", Attached)]);
    }

    #[test]
    fn cli_query_workspaces_prints_focus_and_visibility() {
        use WorkspaceState::Attached;

        let client = MockClient {
            workspaces: vec![
                WorkspaceInfo {
                    is_focused: true,
                    is_visible: true,
                    ..workspace("1", "Left", Attached)
                },
                WorkspaceInfo {
                    is_visible: true,
                    ..workspace("2", "Right", Attached)
                },
                workspace("3", "Left", Attached),
            ],
            monitors: vec![monitor("Left"), monitor("Right")],
        };

        let printed = query_workspaces(&client, &[]).unwrap();
        let flags: Vec<(&str, bool, bool)> = printed
            .iter()
            .map(|ws| (ws.name.as_str(), ws.is_focused, ws.is_visible))
            .collect();
        assert_eq!(
            flags,
            [("1", true, true), ("3", false, false), ("2", false, true)]
        );
    }

    #[test]
    fn cli_query_workspaces_on_an_unknown_monitor_fails() {
        let client = MockClient {
            workspaces: vec![workspace("1", "Left", WorkspaceState::Attached)],
            monitors: vec![monitor("Left")],
        };

        let error = query_workspaces(&client, &["--monitor", "Nope"]).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with(r#"no connected monitor matches "Nope"."#),
            "{error}"
        );
    }
}
