mod config;
mod core;
#[cfg(target_os = "macos")]
mod dome;
#[cfg(target_os = "macos")]
mod objc2_wrapper;
#[cfg(target_os = "macos")]
mod window;

#[cfg(target_os = "macos")]
pub use dome::{check_accessibility, run_app};
