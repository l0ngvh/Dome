use std::collections::{HashMap, HashSet};

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromWindow};

use crate::config::Config;
use crate::core::{
    Child, ContainerPlacement, Dimension, Hub, MonitorId, MonitorLayout, WindowId, WindowPlacement,
};

use super::dome::ContainerOverlayData;
use super::window::{ManagedHwnd, Registry, Taskbar};

pub(super) struct MonitorEntry {
    pub(super) id: MonitorId,
    pub(super) dimension: Dimension,
    pub(super) displayed_windows: HashSet<ManagedHwnd>,
}

impl MonitorEntry {
    pub(super) fn apply_placements(
        &mut self,
        layout: &MonitorLayout,
        registry: &mut Registry,
        taskbar: &mut Taskbar,
        hub: &mut Hub,
        config: &Config,
    ) -> (Vec<WindowPlacement>, Vec<ContainerOverlayData>) {
        match layout {
            MonitorLayout::Fullscreen(window_id) => {
                self.apply_fullscreen(*window_id, registry, taskbar);
                (vec![], vec![])
            }
            MonitorLayout::Normal {
                windows,
                containers,
            } => self.apply_normal(windows, containers, registry, taskbar, hub, config),
        }
    }

    fn apply_fullscreen(
        &mut self,
        window_id: WindowId,
        registry: &mut Registry,
        taskbar: &mut Taskbar,
    ) {
        let current_windows: HashSet<_> = registry
            .get_handle(window_id)
            .map(|h| ManagedHwnd::new(h.hwnd()))
            .into_iter()
            .collect();
        for key in self.displayed_windows.difference(&current_windows) {
            if let Some(handle) = registry.get_handle_by(*key) {
                handle.hide();
                taskbar.delete_tab(handle.hwnd()).ok();
            }
        }
        self.displayed_windows = current_windows;

        if let Some(handle) = registry.get_handle_mut(window_id) {
            handle.set_fullscreen(&self.dimension);
            taskbar.add_tab(handle.hwnd()).ok();
        }
    }

    fn apply_normal(
        &mut self,
        windows: &[WindowPlacement],
        containers: &[ContainerPlacement],
        registry: &mut Registry,
        taskbar: &mut Taskbar,
        hub: &mut Hub,
        config: &Config,
    ) -> (Vec<WindowPlacement>, Vec<ContainerOverlayData>) {
        let border = config.border_size;
        let current_windows: HashSet<_> = windows
            .iter()
            .filter_map(|p| {
                registry
                    .get_handle(p.id)
                    .map(|h| ManagedHwnd::new(h.hwnd()))
            })
            .collect();
        for key in self.displayed_windows.difference(&current_windows) {
            if let Some(handle) = registry.get_handle_by(*key) {
                handle.hide();
                taskbar.delete_tab(handle.hwnd()).ok();
            }
        }
        self.displayed_windows = current_windows;

        let mut window_placements = Vec::new();
        for wp in windows {
            if let Some(handle) = registry.get_handle_mut(wp.id) {
                if let Some([min_w, min_h, max_w, max_h]) =
                    handle.get_constraints(&wp.frame, border)
                {
                    hub.set_window_constraint(wp.id, min_w, min_h, max_w, max_h);
                }
                handle.show(&wp.frame, border, wp.is_float);
                taskbar.add_tab(handle.hwnd()).ok();
            }
            window_placements.push(*wp);
        }

        let mut container_overlays = Vec::new();
        for cp in containers {
            if !cp.is_tabbed && !cp.is_focused {
                continue;
            }
            let tab_titles = if cp.is_tabbed {
                collect_tab_titles(hub, registry, cp.id)
            } else {
                vec![]
            };
            container_overlays.push(ContainerOverlayData {
                placement: *cp,
                tab_titles,
            });
        }

        (window_placements, container_overlays)
    }
}

fn collect_tab_titles(
    hub: &Hub,
    registry: &Registry,
    container_id: crate::core::ContainerId,
) -> Vec<String> {
    let container = hub.get_container(container_id);
    container
        .children()
        .iter()
        .map(|c| match c {
            Child::Window(wid) => registry
                .get_handle(*wid)
                .and_then(|h| h.title().map(|s| s.to_owned()))
                .unwrap_or_else(|| "Unknown".to_owned()),
            Child::Container(_) => "Container".to_owned(),
        })
        .collect()
}

pub(super) struct MonitorRegistry {
    pub(super) map: HashMap<isize, MonitorEntry>,
    pub(super) reverse: HashMap<MonitorId, isize>,
    pub(super) primary_handle: isize,
}

impl MonitorRegistry {
    pub(super) fn new(primary_handle: isize, primary_id: MonitorId, dimension: Dimension) -> Self {
        let mut map = HashMap::new();
        let mut reverse = HashMap::new();
        map.insert(
            primary_handle,
            MonitorEntry {
                id: primary_id,
                dimension,
                displayed_windows: HashSet::new(),
            },
        );
        reverse.insert(primary_id, primary_handle);
        Self {
            map,
            reverse,
            primary_handle,
        }
    }

    pub(super) fn insert(&mut self, handle: isize, monitor_id: MonitorId, dimension: Dimension) {
        self.map.insert(
            handle,
            MonitorEntry {
                id: monitor_id,
                dimension,
                displayed_windows: HashSet::new(),
            },
        );
        self.reverse.insert(monitor_id, handle);
    }

    pub(super) fn remove_by_id(&mut self, monitor_id: MonitorId) {
        if let Some(handle) = self.reverse.remove(&monitor_id) {
            self.map.remove(&handle);
        }
    }

    pub(super) fn get_entry_mut(&mut self, monitor_id: MonitorId) -> Option<&mut MonitorEntry> {
        self.reverse
            .get(&monitor_id)
            .and_then(|h| self.map.get_mut(h))
    }

    pub(super) fn find_monitor_dimension(&self, hwnd: HWND) -> Option<Dimension> {
        let hmonitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) };
        self.map.get(&(hmonitor.0 as isize)).map(|e| e.dimension)
    }
}
