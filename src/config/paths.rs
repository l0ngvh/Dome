//! Where Dome's files live on each OS.

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
pub(crate) fn log_dir() -> String {
    format!("{}/Library/Logs/dome", home_dir())
}

#[cfg(target_os = "windows")]
pub(crate) fn log_dir() -> String {
    format!("{}\\dome\\logs", roaming_app_data())
}

#[cfg(target_os = "linux")]
pub(crate) fn log_dir() -> String {
    let data_dir = std::env::var("XDG_STATE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}/.local/state", home_dir()));
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
    format!("{}\\dome\\config.lua", roaming_app_data())
}

#[cfg(not(target_os = "windows"))]
pub(super) fn default_path() -> String {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}/.config", home_dir()));
    format!("{config_dir}/dome/config.lua")
}

/// The environment is the first source, because a user who sets `HOME` means it.
/// An empty value counts as unset, since it would otherwise resolve every path
/// against the filesystem root.
#[cfg(target_os = "macos")]
fn home_dir() -> String {
    if let Some(home) = std::env::var("HOME").ok().filter(|s| !s.is_empty()) {
        return home;
    }
    passwd_home()
        .filter(|s| !s.is_empty())
        .expect("no home directory: HOME is unset and the passwd record carries none")
}

#[cfg(target_os = "linux")]
fn home_dir() -> String {
    std::env::var("HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .expect("no home directory: HOME is unset")
}

/// launchd can start Dome with no `HOME` at all, and the passwd record is
/// populated there.
#[cfg(target_os = "macos")]
pub(super) fn passwd_home() -> Option<String> {
    // libc owns the passwd record, so freeing it here would be a double free.
    let passwd = unsafe { libc::getpwuid(libc::getuid()) };
    if passwd.is_null() {
        return None;
    }
    let dir = unsafe { (*passwd).pw_dir };
    if dir.is_null() {
        return None;
    }
    unsafe { std::ffi::CStr::from_ptr(dir) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

/// `APPDATA` is absent for a service account, and the shell knows the path
/// without it.
#[cfg(target_os = "windows")]
fn roaming_app_data() -> String {
    if let Some(dir) = std::env::var("APPDATA").ok().filter(|s| !s.is_empty()) {
        return dir;
    }
    known_folder_roaming_app_data()
        .filter(|s| !s.is_empty())
        .expect("no roaming app data directory: APPDATA is unset and the shell reports none")
}

#[cfg(target_os = "windows")]
fn known_folder_roaming_app_data() -> Option<String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{
        FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    };

    // SHGetKnownFolderPath allocates the buffer, so the caller frees it.
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None) }
        .ok()
        .filter(|path| !path.is_null())?;
    let owned = unsafe { path.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(path.0 as *const std::ffi::c_void)) };
    owned
}
