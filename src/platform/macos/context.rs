use std::{cell::RefCell, collections::HashMap, rc::Rc};

use objc2::rc::Retained;
use objc2_application_services::AXObserver;
use objc2_core_foundation::CFRetained;

use super::overlay::OverlayView;
use super::window::MacWindow;
use crate::config::Config;
use crate::core::{Dimension, Hub, WindowId};

pub(super) type Observers = Rc<RefCell<HashMap<i32, CFRetained<AXObserver>>>>;

pub(super) struct WindowRegistry {
    pid_to_hashes: HashMap<i32, Vec<usize>>,
    hash_to_id: HashMap<usize, WindowId>,
    id_to_hash: HashMap<WindowId, usize>,
    id_to_window: HashMap<WindowId, MacWindow>,
}

impl WindowRegistry {
    fn new() -> Self {
        Self {
            pid_to_hashes: HashMap::new(),
            hash_to_id: HashMap::new(),
            id_to_hash: HashMap::new(),
            id_to_window: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, window_id: WindowId, window: MacWindow) {
        let cf_hash = window.cf_hash();
        let pid = window.pid();
        self.pid_to_hashes.entry(pid).or_default().push(cf_hash);
        self.hash_to_id.insert(cf_hash, window_id);
        self.id_to_hash.insert(window_id, cf_hash);
        self.id_to_window.insert(window_id, window);
    }

    pub(super) fn remove_by_hash(&mut self, cf_hash: usize) -> Option<WindowId> {
        let window_id = self.hash_to_id.remove(&cf_hash)?;
        let window = self.id_to_window.remove(&window_id)?;
        self.id_to_hash.remove(&window_id);
        if let Some(hashes) = self.pid_to_hashes.get_mut(&window.pid()) {
            hashes.retain(|&h| h != cf_hash);
        }
        Some(window_id)
    }

    pub(super) fn remove_by_pid(&mut self, pid: i32) -> Vec<WindowId> {
        let Some(hashes) = self.pid_to_hashes.remove(&pid) else {
            return Vec::new();
        };
        let mut removed = Vec::new();
        for cf_hash in hashes {
            if let Some(window_id) = self.hash_to_id.remove(&cf_hash) {
                self.id_to_hash.remove(&window_id);
                self.id_to_window.remove(&window_id);
                removed.push(window_id);
            }
        }
        removed
    }

    pub(super) fn contains(&self, window: &MacWindow) -> bool {
        self.hash_to_id.contains_key(&window.cf_hash())
    }

    pub(super) fn get(&self, window_id: WindowId) -> Option<&MacWindow> {
        self.id_to_window.get(&window_id)
    }
}

pub(super) struct WindowContext {
    pub(super) hub: Hub,
    pub(super) overlay_view: Retained<OverlayView>,
    pub(super) registry: RefCell<WindowRegistry>,
    pub(super) config: Config,
    pub(super) event_tap: Option<CFRetained<objc2_core_foundation::CFMachPort>>,
}

impl WindowContext {
    pub(super) fn new(overlay_view: Retained<OverlayView>, screen: Dimension, config: Config) -> Self {
        let hub = Hub::new(screen, config.border_size, config.tab_bar_height);

        Self {
            hub,
            overlay_view,
            registry: RefCell::new(WindowRegistry::new()),
            config,
            event_tap: None,
        }
    }
}
