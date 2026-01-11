use std::ffi::c_void;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use objc2::DefinedClass;
use objc2_core_foundation::{
    CFFileDescriptor, CFFileDescriptorContext, CFFileDescriptorNativeDescriptor, CFOptionFlags,
    CFRunLoop, kCFRunLoopDefaultMode,
};

use crate::action::{Action, Actions};

use super::app::AppDelegate;
use super::listeners::handle_actions;

const K_CF_FILE_DESCRIPTOR_READ_CALL_BACK: CFOptionFlags = 1;

pub(super) fn socket_path() -> PathBuf {
    std::env::temp_dir().join("dome.sock")
}

unsafe extern "C-unwind" fn socket_callback(
    fd_ref: *mut CFFileDescriptor,
    _callback_types: CFOptionFlags,
    info: *mut c_void,
) {
    unsafe {
        // Safety: AppDelegate lives until the end of the app
        let delegate: &'static AppDelegate = &*(info as *const AppDelegate);
        let listener = delegate.ivars().listener.get().unwrap();

        if let Ok((stream, _)) = listener.accept() {
            handle_client(stream, delegate);
        }

        if let Some(fd_ref) = fd_ref.as_ref() {
            fd_ref.enable_call_backs(K_CF_FILE_DESCRIPTOR_READ_CALL_BACK);
        }
    }
}

fn handle_client(mut stream: UnixStream, delegate: &'static AppDelegate) {
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
                let actions = Actions::new(vec![action]);
                handle_actions(delegate, &actions);
                "ok\n".to_string()
            }
            Err(e) => {
                tracing::warn!(message = trimmed, "Invalid IPC message: {e}");
                format!("error:invalid action: {e}\n")
            }
        };
        let _ = stream.write_all(response.as_bytes());
    }
}

pub(super) fn register_with_runloop(delegate: &'static AppDelegate) -> anyhow::Result<()> {
    let listener = delegate.ivars().listener.get().unwrap();
    let fd = listener.as_raw_fd() as CFFileDescriptorNativeDescriptor;

    let cf_context = CFFileDescriptorContext {
        version: 0,
        info: delegate as *const AppDelegate as *mut c_void,
        retain: None,
        release: None,
        copyDescription: None,
    };

    let fd_ref =
        unsafe { CFFileDescriptor::new(None, fd, false, Some(socket_callback), &cf_context) }
            .ok_or_else(|| anyhow::anyhow!("Failed to create CFFileDescriptor"))?;

    fd_ref.enable_call_backs(K_CF_FILE_DESCRIPTOR_READ_CALL_BACK);

    let source = CFFileDescriptor::new_run_loop_source(None, Some(&fd_ref), 0)
        .ok_or_else(|| anyhow::anyhow!("Failed to create run loop source"))?;

    CFRunLoop::current()
        .unwrap()
        .add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    std::mem::forget(fd_ref);

    let path = socket_path();
    tracing::info!(path = %path.display(), "IPC server listening");
    Ok(())
}

pub(super) fn try_bind() -> anyhow::Result<UnixListener> {
    let path = socket_path();

    match UnixListener::bind(&path) {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if UnixStream::connect(&path).is_ok() {
                anyhow::bail!("dome is already running")
            }
            std::fs::remove_file(&path)?;
            Ok(UnixListener::bind(&path)?)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn send_action(action: &Action) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(socket_path())?;
    let json = serde_json::to_string(action).map_err(std::io::Error::other)?;
    writeln!(stream, "{json}")?;

    let mut response = String::new();
    BufReader::new(&stream).read_line(&mut response)?;
    Ok(response.trim().to_string())
}
