use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Instant};

use objc2_application_services::AXObserver;
use objc2_core_foundation::{CFRetained, CFRunLoopTimer};
use objc2_core_graphics::CGWindowID;

use super::window::{MacWindow, WindowType};
use crate::core::{FloatWindowId, WindowId};

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

pub(super) struct WindowRegistry {
    pid_to_cg: HashMap<i32, Vec<CGWindowID>>,
    windows: HashMap<CGWindowID, MacWindow>,
    tiling_to_cg: HashMap<WindowId, CGWindowID>,
    float_to_cg: HashMap<FloatWindowId, CGWindowID>,
}

impl WindowRegistry {
    pub(super) fn new() -> Self {
        Self {
            pid_to_cg: HashMap::new(),
            windows: HashMap::new(),
            tiling_to_cg: HashMap::new(),
            float_to_cg: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, window: MacWindow) {
        let cg_id = window.window_id();
        let pid = window.pid();
        match window.window_type() {
            WindowType::Tiling(id) => {
                self.tiling_to_cg.insert(id, cg_id);
            }
            WindowType::Float(id) => {
                self.float_to_cg.insert(id, cg_id);
            }
            WindowType::Popup => {}
        }
        self.pid_to_cg.entry(pid).or_default().push(cg_id);
        self.windows.insert(cg_id, window);
    }

    pub(super) fn remove(&mut self, cg_id: CGWindowID) -> Option<MacWindow> {
        let window = self.windows.remove(&cg_id)?;
        if let Some(ids) = self.pid_to_cg.get_mut(&window.pid()) {
            ids.retain(|&id| id != cg_id);
        }
        match window.window_type() {
            WindowType::Tiling(id) => {
                self.tiling_to_cg.remove(&id);
            }
            WindowType::Float(id) => {
                self.float_to_cg.remove(&id);
            }
            WindowType::Popup => {}
        }
        Some(window)
    }

    pub(super) fn toggle_float(&mut self, tiling_id: WindowId, float_id: FloatWindowId) {
        if let Some(cg_id) = self.tiling_to_cg.remove(&tiling_id) {
            self.float_to_cg.insert(float_id, cg_id);
            if let Some(w) = self.windows.get_mut(&cg_id) {
                w.set_window_type(WindowType::Float(float_id));
            }
        } else if let Some(cg_id) = self.float_to_cg.remove(&float_id) {
            self.tiling_to_cg.insert(tiling_id, cg_id);
            if let Some(w) = self.windows.get_mut(&cg_id) {
                w.set_window_type(WindowType::Tiling(tiling_id));
            }
        }
    }

    pub(super) fn remove_by_pid(&mut self, pid: i32) -> Vec<MacWindow> {
        let Some(cg_ids) = self.pid_to_cg.remove(&pid) else {
            return Vec::new();
        };
        cg_ids
            .into_iter()
            .filter_map(|cg_id| {
                let window = self.windows.remove(&cg_id)?;
                match window.window_type() {
                    WindowType::Tiling(id) => {
                        self.tiling_to_cg.remove(&id);
                    }
                    WindowType::Float(id) => {
                        self.float_to_cg.remove(&id);
                    }
                    WindowType::Popup => {}
                }
                Some(window)
            })
            .collect()
    }

    pub(super) fn contains(&self, cg_id: CGWindowID) -> bool {
        self.windows.contains_key(&cg_id)
    }

    pub(super) fn get(&self, cg_id: CGWindowID) -> Option<&MacWindow> {
        self.windows.get(&cg_id)
    }

    pub(super) fn get_by_tiling_id(&self, tiling_id: WindowId) -> Option<&MacWindow> {
        self.tiling_to_cg
            .get(&tiling_id)
            .and_then(|cg_id| self.windows.get(cg_id))
    }

    pub(super) fn get_by_float_id(&self, float_id: FloatWindowId) -> Option<&MacWindow> {
        self.float_to_cg
            .get(&float_id)
            .and_then(|cg_id| self.windows.get(cg_id))
    }

    pub(super) fn cg_ids_for_pid(&self, pid: i32) -> Vec<CGWindowID> {
        self.pid_to_cg.get(&pid).cloned().unwrap_or_default()
    }

    pub(super) fn update_title(&mut self, cg_id: CGWindowID) {
        if let Some(window) = self.windows.get_mut(&cg_id) {
            window.update_title();
        }
    }
}
