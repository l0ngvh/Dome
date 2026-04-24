use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use interprocess::local_socket::{
    GenericFilePath, ListenerOptions, ToFsName,
    traits::{Listener, Stream},
};

use crate::action::IpcMessage;

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
}

pub(crate) fn start_server<F>(on_message: F) -> anyhow::Result<()>
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
