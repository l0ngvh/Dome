mod event_loop;
mod menu;
mod window;
mod wnd_proc;

pub(crate) use event_loop::{EventLoop, LoopHandle, LoopWaker};
pub use menu::{MenuEntry, MenuItem};
pub use window::AuxiliaryWindowExtWindows;
pub(crate) use window::Window;

use windows::Win32::UI::WindowsAndMessaging::WM_APP;
use windows::core::{PCWSTR, w};

/// Windows baseline DPI (100% scaling).
const BASE_DPI: f32 = 96.0;

/// Wakes the crate's window pump. A thread message, so it never reaches a wnd-proc.
const WM_APP_WAKE: u32 = WM_APP;

/// The tray icon's callback message, posted by the shell to the owner window. The crate
/// picks it, so any value at or above `WM_APP` works.
const WM_APP_TRAY: u32 = WM_APP + 1;

/// The tray icon's identifier within its owner window. The crate installs one icon per
/// window, so a fixed id is enough.
const TRAY_UID: u32 = 1;

/// The single class every auxiliary window shares. Self-exclusion from window
/// management keys on `WS_EX_TOOLWINDOW`, not the class name, so one class is enough.
const CLASS_NAME: PCWSTR = w!("DomeAuxiliaryWindow");
