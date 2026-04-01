use std::collections::HashMap;
use std::sync::Arc;

use crate::core::Dimension;
use crate::platform::windows::external::{HwndId, ManageExternalHwnd};
use crate::platform::windows::handle::OFFSCREEN_POS;

use crate::platform::windows::taskbar::ManageTaskbar;

const DEFAULT_WIDTH: f32 = 800.0;
const DEFAULT_HEIGHT: f32 = 600.0;

struct RecoveryEntry {
    ext: Arc<dyn ManageExternalHwnd>,
    dimension: Dimension,
    is_maximized: bool,
}

pub(super) struct Recovery {
    state: HashMap<HwndId, RecoveryEntry>,
    taskbar: Arc<dyn ManageTaskbar>,
}

impl Recovery {
    pub(super) fn new(taskbar: Arc<dyn ManageTaskbar>) -> Self {
        Self {
            state: HashMap::new(),
            taskbar,
        }
    }

    pub(super) fn track(&mut self, ext: &Arc<dyn ManageExternalHwnd>, dim: Dimension) {
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

        self.state.insert(
            ext.id(),
            RecoveryEntry {
                ext: ext.clone(),
                dimension: original_dim,
                is_maximized,
            },
        );
    }

    pub(super) fn untrack(&mut self, id: HwndId) {
        self.state.remove(&id);
    }

    pub(super) fn restore_all(&self) {
        for entry in self.state.values() {
            entry.ext.recover(entry.dimension, entry.is_maximized);
            self.taskbar.add_tab(entry.ext.id());
        }
    }
}
