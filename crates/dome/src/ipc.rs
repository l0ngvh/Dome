use std::io::{BufRead, BufReader, Read, Write};
use std::time::{Duration, Instant};

use interprocess::local_socket::{
    ListenerOptions,
    traits::{Listener, Stream},
};

use dome_ipc::DomeClient;
use dome_ipc::action::IpcMessage;
use dome_ipc::response::{ErrorCode, Response};
use dome_ipc::socket::socket_name;

const RPC_READ_TIMEOUT: Duration = Duration::from_secs(2);
const RPC_POLL_INTERVAL: Duration = Duration::from_millis(5);
const RPC_MAX_REQUEST_BYTES: usize = 64 * 1024;

pub(crate) struct IpcConnection {
    stream: interprocess::local_socket::Stream,
    layout_path: String,
    drop_response: Option<Response>,
}

impl IpcConnection {
    pub(crate) fn new(stream: interprocess::local_socket::Stream, layout_path: String) -> Self {
        Self {
            stream,
            layout_path,
            drop_response: None,
        }
    }

    pub(crate) fn layout_path(&self) -> &str {
        &self.layout_path
    }

    pub(crate) fn read(mut self) -> anyhow::Result<Option<(Self, IpcMessage)>> {
        self.stream.set_nonblocking(true)?;
        let line = read_request_line(&self.stream)?;
        self.stream.set_nonblocking(false)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        match serde_json::from_str::<IpcMessage>(trimmed) {
            Ok(message) => {
                self.drop_response = Some(Response::error(
                    ErrorCode::ActionFailed,
                    if matches!(message, IpcMessage::Query { .. }) {
                        "Dome stopped before replying"
                    } else {
                        "Dome stopped before completing the action"
                    },
                ));
                Ok(Some((self, message)))
            }
            Err(error) => {
                self.write(Response::error(
                    ErrorCode::InvalidMessage,
                    error.to_string(),
                ))?;
                Ok(None)
            }
        }
    }

    pub(crate) fn write(mut self, response: Response) -> anyhow::Result<()> {
        self.drop_response = None;
        write_response(&self.stream, response)
    }
}

impl Drop for IpcConnection {
    fn drop(&mut self) {
        if let Some(response) = self.drop_response.take() {
            let _ = write_response(&self.stream, response);
        }
    }
}

pub(crate) fn serve(
    listener: interprocess::local_socket::Listener,
    layout_path: String,
    dispatch: impl Fn(IpcConnection) -> anyhow::Result<()>,
) {
    loop {
        match listener.accept() {
            Ok(stream) => {
                let connection = IpcConnection::new(stream, layout_path.clone());
                if let Err(err) = dispatch(connection) {
                    tracing::warn!(%err, "IPC dispatch failed");
                }
            }
            Err(err) => {
                tracing::error!(%err, "IPC accept error");
                break;
            }
        }
    }
}

pub(crate) fn bind_server() -> anyhow::Result<interprocess::local_socket::Listener> {
    #[cfg(unix)]
    prepare_socket_directory(dome_ipc::socket::socket_path().parent().unwrap())?;
    let name = socket_name();
    let listener = match ListenerOptions::new().name(name.clone()).create_sync() {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if DomeClient.ping() {
                anyhow::bail!("dome is already running");
            }
            // Windows named pipes clean themselves up, but Unix socket files can outlive Dome.
            #[cfg(unix)]
            std::fs::remove_file(dome_ipc::socket::socket_path())?;
            ListenerOptions::new().name(name).create_sync()?
        }
        Err(e) => return Err(e.into()),
    };
    tracing::info!("IPC server listening");
    Ok(listener)
}

fn write_response(
    stream: &interprocess::local_socket::Stream,
    response: Response,
) -> anyhow::Result<()> {
    let json = serde_json::to_string(&response)?;
    writeln!(&*stream, "{json}")?;
    Ok(())
}

