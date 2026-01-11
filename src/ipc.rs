use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use interprocess::local_socket::{
    GenericFilePath, ListenerOptions, ToFsName,
    traits::{Listener, Stream},
};

use crate::action::{Action, Actions};

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

    pub fn send_action(&self, action: &Action) -> std::io::Result<String> {
        let mut stream = interprocess::local_socket::Stream::connect(socket_name())?;
        let json = serde_json::to_string(action).map_err(std::io::Error::other)?;
        writeln!(stream, "{json}")?;

        let mut response = String::new();
        BufReader::new(&stream).read_line(&mut response)?;
        Ok(response.trim().to_string())
    }
}

pub(crate) fn start_server<F>(on_action: F)
where
    F: Fn(Actions) + Send + 'static,
{
    std::thread::spawn(move || {
        if let Err(e) = run_server(&on_action) {
            tracing::error!("IPC server error: {e}");
        }
    });
}

fn run_server<F>(on_action: &F) -> anyhow::Result<()>
where
    F: Fn(Actions),
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

    loop {
        let stream = listener.accept()?;
        handle_client(stream, on_action);
    }
}

fn handle_client<F>(stream: interprocess::local_socket::Stream, on_action: &F)
where
    F: Fn(Actions),
{
    let mut stream = stream;
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();

    if reader.read_line(&mut line).is_ok() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        let response = match serde_json::from_str::<Action>(trimmed) {
            Ok(action) => {
                tracing::debug!(?action, "IPC action");
                on_action(Actions::new(vec![action]));
                "ok\n"
            }
            Err(e) => {
                tracing::warn!(message = trimmed, "Invalid IPC message: {e}");
                "error\n"
            }
        };
        let _ = stream.write_all(response.as_bytes());
    }
}
