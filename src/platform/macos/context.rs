use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Instant};

use objc2_application_services::AXObserver;
use objc2_core_foundation::{CFRetained, CFRunLoopTimer};
use objc2_core_graphics::CGWindowID;

use super::window::AXWindow;

pub(super) type Observers = Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>;

pub(super) struct ThrottleState {
    pub(super) last_execution: Option<Instant>,
    pub(super) timer: Option<CFRetained<CFRunLoopTimer>>,
}

impl ThrottleState {
    pub(super) fn new() -> Self {
        Self {
            last_execution: None,
            timer: None,
        }
    }

    pub(super) fn reset(&mut self) {
        if let Some(timer) = self.timer.take() {
            CFRunLoopTimer::invalidate(&timer);
        }
        self.last_execution = Some(Instant::now());
    }
}

pub(super) struct AXRegistry {
    windows: HashMap<CGWindowID, AXWindow>,
    pid_to_cg: HashMap<i32, Vec<CGWindowID>>,
}

impl AXRegistry {
    pub(super) fn new() -> Self {
        Self {
            windows: HashMap::new(),
            pid_to_cg: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, cg_id: CGWindowID, window: AXWindow) {
        let pid = window.pid();
        self.pid_to_cg.entry(pid).or_default().push(cg_id);
        self.windows.insert(cg_id, window);
    }

    pub(super) fn remove(&mut self, cg_id: CGWindowID) -> Option<AXWindow> {
        let window = self.windows.remove(&cg_id)?;
        if let Some(ids) = self.pid_to_cg.get_mut(&window.pid()) {
            ids.retain(|&id| id != cg_id);
        }
        Some(window)
    }

    pub(super) fn get(&self, cg_id: CGWindowID) -> Option<&AXWindow> {
        self.windows.get(&cg_id)
    }

    pub(super) fn contains(&self, cg_id: CGWindowID) -> bool {
        self.windows.contains_key(&cg_id)
    }

    pub(super) fn cg_ids_for_pid(&self, pid: i32) -> Vec<CGWindowID> {
        self.pid_to_cg.get(&pid).cloned().unwrap_or_default()
    }

    pub(super) fn remove_by_pid(&mut self, pid: i32) -> Vec<CGWindowID> {
        let Some(cg_ids) = self.pid_to_cg.remove(&pid) else {
            return Vec::new();
        };
        for &cg_id in &cg_ids {
            self.windows.remove(&cg_id);
        }
        cg_ids
    }

    pub(super) fn is_valid(&self, cg_id: CGWindowID) -> bool {
        self.windows.get(&cg_id).is_some_and(|w| w.is_valid())
    }
}
