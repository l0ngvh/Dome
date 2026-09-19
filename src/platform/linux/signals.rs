//! SIGTERM and SIGINT delivery as an event-loop source.
//!
//! A signal handler may only call async-signal-safe functions, so this one writes a
//! single byte to a pipe and returns. The read end sits in the event loop, which turns
//! the byte into an ordinary event on the main thread and stops the loop from there.

use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
use std::sync::atomic::{AtomicI32, Ordering};

use anyhow::Result;
use calloop::generic::Generic;
use calloop::{Interest, LoopHandle, Mode, PostAction};

use super::CalloopData;

static WAKE_FD: AtomicI32 = AtomicI32::new(-1);

extern "C" fn write_to_wake_pipe(_signal: libc::c_int) {
    let fd = WAKE_FD.load(Ordering::Relaxed);
    if fd < 0 {
        return;
    }
    let byte = [0u8];
    // A full pipe already carries an unread wakeup, and a closed one means the loop is
    // gone, so neither a short write nor an error leaves anything to do here.
    unsafe {
        libc::write(fd, byte.as_ptr().cast(), 1);
    }
}

/// Stop the event loop when the process receives SIGTERM or SIGINT, so the shutdown
/// path runs instead of the default disposition killing the process outright.
pub(super) fn stop_loop_on_terminate(handle: &LoopHandle<'static, CalloopData>) -> Result<()> {
    let mut fds = [0 as libc::c_int; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
        return Err(std::io::Error::last_os_error()).map_err(anyhow::Error::from);
    }
    let read_end = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let write_end = unsafe { OwnedFd::from_raw_fd(fds[1]) };

    WAKE_FD.store(write_end.into_raw_fd(), Ordering::Relaxed);
    for signal in [libc::SIGTERM, libc::SIGINT] {
        if unsafe { libc::signal(signal, write_to_wake_pipe as libc::sighandler_t) }
            == libc::SIG_ERR
        {
            return Err(std::io::Error::last_os_error()).map_err(anyhow::Error::from);
        }
    }

    let source = Generic::new(read_end, Interest::READ, Mode::Level);
    handle
        .insert_source(source, |_, fd, data| {
            let mut buffer = [0u8; 8];
            unsafe { libc::read(fd.as_raw_fd(), buffer.as_mut_ptr().cast(), buffer.len()) };
            tracing::info!("Shutting down on a terminate signal");
            data.state.loop_signal.stop();
            Ok(PostAction::Continue)
        })
        .map_err(|e| anyhow::anyhow!("failed to insert the signal pipe: {e}"))?;
    Ok(())
}
