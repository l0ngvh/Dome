use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc::SyncSender;
use std::time::{Duration, Instant};

use interprocess::local_socket::{
    ListenerOptions,
    traits::{Listener, Stream},
};

use dome_ipc::DomeClient;
use dome_ipc::action::{Actions, IpcMessage, Query};
use dome_ipc::response::{ErrorCode, Response};
use dome_ipc::socket::socket_name;

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
const RPC_READ_TIMEOUT: Duration = Duration::from_secs(2);
const RPC_POLL_INTERVAL: Duration = Duration::from_millis(5);
const RPC_MAX_REQUEST_BYTES: usize = 64 * 1024;

pub(crate) fn start_server<F>(export_layout_path: String, dispatch: F) -> anyhow::Result<()>
where
    F: Fn(IpcEvent) -> anyhow::Result<()> + Send + Clone + 'static,
{
    let on_message = move |msg: IpcMessage| -> Response {
        match msg {
            IpcMessage::Action { action } => {
                match dispatch(IpcEvent::Action(Actions::new(vec![action]))) {
                    Ok(()) => Response::ok(serde_json::Value::Null),
                    Err(e) => Response::error(ErrorCode::ActionFailed, e.to_string()),
                }
            }
            IpcMessage::Query { query } => {
                let (reply, resp_rx) = std::sync::mpsc::sync_channel(1);
                match dispatch(IpcEvent::Query { query, reply }) {
                    Ok(()) => match resp_rx.recv_timeout(QUERY_TIMEOUT) {
                        // The reply is already-serialized JSON, parsed to a Value so
                        // it rides inside the envelope's data field rather than being
                        // re-escaped as a string.
                        Ok(json) => match serde_json::from_str::<serde_json::Value>(&json) {
                            Ok(value) => Response::ok(value),
                            Err(e) => Response::error(ErrorCode::ActionFailed, e.to_string()),
                        },
                        Err(_) => Response::error(ErrorCode::QueryTimedOut, "query timed out"),
                    },
                    Err(e) => Response::error(ErrorCode::ActionFailed, e.to_string()),
                }
            }
            IpcMessage::ExportLayout => {
                match dispatch(IpcEvent::ExportLayout(export_layout_path.clone())) {
                    Ok(()) => Response::ok(serde_json::Value::Null),
                    Err(e) => Response::error(ErrorCode::ActionFailed, e.to_string()),
                }
            }
        }
    };
    listen(on_message)
}

fn listen<F>(on_message: F) -> anyhow::Result<()>
where
    F: Fn(IpcMessage) -> Response + Send + Clone + 'static,
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
            std::fs::remove_file(dome_ipc::socket::socket_path())?;
            ListenerOptions::new().name(name).create_sync()?
        }
        Err(e) => return Err(e.into()),
    };
    tracing::info!("IPC server listening");

    std::thread::spawn(move || serve(listener, on_message));
    Ok(())
}

fn serve<F>(listener: interprocess::local_socket::Listener, on_message: F)
where
    F: Fn(IpcMessage) -> Response + Send + Clone + 'static,
{
    loop {
        match listener.accept() {
            Ok(stream) => {
                let on_message = on_message.clone();
                std::thread::spawn(move || {
                    if let Err(err) = handle_client(stream, &on_message) {
                        tracing::warn!(%err, "IPC connection failed");
                    }
                });
            }
            Err(err) => {
                tracing::error!(%err, "IPC accept error");
                break;
            }
        }
    }
}

fn handle_client<F>(
    stream: interprocess::local_socket::Stream,
    on_message: &F,
) -> anyhow::Result<()>
where
    F: Fn(IpcMessage) -> Response,
{
    let mut stream = stream;
    stream.set_nonblocking(true)?;
    let line = read_request_line(&stream)?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let response = match serde_json::from_str::<IpcMessage>(trimmed) {
        Ok(msg) => on_message(msg),
        Err(e) => Response::error(ErrorCode::InvalidMessage, e.to_string()),
    };
    stream.set_nonblocking(false)?;
    let json = serde_json::to_string(&response)?;
    writeln!(stream, "{json}")?;
    Ok(())
}

