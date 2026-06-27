use std::env;
use std::fs;
use std::path::PathBuf;

use windows::core::{HSTRING, Interface, PCWSTR};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoInitializeEx,
                                  COINIT_APARTMENTTHREADED, IPersistFile};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

const LINK_NAME: &str = "Dome.lnk";

/// Places or removes a shortcut in the Startup folder
/// (`%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`)
/// so Dome starts (or stops starting) at login.
///
/// Best-effort: errors are logged but not propagated — failing to register
/// a login item should never prevent Dome from running.
pub(super) fn sync_login_item(enabled: bool) {
    let startup_dir = match startup_folder() {
        Some(d) => d,
        None => {
            tracing::warn!("Cannot determine Startup folder (APPDATA not set)");
            return;
        }
    };
    let shortcut_path = startup_dir.join(LINK_NAME);

    if enabled {
        let exe_path = match current_exe_abs() {
            Some(p) => p,
            None => return,
        };
        create_shortcut(&exe_path, &shortcut_path);
    } else {
        match fs::remove_file(&shortcut_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!("Failed to remove startup shortcut: {e}"),
        }
    }
}

fn startup_folder() -> Option<PathBuf> {
    let appdata = env::var("APPDATA").ok()?;
    Some(PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup"))
}

fn current_exe_abs() -> Option<String> {
    let p = match env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Cannot determine executable path for login item: {e}");
            return None;
        }
    };
    let s = p.to_string_lossy();
    if s.contains(r"\target\debug\") || s.contains(r"\target\release\") {
        tracing::warn!("start_at_login ignored: running from a development build");
        return None;
    }
    Some(s.into_owned())
}

fn create_shortcut(exe_path: &str, shortcut_path: &std::path::Path) {
    // COM may not be initialised on this thread (e.g. config-watcher callback).
    // Safe to call when already initialised with the same model.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let shell_link: IShellLinkW = match unsafe {
        CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
    } {
        Ok(sl) => sl,
        Err(e) => {
            tracing::warn!("Failed to create ShellLink COM object: {e}");
            return;
        }
    };

    let exe_hs = HSTRING::from(exe_path);
    if let Err(e) = unsafe { shell_link.SetPath(PCWSTR(exe_hs.as_ptr())) } {
        tracing::warn!("Failed to set shortcut target path: {e}");
        return;
    }

    let args_hs = HSTRING::from("launch");
    if let Err(e) = unsafe { shell_link.SetArguments(PCWSTR(args_hs.as_ptr())) } {
        tracing::warn!("Failed to set shortcut arguments: {e}");
        return;
    }

    let desc_hs = HSTRING::from("Dome");
    if let Err(e) = unsafe { shell_link.SetDescription(PCWSTR(desc_hs.as_ptr())) } {
        tracing::warn!("Failed to set shortcut description: {e}");
        return;
    }

    if let Some(parent) = std::path::Path::new(exe_path).parent() {
        let wd_hs = HSTRING::from(parent.to_string_lossy().as_ref());
        if let Err(e) = unsafe { shell_link.SetWorkingDirectory(PCWSTR(wd_hs.as_ptr())) } {
            tracing::warn!("Failed to set shortcut working directory: {e}");
            return;
        }
    }

    let persist_file: IPersistFile = match shell_link.cast() {
        Ok(pf) => pf,
        Err(e) => {
            tracing::warn!("Failed to query IPersistFile from IShellLink: {e}");
            return;
        }
    };

    if let Some(parent) = shortcut_path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        tracing::warn!("Failed to create Startup folder: {e}");
        return;
    }

    let path_hs = HSTRING::from(shortcut_path.to_string_lossy().as_ref());
    if let Err(e) = unsafe { persist_file.Save(PCWSTR(path_hs.as_ptr()), true) } {
        tracing::warn!("Failed to save startup shortcut: {e}");
    }
}
