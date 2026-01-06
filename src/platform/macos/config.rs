use std::ffi::c_void;
use std::fs::File;
use std::os::unix::io::{AsRawFd, IntoRawFd};

use objc2::DefinedClass;
use objc2_core_foundation::{
    CFFileDescriptor, CFFileDescriptorContext, CFFileDescriptorNativeDescriptor, CFOptionFlags,
    CFRunLoop, kCFRunLoopDefaultMode,
};

use super::app::AppDelegate;
use super::handler::render_workspace;
use crate::config::Config;

const K_CF_FILE_DESCRIPTOR_READ_CALL_BACK: CFOptionFlags = 1;

unsafe extern "C-unwind" fn config_callback(
    fd_ref: *mut CFFileDescriptor,
    _callback_types: CFOptionFlags,
    info: *mut c_void,
) {
    unsafe {
        let delegate: &'static AppDelegate = &*(info as *const AppDelegate);
        let ivars = delegate.ivars();

        // Drain the kqueue event
        let kq_fd = ivars.config_fd.get().unwrap().as_raw_fd();
        let mut event: libc::kevent = std::mem::zeroed();
        let timeout = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        libc::kevent(kq_fd, std::ptr::null(), 0, &mut event, 1, &timeout);

        match Config::load(&ivars.config_path) {
            Ok(new_config) => {
                tracing::info!("Config reloaded successfully");
                ivars
                    .hub
                    .borrow_mut()
                    .sync_config(new_config.tab_bar_height, new_config.automatic_tiling);
                *ivars.config.borrow_mut() = new_config;
                if let Err(e) = render_workspace(delegate) {
                    tracing::warn!("Failed to render workspace after config reload: {e:#}");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to reload config: {e}, keeping current config");
            }
        }

        if let Some(fd_ref) = fd_ref.as_ref() {
            fd_ref.enable_call_backs(K_CF_FILE_DESCRIPTOR_READ_CALL_BACK);
        }
    }
}

pub(super) fn setup_config_watcher(delegate: &'static AppDelegate) -> anyhow::Result<()> {
    let file = File::open(&delegate.ivars().config_path)?;
    // file_fd must stay open for kqueue to watch it. Both file_fd and kqueue fd
    // live for the app's lifetime and are cleaned up by the OS on process exit.
    let file_fd = file.into_raw_fd();

    let kq = unsafe { libc::kqueue() };
    if kq < 0 {
        anyhow::bail!("Failed to create kqueue");
    }

    let mut event: libc::kevent = unsafe { std::mem::zeroed() };
    event.ident = file_fd as usize;
    event.filter = libc::EVFILT_VNODE;
    event.flags = libc::EV_ADD | libc::EV_CLEAR;
    event.fflags = libc::NOTE_WRITE | libc::NOTE_ATTRIB;

    let ret = unsafe { libc::kevent(kq, &event, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
    if ret < 0 {
        anyhow::bail!("Failed to register kevent");
    }

    let context = CFFileDescriptorContext {
        version: 0,
        info: delegate as *const AppDelegate as *mut c_void,
        retain: None,
        release: None,
        copyDescription: None,
    };

    let fd_ref = unsafe {
        CFFileDescriptor::new(
            None,
            kq as CFFileDescriptorNativeDescriptor,
            true,
            Some(config_callback),
            &context,
        )
    }
    .ok_or_else(|| anyhow::anyhow!("Failed to create CFFileDescriptor for config"))?;

    fd_ref.enable_call_backs(K_CF_FILE_DESCRIPTOR_READ_CALL_BACK);

    let source = CFFileDescriptor::new_run_loop_source(None, Some(&fd_ref), 0)
        .ok_or_else(|| anyhow::anyhow!("Failed to create run loop source for config"))?;

    CFRunLoop::current()
        .unwrap()
        .add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    let _ = delegate.ivars().config_fd.set(fd_ref);

    tracing::info!(path = %delegate.ivars().config_path, "Config watcher listening");
    Ok(())
}
