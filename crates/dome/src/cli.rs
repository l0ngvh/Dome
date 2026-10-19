use std::io::Write;

use clap::{Parser, Subcommand};
use serde::de::DeserializeOwned;

use crate::action::{
    Action, FocusTarget, MasterTarget, MinimizedWindow, MonitorDetails, MonitorTarget, MoveTarget,
    Query, TabDirection, ToggleTarget, WorkspaceInfo, WorkspaceState,
};
use dome_core::WindowId;

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    execute(
        dispatch_from(cli.command),
        &crate::DomeClient,
        &mut std::io::stdout(),
    )
}

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
    Grow,
    Shrink,
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
    SaveLayout,
    ApplyLayout,
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
    SaveLayout,
    ApplyLayout,
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
            CliCommand::Grow => Dispatch::Action(Action::Grow),
            CliCommand::Shrink => Dispatch::Action(Action::Shrink),
            CliCommand::IncreaseMasterCount => master(MasterTarget::More),
            CliCommand::DecreaseMasterCount => master(MasterTarget::Fewer),
            CliCommand::Execute { command } => Dispatch::Action(Action::Execute { command }),
            CliCommand::Exit => Dispatch::Action(Action::Exit),
            CliCommand::Close => Dispatch::Action(Action::Close),
            CliCommand::Mode { name } => Dispatch::Action(Action::Mode { name }),
            CliCommand::SaveLayout => Dispatch::SaveLayout,
            CliCommand::ApplyLayout => Dispatch::ApplyLayout,
            CliCommand::Query { query } => Dispatch::Query(query),
            CliCommand::Generate { bar } => Dispatch::Generate(bar),
            CliCommand::UnminimizeWindow { id } => {
                // WindowId's Deserialize impl accepts a bare integer, and every u64
                // fits in usize on the 64-bit targets Dome supports.
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
    fn save_layout(&self) -> anyhow::Result<()>;
    fn apply_layout(&self) -> anyhow::Result<()>;
    fn query<T: DeserializeOwned>(&self, query: &Query) -> anyhow::Result<T>;
}

impl Client for crate::DomeClient {
    fn ping(&self) -> bool {
        crate::DomeClient::ping(self)
    }

    fn action(&self, action: &Action) -> anyhow::Result<()> {
        crate::DomeClient::action(self, action)
    }

    fn save_layout(&self) -> anyhow::Result<()> {
        crate::DomeClient::save_layout(self)
    }

    fn apply_layout(&self) -> anyhow::Result<()> {
        crate::DomeClient::apply_layout(self)
    }

