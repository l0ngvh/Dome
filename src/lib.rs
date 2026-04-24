mod action;
mod config;
mod core;
mod ipc;
mod logging;
mod overlay;
mod platform;

pub use action::{Action, FocusTarget, HubAction, IpcMessage, MoveTarget, Query, ToggleTarget};
pub use ipc::DomeClient;

#[cfg(target_os = "macos")]
pub use platform::macos::run_app;

#[cfg(target_os = "windows")]
pub use platform::windows::run_app;
