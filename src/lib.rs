pub mod cli;
mod config;
mod core;
mod font;
mod integrations;
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

pub use dome_ipc::action;
pub use dome_ipc::{
    Action, DomeClient, FocusTarget, IpcMessage, MasterTarget, MonitorTarget, MoveTarget, Query,
    TabDirection, ToggleTarget, WindowId,
};

#[cfg(target_os = "macos")]
pub use platform::macos::run_app;

#[cfg(target_os = "windows")]
pub use platform::windows::run_app;
