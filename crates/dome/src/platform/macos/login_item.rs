use std::path::Path;
use std::process::Command;

const LABEL: &str = "com.longvh.dome";
const PLIST_NAME: &str = "com.longvh.dome.plist";

/// Returns the enclosing application bundle path, if present.
pub(super) fn detect_bundle_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
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
/// Errors are logged but not propagated, because failing to
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

        // Unload the old job in case its executable path changed.
        if Path::new(&plist_path).exists() {
            run_launchctl("bootout", &[&format!("gui/{uid}/{LABEL}")]);
        }

        if let Err(e) = std::fs::create_dir_all(format!("{home}/Library/LaunchAgents")) {
            tracing::warn!("Failed to create LaunchAgents directory: {e}");
            return;
        }

        let plist_content = login_item_plist(&exe_path);

        if let Err(e) = std::fs::write(&plist_path, plist_content) {
            tracing::warn!("Failed to write login item plist: {e}");
            return;
        }

        run_launchctl("bootstrap", &[&format!("gui/{uid}"), &plist_path]);
    } else {
        run_launchctl("bootout", &[&format!("gui/{uid}/{LABEL}")]);

        match std::fs::remove_file(&plist_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!("Failed to remove login item plist: {e}"),
        }
    }
}

fn login_item_plist(exe_path: &str) -> String {
    let exe_path = exe_path
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
        .replace('"', "&quot;");
    format!(
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
    )
}

fn run_launchctl(operation: &str, arguments: &[&str]) {
    match Command::new("launchctl")
        .arg(operation)
        .args(arguments)
        .output()
    {
        Ok(output) if output.status.success() => {}
        // A service-target bootout returns ESRCH when the job is already unloaded.
        Ok(output) if operation == "bootout" && output.status.code() == Some(libc::ESRCH) => {}
        Ok(output) => tracing::warn!(
            operation, status = %output.status, stderr = %String::from_utf8_lossy(&output.stderr),
            "launchctl failed for login item"
        ),
        Err(error) => tracing::warn!(operation, %error, "Failed to run launchctl for login item"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn generate_login_item_plist_escapes_executable_path() {
        let plist = super::login_item_plist("/Apps/Tom & \"Jerry\" <work> 'test'/dome");
        assert!(plist.contains(
            "<string>/Apps/Tom &amp; &quot;Jerry&quot; &lt;work&gt; &apos;test&apos;/dome</string>"
        ));
        assert!(plist.contains("<string>launch</string>"));
    }
}