    fn query<T: DeserializeOwned>(&self, query: &Query) -> anyhow::Result<T> {
        crate::DomeClient::query(self, query)
    }
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
        Dispatch::SaveLayout => {
            client.save_layout()?;
        }
        Dispatch::ApplyLayout => {
            client.apply_layout()?;
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

// Numeric names sort before text names.
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
    use std::cell::RefCell;

    use super::*;
    use crate::action::MonitorFrame;
    use serde_json::{Value, json};

    enum MemoryClient {
        AlreadyRunning,
        Action(RefCell<Option<Action>>),
        Workspaces {
            rows: Vec<WorkspaceInfo>,
            monitors: Vec<MonitorDetails>,
        },
    }

    impl Client for MemoryClient {
        fn ping(&self) -> bool {
            assert!(matches!(self, Self::AlreadyRunning), "unexpected ping");
            true
        }

        fn action(&self, action: &Action) -> anyhow::Result<()> {
            let Self::Action(received) = self else {
                panic!("unexpected action {action:?}")
            };
            received.replace(Some(action.clone()));
            Ok(())
        }

        fn save_layout(&self) -> anyhow::Result<()> {
            panic!("unexpected save-layout")
        }

        fn apply_layout(&self) -> anyhow::Result<()> {
            panic!("unexpected apply-layout")
        }

        fn query<T: DeserializeOwned>(&self, query: &Query) -> anyhow::Result<T> {
            let Self::Workspaces { rows, monitors } = self else {
                panic!("unexpected query {query:?}")
            };
            let reply = match query {
                Query::Workspaces => serde_json::to_value(rows)?,
                Query::Monitors => serde_json::to_value(monitors)?,
                Query::MinimizedWindows => panic!("unexpected minimized query"),
            };
            Ok(serde_json::from_value(reply)?)
        }
    }

    fn run_command(
        arguments: &[&str],
        client: &MemoryClient,
        output: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(std::iter::once("dome").chain(arguments.iter().copied()))?;
        execute(dispatch_from(cli.command), client, output)
    }

    fn assert_action(arguments: &[&str], expected: Value) {
        let client = MemoryClient::Action(RefCell::new(None));
        let mut output = Vec::new();
        run_command(arguments, &client, &mut output).unwrap();
        let MemoryClient::Action(received) = client else {
            unreachable!()
        };
        let action = received
            .into_inner()
            .expect("command must reach the client");
        assert_eq!(
            serde_json::to_value(action).unwrap(),
            expected,
            "{arguments:?}"
        );
        assert!(
            output.is_empty(),
            "successful actions print no query output"
        );
    }

    fn monitor(unique_name: &str) -> MonitorDetails {
        MonitorDetails {
            device_name: unique_name.to_owned(),
            unique_name: unique_name.to_owned(),
            cg_display_id: None,
            gdi_device: None,
            work_area: MonitorFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
        }
    }

    fn workspace(name: &str, monitor: &str, state: WorkspaceState) -> WorkspaceInfo {
        WorkspaceInfo {
            name: name.to_owned(),
            monitor: monitor.to_owned(),
            state,
            is_focused: false,
            is_visible: false,
        }
    }

    fn query_workspaces(client: &MemoryClient, flags: &[&str]) -> Value {
        let arguments = [&["query", "workspaces"][..], flags].concat();
        let mut output = Vec::new();
        run_command(&arguments, client, &mut output).unwrap();
        assert_eq!(output.last(), Some(&b'\n'));
        serde_json::from_slice(&output).unwrap()
    }

    #[test]
    fn unminimize_window_parses_ids_and_rejects_invalid_input() {
        for id in [0, 7, u64::MAX] {
            assert_action(
                &["unminimize-window", &id.to_string()],
                json!({"type": "unminimize_window", "id": id}),
            );
        }
        for id in ["-1", "18446744073709551616", "not-a-number"] {
            let client = MemoryClient::Action(RefCell::new(None));
            let mut output = Vec::new();
            assert!(run_command(&["unminimize-window", id], &client, &mut output).is_err());
            let MemoryClient::Action(received) = client else {
                unreachable!()
            };
            assert!(received.into_inner().is_none());
            assert!(output.is_empty());
        }
    }

    #[test]
    fn launch_while_already_running_rejects_without_starting_dome() {
        let mut output = Vec::new();
        let error = run_command(
            &[
                "launch",
                "--config",
                "custom.lua",
                "--layout",
                "custom.json",
            ],
            &MemoryClient::AlreadyRunning,
            &mut output,
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "dome is already running");
        assert!(output.is_empty());
    }

    #[test]
    fn focus_and_move_to_monitor_preserve_literal_names() {
        for (command, action) in [("focus-monitor", "focus"), ("move-to-monitor", "move")] {
            for name in ["left", "DELL U2720Q #1"] {
                assert_action(
                    &[command, name],
                    json!({"type": action, "target": {"monitor": {"target": {"name": name}}}}),
                );
            }
        }
    }

    #[test]
    fn focus_and_move_directions_distinguish_windows_from_monitors() {
        for (direction, focus_command, move_command, focus_monitor_command, move_monitor_command) in [
            (
                "left",
                "focus-left",
                "move-left",
                "focus-monitor-left",
                "move-to-monitor-left",
            ),
            (
                "right",
                "focus-right",
                "move-right",
                "focus-monitor-right",
                "move-to-monitor-right",
            ),
            (
                "up",
                "focus-up",
                "move-up",
                "focus-monitor-up",
                "move-to-monitor-up",
            ),
            (
                "down",
                "focus-down",
                "move-down",
                "focus-monitor-down",
                "move-to-monitor-down",
            ),
        ] {
            for (command, action) in [(focus_command, "focus"), (move_command, "move")] {
                assert_action(&[command], json!({"type": action, "target": direction}));
            }
            for (command, action) in [
                (focus_monitor_command, "focus"),
                (move_monitor_command, "move"),
            ] {
                assert_action(
                    &[command],
                    json!({"type": action, "target": {"monitor": {"target": direction}}}),
                );
            }
        }
    }

    #[test]
    fn focus_and_move_workspaces_preserve_names_and_monitor_selectors() {
        for (command, action) in [("focus-workspace", "focus"), ("move-to-workspace", "move")] {
            assert_action(
                &[command, "01"],
                json!({"type": action, "target": {"workspace": {"name": "01", "monitor": null}}}),
            );
            for monitor in ["DELL U2720Q #1", r"\\.\DISPLAY2"] {
                assert_action(
                    &[command, "editor's work", "--monitor", monitor],
                    json!({"type": action, "target": {"workspace": {"name": "editor's work", "monitor": monitor}}}),
                );
            }
        }
    }

    #[test]
    fn query_workspaces_groups_monitors_before_parked_and_sorts_names() {
        use WorkspaceState::{Attached, Parked};

        let client = MemoryClient::Workspaces {
            rows: vec![
                workspace("1", "Old", Parked),
                workspace("web", "Left", Attached),
                workspace("+3", "Left", Attached),
                workspace("-1", "Left", Attached),
                workspace("170141183460469231731687303715884105728", "Left", Attached),
                workspace("10", "Left", Attached),
                workspace("2", "Gone", Parked),
                workspace("Mail", "Left", Attached),
                workspace("2", "Left", Attached),
                workspace("1", "Left", Attached),
                workspace("01", "Left", Attached),
                workspace("1", "Unlisted", Attached),
                workspace("5", "Right", Attached),
                workspace("4", "Right", Attached),
                workspace("1", "Gone", Parked),
            ],
            monitors: vec![monitor("Right"), monitor("Left")],
        };

        let printed: Vec<WorkspaceInfo> =
            serde_json::from_value(query_workspaces(&client, &[])).unwrap();
        assert_eq!(
            printed,
            vec![
                workspace("4", "Right", Attached),
                workspace("5", "Right", Attached),
                workspace("-1", "Left", Attached),
                workspace("01", "Left", Attached),
                workspace("1", "Left", Attached),
                workspace("2", "Left", Attached),
                workspace("+3", "Left", Attached),
                workspace("10", "Left", Attached),
                workspace("170141183460469231731687303715884105728", "Left", Attached),
                workspace("Mail", "Left", Attached),
                workspace("web", "Left", Attached),
                workspace("1", "Unlisted", Attached),
                workspace("1", "Gone", Parked),
                workspace("2", "Gone", Parked),
                workspace("1", "Old", Parked),
            ]
        );
    }

    #[test]
    fn query_workspaces_filters_attached_rows_by_monitor_name_or_device() {
        use WorkspaceState::{Attached, Parked};

        let client = MemoryClient::Workspaces {
            rows: vec![
                workspace("b", "Left", Attached),
                workspace("2", "Right", Attached),
                workspace("1", "Left", Parked),
                workspace("a", "Left", Attached),
                workspace("10", "Left", Attached),
                WorkspaceInfo {
                    is_focused: true,
                    is_visible: true,
                    ..workspace("9", "Left", Attached)
                },
            ],
            monitors: vec![
                monitor("Right"),
                MonitorDetails {
                    gdi_device: Some(r"\\.\DISPLAY2".to_owned()),
                    ..monitor("Left")
                },
            ],
        };

        for selector in ["Left", r"\\.\DISPLAY2"] {
            assert_eq!(
                query_workspaces(&client, &["--monitor", selector]),
                json!([
                    {"name":"9", "monitor":"Left", "state":"attached", "is_focused":true, "is_visible":true},
                    {"name":"10", "monitor":"Left", "state":"attached", "is_focused":false, "is_visible":false},
                    {"name":"a", "monitor":"Left", "state":"attached", "is_focused":false, "is_visible":false},
                    {"name":"b", "monitor":"Left", "state":"attached", "is_focused":false, "is_visible":false},
                ])
            );
        }
    }

    #[test]
    fn query_workspaces_rejects_unknown_monitor_without_printing_rows() {
        let client = MemoryClient::Workspaces {
            rows: vec![workspace("1", "Left", WorkspaceState::Attached)],
            monitors: vec![monitor("Left")],
        };
        let mut output = Vec::new();
        let error = run_command(
            &["query", "workspaces", "--monitor", "Nope"],
            &client,
            &mut output,
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "no connected monitor matches \"Nope\". Run `dome query monitors` to list each unique_name and gdi_device."
        );
        assert!(output.is_empty());
    }
}
