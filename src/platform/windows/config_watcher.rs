use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::mpsc::Sender;

use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_LAST_WRITE,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadDirectoryChangesW,
};

use super::hub::HubEvent;
use crate::config::Config;

pub(super) fn start_config_watcher(config_path: String, sender: Sender<HubEvent>) {
    std::thread::spawn(move || {
        if let Err(e) = watch_config(&config_path, &sender) {
            tracing::error!("Config watcher error: {e}");
        }
    });
}

fn watch_config(config_path: &str, sender: &Sender<HubEvent>) -> anyhow::Result<()> {
    let path = Path::new(config_path);
    let dir = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent dir"))?;

    let dir_wide: Vec<u16> = OsStr::new(dir).encode_wide().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            windows::core::PCWSTR(dir_wide.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )?
    };

    tracing::info!(path = config_path, "Config watcher started");

    let mut buffer = [0u8; 1024];
    loop {
        let mut bytes_returned = 0u32;
        unsafe {
            ReadDirectoryChangesW(
                handle,
                buffer.as_mut_ptr() as *mut _,
                buffer.len() as u32,
                false,
                FILE_NOTIFY_CHANGE_LAST_WRITE,
                Some(&mut bytes_returned),
                None,
                None,
            )?;
        }

        match Config::load(config_path) {
            Ok(config) => {
                if sender.send(HubEvent::ConfigReloaded(config)).is_err() {
                    break;
                }
            }
            Err(e) => tracing::warn!("Failed to reload config: {e}"),
        }
    }

    Ok(())
}
