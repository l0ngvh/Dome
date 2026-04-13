use std::env;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "Dome";

/// Registers or unregisters Dome in the Windows Registry `HKCU\...\Run` key
/// so it starts (or stops starting) at login.
///
/// Best-effort: errors are logged but not propagated — failing to register
/// a login item should never prevent Dome from running.
pub(super) fn sync_login_item(enabled: bool) {
    if enabled {
        let exe_path = match env::current_exe() {
            Ok(p) => {
                let s = p.to_string_lossy();
                // Same dev-build guard as macOS: don't register a cargo build artifact.
                if s.contains(r"\target\debug\") || s.contains(r"\target\release\") {
                    tracing::warn!("start_at_login ignored: running from a development build");
                    return;
                }
                s.into_owned()
            }
            Err(e) => {
                tracing::warn!("Cannot determine executable path for login item: {e}");
                return;
            }
        };

        let key = match windows_registry::CURRENT_USER.create(RUN_KEY) {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!("Failed to open registry Run key: {e}");
                return;
            }
        };
        // Quoted path + "launch" subcommand, matching the macOS LaunchAgent ProgramArguments.
        if let Err(e) = key.set_string(VALUE_NAME, format!("\"{exe_path}\" launch")) {
            tracing::warn!("Failed to set registry login item: {e}");
        }
    } else {
        let key = match windows_registry::CURRENT_USER.open(RUN_KEY) {
            Ok(k) => k,
            Err(_) => return, // Key doesn't exist, nothing to remove
        };
        match key.remove_value(VALUE_NAME) {
            Ok(()) => {}
            Err(e) => {
                // Named constant for the HRESULT so the magic number is documented.
                const FILE_NOT_FOUND: i32 = 0x80070002_u32 as i32;
                if e.code().0 != FILE_NOT_FOUND {
                    tracing::warn!("Failed to remove registry login item: {e}");
                }
            }
        }
    }
}
