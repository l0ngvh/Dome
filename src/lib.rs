mod action;
pub mod cli;
mod config;
mod core;
mod font;
mod ipc;
mod keymap;
mod log_dedup;
mod logging;
mod overlay;
mod platform;
mod theme;

#[expect(
    unused_imports,
    reason = "debug_once and warn_once reserved for future callers"
)]
pub(crate) use log_dedup::{debug_once, trace_once, warn_once};

pub use action::{
    Action, FocusTarget, IpcMessage, MasterTarget, MonitorTarget, MoveTarget, Query, TabDirection,
    ToggleTarget,
};
pub use ipc::DomeClient;

#[cfg(target_os = "macos")]
pub use platform::macos::run_app;

#[cfg(target_os = "windows")]
pub use platform::windows::run_app;
