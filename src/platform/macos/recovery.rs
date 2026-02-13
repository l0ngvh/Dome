use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use objc2_core_graphics::CGWindowID;

use crate::core::Dimension;

use super::accessibility::AXWindow;

struct WindowState {
    window: AXWindow,
    original_dim: Dimension,
}

static RECOVERY_STATE: LazyLock<Mutex<HashMap<CGWindowID, WindowState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// Unlike on Windows, we can't reliably tell a window is hidden by us, as we can't move windows
// completely offscreen and have to depend on screen size. Screen size can change, and plugging
// multiple monitors can make the exact placement of where we hide windows fuzzy
// This has the side effect of moving all windows from different monitor on exit/crash, but that is
// acceptable
pub(super) fn track(window: AXWindow, screen: Dimension) {
    let Ok((width, height)) = window.get_size() else {
        return;
    };
    let original_dim = default_position(screen, width as f32, height as f32);
    if let Ok(mut state) = RECOVERY_STATE.lock() {
        state.insert(
            window.cg_id(),
            WindowState {
                window,
                original_dim,
            },
        );
    }
}

fn default_position(screen: Dimension, width: f32, height: f32) -> Dimension {
    Dimension {
        x: screen.x + (screen.width - width) / 2.0,
        y: screen.y + (screen.height - height) / 2.0,
        width,
        height,
    }
}

pub(super) fn untrack(cg_id: CGWindowID) {
    if let Ok(mut state) = RECOVERY_STATE.lock() {
        state.remove(&cg_id);
    }
}

pub(super) fn restore_all() {
    if let Ok(mut state) = RECOVERY_STATE.lock() {
        for window_state in state.values_mut() {
            let dim = window_state.original_dim;
            let _ = window_state.window.set_frame(
                dim.x as i32,
                dim.y as i32,
                dim.width as i32,
                dim.height as i32,
            );
        }
    }
}

pub(super) fn install_handlers() {
    unsafe {
        libc::signal(libc::SIGINT, signal_handler as usize);
        libc::signal(libc::SIGTERM, signal_handler as usize);
        libc::signal(libc::SIGHUP, signal_handler as usize);
    }
}

extern "C" fn signal_handler(sig: libc::c_int) {
    restore_all();
    unsafe {
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}
