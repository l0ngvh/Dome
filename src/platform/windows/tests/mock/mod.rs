mod display;
mod scene;
mod window;
mod z_order;

use std::sync::{Arc, Mutex};

use super::env::FocusTarget;
use crate::core::Dimension;
use crate::platform::windows::external::HwndId;
use crate::platform::windows::taskbar::ManageTaskbar;

pub(crate) use display::MockDisplay;
pub(crate) use scene::{MockSceneSender, OverlayReport};
pub(crate) use window::MockExternalHwnd;
pub(crate) use z_order::ZOrderStack;

/// The env state a mock writes into. The env hands it over once, at construction.
#[derive(Clone)]
pub(crate) struct MockWiring {
    pub(crate) moves: MoveLog,
    pub(crate) z_stack: ZOrderStack,
    pub(crate) focus_target: Arc<Mutex<FocusTarget>>,
}

/// Every rect a mock was told to take, in the order Dome asked for it.
#[derive(Clone, Default)]
pub(crate) struct MoveLog {
    entries: Arc<Mutex<Vec<(HwndId, Dimension)>>>,
}

impl MoveLog {
    pub(crate) fn record(&self, hwnd: HwndId, dim: Dimension) {
        self.entries.lock().unwrap().push((hwnd, dim));
    }

    pub(crate) fn take(&self) -> Vec<(HwndId, Dimension)> {
        std::mem::take(&mut *self.entries.lock().unwrap())
    }

    pub(crate) fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }

    pub(crate) fn contains(&self, hwnd: HwndId) -> bool {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .any(|&(id, _)| id == hwnd)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.lock().unwrap().is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
}

pub(crate) struct NoopTaskbar;

impl ManageTaskbar for NoopTaskbar {
    fn add_tab(&self, _: HwndId) {}
    fn delete_tab(&self, _: HwndId) {}
}
