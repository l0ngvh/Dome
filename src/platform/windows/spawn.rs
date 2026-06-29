use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;

/// Launches a command via `ShellExecuteW` (the Windows "open" verb).
///
/// Handles executables, documents, URLs, and folders. No intermediate
/// `cmd.exe` window — the target opens directly. Splits the first
/// whitespace-separated token as the program; the rest is passed as
/// arguments. Empty command is a no-op.
///
/// Returns `Ok(())` when `ShellExecuteW` reports success (>32). Returns
/// `Err` on failure, including when the user cancels a UAC prompt or
/// the association is missing.
pub(super) fn spawn(command: &str) -> Result<(), anyhow::Error> {
    let command = command.trim();
    if command.is_empty() {
        return Ok(());
    }

    let (program, args) = split_first_arg(command);

    let operation_wide: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
    let program_wide: Vec<u16> = program.encode_utf16().chain(std::iter::once(0)).collect();
    let args_wide: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();

    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),                     // hwnd
            PCWSTR::from_raw(operation_wide.as_ptr()), // lpOperation
            PCWSTR::from_raw(program_wide.as_ptr()),   // lpFile
            PCWSTR::from_raw(args_wide.as_ptr()),      // lpParameters
            PCWSTR::null(),                            // lpDirectory
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns a value > 32 on success (as an HINSTANCE).
    // Values <= 32 are error codes per the Win32 convention.
    if result.0 as isize > 32 {
        Ok(())
    } else {
        let code = result.0 as isize;
        let msg = match code {
            0 => "out of memory or resources".into(),
            2 => "file not found".into(),
            3 => "path not found".into(),
            5 => "access denied / UAC canceled".into(),
            8 => "out of memory".into(),
            10 => "bad executable (16-bit on 64-bit system)".into(),
            11 => "invalid EXE / missing association".into(),
            26 => "sharing violation".into(),
            27 => "incomplete association".into(),
            28 => "DDE timeout".into(),
            29 => "DDE failed".into(),
            30 => "DDE busy".into(),
            31 => "no association".into(),
            32 => "DLL not found".into(),
            _ => format!("unknown error code {}", code),
        };
        anyhow::bail!("ShellExecuteW failed ({}): {msg}", code)
    }
}

/// Splits `input` into `(first_token, rest)` on the first whitespace run.
///
/// Handles quoted tokens: `"foo bar" baz` → `(foo bar, baz)`.
/// Treats leading whitespace as part of the separator (no program).
///
/// Owns the return values (two `String`s) so the caller can encode them
/// to wide strings independently.
fn split_first_arg(input: &str) -> (String, String) {
    let input = input.trim_start();
    if input.is_empty() {
        return (String::new(), String::new());
    }

    if let Some(rest) = input.strip_prefix('"') {
        // Quoted program: scan to the closing quote.
        if let Some(end) = rest.find('"') {
            let program = &rest[..end];
            let after = rest[end + 1..].trim_start();
            return (program.to_string(), after.to_string());
        }
        // No closing quote: treat the rest as the program name,
        // with the leading quote stripped.
        (rest.to_string(), String::new())
    } else if let Some(idx) = input.find(char::is_whitespace) {
        let program = &input[..idx];
        let args = input[idx..].trim_start();
        (program.to_string(), args.to_string())
    } else {
        (input.to_string(), String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_simple() {
        assert_eq!(
            split_first_arg("notepad.exe foo.txt"),
            ("notepad.exe".into(), "foo.txt".into())
        );
    }

    #[test]
    fn split_no_args() {
        assert_eq!(
            split_first_arg("firefox.exe"),
            ("firefox.exe".into(), "".into())
        );
    }

    #[test]
    fn split_quoted_program() {
        assert_eq!(
            split_first_arg("\"C:\\Program Files\\App\\app.exe\" --flag"),
            ("C:\\Program Files\\App\\app.exe".into(), "--flag".into())
        );
    }

    #[test]
    fn split_unclosed_quote() {
        assert_eq!(
            split_first_arg("\"C:\\Program Files"),
            ("C:\\Program Files".into(), "".into())
        );
    }

    #[test]
    fn split_empty() {
        assert_eq!(split_first_arg(""), ("".into(), "".into()));
    }

    #[test]
    fn split_only_whitespace() {
        assert_eq!(split_first_arg("   "), ("".into(), "".into()));
    }

    #[test]
    fn split_preserves_inner_spaces_in_args() {
        assert_eq!(
            split_first_arg("code --install-extension rust-lang.rust-analyzer"),
            (
                "code".into(),
                "--install-extension rust-lang.rust-analyzer".into()
            )
        );
    }
}
