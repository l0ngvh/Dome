mod action;
mod config;
mod core;
mod platform;

pub use action::{Action, FocusTarget, MoveTarget, ToggleTarget};

#[cfg(target_os = "macos")]
pub use platform::macos::{run_app, send_action};

#[cfg(target_os = "windows")]
pub use platform::windows::{run_app, send_action};
