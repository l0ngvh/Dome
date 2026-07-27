use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::time::Duration;

use interprocess::local_socket::{
    GenericFilePath, ListenerOptions, ToFsName,
    traits::{Listener, Stream},
};

use crate::action::{Actions, IpcMessage, Query};

pub(crate) enum IpcEvent {
    Action(Actions),
    Query {
        query: Query,
        reply: SyncSender<String>,
    },
    // ExportLayout carries the path so start_server owns it, not each platform.
    ExportLayout(String),
}

const QUERY_TIMEOUT: Duration = Duration::from_secs(1);
const QUERY_TIMEOUT_JSON: &str = r#"{"error":"query timed out"}"#;

fn socket_path() -> PathBuf {
    #[cfg(unix)]
    {
        std::env::temp_dir().join("dome.sock")
    }
    #[cfg(windows)]
    {
        PathBuf::from(r"\\.\pipe\dome")
    }
}

fn socket_name() -> interprocess::local_socket::Name<'static> {
    socket_path().to_fs_name::<GenericFilePath>().unwrap()
}

#[derive(Default)]
pub struct DomeClient;

impl DomeClient {
    pub fn ping(&self) -> bool {
        interprocess::local_socket::Stream::connect(socket_name()).is_ok()
    }

    fn send(&self, msg: &IpcMessage) -> std::io::Result<String> {
        let mut stream = interprocess::local_socket::Stream::connect(socket_name())?;
        let json = serde_json::to_string(msg).map_err(std::io::Error::other)?;
        writeln!(stream, "{json}")?;

        let mut response = String::new();
        BufReader::new(&stream).read_line(&mut response)?;
        Ok(response.trim().to_string())
    }

    pub fn send_action(&self, action: &crate::action::Action) -> std::io::Result<String> {
        self.send(&IpcMessage::Action(action.clone()))
    }

    pub fn send_query(&self, query: &crate::action::Query) -> std::io::Result<String> {
        self.send(&IpcMessage::Query(query.clone()))
    }

    pub fn send_export_layout(&self) -> std::io::Result<String> {
        self.send(&IpcMessage::ExportLayout)
    }
}

pub(crate) fn start_server<F>(export_layout_path: String, dispatch: F) -> anyhow::Result<()>
where
    F: Fn(IpcEvent) -> anyhow::Result<()> + Send + 'static,
{
    let on_message = move |msg: IpcMessage| -> anyhow::Result<String> {
        match msg {
            IpcMessage::Action(action) => {
                dispatch(IpcEvent::Action(Actions::new(vec![action])))?;
                Ok("ok".to_string())
            }
            IpcMessage::Query(query) => {
                let (reply, resp_rx) = std::sync::mpsc::sync_channel(1);
                dispatch(IpcEvent::Query { query, reply })?;
                match resp_rx.recv_timeout(QUERY_TIMEOUT) {
                    Ok(json) => Ok(json),
                    Err(_) => Ok(QUERY_TIMEOUT_JSON.to_string()),
                }
            }
            IpcMessage::ExportLayout => {
                dispatch(IpcEvent::ExportLayout(export_layout_path.clone()))?;
                Ok("ok".to_string())
            }
        }
    };
    listen(on_message)
}

fn listen<F>(on_message: F) -> anyhow::Result<()>
where
    F: Fn(IpcMessage) -> anyhow::Result<String> + Send + 'static,
{
    let name = socket_name();
    let listener = match ListenerOptions::new().name(name.clone()).create_sync() {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if DomeClient.ping() {
                anyhow::bail!("dome is already running");
            }
            // Stale socket file (Unix only, Windows named pipes auto-cleanup)
            #[cfg(unix)]
            std::fs::remove_file(socket_path())?;
            ListenerOptions::new().name(name).create_sync()?
        }
        Err(e) => return Err(e.into()),
    };
    tracing::info!("IPC server listening");

    std::thread::spawn(move || {
        loop {
            match listener.accept() {
                Ok(stream) => {
                    if let Err(e) = handle_client(stream, &on_message) {
                        tracing::debug!("IPC client handler stopped: {e}");
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("IPC accept error: {e}");
                    break;
                }
            }
        }
    });
    Ok(())
}

fn handle_client<F>(
    stream: interprocess::local_socket::Stream,
    on_message: &F,
) -> anyhow::Result<()>
where
    F: Fn(IpcMessage) -> anyhow::Result<String>,
{
    let mut stream = stream;
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();

    if reader.read_line(&mut line).is_ok() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let msg = match serde_json::from_str::<IpcMessage>(trimmed) {
            Ok(msg) => msg,
            Err(e) => {
                tracing::debug!(message = trimmed, "Invalid IPC message: {e}");
                if let Err(write_err) = writeln!(stream, "error") {
                    tracing::debug!("Failed to write error response: {write_err}");
                }
                return Ok(());
            }
        };
        tracing::debug!(?msg, "IPC message");
        match on_message(msg) {
            Ok(response) => {
                if let Err(write_err) = writeln!(stream, "{response}") {
                    tracing::debug!("Failed to write response: {write_err}");
                }
            }
            Err(e) => {
                if let Err(write_err) = writeln!(stream, "error") {
                    tracing::debug!("Failed to write error response: {write_err}");
                }
                return Err(e);
            }
        }
    }
    Ok(())
}
