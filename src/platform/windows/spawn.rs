use std::collections::{BTreeMap, HashMap};
use std::ffi::c_void;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::core::PWSTR;

/// Runs `command` through `cmd.exe /C`, so a full command line works: pipes,
/// `&&`, redirects, and any program on PATH. There is no shell "open" verb, so
/// open a URL, document, or folder with `start <target>` inside the command.
///
/// `env` is layered over Dome's own environment for the child, so a `PATH` set
/// in config reaches the program cmd.exe resolves. `CREATE_NO_WINDOW` keeps the
/// intermediate cmd.exe from flashing a console.
pub(super) fn spawn(command: &str, env: &HashMap<String, String>) -> Result<(), anyhow::Error> {
    let command = command.trim();
    if command.is_empty() {
        return Ok(());
    }

    // CreateProcessW may write to the command-line buffer, so it must be mutable.
    let mut command_line: Vec<u16> = format!("cmd.exe /C {command}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let env_block = env_block(std::env::vars(), env);

    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();

    unsafe {
        CreateProcessW(
            None,
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
            Some(env_block.as_ptr() as *const c_void),
            None,
            &startup,
            &mut info,
        )
    }?;

    // Dome does not wait on the child, so release the handles it owns.
    unsafe {
        CloseHandle(info.hProcess).ok();
        CloseHandle(info.hThread).ok();
    }
    Ok(())
}

// A UTF-16 environment block: KEY=VALUE strings, each nul-terminated, closed by
// a final nul. A user override replaces the inherited value for the same key.
// BTreeMap dedupes by key and keeps the order stable for tests.
fn env_block(
    base: impl Iterator<Item = (String, String)>,
    overrides: &HashMap<String, String>,
) -> Vec<u16> {
    let mut merged: BTreeMap<String, String> = base.collect();
    for (key, value) in overrides {
        merged.insert(key.clone(), value.clone());
    }
    let mut block = Vec::new();
    for (key, value) in merged {
        block.extend(format!("{key}={value}").encode_utf16());
        block.push(0);
    }
    block.push(0);
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_block_overrides_inherited_and_double_terminates() {
        let base = [
            ("PATH".to_string(), "/a".to_string()),
            ("HOME".to_string(), "/h".to_string()),
        ]
        .into_iter();
        let mut overrides = HashMap::new();
        overrides.insert("PATH".to_string(), "/b".to_string());
        overrides.insert("EDITOR".to_string(), "nvim".to_string());

        let block = env_block(base, &overrides);
        let decoded = String::from_utf16(&block).unwrap();
        let entries: Vec<&str> = decoded.split('\0').filter(|s| !s.is_empty()).collect();

        assert!(entries.contains(&"PATH=/b"));
        assert!(entries.contains(&"HOME=/h"));
        assert!(entries.contains(&"EDITOR=nvim"));
        assert_eq!(entries.len(), 3);
        assert_eq!(&block[block.len() - 2..], &[0, 0]);
    }
}
