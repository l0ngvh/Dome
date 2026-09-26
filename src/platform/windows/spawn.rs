use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use windows::Win32::System::Threading::CREATE_NO_WINDOW;

/// Runs `command` through `cmd.exe /C`, so a full command line works: pipes,
/// `&&`, redirects, and any program on PATH.
pub(super) fn spawn(command: &str, env: &HashMap<String, String>) -> Result<(), anyhow::Error> {
    let command = command.trim();
    if command.is_empty() {
        return Ok(());
    }

    // `Command` builds the child's environment block itself, so Windows does
    // the case-insensitive name matching and the sort. An override then
    // replaces an inherited entry by Windows' own rule, not by a hand-rolled
    // one. Windows sorts and folds case by an undocumented rule, so a block
    // built by hand cannot match it. See
    // https://nullprogram.com/blog/2023/08/23/ and
    // https://learn.microsoft.com/en-us/windows/win32/procthread/changing-environment-variables
    // `raw_arg` splices the command line in without quoting, so the shell
    // metacharacters survive. `CREATE_NO_WINDOW` keeps the intermediate
    // cmd.exe from flashing a console. Dome does not wait on the child, so the
    // returned handle is dropped.
    Command::new("cmd.exe")
        .arg("/C")
        .raw_arg(command)
        .envs(env)
        .creation_flags(CREATE_NO_WINDOW.0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}
