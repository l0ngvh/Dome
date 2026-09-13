//! Where Dome's files live on each OS. Every path here comes from an
//! environment variable.

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
pub(crate) fn log_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{home}/Library/Logs/dome")
}

#[cfg(target_os = "windows")]
pub(crate) fn log_dir() -> String {
    let config_dir = std::env::var("APPDATA").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        format!("{home}\\AppData\\Roaming")
    });
    format!("{config_dir}\\dome\\logs")
}

#[cfg(target_os = "linux")]
pub(crate) fn log_dir() -> String {
    let data_dir = std::env::var("XDG_STATE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/.local/state")
        });
    format!("{data_dir}/dome")
}

pub(crate) fn layout_default_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .expect("config path must have a parent directory")
        .join("layout.lua")
}

#[cfg(target_os = "windows")]
pub(super) fn default_path() -> String {
    let config_dir = std::env::var("APPDATA").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        format!("{home}\\AppData\\Roaming")
    });
    format!("{config_dir}\\dome\\config.lua")
}

#[cfg(not(target_os = "windows"))]
pub(super) fn default_path() -> String {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/.config")
        });
    format!("{config_dir}/dome/config.lua")
}
