use std::{cell::RefCell, collections::HashMap, collections::HashSet, rc::Rc, time::Instant};

use objc2_application_services::AXObserver;
use objc2_core_foundation::{CFRetained, CFRunLoopTimer};
use objc2_core_graphics::CGWindowID;

use super::window::MacWindow;
use crate::core::{FloatWindowId, WindowId};

pub(super) type Observers = Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>;

pub(super) enum RemovedWindow {
    Tiling(WindowId, MacWindow),
    Float(FloatWindowId, MacWindow),
}

pub(super) struct ThrottleState {
    pub(super) last_execution: Option<Instant>,
    pub(super) pending_pids: HashSet<i32>,
    pub(super) pending_focus_sync: bool,
    pub(super) timer: Option<CFRetained<CFRunLoopTimer>>,
}

impl ThrottleState {
    pub(super) fn new() -> Self {
        Self {
            last_execution: None,
            pending_pids: HashSet::new(),
            pending_focus_sync: false,
            timer: None,
        }
    }

    pub(super) fn reset(&mut self) {
        if let Some(timer) = self.timer.take() {
            CFRunLoopTimer::invalidate(&timer);
        }
        self.pending_pids.clear();
        self.pending_focus_sync = false;
        self.last_execution = Some(Instant::now());
    }
}

pub(super) struct WindowRegistry {
    pid_to_window_ids: HashMap<i32, Vec<CGWindowID>>,
    window_id_to_tiling: HashMap<CGWindowID, WindowId>,
    window_id_to_float: HashMap<CGWindowID, FloatWindowId>,
    tiling_to_window: HashMap<WindowId, MacWindow>,
    float_to_window: HashMap<FloatWindowId, MacWindow>,
}

impl WindowRegistry {
    pub(super) fn new() -> Self {
        Self {
            pid_to_window_ids: HashMap::new(),
            window_id_to_tiling: HashMap::new(),
            window_id_to_float: HashMap::new(),
            tiling_to_window: HashMap::new(),
            float_to_window: HashMap::new(),
        }
    }

    pub(super) fn insert_tiling(&mut self, tiling_id: WindowId, window: MacWindow) {
        let window_id = window.window_id();
        let pid = window.pid();
        self.pid_to_window_ids
            .entry(pid)
            .or_default()
            .push(window_id);
        self.window_id_to_tiling.insert(window_id, tiling_id);
        self.tiling_to_window.insert(tiling_id, window);
    }

    pub(super) fn insert_float(&mut self, float_id: FloatWindowId, window: MacWindow) {
        let window_id = window.window_id();
        let pid = window.pid();
        self.pid_to_window_ids
            .entry(pid)
            .or_default()
            .push(window_id);
        self.window_id_to_float.insert(window_id, float_id);
        self.float_to_window.insert(float_id, window);
    }

    pub(super) fn remove_by_window_id(&mut self, window_id: CGWindowID) -> Option<RemovedWindow> {
        if let Some(tiling_id) = self.window_id_to_tiling.remove(&window_id) {
            let window = self.tiling_to_window.remove(&tiling_id)?;
            if let Some(ids) = self.pid_to_window_ids.get_mut(&window.pid()) {
                ids.retain(|&id| id != window_id);
            }
            return Some(RemovedWindow::Tiling(tiling_id, window));
        }
        if let Some(float_id) = self.window_id_to_float.remove(&window_id) {
            let window = self.float_to_window.remove(&float_id)?;
            if let Some(ids) = self.pid_to_window_ids.get_mut(&window.pid()) {
                ids.retain(|&id| id != window_id);
            }
            return Some(RemovedWindow::Float(float_id, window));
        }
        None
    }

    pub(super) fn toggle_float(&mut self, tiling_id: WindowId, float_id: FloatWindowId) {
        if let Some(w) = self.tiling_to_window.remove(&tiling_id) {
            let window_id = w.window_id();
            self.window_id_to_tiling.remove(&window_id);
            self.window_id_to_float.insert(window_id, float_id);
            self.float_to_window.insert(float_id, w);
        } else if let Some(w) = self.float_to_window.remove(&float_id) {
            let window_id = w.window_id();
            self.window_id_to_float.remove(&window_id);
            self.window_id_to_tiling.insert(window_id, tiling_id);
            self.tiling_to_window.insert(tiling_id, w);
        }
    }

    #[allow(clippy::type_complexity)]
    pub(super) fn remove_by_pid(
        &mut self,
        pid: i32,
    ) -> (Vec<(WindowId, MacWindow)>, Vec<(FloatWindowId, MacWindow)>) {
        let Some(window_ids) = self.pid_to_window_ids.remove(&pid) else {
            return (Vec::new(), Vec::new());
        };
        let mut removed_tiling = Vec::new();
        let mut removed_float = Vec::new();
        for window_id in window_ids {
            if let Some(tiling_id) = self.window_id_to_tiling.remove(&window_id)
                && let Some(window) = self.tiling_to_window.remove(&tiling_id)
            {
                removed_tiling.push((tiling_id, window));
            }
            if let Some(float_id) = self.window_id_to_float.remove(&window_id)
                && let Some(window) = self.float_to_window.remove(&float_id)
            {
                removed_float.push((float_id, window));
            }
        }
        (removed_tiling, removed_float)
    }

    pub(super) fn contains(&self, window: &MacWindow) -> bool {
        let window_id = window.window_id();
        self.window_id_to_tiling.contains_key(&window_id)
            || self.window_id_to_float.contains_key(&window_id)
    }

    pub(super) fn get_tiling(&self, tiling_id: WindowId) -> Option<&MacWindow> {
        self.tiling_to_window.get(&tiling_id)
    }

    pub(super) fn get_float(&self, float_id: FloatWindowId) -> Option<&MacWindow> {
        self.float_to_window.get(&float_id)
    }

    pub(super) fn get_tiling_by_window_id(&self, window_id: CGWindowID) -> Option<WindowId> {
        self.window_id_to_tiling.get(&window_id).copied()
    }

    pub(super) fn get_float_by_window_id(&self, window_id: CGWindowID) -> Option<FloatWindowId> {
        self.window_id_to_float.get(&window_id).copied()
    }

    pub(super) fn get_by_window_id(&self, window_id: CGWindowID) -> Option<&MacWindow> {
        self.window_id_to_tiling
            .get(&window_id)
            .and_then(|id| self.tiling_to_window.get(id))
            .or_else(|| {
                self.window_id_to_float
                    .get(&window_id)
                    .and_then(|id| self.float_to_window.get(id))
            })
    }

    pub(super) fn window_ids_for_pid(&self, pid: i32) -> Vec<CGWindowID> {
        self.pid_to_window_ids
            .get(&pid)
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn update_title(&mut self, window_id: CGWindowID) {
        if let Some(tiling_id) = self.window_id_to_tiling.get(&window_id)
            && let Some(window) = self.tiling_to_window.get_mut(tiling_id)
        {
            window.update_title();
        } else if let Some(float_id) = self.window_id_to_float.get(&window_id)
            && let Some(window) = self.float_to_window.get_mut(float_id)
        {
            window.update_title();
        }
    }
}