// interprocess 2.2.3 has set_nonblocking but no set_read_timeout, so a slow or
// silent client is bounded by polling the non-blocking stream and dropping it
// once RPC_READ_TIMEOUT elapses.
fn read_request_line(stream: &interprocess::local_socket::Stream) -> anyhow::Result<String> {
    let deadline = Instant::now() + RPC_READ_TIMEOUT;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        anyhow::ensure!(
            line.len() <= RPC_MAX_REQUEST_BYTES,
            "RPC request exceeds {RPC_MAX_REQUEST_BYTES} bytes"
        );
        match reader.read_line(&mut line) {
            Ok(0) => return Ok(line),
            Ok(_) if line.ends_with('\n') => return Ok(line),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                anyhow::ensure!(Instant::now() < deadline, "RPC read timed out");
                std::thread::sleep(RPC_POLL_INTERVAL);
            }
            Err(e) => return Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use interprocess::local_socket::{GenericFilePath, ToFsName};

    fn temp_socket_path() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        #[cfg(unix)]
        {
            std::env::temp_dir().join(format!("dome-ipc-test-{pid}-{n}.sock"))
        }
        #[cfg(windows)]
        {
            PathBuf::from(format!(r"\\.\pipe\dome-ipc-test-{pid}-{n}"))
        }
    }

    fn start_test_server<F>(on_message: F) -> PathBuf
    where
        F: Fn(IpcMessage) -> Response + Send + Clone + 'static,
    {
        let path = temp_socket_path();
        let _ = std::fs::remove_file(&path);
        let name = path.clone().to_fs_name::<GenericFilePath>().unwrap();
        let listener = ListenerOptions::new().name(name).create_sync().unwrap();
        std::thread::spawn(move || serve(listener, on_message));
        path
    }

    fn connect(path: &Path) -> interprocess::local_socket::Stream {
        let name = path.to_path_buf().to_fs_name::<GenericFilePath>().unwrap();
        interprocess::local_socket::Stream::connect(name).unwrap()
    }

    fn round_trip(path: &Path, request: &str) -> Response {
        let mut stream = connect(path);
        writeln!(stream, "{request}").unwrap();
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).unwrap();
        serde_json::from_str(line.trim()).unwrap()
    }

    const EXIT_REQUEST: &str = r#"{"type":"action","action":{"type":"exit"}}"#;

    #[test]
    fn rpc_accept_loop_survives_dispatch_error() {
        let calls = Arc::new(AtomicU32::new(0));
        let path = start_test_server({
            let calls = calls.clone();
            move |_msg: IpcMessage| {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    Response::error(ErrorCode::ActionFailed, "boom")
                } else {
                    Response::ok(serde_json::Value::Null)
                }
            }
        });

        let first = round_trip(&path, EXIT_REQUEST);
        assert!(
            matches!(first, Response::Error { .. }),
            "first request errors"
        );

        let second = round_trip(&path, EXIT_REQUEST);
        assert!(
            matches!(second, Response::Ok { .. }),
            "the loop survived the error and served a second connection"
        );
    }

    #[test]
    fn rpc_slow_client_does_not_block_others() {
        let path = start_test_server(|_msg: IpcMessage| Response::ok(serde_json::Value::Null));

        let slow = connect(&path);

        let start = Instant::now();
        let response = round_trip(&path, EXIT_REQUEST);
        assert!(matches!(response, Response::Ok { .. }));
        assert!(
            start.elapsed() < RPC_READ_TIMEOUT,
            "a silent client must not stall another connection"
        );

        let mut buf = String::new();
        let read = BufReader::new(&slow).read_line(&mut buf).unwrap_or(0);
        assert_eq!(read, 0, "the silent client is dropped without a reply");
    }
}