#[cfg(unix)]
fn prepare_socket_directory(directory: &std::path::Path) -> anyhow::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};

    let parent_path = directory.parent().expect("socket directory has parent");
    let parent_directory = std::fs::File::options()
        .read(true)
        .custom_flags(libc::O_DIRECTORY)
        .open(parent_path)?;
    let parent = parent_directory.metadata()?;
    let user = unsafe { libc::geteuid() };
    anyhow::ensure!(
        parent.is_dir()
            && (parent.uid() == 0 || parent.uid() == user)
            && (parent.mode() & 0o022 == 0 || parent.mode() & 0o1000 != 0),
        "IPC parent directory must be trusted and protect entries from other users"
    );
    #[cfg(target_os = "macos")]
    ensure_no_extended_access_control(&parent_directory, parent_path)?;
    match std::fs::DirBuilder::new().mode(0o700).create(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let child_directory = std::fs::File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(directory)?;
    let metadata = child_directory.metadata()?;
    anyhow::ensure!(
        metadata.is_dir() && metadata.uid() == user && metadata.mode() & 0o777 == 0o700,
        "IPC directory must be owned by the current user with mode 0700: {}",
        directory.display()
    );
    #[cfg(target_os = "macos")]
    ensure_no_extended_access_control(&child_directory, directory)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn ensure_no_extended_access_control(
    directory: &std::fs::File,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    use std::os::fd::AsRawFd;

    // These macOS sys/stat.h and sys/fcntl.h APIs are not exposed by our libc crate.
    unsafe extern "C" {
        fn filesec_init() -> *mut libc::c_void;
        fn filesec_free(security: *mut libc::c_void);
        #[cfg_attr(target_arch = "x86_64", link_name = "fstatx_np$INODE64")]
        fn fstatx_np(
            descriptor: libc::c_int,
            metadata: *mut libc::stat,
            security: *mut libc::c_void,
        ) -> libc::c_int;
        fn filesec_query_property(
            security: *mut libc::c_void,
            property: libc::c_uint,
            present: *mut libc::c_int,
        ) -> libc::c_int;
    }
    const FILESEC_OWNER: libc::c_uint = 1;
    const FILESEC_ACL: libc::c_uint = 5;

    let security = unsafe { filesec_init() };
    anyhow::ensure!(
        !security.is_null(),
        "Cannot allocate extended ACL metadata for IPC directory {}: {}",
        path.display(),
        std::io::Error::last_os_error()
    );
    let result = (|| {
        let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
        anyhow::ensure!(
            unsafe { fstatx_np(directory.as_raw_fd(), metadata.as_mut_ptr(), security) } == 0,
            "Cannot read extended ACL for IPC directory {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        );
        let mut present = 0;
        // fstatx_np can leave security unpopulated after failing to grow its ACL buffer.
        // A completed read always sets FILESEC_OWNER, even when no ACL exists.
        anyhow::ensure!(
            unsafe { filesec_query_property(security, FILESEC_OWNER, &mut present) } == 0
                && present != 0,
            "Incomplete security metadata for IPC directory {}",
            path.display()
        );
        present = 0;
        anyhow::ensure!(
            unsafe { filesec_query_property(security, FILESEC_ACL, &mut present) } == 0,
            "Cannot inspect extended ACL for IPC directory {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        );
        anyhow::ensure!(
            present == 0,
            "Extended ACLs are not supported on IPC directory {}. Use a temporary directory without an extended ACL",
            path.display()
        );
        Ok(())
    })();
    unsafe { filesec_free(security) };
    result
}

// Replace polling when interprocess supports socket read timeouts.
fn read_request_line(stream: &interprocess::local_socket::Stream) -> anyhow::Result<String> {
    let deadline = Instant::now() + RPC_READ_TIMEOUT;
    let mut reader = BufReader::new(stream).take((RPC_MAX_REQUEST_BYTES + 1) as u64);
    let mut line = String::new();
    loop {
        let result = reader.read_line(&mut line);
        anyhow::ensure!(
            line.len() <= RPC_MAX_REQUEST_BYTES,
            "RPC request exceeds {RPC_MAX_REQUEST_BYTES} bytes"
        );
        match result {
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
pub(crate) mod tests {
    use super::IpcConnection;
    use interprocess::local_socket::{
        GenericFilePath, ListenerOptions, ToFsName,
        traits::{Listener, Stream},
    };
    use std::io::{BufRead, BufReader};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use dome_ipc::response::{ErrorCode, Response};
    use std::io::Write;
    use std::time::Duration;

    const EXIT_REQUEST: &str = r#"{"type":"action","action":{"type":"exit"}}"#;

    #[cfg(unix)]
    #[test]
    fn prepare_socket_directory_in_shared_parent_restricts_access() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

        let parent = tempfile::tempdir().unwrap();
        std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o1777)).unwrap();
        let directory = parent.path().join("dome");
        super::prepare_socket_directory(&directory).unwrap();
        let metadata = std::fs::symlink_metadata(&directory).unwrap();
        assert_eq!(metadata.mode() & 0o777, 0o700);
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
        super::prepare_socket_directory(&directory).unwrap();

        #[cfg(target_os = "macos")]
        {
            use std::os::unix::fs::DirBuilderExt;

            let chmod = |path: &std::path::Path, arguments: &[&str]| {
                let output = std::process::Command::new("/bin/chmod")
                    .args(arguments)
                    .arg(path)
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{output:?}");
            };
            let assert_access_control_rejected =
                |path: &std::path::Path, rejected: &std::path::Path| {
                    let error = super::prepare_socket_directory(path)
                        .unwrap_err()
                        .to_string();
                    assert!(error.contains("Extended ACLs are not supported"), "{error}");
                    assert!(error.contains(rejected.to_str().unwrap()), "{error}");
                };

            chmod(&directory, &["+a", "group:everyone allow search"]);
            assert_eq!(std::fs::metadata(&directory).unwrap().mode() & 0o777, 0o700);
            assert_access_control_rejected(&directory, &directory);
            chmod(&directory, &["-N"]);
            super::prepare_socket_directory(&directory).unwrap();

            for (name, permission) in [
                ("direct", "group:everyone allow search,delete_child"),
                (
                    "inherited",
                    "group:everyone allow search,file_inherit,directory_inherit,only_inherit",
                ),
            ] {
                chmod(parent.path(), &["+a", permission]);
                assert_eq!(
                    std::fs::metadata(parent.path()).unwrap().mode() & 0o1777,
                    0o1777
                );
                assert_access_control_rejected(&directory, parent.path());
                let child = parent.path().join(name);
                assert_access_control_rejected(&child, parent.path());
                assert!(!child.exists());
                if name == "inherited" {
                    std::fs::DirBuilder::new()
                        .mode(0o700)
                        .create(&child)
                        .unwrap();
                }
                chmod(parent.path(), &["-N"]);
                if name == "inherited" {
                    assert_eq!(std::fs::metadata(&child).unwrap().mode() & 0o777, 0o700);
                    assert_access_control_rejected(&child, &child);
                    chmod(&child, &["-N"]);
                }
                super::prepare_socket_directory(&child).unwrap();
            }
        }

        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(super::prepare_socket_directory(&directory).is_err());
        let link = parent.path().join("link");
        symlink(&directory, &link).unwrap();
        assert!(super::prepare_socket_directory(&link).is_err());

        std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(super::prepare_socket_directory(&parent.path().join("unsafe")).is_err());
        assert!(!parent.path().join("unsafe").exists());
    }

    #[test]
    fn serve_after_dispatch_failure_replies_to_next_client() {
        let name = socket_path().to_fs_name::<GenericFilePath>().unwrap();
        let listener = ListenerOptions::new()
            .name(name.clone())
            .nonblocking(interprocess::local_socket::ListenerNonblockingMode::Accept)
            .create_sync()
            .unwrap();
        let first_client = interprocess::local_socket::Stream::connect(name.clone()).unwrap();
        let (client_sender, client_receiver) = std::sync::mpsc::channel();
        let (completed, completion) = std::sync::mpsc::channel();
        let serving = std::thread::spawn(move || {
            let dispatches = std::cell::Cell::new(0);
            super::serve(listener, String::new(), |connection| {
                dispatches.set(dispatches.get() + 1);
                if dispatches.get() == 1 {
                    let mut client = interprocess::local_socket::Stream::connect(name.clone())?;
                    writeln!(client, "{EXIT_REQUEST}")?;
                    client_sender.send(client).unwrap();
                    anyhow::bail!("test dispatch failure");
                }
                let (connection, _) = connection.read()?.expect("complete request");
                connection.write(Response::ok(serde_json::json!({"recovered": true})))
            });
            completed.send(dispatches.get()).unwrap();
        });
        let next_client = client_receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(
            read_response(next_client),
            Response::ok(serde_json::json!({"recovered": true}))
        );
        assert_eq!(completion.recv_timeout(Duration::from_secs(5)).unwrap(), 2);
        serving.join().unwrap();
        drop(first_client);
    }

    #[test]
    fn read_malformed_request_returns_invalid_message_envelope() {
        let (connection, mut client) = connection_pair("");
        writeln!(client, "not json").unwrap();
        assert!(connection.read().unwrap().is_none());
        assert!(matches!(
            read_response(client),
            Response::Error {
                error: dome_ipc::response::RpcError {
                    code: ErrorCode::InvalidMessage,
                    ..
                }
            }
        ));
    }

    #[test]
    fn read_silent_request_has_bounded_deadline() {
        let (connection, silent_client) = connection_pair("");
        let (completed, completion) = std::sync::mpsc::channel();
        let reading = std::thread::spawn(move || {
            let result = connection.read();
            let _ = completed.send(result.err().map(|error| error.to_string()));
        });
        let result = completion.recv_timeout(Duration::from_secs(5));
        drop(silent_client);
        let error = result.expect("silent request read exceeded its deadline");
        reading.join().unwrap();
        assert!(error.unwrap().contains("read timed out"));
    }

    #[test]
    fn drop_unanswered_connection_reports_stopped_error() {
        for (request, message) in [
            (
                r#"{"type":"query","query":{"type":"workspaces"}}"#,
                "Dome stopped before replying",
            ),
            (EXIT_REQUEST, "Dome stopped before completing the action"),
        ] {
            let (connection, mut client) = connection_pair("");
            writeln!(client, "{request}").unwrap();
            let (connection, _) = connection.read().unwrap().unwrap();
            drop(connection);
            assert_eq!(
                read_response(client),
                Response::error(ErrorCode::ActionFailed, message)
            );
        }
    }

    #[test]
    fn read_complete_oversized_line_rejects_request() {
        let (connection, mut client) = connection_pair("");
        let sending = std::thread::spawn(move || {
            let _ = writeln!(client, "{}", "x".repeat(64 * 1024 + 1));
        });
        let result = connection.read();
        sending.join().unwrap();
        assert!(result.err().unwrap().to_string().contains("exceeds"));
    }

    fn socket_path() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let number = COUNTER.fetch_add(1, Ordering::Relaxed);
        let process = std::process::id();
        #[cfg(unix)]
        {
            std::env::temp_dir().join(format!("dome-ipc-test-{process}-{number}.sock"))
        }
        #[cfg(windows)]
        {
            PathBuf::from(format!(r"\\.\pipe\dome-ipc-test-{process}-{number}"))
        }
    }

    pub(crate) fn connection_pair(
        layout_path: &str,
    ) -> (IpcConnection, interprocess::local_socket::Stream) {
        let path = socket_path();
        let name = path.to_fs_name::<GenericFilePath>().unwrap();
        let listener = ListenerOptions::new()
            .name(name.clone())
            .create_sync()
            .unwrap();
        let client = interprocess::local_socket::Stream::connect(name).unwrap();
        let stream = listener.accept().unwrap();
        (IpcConnection::new(stream, layout_path.to_owned()), client)
    }

    fn read_response(stream: interprocess::local_socket::Stream) -> Response {
        let (sender, receiver) = std::sync::mpsc::channel();
        let reading = std::thread::spawn(move || {
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            sender
                .send(serde_json::from_str(line.trim()).unwrap())
                .unwrap();
        });
        let response = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("IPC response stalled");
        reading.join().unwrap();
        response
    }
}
