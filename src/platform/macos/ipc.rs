use std::ffi::c_void;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use objc2_core_foundation::{
    CFFileDescriptor, CFFileDescriptorContext, CFFileDescriptorNativeDescriptor, CFOptionFlags,
    CFRunLoop, kCFRunLoopDefaultMode,
};

use super::context::WindowContext;

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
        let context = &mut *(info as *mut WindowContext);

        if let Ok((stream, _)) = context.listener.accept() {
            handle_client(stream, context);
        }

        if let Some(fd_ref) = fd_ref.as_ref() {
            fd_ref.enable_call_backs(K_CF_FILE_DESCRIPTOR_READ_CALL_BACK);
        }
    }
}

fn handle_client(mut stream: UnixStream, context: &mut WindowContext) {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();

    if reader.read_line(&mut line).is_ok() {
        let cmd = line.trim();
        let response = match handle_command(cmd, context) {
            Ok(()) => "ok\n".to_string(),
            Err(e) => format!("error:{e}\n"),
        };
        let _ = stream.write_all(response.as_bytes());
    }
}

fn handle_command(cmd: &str, _context: &mut WindowContext) -> Result<(), String> {
    // TODO: dispatch to hub/core
    tracing::info!("Received IPC command: {cmd}");
    Ok(())
}

pub(super) fn register_with_runloop(context: *mut WindowContext) -> anyhow::Result<()> {
    let fd = unsafe { (*context).listener.as_raw_fd() as CFFileDescriptorNativeDescriptor };

    let cf_context = CFFileDescriptorContext {
        version: 0,
        info: context as *mut c_void,
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
