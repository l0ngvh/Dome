use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows::core::BOOL;

use crate::core::Dimension;
use crate::platform::windows::OFFSCREEN_POS;
use crate::platform::windows::external::{HwndId, ManageExternalHwnd};

use crate::platform::windows::taskbar::Taskbar;

const DEFAULT_WIDTH: f32 = 800.0;
const DEFAULT_HEIGHT: f32 = 600.0;

struct RecoveryEntry {
    ext: Arc<dyn ManageExternalHwnd>,
    dimension: Dimension,
    is_maximized: bool,
}

static RECOVERY_STATE: LazyLock<Mutex<HashMap<HwndId, RecoveryEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn track(ext: &Arc<dyn ManageExternalHwnd>, dim: Dimension) {
    // These windows belongs to previous crashed Dome instances
    let original_dim = if dim.x <= OFFSCREEN_POS || dim.y <= OFFSCREEN_POS {
        Dimension {
            x: 100.0,
            y: 100.0,
            width: if dim.width > 0.0 {
                dim.width
            } else {
                DEFAULT_WIDTH
            },
            height: if dim.height > 0.0 {
                dim.height
            } else {
                DEFAULT_HEIGHT
            },
        }
    } else {
        dim
    };
    let is_maximized = ext.is_maximized();

    if let Ok(mut state) = RECOVERY_STATE.lock() {
        state.insert(
            ext.id(),
            RecoveryEntry {
                ext: ext.clone(),
                dimension: original_dim,
                is_maximized,
            },
        );
    }
}

pub(super) fn untrack(id: HwndId) {
    if let Ok(mut state) = RECOVERY_STATE.lock() {
        state.remove(&id);
    }
}

pub(super) fn restore_all() {
    if let Ok(state) = RECOVERY_STATE.lock() {
        for entry in state.values() {
            entry.ext.recover(entry.dimension, entry.is_maximized);
        }

        if let Ok(taskbar) = Taskbar::new() {
            for entry in state.values() {
                let hwnd: HWND = entry.ext.id().into();
                taskbar.add_tab(hwnd).ok();
            }
        }
    }
}

pub(in crate::platform::windows) fn install_handlers() {
    unsafe {
        let _ = SetConsoleCtrlHandler(Some(console_handler), true);
    }
}

unsafe extern "system" fn console_handler(ctrl_type: u32) -> BOOL {
    if ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_BREAK_EVENT || ctrl_type == CTRL_CLOSE_EVENT {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().ok() };
        restore_all();
    }
    BOOL(0)
}
