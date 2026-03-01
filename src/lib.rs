mod action;
mod config;
mod core;
mod ipc;
mod overlay;
mod platform;

pub use action::{Action, FocusTarget, MoveTarget, ToggleTarget};
pub use ipc::DomeClient;

#[cfg(target_os = "macos")]
pub use platform::macos::run_app;

#[cfg(target_os = "windows")]
pub use platform::windows::run_app;
