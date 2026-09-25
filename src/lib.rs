pub mod cli;
mod config;
mod core;
mod font;
mod integrations;
mod ipc;
mod logging;
mod overlay;
mod platform;
mod theme;

#[expect(
    unused_imports,
    reason = "debug_once has no callers, and warn_once callers name crate::logging directly"
)]
pub(crate) use logging::{debug_once, trace_once, warn_once};

pub use dome_ipc::action;
pub use dome_ipc::{
    Action, DomeClient, FocusTarget, IpcMessage, MasterTarget, MonitorTarget, MoveTarget, Query,
    TabDirection, ToggleTarget, WindowId,
};

#[cfg(target_os = "macos")]
pub use platform::macos::run_app;

#[cfg(target_os = "windows")]
pub use platform::windows::run_app;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn run_app(_config_path: Option<String>, _layout_path: Option<String>) -> anyhow::Result<()> {
    anyhow::bail!("dome has no window backend on this platform")
}
