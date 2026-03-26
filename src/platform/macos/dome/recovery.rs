use std::collections::HashMap;
use std::sync::Arc;

use objc2_core_graphics::CGWindowID;

use crate::core::Dimension;

use super::super::accessibility::AXWindowApi;

struct WindowState {
    window: Arc<dyn AXWindowApi>,
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
    // completely offscreen and have to depend on screen size. Screen size can change, and plugging
    // multiple monitors can make the exact placement of where we hide windows fuzzy
    // This has the side effect of moving all windows from different monitor on exit/crash, but that is
    // acceptable
    pub(super) fn track(&mut self, window: Arc<dyn AXWindowApi>, screen: Dimension) {
        let Ok((width, height)) = window.get_size() else {
            return;
        };
        let original_dim = default_position(screen, width as f32, height as f32);
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

fn default_position(screen: Dimension, width: f32, height: f32) -> Dimension {
    Dimension {
        x: screen.x + (screen.width - width) / 2.0,
        y: screen.y + (screen.height - height) / 2.0,
        width,
        height,
    }
}
