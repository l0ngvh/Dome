mod config;
#[cfg(all(target_os = "macos", not(test)))]
mod window;
#[cfg(all(target_os = "macos", not(test)))]
mod window_manager;
mod workspace;

#[cfg(all(target_os = "macos", not(test)))]
pub use window_manager::WindowManager;
