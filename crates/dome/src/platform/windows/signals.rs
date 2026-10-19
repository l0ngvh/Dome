use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
use windows::core::BOOL;

static CONSOLE_THREAD: AtomicU32 = AtomicU32::new(0);

/// Posts quit to the creating thread's message pump on console shutdown.
/// Only one console signal owner may be installed in a process.
pub(super) struct ConsoleSignals {
    _private: (),
}

impl ConsoleSignals {
    pub(super) fn for_current_thread() -> anyhow::Result<Self> {
        let thread = unsafe { GetCurrentThreadId() };
        CONSOLE_THREAD
            .compare_exchange(0, thread, Ordering::Relaxed, Ordering::Relaxed)
            .map_err(|_| anyhow::anyhow!("console signals already installed"))?;
        if let Err(error) = unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), true) } {
            CONSOLE_THREAD.store(0, Ordering::Relaxed);
            return Err(error.into());
        }
        Ok(Self { _private: () })
    }
}

impl Drop for ConsoleSignals {
    fn drop(&mut self) {
        unsafe {
            SetConsoleCtrlHandler(Some(console_ctrl_handler), false).ok();
        }
        CONSOLE_THREAD.store(0, Ordering::Relaxed);
    }
}

unsafe extern "system" fn console_ctrl_handler(control: u32) -> BOOL {
    match control {
        CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT => {
            let thread = CONSOLE_THREAD.load(Ordering::Relaxed);
            if thread == 0 {
                return BOOL(0);
            }
            unsafe {
                PostThreadMessageW(thread, WM_QUIT, WPARAM(0), LPARAM(0)).ok();
            }
            // The process is killed after a close handler returns, so allow cleanup to run first.
            if control == CTRL_CLOSE_EVENT {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            BOOL(1)
        }
        _ => BOOL(0),
    }
}
