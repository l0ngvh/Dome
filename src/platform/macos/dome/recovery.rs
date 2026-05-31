use std::collections::HashMap;
use std::sync::Arc;

use objc2_core_graphics::CGWindowID;

use crate::core::{Dimension, Length};

use super::super::accessibility::ExternalWindow;

struct WindowState {
    window: Arc<dyn ExternalWindow>,
    original_dim: Dimension,
}

pub(super) struct Recovery {
    state: HashMap<CGWindowID, WindowState>,
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
    pub(super) fn track(
        &mut self,
        window: Arc<dyn ExternalWindow>,
        w: i32,
        h: i32,
        monitor: Dimension,
    ) {
        let original_dim = default_position(monitor, w as f32, h as f32);
        self.state.insert(
            window.cg_id(),
            WindowState {
                window,
                original_dim,
            },
        );
    }

    pub(super) fn untrack(&mut self, cg_id: CGWindowID) {
        self.state.remove(&cg_id);
    }

    pub(super) fn restore_all(&self) {
        for window_state in self.state.values() {
            let _ = window_state.window.set_frame(window_state.original_dim);
        }
    }
}

fn default_position(monitor: Dimension, width: f32, height: f32) -> Dimension {
    Dimension::new(
        monitor.x + (monitor.width - Length::new(width)) / 2.0,
        monitor.y + (monitor.height - Length::new(height)) / 2.0,
        Length::new(width),
        Length::new(height),
    )
}
