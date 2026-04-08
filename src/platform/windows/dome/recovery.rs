use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::platform::windows::external::{HwndId, ManageExternalHwnd};
use crate::platform::windows::taskbar::ManageTaskbar;

struct RecoveryEntry {
    ext: Arc<dyn ManageExternalHwnd>,
    is_maximized: bool,
}

pub(super) struct Recovery {
    state: HashMap<HwndId, RecoveryEntry>,
    taskbar: Rc<dyn ManageTaskbar>,
}

impl Recovery {
    pub(super) fn new(taskbar: Rc<dyn ManageTaskbar>) -> Self {
        Self {
            state: HashMap::new(),
            taskbar,
        }
    }

    pub(super) fn track(&mut self, ext: &Arc<dyn ManageExternalHwnd>) {
        let is_maximized = ext.is_maximized();
        self.state.insert(
            ext.id(),
            RecoveryEntry {
                ext: ext.clone(),
                is_maximized,
            },
        );
    }

    pub(super) fn untrack(&mut self, id: HwndId) {
        self.state.remove(&id);
    }

    pub(super) fn restore_all(&self) {
        for entry in self.state.values() {
            entry.ext.recover(entry.is_maximized);
            self.taskbar.add_tab(entry.ext.id());
        }
    }
}
