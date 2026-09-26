#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) mod keymap;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) mod render;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) mod tab_bar;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) mod shell_menu;
