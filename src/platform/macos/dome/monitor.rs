use std::collections::{HashMap, HashSet};

use crate::core::{Dimension, MonitorId, WindowId};

use super::super::MonitorInfo;

type DisplayId = u32;

pub(super) struct MonitorEntry {
    pub(super) id: MonitorId,
    pub(super) screen: MonitorInfo,
    pub(super) displayed_windows: HashSet<WindowId>,
}

pub(super) struct MonitorRegistry {
    map: HashMap<DisplayId, MonitorEntry>,
    reverse: HashMap<MonitorId, DisplayId>,
    primary_display_id: DisplayId,
}

impl MonitorRegistry {
    pub(super) fn new(primary: &MonitorInfo, primary_monitor_id: MonitorId) -> Self {
        let mut map = HashMap::new();
        let mut reverse = HashMap::new();
        map.insert(
            primary.display_id,
            MonitorEntry {
                id: primary_monitor_id,
                screen: primary.clone(),
                displayed_windows: HashSet::new(),
            },
        );
        reverse.insert(primary_monitor_id, primary.display_id);
        Self {
            map,
            reverse,
            primary_display_id: primary.display_id,
        }
    }

    pub(super) fn contains(&self, display_id: DisplayId) -> bool {
        self.map.contains_key(&display_id)
    }

    pub(super) fn get(&self, display_id: DisplayId) -> Option<MonitorId> {
        self.map.get(&display_id).map(|e| e.id)
    }

    pub(super) fn get_entry(&self, monitor_id: MonitorId) -> &MonitorEntry {
        self.reverse
            .get(&monitor_id)
            .and_then(|d| self.map.get(d))
            .unwrap()
    }

    pub(super) fn get_entry_mut(&mut self, monitor_id: MonitorId) -> &mut MonitorEntry {
        self.reverse
            .get(&monitor_id)
            .and_then(|d| self.map.get_mut(d))
            .unwrap()
    }

    pub(super) fn primary_monitor_id(&self) -> MonitorId {
        self.get(self.primary_display_id).unwrap()
    }

    pub(super) fn set_primary_display_id(&mut self, display_id: DisplayId) {
        self.primary_display_id = display_id;
    }

    pub(super) fn replace_primary(&mut self, new_screen: &MonitorInfo) {
        debug_assert!(!self.map.contains_key(&new_screen.display_id));
        if let Some(mut entry) = self.map.remove(&self.primary_display_id) {
            let old = self.primary_display_id;
            let monitor_id = entry.id;
            entry.screen = new_screen.clone();
            self.map.insert(new_screen.display_id, entry);
            self.reverse.insert(monitor_id, new_screen.display_id);
            self.primary_display_id = new_screen.display_id;
            tracing::info!(old, new = new_screen.display_id, "Primary monitor replaced");
        }
    }

    pub(super) fn insert(&mut self, screen: &MonitorInfo, monitor_id: MonitorId) {
        self.map.insert(
            screen.display_id,
            MonitorEntry {
                id: monitor_id,
                screen: screen.clone(),
                displayed_windows: HashSet::new(),
            },
        );
        self.reverse.insert(monitor_id, screen.display_id);
    }

    pub(super) fn remove_displayed_window(&mut self, window_id: WindowId) {
        for entry in self.map.values_mut() {
            entry.displayed_windows.remove(&window_id);
        }
    }

    /// Whether any monitor currently tracks this window as visible on screen.
    /// Used to decide if a window exiting native fullscreen should stay visible
    /// or be minimized (unfocused workspace means not displayed).
    pub(super) fn is_displayed(&self, window_id: WindowId) -> bool {
        self.map
            .values()
            .any(|entry| entry.displayed_windows.contains(&window_id))
    }

    fn remove_by_id(&mut self, monitor_id: MonitorId) {
        if let Some(display_id) = self.reverse.remove(&monitor_id) {
            self.map.remove(&display_id);
        }
    }

    pub(super) fn remove_stale(&mut self, current: &HashSet<DisplayId>) -> Vec<MonitorId> {
        let stale: Vec<_> = self
            .map
            .iter()
            .filter(|(key, _)| !current.contains(key))
            .map(|(_, e)| e.id)
            .collect();
        for &id in &stale {
            self.remove_by_id(id);
        }
        stale
    }

    pub(super) fn all_screens(&self) -> Vec<MonitorInfo> {
        self.map.values().map(|e| e.screen.clone()).collect()
    }

    pub(super) fn find_monitor_at(&self, x: f32, y: f32) -> Option<&MonitorInfo> {
        self.map
            .values()
            .find(|e| {
                let d = &e.screen.dimension;
                x >= d.x && x < d.x + d.width && y >= d.y && y < d.y + d.height
            })
            .map(|e| &e.screen)
    }

    pub(super) fn update_screen(&mut self, screen: &MonitorInfo) -> Option<(MonitorId, Dimension)> {
        let entry = self.map.get_mut(&screen.display_id)?;
        let old_dim = entry.screen.dimension;
        entry.screen = screen.clone();
        Some((entry.id, old_dim))
    }
}
