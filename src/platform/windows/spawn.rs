use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::core::PWSTR;

/// Runs `command` through `cmd.exe /C`, so a full command line works: pipes,
/// `&&`, redirects, and any program on PATH. There is no shell "open" verb, so
/// open a URL, document, or folder with `start <target>` inside the command.
pub(super) fn spawn(command: &str) -> Result<(), anyhow::Error> {
    let command = command.trim();
    if command.is_empty() {
        return Ok(());
    }

    // CreateProcessW may write to the command-line buffer, so it must be mutable.
    let mut command_line: Vec<u16> = format!("cmd.exe /C {command}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

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
            // Keeps the intermediate cmd.exe from flashing a console.
            CREATE_NO_WINDOW,
            None,
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
