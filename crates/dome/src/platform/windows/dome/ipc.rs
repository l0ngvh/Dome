use super::Dome;
use crate::config::PreferredLayouts;
use crate::ipc::IpcConnection;
use crate::platform::messages::{HubMessage, HubSender};
use crate::platform::windows::event_loop::Shutdown;
use dome_ipc::action::{Action, IpcMessage};
use dome_ipc::response::{ErrorCode, Response};
use std::sync::Mutex;

/// Waits for the operation and response write without an operation deadline.
pub(in crate::platform::windows) fn handle_connection(
    connection: IpcConnection,
    hub: &HubSender,
    operations: &Mutex<()>,
    shutdown: &Shutdown,
) -> anyhow::Result<()> {
    let Some((connection, message)) = connection.read()? else {
        return Ok(());
    };
    let exit = matches!(
        message,
        IpcMessage::Action {
            action: Action::Exit
        }
    );
    let response = {
        let _serial = operations.lock().unwrap();
        handle_ipc_message(message, connection.layout_path(), hub)
    };
    let succeeded = matches!(response, Response::Ok { .. });
    let written = connection.write(response);
    if exit && succeeded {
        shutdown.terminate();
    }
    written
}

// Must run on an IPC worker, never on the native owner thread.
pub(super) fn handle_ipc_message(
    message: IpcMessage,
    layout_path: &str,
    hub: &HubSender,
) -> Response {
    match message {
        IpcMessage::Action {
            action: Action::Exit,
        } => {}
        IpcMessage::Action {
            action: Action::Execute { command },
        } => {
            let environment = match pollster::block_on(hub.environment()) {
                Ok(environment) => environment,
                Err(error) => {
                    return Response::error(ErrorCode::ActionFailed, format!("{error:#}"));
                }
            };
            if let Err(error) = crate::platform::windows::spawn::spawn(&command, &environment) {
                return Response::error(ErrorCode::ActionFailed, format!("{error:#}"));
            }
        }
        IpcMessage::Action { action } => {
            if let Err(error) = pollster::block_on(hub.execute_action(action)) {
                return Response::error(ErrorCode::ActionFailed, format!("{error:#}"));
            }
        }
        IpcMessage::SaveLayout => {
            let layouts = match pollster::block_on(hub.capture_layouts()) {
                Ok(layouts) => layouts,
                Err(error) => {
                    return Response::error(ErrorCode::ActionFailed, format!("{error:#}"));
                }
            };
            if let Err(error) = layouts.save(std::path::Path::new(layout_path)) {
                return Response::error(ErrorCode::ActionFailed, format!("{error:#}"));
            }
        }
        IpcMessage::ApplyLayout => {
            let layouts = match std::fs::read_to_string(layout_path)
                .map_err(anyhow::Error::from)
                .and_then(|source| PreferredLayouts::from_lua(layout_path, &source))
            {
                Ok(layouts) => layouts,
                Err(error) => {
                    return Response::error(ErrorCode::ActionFailed, format!("{error:#}"));
                }
            };
            if let Err(error) = pollster::block_on(hub.apply_layouts(layouts)) {
                return Response::error(ErrorCode::ActionFailed, format!("{error:#}"));
            }
        }
        IpcMessage::Query { query } => {
            let value = match pollster::block_on(hub.query(query)) {
                Ok(value) => value,
                Err(error) => {
                    return Response::error(ErrorCode::ActionFailed, format!("{error:#}"));
                }
            };
            return Response::ok(value);
        }
    }
    Response::ok(serde_json::Value::Null)
}

impl Dome {
    pub(super) fn handle_hub_message(&mut self, message: HubMessage) {
        match message {
            HubMessage::Action { action, reply } => {
                let _ = reply.send(self.handle_action(&action));
            }
            HubMessage::Environment { reply } => {
                let _ = reply.send(self.env.clone());
            }
            HubMessage::CaptureLayouts { reply } => {
                let _ = reply.send(self.hub.capture_live_layouts());
            }
            HubMessage::ApplyLayouts { layouts, reply } => {
                self.hub.apply_preferred_layouts(layouts);
                self.apply_layout();
                let _ = reply.send(());
            }
            HubMessage::Query { query, reply } => {
                let value = match query {
                    crate::action::Query::Workspaces => self.query_workspaces_value(),
                    crate::action::Query::MinimizedWindows => self.query_minimized_windows_value(),
                    crate::action::Query::Monitors => self.query_monitors_value(),
                };
                let _ = reply.send(value);
            }
        }
    }
}
