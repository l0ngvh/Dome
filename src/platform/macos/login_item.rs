use std::path::Path;
use std::process::Command;

const LABEL: &str = "com.dome-wm.dome";
const PLIST_NAME: &str = "com.dome-wm.dome.plist";

/// Detects if the current binary is running inside a `Dome.app` bundle.
/// Returns the path to the `.app` directory (e.g. `/Applications/Dome.app`) if so.
pub(super) fn detect_bundle_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    // Walk up ancestors looking for *.app/Contents/MacOS — the standard bundle layout.
    for ancestor in exe.ancestors() {
        if ancestor.extension().is_some_and(|ext| ext == "app")
            && ancestor.join("Contents/MacOS").is_dir()
        {
            return ancestor.to_str().map(String::from);
        }
    }
    None
}

/// Writes or removes a LaunchAgent plist so Dome starts (or stops starting) at login.
///
/// Uses `launchctl bootstrap`/`bootout` (not the deprecated `load`/`unload`).
/// Best-effort: all errors are logged but not propagated, because failing to
/// register a login item should never prevent Dome from running.
pub(super) fn sync_login_item(enabled: bool, bundle_path: Option<&str>) {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("Cannot determine HOME for login item: {e}");
            return;
        }
    };
    let plist_path = format!("{home}/Library/LaunchAgents/{PLIST_NAME}");
    // SAFETY: libc::getuid() is a trivial syscall with no preconditions.
    let uid = unsafe { libc::getuid() };

    if enabled {
        let exe_path = match bundle_path {
            Some(bp) => format!("{bp}/Contents/MacOS/dome"),
            None => match std::env::current_exe() {
                Ok(p) => {
                    let s = p.to_string_lossy();
                    if s.contains("/target/debug/") || s.contains("/target/release/") {
                        tracing::warn!("start_at_login ignored: running from a development build");
                        return;
                    }
                    s.into_owned()
                }
                Err(e) => {
                    tracing::warn!("Cannot determine executable path for login item: {e}");
                    return;
                }
            },
        };

        // Bootout first if plist already exists (handles moved binary / changed path).
        if Path::new(&plist_path).exists() {
            let _ = Command::new("launchctl")
                .args(["bootout", &format!("gui/{uid}"), &plist_path])
                .output();
        }

        if let Err(e) = std::fs::create_dir_all(format!("{home}/Library/LaunchAgents")) {
            tracing::warn!("Failed to create LaunchAgents directory: {e}");
            return;
        }

        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe_path}</string>
        <string>launch</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#
        );

        if let Err(e) = std::fs::write(&plist_path, plist_content) {
            tracing::warn!("Failed to write login item plist: {e}");
            return;
        }

        if let Err(e) = Command::new("launchctl")
            .args(["bootstrap", &format!("gui/{uid}"), &plist_path])
            .output()
        {
            tracing::warn!("Failed to bootstrap login item: {e}");
        }
    } else {
        if let Err(e) = Command::new("launchctl")
            .args(["bootout", &format!("gui/{uid}"), &plist_path])
            .output()
        {
            tracing::warn!("Failed to bootout login item: {e}");
        }

        match std::fs::remove_file(&plist_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!("Failed to remove login item plist: {e}"),
        }
    }
}
