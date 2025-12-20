mod config;
mod core;
#[cfg(target_os = "macos")]
mod window;
#[cfg(target_os = "macos")]
mod window_manager;

#[cfg(target_os = "macos")]
pub use window_manager::{check_accessibility, run_app};
