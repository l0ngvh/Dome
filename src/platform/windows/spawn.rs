use std::collections::{HashMap, HashSet};
use std::ffi::{OsString, c_void};
use std::os::windows::ffi::OsStrExt;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::core::PWSTR;

/// Runs `command` through `cmd.exe /C`, so a full command line works: pipes,
/// `&&`, redirects, and any program on PATH. There is no shell "open" verb, so
/// open a URL, document, or folder with `start <target>` inside the command.
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

    let mut block = build_env_block(env);
    let mut flags = CREATE_NO_WINDOW;
    let environment = match &mut block {
        Some(block) => {
            flags |= CREATE_UNICODE_ENVIRONMENT;
            Some(block.as_ptr() as *const c_void)
        }
        None => None,
    };

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
            flags,
            environment,
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

/// The child's environment block, with `overrides` layered over Dome's own.
/// `None` when nothing is overridden, so the caller inherits Dome's block.
fn build_env_block(overrides: &HashMap<String, String>) -> Option<Vec<u16>> {
    if overrides.is_empty() {
        return None;
    }
    Some(merge_env_block(
        &std::env::vars_os().collect::<Vec<_>>(),
        overrides,
    ))
}

fn merge_env_block(
    inherited: &[(OsString, OsString)],
    overrides: &HashMap<String, String>,
) -> Vec<u16> {
    // A Windows name is case-insensitive, so an override replaces an inherited
    // entry whatever its case.
    let replaced: HashSet<String> = overrides.keys().map(|k| k.to_uppercase()).collect();

    let mut entries: Vec<(OsString, OsString)> = inherited
        .iter()
        .filter(|(name, _)| {
            name.to_str()
                .is_none_or(|name| !replaced.contains(&name.to_uppercase()))
        })
        .cloned()
        .collect();
    for (name, value) in overrides {
        entries.push((OsString::from(name), OsString::from(value)));
    }

    // CreateProcessW reads a sorted block, so sort by the case-folded name.
    entries.sort_by_key(|(name, _)| name.to_string_lossy().to_uppercase());

    let mut block: Vec<u16> = Vec::new();
    for (name, value) in entries {
        block.extend(name.encode_wide());
        block.push(u16::from(b'='));
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::ffi::OsStringExt;

    fn inherited(entries: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        entries
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value)))
            .collect()
    }

    fn decode(block: &[u16]) -> Vec<String> {
        block
            .split(|&unit| unit == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| OsString::from_wide(entry).to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn build_env_block_inherits_when_nothing_is_overridden() {
        assert!(build_env_block(&HashMap::new()).is_none());
    }

    #[test]
    fn merge_env_block_replaces_an_inherited_entry_whatever_its_case() {
        let base = inherited(&[("Path", "C:\\a"), ("HOME", "C:\\users\\x")]);
        let overrides = HashMap::from([("PATH".to_string(), "C:\\b".to_string())]);
        let entries = decode(&merge_env_block(&base, &overrides));
        assert!(entries.contains(&"PATH=C:\\b".to_string()));
        assert_eq!(
            entries
                .iter()
                .filter(|e| e.to_uppercase().starts_with("PATH="))
                .count(),
            1
        );
    }

    #[test]
    fn merge_env_block_keeps_an_entry_no_override_names() {
        let base = inherited(&[("Path", "C:\\a"), ("HOME", "C:\\users\\x")]);
        let overrides = HashMap::from([("PATH".to_string(), "C:\\b".to_string())]);
        let entries = decode(&merge_env_block(&base, &overrides));
        assert!(entries.contains(&"HOME=C:\\users\\x".to_string()));
    }

    #[test]
    fn merge_env_block_keeps_a_drive_current_directory_entry() {
        let base = inherited(&[("=C:", "C:\\work"), ("Path", "C:\\a")]);
        let overrides = HashMap::from([("PATH".to_string(), "C:\\b".to_string())]);
        let entries = decode(&merge_env_block(&base, &overrides));
        assert!(entries.contains(&"=C:=C:\\work".to_string()));
    }

    #[test]
    fn merge_env_block_adds_an_override_not_inherited() {
        let base = inherited(&[("Path", "C:\\a")]);
        let overrides = HashMap::from([("DOME_TEST".to_string(), "1".to_string())]);
        let entries = decode(&merge_env_block(&base, &overrides));
        assert!(entries.contains(&"DOME_TEST=1".to_string()));
    }

    // Windows requires the block sorted by name, case-insensitively. See
    // https://learn.microsoft.com/en-us/windows/win32/procthread/changing-environment-variables
    #[test]
    fn merge_env_block_sorts_names_case_insensitively() {
        let base = inherited(&[("C", "3"), ("b", "2")]);
        let overrides = HashMap::from([("A".to_string(), "1".to_string())]);
        let entries = decode(&merge_env_block(&base, &overrides));
        assert_eq!(entries, ["A=1", "b=2", "C=3"]);
    }

    #[test]
    fn merge_env_block_ends_with_two_nulls() {
        let base = inherited(&[("Path", "C:\\a")]);
        let overrides = HashMap::from([("PATH".to_string(), "C:\\b".to_string())]);
        let block = merge_env_block(&base, &overrides);
        assert!(block.ends_with(&[0, 0]), "{block:?}");
    }
}
