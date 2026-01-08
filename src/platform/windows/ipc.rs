use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::mpsc::Sender;

use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_NONE, OPEN_EXISTING, ReadFile, WriteFile,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::core::PCWSTR;

use super::hub::HubEvent;
use crate::action::Action;

const PIPE_NAME: &str = r"\\.\pipe\dome";
const PIPE_ACCESS_DUPLEX: FILE_FLAGS_AND_ATTRIBUTES = FILE_FLAGS_AND_ATTRIBUTES(0x00000003);
const GENERIC_READ_WRITE: u32 = 0xC0000000;

fn pipe_name_wide() -> Vec<u16> {
    OsStr::new(PIPE_NAME).encode_wide().chain(Some(0)).collect()
}

pub(super) fn start_server(sender: Sender<HubEvent>) {
    std::thread::spawn(move || {
        if let Err(e) = run_server(&sender) {
            tracing::error!("IPC server error: {e}");
        }
    });
}

fn run_server(sender: &Sender<HubEvent>) -> anyhow::Result<()> {
    loop {
        let name = pipe_name_wide();
        let pipe = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                PIPE_UNLIMITED_INSTANCES,
                512,
                512,
                0,
                None,
            )
        };

        if pipe == INVALID_HANDLE_VALUE {
            anyhow::bail!("CreateNamedPipeW failed");
        }

        if unsafe { ConnectNamedPipe(pipe, None) }.is_err() {
            unsafe { CloseHandle(pipe)? };
            continue;
        }

        handle_client(pipe, sender);

        unsafe {
            DisconnectNamedPipe(pipe)?;
            CloseHandle(pipe)?;
        }
    }
}

fn handle_client(pipe: HANDLE, sender: &Sender<HubEvent>) {
    let mut buf = [0u8; 4096];
    let mut bytes_read = 0u32;

    if unsafe { ReadFile(pipe, Some(&mut buf), Some(&mut bytes_read), None) }.is_err() {
        return;
    }

    let msg = String::from_utf8_lossy(&buf[..bytes_read as usize]);
    let response = match serde_json::from_str::<Action>(msg.trim()) {
        Ok(action) => {
            sender.send(HubEvent::Action(action)).ok();
            "ok\n"
        }
        Err(e) => {
            tracing::warn!("Invalid IPC message: {e}");
            "error\n"
        }
    };

    if let Err(e) = unsafe { WriteFile(pipe, Some(response.as_bytes()), None, None) } {
        tracing::warn!("WriteFile failed: {e}");
    }
}

pub fn send_action(action: &Action) -> anyhow::Result<()> {
    let name = pipe_name_wide();
    let pipe = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            GENERIC_READ_WRITE,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            Default::default(),
            None,
        )?
    };

    let json = serde_json::to_string(&action)?;
    unsafe { WriteFile(pipe, Some(json.as_bytes()), None, None)? };

    let mut buf = [0u8; 256];
    let mut bytes_read = 0u32;
    unsafe { ReadFile(pipe, Some(&mut buf), Some(&mut bytes_read), None)? };

    unsafe { CloseHandle(pipe)? };

    let response = String::from_utf8_lossy(&buf[..bytes_read as usize]);
    if response.trim() == "ok" {
        Ok(())
    } else {
        anyhow::bail!("IPC error: {}", response.trim())
    }
}
