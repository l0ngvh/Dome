mod config;
mod core;
mod platform;

#[cfg(target_os = "macos")]
pub use platform::macos::run_app;
