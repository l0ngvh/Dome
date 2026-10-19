use std::collections::HashMap;

use dome_core::{Length, PixelRect, WindowId};

use super::super::window_server::WindowServer;

pub(super) struct Recovery {
    state: HashMap<WindowId, WindowState>,
}

impl Recovery {
    pub(super) fn new() -> Self {
        Self {
            state: HashMap::new(),
        }
    }

    // Unlike on Windows, we can't reliably tell a window is hidden by us, as we can't move windows
    // completely offscreen and have to depend on monitor size. Monitor size can change, and plugging
    // multiple monitors can make the exact placement of where we hide windows fuzzy
    // This has the side effect of moving all windows from different monitor on exit/crash, but that is
    // acceptable
    pub(super) fn track(&mut self, window_id: WindowId, w: Length, h: Length, monitor: PixelRect) {
        let original_rect = default_position(monitor, w, h);
        self.state.insert(window_id, WindowState { original_rect });
    }

    pub(super) fn untrack(&mut self, window_id: WindowId) {
        self.state.remove(&window_id);
    }

    pub(super) fn restore_all(&self, server: &dyn WindowServer) {
        for (&window_id, window_state) in &self.state {
            let _ = server.set_frame(window_id, window_state.original_rect);
        }
    }
}

struct WindowState {
    original_rect: PixelRect,
}

fn default_position(monitor: PixelRect, width: Length, height: Length) -> PixelRect {
    PixelRect::from_pixels(
        monitor.x() + (monitor.width() - width) / 2,
        monitor.y() + (monitor.height() - height) / 2,
        width,
        height,
    )
}
