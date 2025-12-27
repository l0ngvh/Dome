use std::{cell::RefCell, collections::HashMap, collections::HashSet, rc::Rc, time::Instant};

use objc2::rc::Retained;
use objc2_application_services::AXObserver;
use objc2_core_foundation::{CFRetained, CFRunLoopTimer};

use super::overlay::OverlayView;
use super::window::MacWindow;
use crate::config::Config;
use crate::core::{Dimension, FloatWindowId, Hub, WindowId};

pub(super) type Observers = Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>;

pub(super) struct ThrottleState {
    pub(super) last_execution: Option<Instant>,
    pub(super) pending_pids: HashSet<i32>,
    pub(super) pending_focus_sync: bool,
    pub(super) timer: Option<CFRetained<CFRunLoopTimer>>,
}

impl ThrottleState {
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
    pid_to_hashes: HashMap<i32, Vec<usize>>,
    hash_to_tiling: HashMap<usize, WindowId>,
    hash_to_float: HashMap<usize, FloatWindowId>,
    tiling_to_window: HashMap<WindowId, MacWindow>,
    float_to_window: HashMap<FloatWindowId, MacWindow>,
}

impl WindowRegistry {
    fn new() -> Self {
        Self {
            pid_to_hashes: HashMap::new(),
            hash_to_tiling: HashMap::new(),
            hash_to_float: HashMap::new(),
            tiling_to_window: HashMap::new(),
            float_to_window: HashMap::new(),
        }
    }

    pub(super) fn insert_tiling(&mut self, window_id: WindowId, window: MacWindow) {
        let cf_hash = window.cf_hash();
        let pid = window.pid();
        self.pid_to_hashes.entry(pid).or_default().push(cf_hash);
        self.hash_to_tiling.insert(cf_hash, window_id);
        self.tiling_to_window.insert(window_id, window);
    }

    pub(super) fn insert_float(&mut self, float_id: FloatWindowId, window: MacWindow) {
        let cf_hash = window.cf_hash();
        let pid = window.pid();
        self.pid_to_hashes.entry(pid).or_default().push(cf_hash);
        self.hash_to_float.insert(cf_hash, float_id);
        self.float_to_window.insert(float_id, window);
    }

    pub(super) fn remove_tiling_by_hash(&mut self, cf_hash: usize) -> Option<WindowId> {
        let window_id = self.hash_to_tiling.remove(&cf_hash)?;
        let window = self.tiling_to_window.remove(&window_id)?;
        if let Some(hashes) = self.pid_to_hashes.get_mut(&window.pid()) {
            hashes.retain(|&h| h != cf_hash);
        }
        Some(window_id)
    }

    pub(super) fn remove_float_by_hash(&mut self, cf_hash: usize) -> Option<FloatWindowId> {
        let float_id = self.hash_to_float.remove(&cf_hash)?;
        let window = self.float_to_window.remove(&float_id)?;
        if let Some(hashes) = self.pid_to_hashes.get_mut(&window.pid()) {
            hashes.retain(|&h| h != cf_hash);
        }
        Some(float_id)
    }

    pub(super) fn toggle_float(&mut self, window_id: WindowId, float_id: FloatWindowId) {
        if let Some(w) = self.tiling_to_window.remove(&window_id) {
            let h = w.cf_hash();
            self.hash_to_tiling.remove(&h);
            self.hash_to_float.insert(h, float_id);
            self.float_to_window.insert(float_id, w);
        } else if let Some(w) = self.float_to_window.remove(&float_id) {
            let h = w.cf_hash();
            self.hash_to_float.remove(&h);
            self.hash_to_tiling.insert(h, window_id);
            self.tiling_to_window.insert(window_id, w);
        }
    }

    pub(super) fn remove_by_pid(&mut self, pid: i32) -> (Vec<WindowId>, Vec<FloatWindowId>) {
        let Some(hashes) = self.pid_to_hashes.remove(&pid) else {
            return (Vec::new(), Vec::new());
        };
        let mut removed_tiling = Vec::new();
        let mut removed_float = Vec::new();
        for cf_hash in hashes {
            if let Some(window_id) = self.hash_to_tiling.remove(&cf_hash) {
                self.tiling_to_window.remove(&window_id);
                removed_tiling.push(window_id);
            }
            if let Some(float_id) = self.hash_to_float.remove(&cf_hash) {
                self.float_to_window.remove(&float_id);
                removed_float.push(float_id);
            }
        }
        (removed_tiling, removed_float)
    }

    pub(super) fn contains(&self, window: &MacWindow) -> bool {
        let h = window.cf_hash();
        self.hash_to_tiling.contains_key(&h) || self.hash_to_float.contains_key(&h)
    }

    pub(super) fn get_tiling(&self, window_id: WindowId) -> Option<&MacWindow> {
        self.tiling_to_window.get(&window_id)
    }

    pub(super) fn get_float(&self, float_id: FloatWindowId) -> Option<&MacWindow> {
        self.float_to_window.get(&float_id)
    }

    pub(super) fn get_tiling_by_hash(&self, cf_hash: usize) -> Option<WindowId> {
        self.hash_to_tiling.get(&cf_hash).copied()
    }

    pub(super) fn get_float_by_hash(&self, cf_hash: usize) -> Option<FloatWindowId> {
        self.hash_to_float.get(&cf_hash).copied()
    }

    pub(super) fn hashes_for_pid(&self, pid: i32) -> Vec<usize> {
        self.pid_to_hashes.get(&pid).cloned().unwrap_or_default()
    }
}

pub(super) struct WindowContext {
    pub(super) hub: Hub,
    pub(super) tiling_overlay: Retained<OverlayView>,
    pub(super) float_overlay: Retained<OverlayView>,
    pub(super) registry: RefCell<WindowRegistry>,
    pub(super) config: Config,
    pub(super) event_tap: Option<CFRetained<objc2_core_foundation::CFMachPort>>,
    pub(super) throttle: ThrottleState,
}

impl WindowContext {
    pub(super) fn new(
        tiling_overlay: Retained<OverlayView>,
        float_overlay: Retained<OverlayView>,
        screen: Dimension,
        config: Config,
    ) -> Self {
        let hub = Hub::new(screen, config.border_size, config.tab_bar_height);

        Self {
            hub,
            tiling_overlay,
            float_overlay,
            registry: RefCell::new(WindowRegistry::new()),
            config,
            event_tap: None,
            throttle: ThrottleState {
                last_execution: None,
                pending_pids: HashSet::new(),
                pending_focus_sync: false,
                timer: None,
            },
        }
    }
}
